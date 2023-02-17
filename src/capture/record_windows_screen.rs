use std::{thread, time::Duration};
use std::fs::File;
use std::io::Write;
use std::time::Instant;
use windows_rust_record::windows_screen_capture::WindowsScreenCapture;
use crate::encoder::ffmpeg::FfmpegEncoder;
use crate::result::Result;

pub async fn record(mut windows_screen_capture: WindowsScreenCapture, mut encoder: FfmpegEncoder) {
    let mut receiver = windows_screen_capture.get_frame_receiver().unwrap();
    windows_screen_capture.start_capture_session();

    let mut ticker =
        tokio::time::interval(Duration::from_millis((1000 / 30) as u64));
    
    let test_frames = 420;
    let mut count = 0;
    
    // create file
    let mut file = File::create("test.raw").unwrap();
    while let Some(frame) = receiver.recv().await {
        let frame_time = frame.SystemRelativeTime().unwrap().Duration;
        let (resource, frame_bits) = unsafe { windows_screen_capture.get_frame_content(frame).unwrap() };
        
        // encode here
        let encoded = encoder.encode(frame_bits, frame_time).unwrap();
        write(&mut file, &encoded).await.unwrap();

        unsafe {
            windows_screen_capture.unmap_d3d_context(&resource);
        }

        if count == test_frames {
            break;
        }
        count += 1;

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

