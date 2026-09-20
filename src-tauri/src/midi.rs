use midir::{Ignore, MidiInput, MidiInputConnection, MidiOutput};
use serde::Serialize;
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter, State};
use uuid::Uuid;

#[cfg(unix)]
use midir::{
    os::unix::{VirtualInput, VirtualOutput},
    MidiOutputConnection,
};

#[derive(Default)]
pub(crate) struct MidiRuntime {
    monitor: Option<MidiInputConnection<()>>,
    routes: Vec<(String, MidiInputConnection<()>)>,
    #[cfg(unix)]
    virtual_inputs: Vec<(String, MidiInputConnection<()>)>,
    #[cfg(unix)]
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

    Ok(format!(
        "{route_id}:{source_name} -> {destination_name}"
    ))
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

#[cfg(unix)]
#[tauri::command]
pub(crate) fn create_virtual_midi_bus(
    app: AppHandle,
    runtime: State<Mutex<MidiRuntime>>,
    name: String,
) -> Result<String, String> {
    let trimmed = name.trim();

    if trimmed.is_empty() {
        return Err("Virtual bus name cannot be empty".into());
    }

    let mut input =
        MidiInput::new("LumaLink virtual input").map_err(|error| error.to_string())?;
    input.ignore(Ignore::None);

    let input_name = format!("{trimmed} · IN");
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

    let output_name = format!("{trimmed} · OUT");

    let output_connection = output
        .create_virtual(&output_name)
        .map_err(|error| error.to_string())?;

    let mut guard = runtime
        .lock()
        .map_err(|_| "MIDI runtime lock poisoned".to_string())?;

    guard
        .virtual_inputs
        .push((trimmed.to_string(), input_connection));

    guard
        .virtual_outputs
        .push((trimmed.to_string(), output_connection));

    Ok(format!("Created CoreMIDI virtual bus: {trimmed}"))
}

#[cfg(not(unix))]
#[tauri::command]
pub(crate) fn create_virtual_midi_bus(
    _app: AppHandle,
    _runtime: State<Mutex<MidiRuntime>>,
    _name: String,
) -> Result<String, String> {
    Err(
        "Windows virtual buses are not active in this build yet. Physical MIDI routing works now; native Windows MIDI Services virtual-device support is the next backend module."
            .into(),
    )
}

#[cfg(test)]
mod tests {
    #[test]
    fn midi_status_channel_math_is_stable() {
        for channel in 0..16u8 {
            assert_eq!((0xB0 | channel) & 0x0F, channel);
        }
    }
}
