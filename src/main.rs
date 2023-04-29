use crate::result::Result;
use crate::encoder::ffmpeg::FfmpegEncoder;
use clap::Parser;
use windows_rust_record::{
    display::Display,
    windows_screen_capture::WindowsScreenCapture,
};
use crate::capture::record_windows_screen::record;

// Test WebSocket imports
use futures_util::{SinkExt, StreamExt};
use std::net::SocketAddr;
use tokio::net::{TcpListener, TcpStream};
use tokio_tungstenite::{accept_async, tungstenite::Error, WebSocketStream};

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

async fn get_websocket(peer:SocketAddr, stream: TcpStream) -> WebSocketStream<TcpStream> {
    let mut ws_stream = accept_async(stream).await.expect("Failed to accept");

    println!("New WebSocket connection: {}", peer);

    ws_stream
}


#[tokio::main]
async fn main() -> Result<()> {
    // TODO: Refactor web socket usage
    let addr = "127.0.0.1:9002";
    let listener = TcpListener::bind(&addr).await.expect("Can't listen");
    let (stream, _) = listener.accept().await.unwrap();
    let peer_addr = stream.peer_addr().expect("connected strings should have a peer address");
    let mut ws_stream: WebSocketStream<TcpStream> = get_websocket(peer_addr, stream).await;

    // Video streaming logic
    let args = Args::parse();

    let displays = Display::enumerate_displays()?;
    let display = displays.iter().nth(args.display).unwrap();
    let (width, height) = display.resolution.clone();
    
    let windows_screen_capture = WindowsScreenCapture::new(display)?;
    let encoder = FfmpegEncoder::new(width, height);

    record(windows_screen_capture, encoder, ws_stream).await;

    Ok(())
}
