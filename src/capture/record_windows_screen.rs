use std::{thread, time::Duration};
use std::fs::File;
use std::io::Write;
use std::time::Instant;
use ac_ffmpeg::time::Timestamp;
use windows_rust_record::windows_screen_capture::WindowsScreenCapture;
use crate::encoder::ffmpeg::FfmpegEncoder;
use crate::result::Result;
// WebSocket Imports
use futures_util::{SinkExt, StreamExt};
use tokio::net::{TcpStream};
use tokio_tungstenite::{WebSocketStream};
use tokio_tungstenite::tungstenite::Message;
use std::slice;

fn pixelize_1440p(byte_slice: &mut [u8], pixel_squash: usize) {
    let columns = 2560;
    let rows = 1440;

    for i in (0..rows).step_by(pixel_squash) {
        for j in (0..columns).step_by(pixel_squash) {
            // Calculate b,g,r,a sums
            let mut blue_sum: usize = 0;
            let mut green_sum: usize = 0;
            let mut red_sum: usize = 0;
            let mut alpha_sum: usize = 0;

            for x in 0..pixel_squash {
                blue_sum = 0;
                green_sum = 0;
                red_sum = 0;
                alpha_sum = 0;

                for y in 0..pixel_squash {
                    // Get index in array for current pixel
                    let byte_row_offset = (i + x) * 10_240;
                    let pixel_index = ((j + y) * 4) + byte_row_offset;

                    if pixel_index + 3 >= byte_slice.len() {
                        break;
                    }

                    let blue = byte_slice[pixel_index];
                    let green = byte_slice[pixel_index + 1];
                    let red = byte_slice[pixel_index + 2];
                    let alpha = byte_slice[pixel_index + 3];

                    blue_sum += blue as usize;
                    green_sum += green as usize;
                    red_sum += red as usize;
                    alpha_sum += alpha as usize;
                }
            }

            let blue_avg = (blue_sum / pixel_squash) as u8;
            let green_avg = (green_sum / pixel_squash) as u8;
            let red_avg = (red_sum / pixel_squash) as u8;
            let alpha_avg = (alpha_sum / pixel_squash) as u8;

            for x in 0..pixel_squash {
                for y in 0..pixel_squash {
                    // Get index in array for current pixel
                    let byte_row_offset = (i + x) * 10_240;
                    let pixel_index = ((j + y) * 4) + byte_row_offset;

                    if pixel_index + 3 >= byte_slice.len() {
                        break;
                    }

                    byte_slice[pixel_index] = blue_avg;
                    byte_slice[pixel_index + 1] = green_avg;
                    byte_slice[pixel_index + 2] = red_avg;
                    byte_slice[pixel_index + 3] = alpha_avg;
                }
            }
        }

    }

}

pub async fn record(
        mut windows_screen_capture: WindowsScreenCapture, 
        mut encoder: FfmpegEncoder,
        // mut ws_stream: WebSocketStream<TcpStream>,
    ) {
    let mut receiver = windows_screen_capture.get_frame_receiver().unwrap();
    windows_screen_capture.start_capture_session();

    let mut ticker =
        tokio::time::interval(Duration::from_millis((1000 / 30) as u64));
    
    let test_frames = 150;
    let mut frame_idx: i64 = 0;

    // create file
    let mut file = File::create("test.raw").unwrap();

    while let Some(frame) = receiver.recv().await {
        let frame_time = frame.SystemRelativeTime().unwrap().Duration;
        let (resource, frame_bits) = unsafe { windows_screen_capture.get_frame_content(frame).unwrap() };

        let ptr = frame_bits.as_ptr() as *mut u8;
        let len = frame_bits.len();
        let mut frame_bits_as_mut = unsafe { slice::from_raw_parts_mut(ptr, len) };

        // hardcoded_pixelize(frame_bits_as_mut);
        pixelize_1440p(frame_bits_as_mut, 16);
        
        // encode here
        let encoded = encoder.encode(frame_bits, frame_time).unwrap();
        write(&mut file, &encoded).await.unwrap();
        
        // ws_stream.send(Message::binary(encoded)).await.unwrap();

        unsafe {
            windows_screen_capture.unmap_d3d_context(&resource);
        }

        if frame_idx == test_frames {
            break;
        }
        frame_idx += 1;

        ticker.tick().await;
    }

    windows_screen_capture.session.Close().unwrap();
    file.flush().unwrap();

    thread::sleep(Duration::from_millis(25));
}

async fn write(file: &mut File, input: &Vec<u8>) -> Result<()> {
    file.write_all(input)?;
    Ok(())
}

