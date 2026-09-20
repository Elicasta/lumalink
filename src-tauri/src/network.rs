use serde::Serialize;
use std::{
    env,
    net::UdpSocket,
    sync::atomic::{AtomicBool, Ordering},
    thread,
    time::Duration,
};

pub const DISCOVERY_PORT: u16 = 49777;
const DISCOVERY_REQUEST: &[u8] = b"LUMALINK_DISCOVER_V1";
static DISCOVERY_ACTIVE: AtomicBool = AtomicBool::new(false);

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NetworkDiscoveryStatus {
    pub enabled: bool,
    pub port: u16,
    pub node_name: String,
    pub platform: String,
    pub protocol_version: u8,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct NetworkDiscoveryResponse {
    protocol: &'static str,
    protocol_version: u8,
    node_name: String,
    platform: String,
    app_version: &'static str,
    capabilities: Vec<&'static str>,
    suggested_pro_presenter_port: u16,
}

fn node_name() -> String {
    env::var("COMPUTERNAME")
        .or_else(|_| env::var("HOSTNAME"))
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "LumaLink".into())
}

fn platform_name() -> String {
    env::consts::OS.to_string()
}

pub fn discovery_status() -> NetworkDiscoveryStatus {
    NetworkDiscoveryStatus {
        enabled: DISCOVERY_ACTIVE.load(Ordering::Relaxed),
        port: DISCOVERY_PORT,
        node_name: node_name(),
        platform: platform_name(),
        protocol_version: 1,
    }
}

pub fn start_discovery_responder() -> Result<(), String> {
    let socket = UdpSocket::bind(("0.0.0.0", DISCOVERY_PORT))
        .map_err(|error| format!("Could not bind LumaLink discovery port {DISCOVERY_PORT}: {error}"))?;
    socket
        .set_read_timeout(Some(Duration::from_secs(1)))
        .map_err(|error| error.to_string())?;

    thread::Builder::new()
        .name("lumalink-network-discovery".into())
        .spawn(move || {
            let response = NetworkDiscoveryResponse {
                protocol: "lumalink.discovery",
                protocol_version: 1,
                node_name: node_name(),
                platform: platform_name(),
                app_version: env!("CARGO_PKG_VERSION"),
                capabilities: vec!["midi", "ndi", "lan-discovery", "propresenter-discovery"],
                suggested_pro_presenter_port: 50001,
            };
            let response_bytes = match serde_json::to_vec(&response) {
                Ok(value) => value,
                Err(error) => {
                    DISCOVERY_ACTIVE.store(false, Ordering::Relaxed);
                    eprintln!("LumaLink discovery serialization failed: {error}");
                    return;
                }
            };

            let mut buffer = [0_u8; 512];
            loop {
                match socket.recv_from(&mut buffer) {
                    Ok((size, source)) => {
                        if &buffer[..size] == DISCOVERY_REQUEST {
                            if let Err(error) = socket.send_to(&response_bytes, source) {
                                eprintln!("LumaLink discovery response failed: {error}");
                            }
                        }
                    }
                    Err(error)
                        if error.kind() == std::io::ErrorKind::WouldBlock
                            || error.kind() == std::io::ErrorKind::TimedOut => {}
                    Err(error) => {
                        eprintln!("LumaLink discovery listener failed: {error}");
                        thread::sleep(Duration::from_secs(1));
                    }
                }
            }
        })
        .map_err(|error| format!("Could not start LumaLink discovery thread: {error}"))?;

    DISCOVERY_ACTIVE.store(true, Ordering::Relaxed);
    Ok(())
}

#[tauri::command]
pub fn network_discovery_status() -> NetworkDiscoveryStatus {
    discovery_status()
}

#[cfg(test)]
mod tests {
    use super::{discovery_status, DISCOVERY_PORT};

    #[test]
    fn reports_stable_discovery_protocol() {
        let status = discovery_status();
        assert!(!status.enabled);
        assert_eq!(status.port, DISCOVERY_PORT);
        assert_eq!(status.protocol_version, 1);
        assert!(!status.node_name.is_empty());
    }
}
