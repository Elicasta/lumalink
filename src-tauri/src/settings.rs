use serde::{Deserialize, Serialize};
use std::{fs, path::PathBuf};
use tauri::{AppHandle, Manager};

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct VirtualBusRecord {
    pub id: String,
    pub name: String,
    pub backend: String,
    pub association_id: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MidiRouteTransform {
    #[serde(default)]
    pub input_channel: Option<u8>,
    #[serde(default)]
    pub output_channel: Option<u8>,
    #[serde(default)]
    pub transpose: i8,
    #[serde(default = "default_velocity_percent")]
    pub velocity_percent: u8,
    #[serde(default)]
    pub cc_from: Option<u8>,
    #[serde(default)]
    pub cc_to: Option<u8>,
    #[serde(default)]
    pub block_timing: bool,
    #[serde(default)]
    pub block_sysex: bool,
}

fn default_velocity_percent() -> u8 {
    100
}

impl Default for MidiRouteTransform {
    fn default() -> Self {
        Self {
            input_channel: None,
            output_channel: None,
            transpose: 0,
            velocity_percent: 100,
            cc_from: None,
            cc_to: None,
            block_timing: false,
            block_sysex: false,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MidiRouteRecord {
    pub id: String,
    pub name: String,
    pub input_name: String,
    pub output_name: String,
    pub enabled: bool,
    #[serde(default)]
    pub transform: MidiRouteTransform,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct LumaLinkConfig {
    #[serde(default)]
    pub virtual_buses: Vec<VirtualBusRecord>,
    #[serde(default)]
    pub midi_routes: Vec<MidiRouteRecord>,
}

fn config_path(app: &AppHandle) -> Result<PathBuf, String> {
    let directory = app
        .path()
        .app_config_dir()
        .map_err(|error| format!("Could not resolve LumaLink config directory: {error}"))?;

    fs::create_dir_all(&directory)
        .map_err(|error| format!("Could not create LumaLink config directory: {error}"))?;

    Ok(directory.join("lumalink.json"))
}

pub(crate) fn load_config(app: &AppHandle) -> Result<LumaLinkConfig, String> {
    let path = config_path(app)?;

    if !path.exists() {
        return Ok(LumaLinkConfig::default());
    }

    let text = fs::read_to_string(&path)
        .map_err(|error| format!("Could not read {}: {error}", path.display()))?;

    serde_json::from_str(&text)
        .map_err(|error| format!("Could not parse {}: {error}", path.display()))
}

pub(crate) fn save_config(app: &AppHandle, config: &LumaLinkConfig) -> Result<(), String> {
    let path = config_path(app)?;
    let temporary_path = path.with_extension("json.tmp");

    let json = serde_json::to_string_pretty(config)
        .map_err(|error| format!("Could not serialize LumaLink config: {error}"))?;

    fs::write(&temporary_path, json)
        .map_err(|error| format!("Could not write {}: {error}", temporary_path.display()))?;

    if path.exists() {
        fs::remove_file(&path)
            .map_err(|error| format!("Could not replace {}: {error}", path.display()))?;
    }

    fs::rename(&temporary_path, &path)
        .map_err(|error| format!("Could not finalize {}: {error}", path.display()))
}

pub(crate) fn upsert_virtual_bus(
    app: &AppHandle,
    record: VirtualBusRecord,
) -> Result<VirtualBusRecord, String> {
    let mut config = load_config(app)?;

    if let Some(existing) = config
        .virtual_buses
        .iter_mut()
        .find(|existing| existing.id == record.id || existing.name.eq_ignore_ascii_case(&record.name))
    {
        *existing = record.clone();
    } else {
        config.virtual_buses.push(record.clone());
    }

    save_config(app, &config)?;
    Ok(record)
}

pub(crate) fn remove_virtual_bus(
    app: &AppHandle,
    id: &str,
) -> Result<Option<VirtualBusRecord>, String> {
    let mut config = load_config(app)?;

    let Some(index) = config.virtual_buses.iter().position(|record| record.id == id) else {
        return Ok(None);
    };

    let record = config.virtual_buses.remove(index);
    save_config(app, &config)?;

    Ok(Some(record))
}

pub(crate) fn upsert_midi_route(
    app: &AppHandle,
    record: MidiRouteRecord,
) -> Result<MidiRouteRecord, String> {
    let mut config = load_config(app)?;

    if let Some(existing) = config
        .midi_routes
        .iter_mut()
        .find(|existing| existing.id == record.id)
    {
        *existing = record.clone();
    } else {
        config.midi_routes.push(record.clone());
    }

    save_config(app, &config)?;
    Ok(record)
}

pub(crate) fn remove_midi_route(
    app: &AppHandle,
    id: &str,
) -> Result<Option<MidiRouteRecord>, String> {
    let mut config = load_config(app)?;

    let Some(index) = config.midi_routes.iter().position(|record| record.id == id) else {
        return Ok(None);
    };

    let record = config.midi_routes.remove(index);
    save_config(app, &config)?;

    Ok(Some(record))
}

#[cfg(test)]
mod tests {
    use super::{LumaLinkConfig, MidiRouteRecord, MidiRouteTransform, VirtualBusRecord};

    #[test]
    fn config_json_round_trip_keeps_system_state() {
        let config = LumaLinkConfig {
            virtual_buses: vec![VirtualBusRecord {
                id: "bus-1".into(),
                name: "Lighting".into(),
                backend: "coremidi".into(),
                association_id: None,
            }],
            midi_routes: vec![MidiRouteRecord {
                id: "route-1".into(),
                name: "Keys to Studio".into(),
                input_name: "Keyboard".into(),
                output_name: "LumaStudio".into(),
                enabled: true,
                transform: MidiRouteTransform {
                    input_channel: Some(1),
                    output_channel: Some(2),
                    transpose: 12,
                    velocity_percent: 90,
                    cc_from: Some(1),
                    cc_to: Some(11),
                    block_timing: true,
                    block_sysex: false,
                },
            }],
        };

        let json = serde_json::to_string(&config).unwrap();
        let decoded: LumaLinkConfig = serde_json::from_str(&json).unwrap();

        assert_eq!(decoded.virtual_buses[0].name, "Lighting");
        assert_eq!(decoded.midi_routes[0].transform.transpose, 12);
        assert_eq!(decoded.midi_routes[0].transform.output_channel, Some(2));
    }

    #[test]
    fn legacy_config_without_routes_still_loads() {
        let json = r#"{"virtualBuses":[]}"#;
        let decoded: LumaLinkConfig = serde_json::from_str(json).unwrap();
        assert!(decoded.midi_routes.is_empty());
    }
}
