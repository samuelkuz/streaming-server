use crate::result::Result;
use crate::signal_message::{SignalRequest, SignalResponse, Test};
use std::sync::Arc;

use futures_util::{SinkExt, StreamExt};
use futures_util::stream::SplitStream;
use tokio::net::{TcpStream};
use tokio::sync::Mutex;
use tokio_tungstenite::{connect_async, WebSocketStream, tungstenite::Message, MaybeTlsStream};

use webrtc::ice_transport::ice_server::RTCIceServer;
use webrtc::api::interceptor_registry::register_default_interceptors;
use webrtc::api::media_engine::MediaEngine;
use webrtc::api::APIBuilder;
use webrtc::interceptor::registry::Registry;
use webrtc::peer_connection::RTCPeerConnection;
use webrtc::peer_connection::sdp::session_description::RTCSessionDescription;
use webrtc::peer_connection::configuration::RTCConfiguration;
use webrtc::rtp_transceiver::rtp_codec::RTCRtpCodecCapability;
use webrtc::track::track_local::track_local_static_sample::TrackLocalStaticSample;
use webrtc::track::track_local::TrackLocal;
use webrtc::ice_transport::ice_candidate::RTCIceCandidate;
use webrtc::ice_transport::ice_candidate::RTCIceCandidateInit;

async fn handle_message(message: String, peer_connection: Arc<Mutex<RTCPeerConnection>>) -> Result<()> {
    let signal_message: SignalResponse = serde_json::from_str(message.as_str())?;

    match signal_message {
        SignalResponse::IceCandidate { ice_candidate } => {
            let rtc_ice_candidate: RTCIceCandidateInit = serde_json::from_str(&ice_candidate).expect("Could not serialize String to RTCIceCandidateInit");

            let lock = peer_connection.lock().await;
            println!("Adding ice candidate: {:?} to peer connection", &rtc_ice_candidate);
            lock.add_ice_candidate(rtc_ice_candidate).await?;
            drop(lock);

        },
        SignalResponse::Answer { sdp } => {
            // Temporary hack fix this later
            let test: Test = Test {
                sdp: sdp,
                r#type: "answer".to_owned(),
            };
            let test2 = serde_json::to_string(&test)?;

            let answer: RTCSessionDescription = serde_json::from_str(&test2).expect("Could not serialize String to RTCSessionDescription");
            // let answer: RTCSessionDescription = serde_json::from_str(&sdp).expect("Could not serialize String to RTCSessionDescription");

            let lock = peer_connection.lock().await;
            println!("Adding answer: {:?} to peer connection", &answer);
            lock.set_remote_description(answer).await?;
            drop(lock);
        },
        _ => {}
    }

    Ok(())
}

async fn handle_connection(mut incoming: SplitStream<WebSocketStream<MaybeTlsStream<TcpStream>>>, peer_connection: Arc<Mutex<RTCPeerConnection>>) {
    while let Some(message) = incoming.next().await {
        match message {
            Ok(Message::Text(text)) => {
                // Handle text message
                println!("Received text message: {}\n", text);
                handle_message(text, Arc::clone(&peer_connection)).await.expect("Handling message failed");
            },
            Ok(Message::Close(reason)) => {
                // Handle close message
                println!("Received close message. Reason: {:?}\n", reason);
                break;
            }
            Ok(_) => {
                // Do nothing for the other Message types
            }
            Err(err) => {
                println!("Error on incoming ws, error: {:?}\n", &err);
                break;
            },
        }
    }
}

pub async fn setup_webrtc() -> Result<Arc<TrackLocalStaticSample>> {
    // Setup WebRTC stuff, No TURN server for now
    let local_signal_server = "ws://192.168.50.127:8080";

    let (ws_stream, _) = connect_async(local_signal_server).await.expect("Failed to connect to the server.");

    let (signal_outgoing, signal_incoming) = ws_stream.split();

    // Create a MediaEngine object to configure the supported codec
    let mut m = MediaEngine::default();

    m.register_default_codecs()?;

    // Create a InterceptorRegistry. This is the user configurable RTP/RTCP Pipeline.
    // This provides NACKs, RTCP Reports and other features. If you use `webrtc.NewPeerConnection`
    // this is enabled by default. If you are manually managing You MUST create a InterceptorRegistry
    // for each PeerConnection.
    let mut registry = Registry::new();

    // Use the default set of Interceptors
    registry = register_default_interceptors(registry, &mut m)?;

    // Create a video track
    let video_track: Arc<TrackLocalStaticSample> = Arc::new(TrackLocalStaticSample::new(
        RTCRtpCodecCapability {
            mime_type: "video/H264".to_owned(),
            ..Default::default()
        },
        "video".to_owned(),
        "screen".to_owned(),
    ));

    // Create the API object with the MediaEngine
    let api = Arc::new(
        APIBuilder::new()
            .with_media_engine(m)
            .with_interceptor_registry(registry)
            .build(),
    );

    // Prepare the configuration
    let ice_server_urls = vec!["stun:stun.l.google.com:19302".to_owned(), "stun:stun.services.mozilla.com".to_owned()];

    let ice_servers: Vec<RTCIceServer> = ice_server_urls.iter().map(|ice_server_url| {
        RTCIceServer {
            urls: vec![ice_server_url.into()],
            ..Default::default()
        }
    }).collect();

    let config = RTCConfiguration {
        ice_servers: ice_servers,
        ..Default::default()
    };

    // Create a new RTCPeerConnection
    let peer_connection: Arc<Mutex<RTCPeerConnection>> = Arc::new(Mutex::new(api.new_peer_connection(config).await?));

    let peer_connection_clone = Arc::clone(&peer_connection);

    tokio::spawn(async move {
        handle_connection(signal_incoming, Arc::clone(&peer_connection_clone)).await;
    });

    let lock = peer_connection.lock().await;

    // Add this newly created track to the PeerConnection
    let rtp_sender = lock
        .add_track(Arc::clone(&video_track) as Arc<dyn TrackLocal + Send + Sync>)
        .await?;

    // Read incoming RTCP packets
    // Before these packets are returned they are processed by interceptors. For things
    // like NACK this needs to be called.
    tokio::spawn(async move {
        let mut rtcp_buf = vec![0u8; 1500];
        while let Ok((_, _)) = rtp_sender.read(&mut rtcp_buf).await {}
        Result::<()>::Ok(())
    });

    let signal_outgoing = Arc::new(Mutex::new(signal_outgoing));
    let signal_clone = Arc::clone(&signal_outgoing);

    lock.on_ice_candidate(Box::new(move |candidate: Option<RTCIceCandidate>| {
        let outgoing_clone = Arc::clone(&signal_clone);

        Box::pin(async move {
            if let Some(candidate) = candidate {
                let candidate_json = candidate.to_json().unwrap();
                let candidate_json_string = serde_json::to_string(&candidate_json).unwrap();
                
                let clone = Arc::clone(&outgoing_clone);
                let mut ws_outgoing = clone.lock().await;

                let ice_request = SignalRequest::CallerIceCandidate { id: "asdf".to_owned(), ice_candidate: candidate_json_string };
                let ice_request_json = serde_json::to_string(&ice_request).unwrap();

                ws_outgoing.send(Message::Text(ice_request_json)).await.expect("Sending ice candidate failed.");
            }
        })
    }));

    // Makes an offer, sets the LocalDescription, and starts our UDP listeners
    let offer = lock.create_offer(None).await?;
    let sdp = offer.sdp.clone();
    let offer_request = SignalRequest::Offer { id: "asdf".to_owned(), sdp: sdp };
    let offer_request_json = serde_json::to_string(&offer_request).unwrap();

    let mut outgoing_guard = signal_outgoing.lock().await;
    outgoing_guard.send(Message::Text(offer_request_json)).await?;
    drop(outgoing_guard);

    lock.set_local_description(offer.clone()).await?;
    drop(lock);

    // Return video track from which we will start sending h264 encoded video data to
    Ok(video_track)
}