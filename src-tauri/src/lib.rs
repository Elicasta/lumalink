mod midi;
mod ndi;
mod settings;

use midi::{
    create_virtual_midi_bus,
    delete_midi_route,
    list_midi_devices,
    list_saved_midi_routes,
    list_virtual_midi_buses,
    midi_panic,
    reconnect_enabled_midi_routes,
    remove_virtual_midi_bus,
    restore_saved_midi_routes,
    restore_virtual_midi_buses,
    save_midi_route,
    set_midi_route_enabled,
    start_midi_monitor,
    stop_all_midi_routes,
    stop_midi_monitor,
    virtual_midi_backend_status,
    MidiRuntime,
};
use ndi::{discover_ndi_sources, ndi_runtime_status};
use std::sync::Mutex;
use tauri::{
    menu::{Menu, MenuItem},
    tray::TrayIconBuilder,
    Manager, WindowEvent,
};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .manage(Mutex::new(MidiRuntime::default()))
        .invoke_handler(tauri::generate_handler![
            list_midi_devices,
            start_midi_monitor,
            stop_midi_monitor,
            list_saved_midi_routes,
            save_midi_route,
            set_midi_route_enabled,
            delete_midi_route,
            reconnect_enabled_midi_routes,
            stop_all_midi_routes,
            midi_panic,
            list_virtual_midi_buses,
            virtual_midi_backend_status,
            create_virtual_midi_bus,
            remove_virtual_midi_bus,
            ndi_runtime_status,
            discover_ndi_sources
        ])
        .on_window_event(|window, event| {
            if let WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                let _ = window.hide();
            }
        })
        .setup(|app| {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.set_title("LumaLink");
            }

            let show_item = MenuItem::with_id(
                app,
                "show",
                "Open LumaLink",
                true,
                None::<&str>,
            )?;
            let quit_item = MenuItem::with_id(
                app,
                "quit",
                "Quit LumaLink",
                true,
                None::<&str>,
            )?;
            let tray_menu = Menu::with_items(app, &[&show_item, &quit_item])?;

            TrayIconBuilder::with_id("lumalink-tray")
                .tooltip("LumaLink")
                .menu(&tray_menu)
                .show_menu_on_left_click(false)
                .on_menu_event(|app, event| match event.id().as_ref() {
                    "show" => {
                        if let Some(window) = app.get_webview_window("main") {
                            let _ = window.show();
                            let _ = window.set_focus();
                        }
                    }
                    "quit" => app.exit(0),
                    _ => {}
                })
                .build(app)?;

            let midi_runtime = app.state::<Mutex<MidiRuntime>>();

            if let Err(error) =
                restore_virtual_midi_buses(app.handle(), midi_runtime.inner())
            {
                eprintln!("LumaLink virtual bus restore failed: {error}");
            }

            match restore_saved_midi_routes(app.handle(), midi_runtime.inner()) {
                Ok(statuses) => {
                    for status in statuses {
                        if let Some(error) = status.error {
                            eprintln!(
                                "LumaLink route restore failed for {}: {}",
                                status.route.name,
                                error
                            );
                        }
                    }
                }
                Err(error) => eprintln!("LumaLink route restore failed: {error}"),
            }

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running LumaLink");
}
