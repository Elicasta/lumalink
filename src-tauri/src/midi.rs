use crate::settings::{
    load_config, remove_virtual_bus as remove_saved_virtual_bus, upsert_virtual_bus,
    VirtualBusRecord,
};
use midir::{Ignore, MidiInput, MidiInputConnection, MidiOutput};
use serde::Serialize;
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter, State};
use uuid::Uuid;

#[cfg(target_os = "macos")]
use midir::{
    os::unix::{VirtualInput, VirtualOutput},
    MidiOutputConnection,
};

#[cfg(target_os = "windows")]
use std::process::Command;

#[derive(Default)]
pub(crate) struct MidiRuntime {
    monitor: Option<MidiInputConnection<()>>,
    routes: Vec<(String, MidiInputConnection<()>)>,
    #[cfg(target_os = "macos")]
    virtual_inputs: Vec<(String, MidiInputConnection<()>)>,
    #[cfg(target_os = "macos")]
    virtual_outputs: Vec<(String, MidiOutputConnection)>,
}

#[derive(Serialize)]
pub(crate) struct MidiDevice {
    id: String,
    name: String,
    direction: String,
    index: usize,
}

#[derive(Serialize)]
pub(crate) struct MidiSnapshot {
    inputs: Vec<MidiDevice>,
    outputs: Vec<MidiDevice>,
}

#[derive(Clone, Serialize)]
pub(crate) struct MidiEvent {
    timestamp: u64,
    source: String,
    bytes: Vec<u8>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct VirtualMidiBackendStatus {
    platform: String,
    available: bool,
    message: String,
}

#[tauri::command]
pub(crate) fn list_midi_devices() -> Result<MidiSnapshot, String> {
    let input = MidiInput::new("LumaLink discovery input").map_err(|error| error.to_string())?;
    let output = MidiOutput::new("LumaLink discovery output").map_err(|error| error.to_string())?;

    let inputs = input
        .ports()
        .iter()
        .enumerate()
        .map(|(index, port)| {
            let name = input
                .port_name(port)
                .unwrap_or_else(|_| format!("MIDI Input {}", index + 1));

            MidiDevice {
                id: format!("in:{name}:{index}"),
                name,
                direction: "input".into(),
                index,
            }
        })
        .collect();

    let outputs = output
        .ports()
        .iter()
        .enumerate()
        .map(|(index, port)| {
            let name = output
                .port_name(port)
                .unwrap_or_else(|_| format!("MIDI Output {}", index + 1));

            MidiDevice {
                id: format!("out:{name}:{index}"),
                name,
                direction: "output".into(),
                index,
            }
        })
        .collect();

    Ok(MidiSnapshot { inputs, outputs })
}

#[tauri::command]
pub(crate) fn start_midi_monitor(
    app: AppHandle,
    runtime: State<Mutex<MidiRuntime>>,
    input_index: usize,
) -> Result<(), String> {
    let mut input = MidiInput::new("LumaLink monitor").map_err(|error| error.to_string())?;
    input.ignore(Ignore::None);

    let ports = input.ports();
    let port = ports
        .get(input_index)
        .ok_or_else(|| "MIDI input no longer exists".to_string())?
        .clone();

    let source = input
        .port_name(&port)
        .unwrap_or_else(|_| format!("Input {}", input_index + 1));

    let app_for_event = app.clone();
    let source_for_event = source.clone();

    let connection = input
        .connect(
            &port,
            "LumaLink monitor",
            move |timestamp, message, _| {
                let _ = app_for_event.emit(
                    "midi-event",
                    MidiEvent {
                        timestamp,
                        source: source_for_event.clone(),
                        bytes: message.to_vec(),
                    },
                );
            },
            (),
        )
        .map_err(|error| error.to_string())?;

    runtime
        .lock()
        .map_err(|_| "MIDI runtime lock poisoned".to_string())?
        .monitor = Some(connection);

    Ok(())
}

#[tauri::command]
pub(crate) fn stop_midi_monitor(runtime: State<Mutex<MidiRuntime>>) -> Result<(), String> {
    runtime
        .lock()
        .map_err(|_| "MIDI runtime lock poisoned".to_string())?
        .monitor = None;

    Ok(())
}

#[tauri::command]
pub(crate) fn start_midi_route(
    app: AppHandle,
    runtime: State<Mutex<MidiRuntime>>,
    input_index: usize,
    output_index: usize,
) -> Result<String, String> {
    let mut input = MidiInput::new("LumaLink route input").map_err(|error| error.to_string())?;
    input.ignore(Ignore::None);

    let output = MidiOutput::new("LumaLink route output").map_err(|error| error.to_string())?;

    let input_ports = input.ports();
    let output_ports = output.ports();

    let input_port = input_ports
        .get(input_index)
        .ok_or_else(|| "MIDI source disappeared".to_string())?
        .clone();

    let output_port = output_ports
        .get(output_index)
        .ok_or_else(|| "MIDI destination disappeared".to_string())?
        .clone();

    let source_name = input
        .port_name(&input_port)
        .unwrap_or_else(|_| format!("Input {}", input_index + 1));

    let destination_name = output
        .port_name(&output_port)
        .unwrap_or_else(|_| format!("Output {}", output_index + 1));

    let output_connection = output
        .connect(&output_port, "LumaLink route")
        .map_err(|error| error.to_string())?;

    let shared_output = Arc::new(Mutex::new(output_connection));
    let output_for_callback = Arc::clone(&shared_output);
    let app_for_event = app.clone();
    let event_source = source_name.clone();

    let connection = input
        .connect(
            &input_port,
            "LumaLink route",
            move |timestamp, message, _| {
                if let Ok(mut output) = output_for_callback.lock() {
                    let _ = output.send(message);
                }

                let _ = app_for_event.emit(
                    "midi-event",
                    MidiEvent {
                        timestamp,
                        source: event_source.clone(),
                        bytes: message.to_vec(),
                    },
                );
            },
            (),
        )
        .map_err(|error| error.to_string())?;

    let route_id = Uuid::new_v4().to_string();

    runtime
        .lock()
        .map_err(|_| "MIDI runtime lock poisoned".to_string())?
        .routes
        .push((route_id.clone(), connection));

    Ok(format!("{route_id}:{source_name} -> {destination_name}"))
}

#[tauri::command]
pub(crate) fn stop_all_midi_routes(
    runtime: State<Mutex<MidiRuntime>>,
) -> Result<(), String> {
    runtime
        .lock()
        .map_err(|_| "MIDI runtime lock poisoned".to_string())?
        .routes
        .clear();

    Ok(())
}

#[tauri::command]
pub(crate) fn midi_panic() -> Result<(), String> {
    let discovery =
        MidiOutput::new("LumaLink panic discovery").map_err(|error| error.to_string())?;

    let output_names: Vec<String> = discovery
        .ports()
        .iter()
        .enumerate()
        .map(|(index, port)| {
            discovery
                .port_name(port)
                .unwrap_or_else(|_| format!("Output {}", index + 1))
        })
        .collect();

    let mut failures = Vec::new();

    for (index, name) in output_names.iter().enumerate() {
        let output = MidiOutput::new("LumaLink panic").map_err(|error| error.to_string())?;
        let ports = output.ports();

        let Some(port) = ports.get(index) else {
            continue;
        };

        match output.connect(port, "LumaLink panic") {
            Ok(mut connection) => {
                for channel in 0..16u8 {
                    let _ = connection.send(&[0xB0 | channel, 120, 0]);
                    let _ = connection.send(&[0xB0 | channel, 123, 0]);
                }
            }
            Err(error) => failures.push(format!("{name}: {error}")),
        }
    }

    if failures.is_empty() {
        Ok(())
    } else {
        Err(failures.join("; "))
    }
}

#[tauri::command]
pub(crate) fn list_virtual_midi_buses(app: AppHandle) -> Result<Vec<VirtualBusRecord>, String> {
    Ok(load_config(&app)?.virtual_buses)
}

#[cfg(target_os = "macos")]
fn create_platform_virtual_bus(
    app: &AppHandle,
    runtime: &Mutex<MidiRuntime>,
    name: &str,
) -> Result<Option<String>, String> {
    let mut guard = runtime
        .lock()
        .map_err(|_| "MIDI runtime lock poisoned".to_string())?;

    if guard.virtual_inputs.iter().any(|(existing, _)| existing == name) {
        return Ok(None);
    }

    let mut input =
        MidiInput::new("LumaLink virtual input").map_err(|error| error.to_string())?;
    input.ignore(Ignore::None);

    let input_name = format!("{name} · IN");
    let event_source = input_name.clone();
    let app_for_event = app.clone();

    let input_connection = input
        .create_virtual(
            &input_name,
            move |timestamp, message, _| {
                let _ = app_for_event.emit(
                    "midi-event",
                    MidiEvent {
                        timestamp,
                        source: event_source.clone(),
                        bytes: message.to_vec(),
                    },
                );
            },
            (),
        )
        .map_err(|error| error.to_string())?;

    let output =
        MidiOutput::new("LumaLink virtual output").map_err(|error| error.to_string())?;

    let output_name = format!("{name} · OUT");
    let output_connection = output
        .create_virtual(&output_name)
        .map_err(|error| error.to_string())?;

    guard
        .virtual_inputs
        .push((name.to_string(), input_connection));
    guard
        .virtual_outputs
        .push((name.to_string(), output_connection));

    Ok(None)
}

#[cfg(target_os = "macos")]
fn remove_platform_virtual_bus(
    runtime: &Mutex<MidiRuntime>,
    record: &VirtualBusRecord,
) -> Result<(), String> {
    let mut guard = runtime
        .lock()
        .map_err(|_| "MIDI runtime lock poisoned".to_string())?;

    guard.virtual_inputs.retain(|(name, _)| name != &record.name);
    guard.virtual_outputs.retain(|(name, _)| name != &record.name);

    Ok(())
}

#[cfg(target_os = "windows")]
fn powershell(script: &str) -> Result<String, String> {
    let output = Command::new("pwsh")
        .args([
            "-NoLogo",
            "-NoProfile",
            "-NonInteractive",
            "-Command",
            script,
        ])
        .output()
        .map_err(|error| {
            format!(
                "Windows MIDI Services PowerShell tools require PowerShell 7 and the WindowsMidiServices module: {error}"
            )
        })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
        return Err(if stderr.is_empty() { stdout } else { stderr });
    }

    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

#[cfg(target_os = "windows")]
fn escape_powershell_single_quoted(value: &str) -> String {
    value.replace('\'', "''")
}

#[cfg(target_os = "windows")]
fn windows_midi_services_ready() -> Result<bool, String> {
    let script = "$ErrorActionPreference='Stop'; Import-Module WindowsMidiServices; Start-Midi | Out-Null; if (Get-Command New-MidiBasicLoopback -ErrorAction SilentlyContinue) { 'READY' }";
    Ok(powershell(script)?.contains("READY"))
}

#[cfg(target_os = "windows")]
fn create_platform_virtual_bus(
    _app: &AppHandle,
    _runtime: &Mutex<MidiRuntime>,
    name: &str,
) -> Result<Option<String>, String> {
    if !windows_midi_services_ready()? {
        return Err("Windows MIDI Services basic loopbacks are not available on this PC.".into());
    }

    let escaped_name = escape_powershell_single_quoted(name);
    let script = format!(
        "$ErrorActionPreference='Stop'; Import-Module WindowsMidiServices; Start-Midi | Out-Null;          $existing = Get-MidiBasicLoopback | Where-Object {{ $_.Endpoint.Name -eq '{escaped_name}' }} | Select-Object -First 1;          if ($null -eq $existing) {{ New-MidiBasicLoopback -Name '{escaped_name}' | Out-Null;          $existing = Get-MidiBasicLoopback | Where-Object {{ $_.Endpoint.Name -eq '{escaped_name}' }} | Select-Object -First 1 }};          if ($null -eq $existing) {{ throw 'Windows MIDI Services created no matching loopback endpoint.' }};          $existing.AssociationId.ToString()"
    );

    let association_id = powershell(&script)?;
    Ok(if association_id.is_empty() {
        None
    } else {
        Some(association_id)
    })
}

#[cfg(target_os = "windows")]
fn remove_platform_virtual_bus(
    _runtime: &Mutex<MidiRuntime>,
    record: &VirtualBusRecord,
) -> Result<(), String> {
    let escaped_name = escape_powershell_single_quoted(&record.name);
    let script = format!(
        "$ErrorActionPreference='Stop'; Import-Module WindowsMidiServices; Start-Midi | Out-Null;          Get-MidiBasicLoopback | Where-Object {{ $_.Endpoint.Name -eq '{escaped_name}' }} | Remove-MidiBasicLoopback"
    );

    powershell(&script).map(|_| ())
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
fn create_platform_virtual_bus(
    _app: &AppHandle,
    _runtime: &Mutex<MidiRuntime>,
    _name: &str,
) -> Result<Option<String>, String> {
    Err("Virtual MIDI buses are only implemented on macOS and Windows.".into())
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
fn remove_platform_virtual_bus(
    _runtime: &Mutex<MidiRuntime>,
    _record: &VirtualBusRecord,
) -> Result<(), String> {
    Ok(())
}

#[tauri::command]
pub(crate) fn virtual_midi_backend_status() -> VirtualMidiBackendStatus {
    #[cfg(target_os = "macos")]
    {
        return VirtualMidiBackendStatus {
            platform: "macOS".into(),
            available: true,
            message: "CoreMIDI virtual endpoints are available.".into(),
        };
    }

    #[cfg(target_os = "windows")]
    {
        return match windows_midi_services_ready() {
            Ok(true) => VirtualMidiBackendStatus {
                platform: "Windows".into(),
                available: true,
                message: "Windows MIDI Services basic loopbacks are available.".into(),
            },
            Ok(false) => VirtualMidiBackendStatus {
                platform: "Windows".into(),
                available: false,
                message: "Windows MIDI Services basic loopbacks are not installed or enabled.".into(),
            },
            Err(error) => VirtualMidiBackendStatus {
                platform: "Windows".into(),
                available: false,
                message: error,
            },
        };
    }

    #[allow(unreachable_code)]
    VirtualMidiBackendStatus {
        platform: std::env::consts::OS.into(),
        available: false,
        message: "Virtual MIDI buses are not implemented on this platform.".into(),
    }
}

#[tauri::command]
pub(crate) fn create_virtual_midi_bus(
    app: AppHandle,
    runtime: State<Mutex<MidiRuntime>>,
    name: String,
) -> Result<VirtualBusRecord, String> {
    let trimmed = name.trim();

    if trimmed.is_empty() {
        return Err("Virtual bus name cannot be empty".into());
    }

    let config = load_config(&app)?;
    if config
        .virtual_buses
        .iter()
        .any(|record| record.name.eq_ignore_ascii_case(trimmed))
    {
        return Err(format!("A virtual bus named \"{trimmed}\" already exists."));
    }

    let association_id = create_platform_virtual_bus(&app, &runtime, trimmed)?;

    let backend = if cfg!(target_os = "macos") {
        "coremidi"
    } else if cfg!(target_os = "windows") {
        "windows-midi-services-basic-loopback"
    } else {
        "unsupported"
    };

    upsert_virtual_bus(
        &app,
        VirtualBusRecord {
            id: Uuid::new_v4().to_string(),
            name: trimmed.to_string(),
            backend: backend.into(),
            association_id,
        },
    )
}

#[tauri::command]
pub(crate) fn remove_virtual_midi_bus(
    app: AppHandle,
    runtime: State<Mutex<MidiRuntime>>,
    id: String,
) -> Result<(), String> {
    let config = load_config(&app)?;
    let record = config
        .virtual_buses
        .iter()
        .find(|record| record.id == id)
        .cloned()
        .ok_or_else(|| "Virtual bus not found".to_string())?;

    remove_platform_virtual_bus(&runtime, &record)?;
    remove_saved_virtual_bus(&app, &id)?;

    Ok(())
}

pub(crate) fn restore_virtual_midi_buses(
    app: &AppHandle,
    runtime: &Mutex<MidiRuntime>,
) -> Result<Vec<String>, String> {
    let mut restored = Vec::new();
    let mut config = load_config(app)?;
    let mut changed = false;

    for record in &mut config.virtual_buses {
        match create_platform_virtual_bus(app, runtime, &record.name) {
            Ok(association_id) => {
                if association_id.is_some() && association_id != record.association_id {
                    record.association_id = association_id;
                    changed = true;
                }
                restored.push(record.name.clone());
            }
            Err(error) => {
                eprintln!("LumaLink could not restore virtual bus {}: {}", record.name, error);
            }
        }
    }

    if changed {
        crate::settings::save_config(app, &config)?;
    }

    Ok(restored)
}

#[cfg(test)]
mod tests {
    #[test]
    fn midi_status_channel_math_is_stable() {
        for channel in 0..16u8 {
            assert_eq!((0xB0 | channel) & 0x0F, channel);
        }
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn powershell_single_quote_escaping_is_safe() {
        assert_eq!(super::escape_powershell_single_quoted("Kid's Bus"), "Kid''s Bus");
    }
}
