//! Commandes appelées par l'interface (invoke) et état partagé de l'application.

use std::collections::HashMap;

use tauri::ipc::{InvokeBody, Request};
use tauri::State;
use tokio::sync::Mutex;

use crate::adb;
use crate::android::{AndroidOptions, AndroidSession, Emitter};
use crate::devices::{self, AndroidProps};
use crate::input::{self, InputEvent};
use crate::ios::{self, IosStream, Wda};
use crate::models::{DeviceInfo, DevicesResult, IosStatus, PushResult};

const DOWNLOAD_DIR: &str = "/sdcard/Download/";

/// État partagé : une seule session à la fois (une seule fenêtre, un seul appareil affiché).
#[derive(Default)]
pub struct AppState {
    android: Mutex<Option<AndroidSession>>,
    ios: Mutex<Option<IosStream>>,
    wda: Mutex<Option<Wda>>,
    props: Mutex<HashMap<String, AndroidProps>>,
}

/// Propriétés d'une tablette, lues une fois par numéro de série.
async fn android_props(state: &AppState, serial: &str) -> AndroidProps {
    if let Some(p) = state.props.lock().await.get(serial) {
        return p.clone();
    }
    let (market, maker, model, release, sdk) = tokio::join!(
        adb::getprop(serial, "ro.product.marketname"),
        adb::getprop(serial, "ro.product.manufacturer"),
        adb::getprop(serial, "ro.product.model"),
        adb::getprop(serial, "ro.build.version.release"),
        adb::getprop(serial, "ro.build.version.sdk"),
    );
    let props = AndroidProps {
        market_name: market.unwrap_or_default(),
        manufacturer: maker.unwrap_or_default(),
        model: model.unwrap_or_default(),
        release: release.unwrap_or_default(),
        api: sdk.ok().and_then(|s| s.parse().ok()).unwrap_or(0),
    };
    state.props.lock().await.insert(serial.to_string(), props.clone());
    props
}

async fn stop_android(state: &AppState) {
    let session = state.android.lock().await.take();
    if let Some(session) = session {
        session.stop().await;
    }
}

async fn stop_ios(state: &AppState) {
    let stream = state.ios.lock().await.take();
    if let Some(stream) = stream {
        stream.stop();
    }
}

/// Arrêt propre à la fermeture de l'application.
pub async fn shutdown(state: &AppState) {
    stop_android(state).await;
    stop_ios(state).await;
}

/// Nom de fichier sûr pour Windows et Android.
fn safe_name(name: &str) -> String {
    let cleaned: String = name
        .chars()
        .map(|c| if "\\/:*?\"<>|".contains(c) || c.is_control() { '_' } else { c })
        .take(200)
        .collect();
    if cleaned.is_empty() {
        "fichier".into()
    } else {
        cleaned
    }
}

/// Décodage minimal des pourcentages d'un en-tête (chemins non ASCII).
fn percent_decode(input: &str) -> String {
    let bytes = input.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let Ok(v) = u8::from_str_radix(&input[i + 1..i + 3], 16) {
                out.push(v);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

#[tauri::command]
pub async fn devices_list(state: State<'_, AppState>) -> Result<DevicesResult, String> {
    let mut devices_out = Vec::new();
    let mut errors = Vec::new();
    match adb::list_devices().await {
        Ok(list) => {
            for d in list {
                if d.state != "device" {
                    devices_out.push(DeviceInfo {
                        kind: "android".into(),
                        id: d.serial,
                        name: if d.model.is_empty() { "Appareil Android".into() } else { d.model },
                        state: if d.state == "unauthorized" { "unauthorized".into() } else { "offline".into() },
                        detail: Some(if d.state == "unauthorized" {
                            "Acceptez le débogage USB sur l'appareil.".into()
                        } else {
                            "Hors ligne. Rebranchez le câble USB.".into()
                        }),
                        hint: None,
                    });
                    continue;
                }
                let props = android_props(&state, &d.serial).await;
                let desc = devices::describe_android(&props);
                devices_out.push(DeviceInfo {
                    kind: "android".into(),
                    id: d.serial,
                    name: desc.name,
                    state: "ready".into(),
                    detail: Some(desc.detail),
                    hint: desc.hint,
                });
            }
        }
        Err(e) => errors.push(format!("Android : {e}")),
    }
    devices_out.extend(ios::list_devices().await);
    Ok(DevicesResult { devices: devices_out, errors })
}

#[tauri::command]
pub async fn android_start(options: AndroidOptions, on_event: Emitter, state: State<'_, AppState>) -> Result<(), String> {
    stop_ios(&state).await;
    stop_android(&state).await;
    let props = android_props(&state, &options.serial).await;
    if !devices::android_supported(props.api) {
        return Err(format!(
            "Android {} n'est pas pris en charge (minimum Android 5.0).",
            if props.release.is_empty() { "?" } else { props.release.as_str() }
        ));
    }
    // Le son n'existe qu'à partir d'Android 11 : on ne le demande pas plus bas.
    let mut opts = options;
    opts.audio = opts.audio && devices::android_audio_supported(props.api);
    let session = AndroidSession::start(opts, on_event).await?;
    *state.android.lock().await = Some(session);
    Ok(())
}

#[tauri::command]
pub async fn android_stop(state: State<'_, AppState>) -> Result<(), String> {
    stop_android(&state).await;
    Ok(())
}

#[tauri::command]
pub async fn android_input(event: InputEvent, state: State<'_, AppState>) -> Result<(), String> {
    let guard = state.android.lock().await;
    if let Some(session) = guard.as_ref() {
        let (width, height) = session.screen_size();
        for msg in input::input_to_messages(&event, width, height) {
            session.send(msg);
        }
    }
    Ok(())
}

#[tauri::command]
pub async fn files_list(serial: String) -> Result<Vec<String>, String> {
    adb::list_dir(&serial, DOWNLOAD_DIR).await
}

#[tauri::command]
pub async fn files_push(serial: String, paths: Vec<String>) -> Vec<PushResult> {
    let mut results = Vec::new();
    for path in paths.into_iter().take(200) {
        let file = std::path::Path::new(&path);
        let name = safe_name(&file.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default());
        let remote = format!("{DOWNLOAD_DIR}{name}");
        match adb::push(&serial, &path, &remote).await {
            Ok(()) => results.push(PushResult { name, ok: true, error: None }),
            Err(e) => results.push(PushResult { name, ok: false, error: Some(e) }),
        }
    }
    results
}

#[tauri::command]
pub async fn files_pull(serial: String, name: String, local_path: String) -> Result<(), String> {
    let remote = format!("{DOWNLOAD_DIR}{}", safe_name(&name));
    adb::pull(&serial, &remote, &local_path).await
}

/// Écrit des octets bruts (enregistrement, capture) à un chemin choisi par l'utilisateur.
/// Le chemin arrive dans l'en-tête "path" (pourcentage-encodé) ; le corps est envoyé en binaire.
#[tauri::command]
pub async fn save_bytes(request: Request<'_>) -> Result<(), String> {
    let raw_path = request
        .headers()
        .get("path")
        .and_then(|v| v.to_str().ok())
        .ok_or("Chemin d'enregistrement manquant.")?;
    let path = percent_decode(raw_path);
    let bytes = match request.body() {
        InvokeBody::Raw(bytes) => bytes,
        InvokeBody::Json(_) => return Err("Données binaires attendues.".into()),
    };
    tokio::fs::write(&path, bytes).await.map_err(|e| format!("Écriture impossible : {e}"))
}

#[tauri::command]
pub async fn apple_certs_status() -> bool {
    ios::apple_certs_installed().await
}

#[tauri::command]
pub async fn apple_certs_install() -> Result<(), String> {
    ios::install_apple_certs().await
}

#[tauri::command]
pub async fn ios_status(udid: Option<String>) -> IosStatus {
    ios::status(udid).await
}

#[tauri::command]
pub async fn ios_start(udid: String, on_event: Emitter, state: State<'_, AppState>) -> Result<(), String> {
    stop_android(&state).await;
    stop_ios(&state).await;
    ios::prepare(&udid).await?;
    let stream = IosStream::start(udid, on_event).await?;
    *state.ios.lock().await = Some(stream);
    Ok(())
}

#[tauri::command]
pub async fn ios_stop(state: State<'_, AppState>) -> Result<(), String> {
    stop_ios(&state).await;
    Ok(())
}

#[tauri::command]
pub async fn ios_tunnel() -> Result<(), String> {
    ios::start_tunnel_elevated().await
}

#[tauri::command]
pub async fn ios_pair(udid: String) -> Result<(), String> {
    ios::pair(&udid).await
}

#[tauri::command]
pub async fn ios_reveal(udid: String) -> Result<(), String> {
    ios::reveal_developer_mode(&udid).await
}

#[tauri::command]
pub async fn ios_tap(nx: f64, ny: f64, state: State<'_, AppState>) -> Result<(), String> {
    let mut guard = state.wda.lock().await;
    let wda = guard.get_or_insert_with(Wda::new);
    wda.tap(nx, ny).await
}

#[tauri::command]
pub async fn ios_swipe(fx: f64, fy: f64, tx: f64, ty: f64, state: State<'_, AppState>) -> Result<(), String> {
    let mut guard = state.wda.lock().await;
    let wda = guard.get_or_insert_with(Wda::new);
    wda.swipe(fx, fy, tx, ty).await
}

#[tauri::command]
pub async fn ios_home(state: State<'_, AppState>) -> Result<(), String> {
    let mut guard = state.wda.lock().await;
    let wda = guard.get_or_insert_with(Wda::new);
    wda.home().await
}
