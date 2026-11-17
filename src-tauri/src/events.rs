//! Format des événements envoyés à l'interface par un canal binaire.
//! Chaque message commence par un octet de type ; le reste dépend du type. Entiers big-endian.
//!
//! - READY / SIZE : JSON UTF-8.
//! - VIDEO_CONFIG : codec (u16 + UTF-8), puis description avcC.
//! - VIDEO_FRAME : clé (u8), horodatage µs (u64), données AVCC.
//! - AUDIO_CONFIG : description OpusHead.
//! - AUDIO_FRAME : horodatage µs (u64), paquet Opus.
//! - CLOSED : raison en UTF-8.
//! - IOS_FRAME : image JPEG.

pub const READY: u8 = 1;
pub const SIZE: u8 = 2;
pub const VIDEO_CONFIG: u8 = 3;
pub const VIDEO_FRAME: u8 = 4;
pub const AUDIO_CONFIG: u8 = 5;
pub const AUDIO_FRAME: u8 = 6;
pub const CLOSED: u8 = 7;
pub const IOS_FRAME: u8 = 8;

pub fn raw(kind: u8, payload: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(1 + payload.len());
    out.push(kind);
    out.extend_from_slice(payload);
    out
}

pub fn json(kind: u8, value: &serde_json::Value) -> Vec<u8> {
    raw(kind, value.to_string().as_bytes())
}

pub fn video_config(codec: &str, description: &[u8]) -> Vec<u8> {
    let mut payload = Vec::with_capacity(2 + codec.len() + description.len());
    payload.extend_from_slice(&(codec.len() as u16).to_be_bytes());
    payload.extend_from_slice(codec.as_bytes());
    payload.extend_from_slice(description);
    raw(VIDEO_CONFIG, &payload)
}

pub fn timed_frame(kind: u8, key: Option<bool>, timestamp: u64, data: &[u8]) -> Vec<u8> {
    let mut payload = Vec::with_capacity(9 + data.len());
    if let Some(k) = key {
        payload.push(u8::from(k));
    }
    payload.extend_from_slice(&timestamp.to_be_bytes());
    payload.extend_from_slice(data);
    raw(kind, &payload)
}
