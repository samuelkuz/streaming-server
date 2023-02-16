use crate::result::Result;
use crate::encoder::ffmpeg::FfmpegEncoder;
use clap::Parser;
use windows_rust_record::{
    display::Display,
    windows_screen_capture::WindowsScreenCapture,
};
use crate::capture::record_windows_screen::record;

mod capture;
mod encoder;
mod result;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    // Monitor Display to record/stream
    #[arg(short, long, default_value="0")]
    display: usize,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    let displays = Display::enumerate_displays()?;
    let display = displays.iter().nth(args.display).unwrap();
    let (width, height) = display.resolution.clone();
    
    let windows_screen_capture = WindowsScreenCapture::new(display)?;
    let encoder = FfmpegEncoder::new(width, height);

    record(windows_screen_capture, encoder).await;

    Ok(())
}
