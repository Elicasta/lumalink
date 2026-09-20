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

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct LumaLinkConfig {
    #[serde(default)]
    pub virtual_buses: Vec<VirtualBusRecord>,
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

#[cfg(test)]
mod tests {
    use super::{LumaLinkConfig, VirtualBusRecord};

    #[test]
    fn config_json_round_trip_keeps_virtual_bus_identity() {
        let config = LumaLinkConfig {
            virtual_buses: vec![VirtualBusRecord {
                id: "abc".into(),
                name: "Lighting".into(),
                backend: "coremidi".into(),
                association_id: None,
            }],
        };

        let json = serde_json::to_string(&config).unwrap();
        let decoded: LumaLinkConfig = serde_json::from_str(&json).unwrap();

        assert_eq!(decoded.virtual_buses.len(), 1);
        assert_eq!(decoded.virtual_buses[0].name, "Lighting");
    }
}
