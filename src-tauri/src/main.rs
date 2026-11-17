// Pas de fenêtre de console en version installée.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod adb;
mod android;
mod commands;
mod devices;
mod events;
mod input;
mod ios;
mod mjpeg;
mod models;
mod paths;
mod protocol;

use tauri::{Manager, RunEvent};

fn main() {
    let app = tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .manage(commands::AppState::default())
        .invoke_handler(tauri::generate_handler![
            commands::devices_list,
            commands::android_start,
            commands::android_stop,
            commands::android_input,
            commands::files_list,
            commands::files_push,
            commands::files_pull,
            commands::save_bytes,
            commands::ios_status,
            commands::ios_start,
            commands::ios_stop,
            commands::ios_tunnel,
            commands::ios_pair,
            commands::ios_reveal,
            commands::ios_tap,
            commands::ios_swipe,
            commands::ios_home,
        ])
        .build(tauri::generate_context!())
        .expect("ShareScreen n'a pas pu démarrer");

    app.run(|app, event| {
        if let RunEvent::Exit = event {
            let state = app.state::<commands::AppState>();
            tauri::async_runtime::block_on(commands::shutdown(state.inner()));
        }
    });
}
