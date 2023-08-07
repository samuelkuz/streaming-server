use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SignalRequest {
    Offer {
        sdp: String,
    },
    Answer {
        id: String,
        sdp: String,
    },
    CallerIceCandidate {
        id: String,
        ice_candidate: String,
    },
    GetOffer {
        id: String,
    },
    PeerIceCandidate {
        id: String,
        ice_candidate: String,
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Test {
    pub sdp: String,
    pub r#type: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SignalResponse {
    Session {
        id: String,
    },
    Offer {
        sdp: String,
    },
    Answer {
        sdp: String,
    },
    IceCandidate {
        ice_candidate: String,
    },
}

