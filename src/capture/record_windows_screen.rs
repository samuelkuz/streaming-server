use std::{thread, time::Duration};
use std::fs::File;
use std::io::Write;
use std::time::Instant;
// use ac_ffmpeg::time::Timestamp;
use windows_rust_record::windows_screen_capture::WindowsScreenCapture;
use crate::encoder::ffmpeg::FfmpegEncoder;
use crate::result::Result;
// WebRTC imports
use std::sync::Arc;
use webrtc::media::Sample;
use webrtc::track::track_local::track_local_static_sample::TrackLocalStaticSample;
use bytes::Bytes;

pub async fn record(
        mut windows_screen_capture: WindowsScreenCapture, 
        mut encoder: FfmpegEncoder,
        video_track: Arc<TrackLocalStaticSample>,
    ) {
    let mut receiver = windows_screen_capture.get_frame_receiver().unwrap();
    windows_screen_capture.start_capture_session();

    // Hardcoding this value for now, but will need to add this to a config setup
    let frame_rate = 30;

    let mut ticker =
        tokio::time::interval(Duration::from_millis((1000 / frame_rate) as u64));
    
    let test_frames = 300;
    let mut frame_idx: i64 = 0;

    // create file
    let mut file = File::create("test.raw").unwrap();

    while let Some(frame) = receiver.recv().await {
        //let now = Instant::now();

        let frame_time = frame.SystemRelativeTime().unwrap().Duration;
        let (resource, frame_bits) = unsafe { windows_screen_capture.get_frame_content(frame).unwrap() };
        
        // encode here
        let encoded = encoder.encode(frame_bits, frame_time).unwrap();
        //write(&mut file, &encoded).await.unwrap();
        
        let bytes = Bytes::from(encoded);
        video_track.write_sample(&Sample {
            data: bytes,
            duration: Duration::from_millis((1000. / frame_rate as f64) as u64),
            ..Default::default()
        }).await.expect("Could not write sample");

        unsafe {
            windows_screen_capture.unmap_d3d_context(&resource);
        }

        if frame_idx == test_frames {
            break;
        }
        frame_idx += 1;

        // println!("ms: {}", now.elapsed().as_millis());
        ticker.tick().await;
    }

    windows_screen_capture.session.Close().unwrap();
    file.flush().unwrap();

    thread::sleep(Duration::from_millis(25));
}

async fn _write(file: &mut File, input: &Vec<u8>) -> Result<()> {
    file.write_all(input)?;
    Ok(())
}

