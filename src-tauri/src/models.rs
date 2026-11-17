//! Types renvoyés à l'interface (sérialisés en camelCase, comme src/shared/types.ts).

use serde::Serialize;

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DeviceInfo {
    /// "android" ou "ios".
    pub kind: String,
    /// Numéro de série adb ou UDID iOS.
    pub id: String,
    pub name: String,
    /// ready, unauthorized, offline, untrusted.
    pub state: String,
    pub detail: Option<String>,
    pub hint: Option<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct IosStatus {
    pub ios: bool,
    pub version: String,
    pub paired: bool,
    /// None si inconnu ou non applicable (iOS < 16).
    pub developer_mode: Option<bool>,
    pub tunnel_required: bool,
    pub tunnel: bool,
    pub wda: bool,
    pub message: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DevicesResult {
    pub devices: Vec<DeviceInfo>,
    pub errors: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PushResult {
    pub name: String,
    pub ok: bool,
    pub error: Option<String>,
}
