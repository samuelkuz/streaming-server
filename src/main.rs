use crate::capture::{
    display_info::{enumerate_displays, create_capture_item_for_monitor},
    windows_screen_capture::WindowsScreenCapture,
};
use crate::result::Result;
use crate::encoder::ffmpeg::FfmpegEncoder;
use clap::Parser;

mod capture;
mod encoder;
mod result;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    // Monitor Display   to record/stream
    #[arg(short, long, default_value="0")]
    display: usize,

    // #[arg(long)]
    // test: Option<usize>,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    //Get GraphicsCaptureItem
    let displays = enumerate_displays()?;

    // for (i, display) in displays.iter().enumerate() {
    //     println!(
    //         "display: {} {}x{} {}",
    //         display.display_name,
    //         display.resolution.0,
    //         display.resolution.1,
    //         if i==0 { "(selected)" } else { "" },
    //     );
    // }

    let display = displays.iter().nth(args.display).unwrap();
    
    let graphic_capture_item = create_capture_item_for_monitor(display.handle)?;
    let (width, height) = display.resolution.clone();

    let mut screen_capture = WindowsScreenCapture::new(&graphic_capture_item)?;

    // screen_capture.screenshot()?;

    let encoder = FfmpegEncoder::new(width, height);
    screen_capture.record(encoder).await?;

    Ok(())
}
