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

async fn accept_connection(peer: SocketAddr, stream: TcpStream) {
    if let Err(e) = handle_connection(peer, stream).await {
        match e {
            Error::ConnectionClosed | Error::Protocol(_) | Error::Utf8 => println!("Connection closed"),
            err => println!("Error processing connection: {}", err),
        }
    }
}

async fn handle_connection(peer: SocketAddr, stream: TcpStream) -> std::result::Result<(), Error> {
    let mut ws_stream = accept_async(stream).await.expect("Failed to accept");

    println!("New WebSocket connection: {}", peer);

    while let Some(msg) = ws_stream.next().await {
        let msg = msg?;
        if msg.is_text() || msg.is_binary() {
            println!("msg is: {}", msg);
            ws_stream.send(msg).await?;
        }
    }

    Ok(())
}

async fn get_websocket(peer:SocketAddr, stream: TcpStream) -> WebSocketStream<TcpStream> {
    let mut ws_stream = accept_async(stream).await.expect("Failed to accept");

    println!("New WebSocket connection: {}", peer);

    ws_stream
}


#[tokio::main]
async fn main() -> Result<()> {
    let addr = "127.0.0.1:9002";
    let listener = TcpListener::bind(&addr).await.expect("Can't listen");
    let (stream, _) = listener.accept().await.unwrap();
    let peer_addr = stream.peer_addr().expect("connected strings should have a peer address");

    let mut ws_stream: WebSocketStream<TcpStream> = get_websocket(peer_addr, stream).await;



    // match test {
    //     Ok((stream, _)) => {
    //         let peer = stream.peer_addr().expect("connected strings should have a peer address");

    //         // let temp = tokio::spawn(accept_connection(peer, stream));
    //         // temp.await.unwrap();

    //         let test = accept_connection(peer, stream);
    //     }
    //     _ => panic!("Failed to connect to websocket")
    // }

    // while let Ok((stream, _)) = listener.accept().await {
    //     let peer = stream.peer_addr().expect("connected strings should have a peer address");

    //     tokio::spawn(accept_connection(peer, stream));
    // }


    // let args = Args::parse();

    // let displays = Display::enumerate_displays()?;
    // let display = displays.iter().nth(args.display).unwrap();
    // let (width, height) = display.resolution.clone();
    
    // let windows_screen_capture = WindowsScreenCapture::new(display)?;
    // let encoder = FfmpegEncoder::new(width, height);

    // record(windows_screen_capture, encoder).await;

    Ok(())
}
