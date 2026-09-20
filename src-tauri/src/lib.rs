mod midi;
mod ndi;

use midi::{
    create_virtual_midi_bus,
    list_midi_devices,
    midi_panic,
    start_midi_monitor,
    start_midi_route,
    stop_all_midi_routes,
    stop_midi_monitor,
    MidiRuntime,
};
use ndi::{discover_ndi_sources, ndi_runtime_status};
use std::sync::Mutex;
use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .manage(Mutex::new(MidiRuntime::default()))
        .invoke_handler(tauri::generate_handler![
            list_midi_devices,
            start_midi_monitor,
            stop_midi_monitor,
            start_midi_route,
            stop_all_midi_routes,
            midi_panic,
            create_virtual_midi_bus,
            ndi_runtime_status,
            discover_ndi_sources
        ])
        .setup(|app| {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.set_title("LumaLink");
            }
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running LumaLink");
}
