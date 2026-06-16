use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

use crate::error::PlatformError;
use crate::traits::network::Network;
use lingxia_messaging::invoke_callback;
use serde::Serialize;
use windows::Networking::Connectivity::{
    ConnectionProfile, NetworkConnectivityLevel, NetworkInformation,
    NetworkStatusChangedEventHandler,
};
use windows::Networking::HostNameType;

use super::Platform;

const CALLBACK_ERR_INTERNAL: u32 = 1000;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct WindowsNetworkInfo {
    is_connected: bool,
    network_type: &'static str,
    ipv4: Vec<String>,
    ipv6: Vec<String>,
}

static NETWORK_LISTENERS: OnceLock<Mutex<HashMap<u64, i64>>> = OnceLock::new();

fn network_listeners() -> &'static Mutex<HashMap<u64, i64>> {
    NETWORK_LISTENERS.get_or_init(|| Mutex::new(HashMap::new()))
}

fn map_windows_error(context: &str, err: windows::core::Error) -> PlatformError {
    PlatformError::Platform(format!("{context}: {err}"))
}

fn current_profile() -> Option<ConnectionProfile> {
    NetworkInformation::GetInternetConnectionProfile().ok()
}

fn connectivity_level(profile: Option<&ConnectionProfile>) -> NetworkConnectivityLevel {
    profile
        .and_then(|profile| profile.GetNetworkConnectivityLevel().ok())
        .unwrap_or(NetworkConnectivityLevel::None)
}

fn is_connected(level: NetworkConnectivityLevel) -> bool {
    !matches!(level, NetworkConnectivityLevel::None)
}

fn network_type(
    profile: Option<&ConnectionProfile>,
    level: NetworkConnectivityLevel,
) -> &'static str {
    let Some(profile) = profile else {
        return "none";
    };
    if !is_connected(level) {
        return "none";
    }
    if profile.IsWlanConnectionProfile().unwrap_or(false) {
        return "wifi";
    }
    if profile.IsWwanConnectionProfile().unwrap_or(false) {
        return "unknown";
    }
    match profile
        .NetworkAdapter()
        .ok()
        .and_then(|adapter| adapter.IanaInterfaceType().ok())
    {
        Some(6) => "ethernet",
        Some(71) => "wifi",
        _ => "unknown",
    }
}

fn active_adapter_id(profile: Option<&ConnectionProfile>) -> Option<windows::core::GUID> {
    profile
        .and_then(|profile| profile.NetworkAdapter().ok())
        .and_then(|adapter| adapter.NetworkAdapterId().ok())
}

fn host_name_matches_active_adapter(
    host: &windows::Networking::HostName,
    active_id: Option<windows::core::GUID>,
) -> bool {
    let Some(active_id) = active_id else {
        return true;
    };
    host.IPInformation()
        .ok()
        .and_then(|info| info.NetworkAdapter().ok())
        .and_then(|adapter| adapter.NetworkAdapterId().ok())
        .map(|host_adapter_id| host_adapter_id == active_id)
        .unwrap_or(false)
}

fn local_ip_addresses(active_id: Option<windows::core::GUID>) -> (Vec<String>, Vec<String>) {
    let mut ipv4 = Vec::new();
    let mut ipv6 = Vec::new();

    let Ok(hosts) = NetworkInformation::GetHostNames() else {
        return (ipv4, ipv6);
    };
    let Ok(size) = hosts.Size() else {
        return (ipv4, ipv6);
    };

    for index in 0..size {
        let Ok(host) = hosts.GetAt(index) else {
            continue;
        };
        if !host_name_matches_active_adapter(&host, active_id) {
            continue;
        }
        let Ok(raw_name) = host.CanonicalName() else {
            continue;
        };
        let address = raw_name.to_string_lossy();
        if address.trim().is_empty() {
            continue;
        }
        match host.Type() {
            Ok(HostNameType::Ipv4) => ipv4.push(address),
            Ok(HostNameType::Ipv6) => ipv6.push(address),
            _ => {}
        }
    }

    ipv4.sort();
    ipv4.dedup();
    ipv6.sort();
    ipv6.dedup();
    (ipv4, ipv6)
}

fn current_network_info() -> WindowsNetworkInfo {
    let profile = current_profile();
    let level = connectivity_level(profile.as_ref());
    let connected = is_connected(level);
    let (ipv4, ipv6) = if connected {
        local_ip_addresses(active_adapter_id(profile.as_ref()))
    } else {
        (Vec::new(), Vec::new())
    };

    WindowsNetworkInfo {
        is_connected: connected,
        network_type: network_type(profile.as_ref(), level),
        ipv4,
        ipv6,
    }
}

fn current_network_info_json() -> Result<String, PlatformError> {
    serde_json::to_string(&current_network_info())
        .map_err(|err| PlatformError::Platform(format!("serialize network info: {err}")))
}

impl Network for Platform {
    async fn get_network_info(&self) -> Result<String, PlatformError> {
        current_network_info_json()
    }

    fn add_network_change_listener(&self, callback_id: u64) -> Result<(), PlatformError> {
        {
            let listeners = network_listeners().lock().map_err(|_| {
                PlatformError::Platform("network listener registry poisoned".into())
            })?;
            if listeners.contains_key(&callback_id) {
                return Ok(());
            }
        }

        if let Ok(payload) = current_network_info_json() {
            let _ = invoke_callback(callback_id, Ok(payload));
        }

        let handler = NetworkStatusChangedEventHandler::new(move |_| {
            match current_network_info_json() {
                Ok(payload) => {
                    let _ = invoke_callback(callback_id, Ok(payload));
                }
                Err(err) => {
                    log::warn!("Windows network status callback failed: {err}");
                    let _ = invoke_callback(callback_id, Err(CALLBACK_ERR_INTERNAL));
                }
            }
            Ok(())
        });
        let token = NetworkInformation::NetworkStatusChanged(&handler)
            .map_err(|err| map_windows_error("register NetworkStatusChanged", err))?;

        let mut listeners = network_listeners()
            .lock()
            .map_err(|_| PlatformError::Platform("network listener registry poisoned".into()))?;
        listeners.insert(callback_id, token);
        Ok(())
    }

    fn remove_network_change_listener(&self, callback_id: u64) -> Result<(), PlatformError> {
        let token = {
            let mut listeners = network_listeners().lock().map_err(|_| {
                PlatformError::Platform("network listener registry poisoned".into())
            })?;
            listeners.remove(&callback_id)
        };
        if let Some(token) = token {
            NetworkInformation::RemoveNetworkStatusChanged(token)
                .map_err(|err| map_windows_error("remove NetworkStatusChanged", err))?;
        }
        Ok(())
    }
}
