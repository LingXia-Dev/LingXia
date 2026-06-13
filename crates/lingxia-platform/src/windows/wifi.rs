use std::collections::HashMap;
use std::ffi::c_void;
use std::ptr::null_mut;
use std::sync::{Condvar, Mutex, OnceLock};
use std::time::Duration;

use crate::error::PlatformError;
use crate::traits::wifi::{Wifi, WifiConnectRequest, WifiGetConnectedRequest};
use lingxia_messaging::invoke_callback;
use serde::Serialize;
use windows::Networking::Connectivity::{
    ConnectionProfile, NetworkAuthenticationType, NetworkConnectivityLevel, NetworkInformation,
    NetworkStatusChangedEventHandler,
};
use windows::Win32::Foundation::HANDLE;
use windows::Win32::NetworkManagement::WiFi::{
    DOT11_AUTH_ALGO_RSNA_PSK, DOT11_AUTH_ALGO_WPA_PSK, DOT11_CIPHER_ALGO_CCMP,
    DOT11_CIPHER_ALGO_TKIP, DOT11_SSID, L2_NOTIFICATION_DATA, WLAN_API_VERSION_2_0,
    WLAN_AVAILABLE_NETWORK_HAS_PROFILE, WLAN_AVAILABLE_NETWORK_INCLUDE_ALL_ADHOC_PROFILES,
    WLAN_AVAILABLE_NETWORK_INCLUDE_ALL_MANUAL_HIDDEN_PROFILES, WLAN_AVAILABLE_NETWORK_LIST,
    WLAN_CONNECTION_PARAMETERS, WLAN_INTERFACE_INFO, WLAN_INTERFACE_INFO_LIST,
    WLAN_NOTIFICATION_SOURCE_ACM, WLAN_NOTIFICATION_SOURCE_NONE, WlanCloseHandle, WlanConnect,
    WlanEnumInterfaces, WlanFreeMemory, WlanGetAvailableNetworkList, WlanOpenHandle,
    WlanRegisterNotification, WlanScan, WlanSetProfile, dot11_BSS_type_infrastructure,
    wlan_connection_mode_profile, wlan_notification_acm_connection_attempt_fail,
    wlan_notification_acm_connection_complete, wlan_notification_acm_scan_complete,
    wlan_notification_acm_scan_fail,
};
use windows::core::{GUID, PCWSTR};

use super::Platform;

const CALLBACK_ERR_INTERNAL: u32 = 1000;
const WLAN_SUCCESS: u32 = 0;
const SCAN_TIMEOUT: Duration = Duration::from_secs(4);
const CONNECT_TIMEOUT: Duration = Duration::from_secs(15);

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct WindowsWifiInfo {
    ssid: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    bssid: Option<String>,
    secure: bool,
    signal_strength: u8,
    #[serde(skip_serializing_if = "Option::is_none")]
    frequency: Option<u32>,
    connected: bool,
    state: &'static str,
}

#[derive(Debug, Clone)]
struct WlanNetwork {
    ssid: String,
    profile_name: Option<String>,
    secure: bool,
    signal_strength: u8,
    auth: windows::Win32::NetworkManagement::WiFi::DOT11_AUTH_ALGORITHM,
    cipher: windows::Win32::NetworkManagement::WiFi::DOT11_CIPHER_ALGORITHM,
}

struct ScanWaiter {
    interface_id: GUID,
    done: Mutex<bool>,
    ready: Condvar,
}

struct ConnectWaiter {
    interface_id: GUID,
    result: Mutex<Option<bool>>,
    ready: Condvar,
}

static WIFI_LISTENERS: OnceLock<Mutex<HashMap<u64, i64>>> = OnceLock::new();

fn wifi_listeners() -> &'static Mutex<HashMap<u64, i64>> {
    WIFI_LISTENERS.get_or_init(|| Mutex::new(HashMap::new()))
}

fn map_windows_error(context: &str, err: windows::core::Error) -> PlatformError {
    PlatformError::Platform(format!("{context}: {err}"))
}

fn wlan_error(context: &str, code: u32) -> PlatformError {
    PlatformError::Platform(format!("{context} failed with Win32 code {code}"))
}

fn check_wlan(context: &str, code: u32) -> Result<(), PlatformError> {
    if code == WLAN_SUCCESS {
        Ok(())
    } else {
        Err(wlan_error(context, code))
    }
}

unsafe extern "system" fn scan_notification(data: *mut L2_NOTIFICATION_DATA, context: *mut c_void) {
    if data.is_null() || context.is_null() {
        return;
    }

    let data = unsafe { &*data };
    if data.NotificationSource != WLAN_NOTIFICATION_SOURCE_ACM {
        return;
    }
    let code = data.NotificationCode;
    let scan_done = code == wlan_notification_acm_scan_complete.0 as u32
        || code == wlan_notification_acm_scan_fail.0 as u32;
    if !scan_done {
        return;
    }

    let waiter = unsafe { &*(context as *const ScanWaiter) };
    if data.InterfaceGuid != waiter.interface_id {
        return;
    }
    if let Ok(mut done) = waiter.done.lock() {
        *done = true;
        waiter.ready.notify_all();
    }
}

unsafe extern "system" fn connect_notification(
    data: *mut L2_NOTIFICATION_DATA,
    context: *mut c_void,
) {
    if data.is_null() || context.is_null() {
        return;
    }

    let data = unsafe { &*data };
    if data.NotificationSource != WLAN_NOTIFICATION_SOURCE_ACM {
        return;
    }
    let code = data.NotificationCode;
    let result = if code == wlan_notification_acm_connection_complete.0 as u32 {
        Some(true)
    } else if code == wlan_notification_acm_connection_attempt_fail.0 as u32 {
        Some(false)
    } else {
        None
    };
    let Some(result) = result else {
        return;
    };

    let waiter = unsafe { &*(context as *const ConnectWaiter) };
    if data.InterfaceGuid != waiter.interface_id {
        return;
    }
    if let Ok(mut current) = waiter.result.lock() {
        *current = Some(result);
        waiter.ready.notify_all();
    }
}

fn wait_for_scan(client: &WlanClient, interface_id: &GUID, target: Option<&DOT11_SSID>) {
    let waiter = ScanWaiter {
        interface_id: *interface_id,
        done: Mutex::new(false),
        ready: Condvar::new(),
    };
    let mut previous_source = 0u32;
    let register_code = unsafe {
        WlanRegisterNotification(
            client.handle,
            WLAN_NOTIFICATION_SOURCE_ACM,
            false,
            Some(scan_notification),
            Some((&waiter as *const ScanWaiter).cast()),
            None,
            Some(&mut previous_source),
        )
    };

    let scan_code = unsafe {
        WlanScan(
            client.handle,
            interface_id,
            target.map(|ssid| ssid as *const DOT11_SSID),
            None,
            None,
        )
    };
    if register_code == WLAN_SUCCESS
        && scan_code == WLAN_SUCCESS
        && let Ok(done) = waiter.done.lock()
    {
        let _ = waiter
            .ready
            .wait_timeout_while(done, SCAN_TIMEOUT, |done| !*done);
    }

    if register_code == WLAN_SUCCESS {
        let _ = unsafe {
            WlanRegisterNotification(
                client.handle,
                WLAN_NOTIFICATION_SOURCE_NONE,
                false,
                None,
                None,
                None,
                None,
            )
        };
    }
}

fn connect_and_wait(
    client: &WlanClient,
    interface_id: &GUID,
    params: &WLAN_CONNECTION_PARAMETERS,
) -> Result<(), PlatformError> {
    let waiter = ConnectWaiter {
        interface_id: *interface_id,
        result: Mutex::new(None),
        ready: Condvar::new(),
    };
    let mut previous_source = 0u32;
    let register_code = unsafe {
        WlanRegisterNotification(
            client.handle,
            WLAN_NOTIFICATION_SOURCE_ACM,
            false,
            Some(connect_notification),
            Some((&waiter as *const ConnectWaiter).cast()),
            None,
            Some(&mut previous_source),
        )
    };

    let connect_code = unsafe { WlanConnect(client.handle, interface_id, params, None) };
    if connect_code != WLAN_SUCCESS {
        if register_code == WLAN_SUCCESS {
            let _ = unsafe {
                WlanRegisterNotification(
                    client.handle,
                    WLAN_NOTIFICATION_SOURCE_NONE,
                    false,
                    None,
                    None,
                    None,
                    None,
                )
            };
        }
        return Err(wlan_error("WlanConnect", connect_code));
    }

    let result = if register_code == WLAN_SUCCESS {
        waiter
            .result
            .lock()
            .ok()
            .and_then(|result| {
                waiter
                    .ready
                    .wait_timeout_while(result, CONNECT_TIMEOUT, |result| result.is_none())
                    .ok()
                    .and_then(|(result, _)| *result)
            })
            .ok_or_else(|| PlatformError::Platform("WlanConnect timed out".into()))
    } else {
        Ok(true)
    };

    if register_code == WLAN_SUCCESS {
        let _ = unsafe {
            WlanRegisterNotification(
                client.handle,
                WLAN_NOTIFICATION_SOURCE_NONE,
                false,
                None,
                None,
                None,
                None,
            )
        };
    }

    match result {
        Ok(true) => Ok(()),
        Ok(false) => Err(PlatformError::Platform(
            "WlanConnect connection attempt failed".into(),
        )),
        Err(err) => Err(err),
    }
}

struct WlanClient {
    handle: HANDLE,
}

impl WlanClient {
    fn open() -> Result<Self, PlatformError> {
        let mut negotiated = 0u32;
        let mut handle = HANDLE::default();
        let code =
            unsafe { WlanOpenHandle(WLAN_API_VERSION_2_0, None, &mut negotiated, &mut handle) };
        check_wlan("WlanOpenHandle", code)?;
        Ok(Self { handle })
    }
}

impl Drop for WlanClient {
    fn drop(&mut self) {
        let _ = unsafe { WlanCloseHandle(self.handle, None) };
    }
}

struct WlanMemory<T>(*mut T);

impl<T> WlanMemory<T> {
    fn new(ptr: *mut T) -> Self {
        Self(ptr)
    }

    fn as_ptr(&self) -> *mut T {
        self.0
    }
}

impl<T> Drop for WlanMemory<T> {
    fn drop(&mut self) {
        if !self.0.is_null() {
            unsafe { WlanFreeMemory(self.0.cast()) };
        }
    }
}

fn wlan_interfaces(client: &WlanClient) -> Result<Vec<WLAN_INTERFACE_INFO>, PlatformError> {
    let mut raw = null_mut::<WLAN_INTERFACE_INFO_LIST>();
    let code = unsafe { WlanEnumInterfaces(client.handle, None, &mut raw) };
    check_wlan("WlanEnumInterfaces", code)?;
    let list = WlanMemory::new(raw);
    if list.as_ptr().is_null() {
        return Ok(Vec::new());
    }

    let count = unsafe { (*list.as_ptr()).dwNumberOfItems as usize };
    let items =
        unsafe { std::slice::from_raw_parts((*list.as_ptr()).InterfaceInfo.as_ptr(), count) };
    Ok(items.to_vec())
}

fn ssid_from_dot11(ssid: &DOT11_SSID) -> String {
    let len = (ssid.uSSIDLength as usize).min(ssid.ucSSID.len());
    String::from_utf8_lossy(&ssid.ucSSID[..len]).to_string()
}

fn dot11_from_ssid(value: &str) -> Result<DOT11_SSID, PlatformError> {
    let bytes = value.as_bytes();
    if bytes.is_empty() || bytes.len() > 32 {
        return Err(PlatformError::InvalidParameter(
            "Wi-Fi SSID must be 1..32 bytes".to_string(),
        ));
    }
    let mut ssid = DOT11_SSID::default();
    ssid.uSSIDLength = bytes.len() as u32;
    ssid.ucSSID[..bytes.len()].copy_from_slice(bytes);
    Ok(ssid)
}

fn string_from_wide_nul(value: &[u16]) -> Option<String> {
    let end = value
        .iter()
        .position(|unit| *unit == 0)
        .unwrap_or(value.len());
    if end == 0 {
        return None;
    }
    Some(String::from_utf16_lossy(&value[..end]))
}

fn to_wide_nul(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(std::iter::once(0)).collect()
}

fn escape_xml(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

fn available_networks(
    client: &WlanClient,
    interface_id: &GUID,
) -> Result<Vec<WlanNetwork>, PlatformError> {
    let mut raw = null_mut::<WLAN_AVAILABLE_NETWORK_LIST>();
    let flags = WLAN_AVAILABLE_NETWORK_INCLUDE_ALL_ADHOC_PROFILES
        | WLAN_AVAILABLE_NETWORK_INCLUDE_ALL_MANUAL_HIDDEN_PROFILES;
    let code =
        unsafe { WlanGetAvailableNetworkList(client.handle, interface_id, flags, None, &mut raw) };
    check_wlan("WlanGetAvailableNetworkList", code)?;
    let list = WlanMemory::new(raw);
    if list.as_ptr().is_null() {
        return Ok(Vec::new());
    }

    let count = unsafe { (*list.as_ptr()).dwNumberOfItems as usize };
    let items = unsafe { std::slice::from_raw_parts((*list.as_ptr()).Network.as_ptr(), count) };
    let mut by_ssid = HashMap::<String, WlanNetwork>::new();
    for item in items {
        let ssid = ssid_from_dot11(&item.dot11Ssid);
        if ssid.is_empty() {
            continue;
        }
        let signal_strength = item.wlanSignalQuality.min(100) as u8;
        let profile_name = ((item.dwFlags & WLAN_AVAILABLE_NETWORK_HAS_PROFILE) != 0)
            .then(|| string_from_wide_nul(&item.strProfileName))
            .flatten();
        let network = WlanNetwork {
            ssid: ssid.clone(),
            profile_name,
            secure: item.bSecurityEnabled.as_bool(),
            signal_strength,
            auth: item.dot11DefaultAuthAlgorithm,
            cipher: item.dot11DefaultCipherAlgorithm,
        };
        match by_ssid.get(&ssid) {
            Some(existing) if existing.signal_strength >= signal_strength => {}
            _ => {
                by_ssid.insert(ssid, network);
            }
        }
    }
    Ok(by_ssid.into_values().collect())
}

fn scan_available_networks() -> Result<Vec<WlanNetwork>, PlatformError> {
    let client = WlanClient::open()?;
    let interfaces = wlan_interfaces(&client)?;
    let mut networks = Vec::new();
    for interface in interfaces {
        wait_for_scan(&client, &interface.InterfaceGuid, None);
        networks.extend(available_networks(&client, &interface.InterfaceGuid)?);
    }
    networks.sort_by(|a, b| {
        b.signal_strength
            .cmp(&a.signal_strength)
            .then_with(|| a.ssid.cmp(&b.ssid))
    });
    networks.dedup_by(|a, b| a.ssid == b.ssid);
    Ok(networks)
}

fn wifi_list_json() -> Result<String, PlatformError> {
    let items: Vec<WindowsWifiInfo> = scan_available_networks()?
        .into_iter()
        .map(|network| WindowsWifiInfo {
            ssid: network.ssid,
            bssid: None,
            secure: network.secure,
            signal_strength: network.signal_strength,
            frequency: None,
            connected: false,
            state: "available",
        })
        .collect();
    serde_json::to_string(&items)
        .map_err(|err| PlatformError::Platform(format!("serialize Wi-Fi list: {err}")))
}

fn profile_xml(network: &WlanNetwork, password: Option<&str>) -> Result<String, PlatformError> {
    let name = escape_xml(&network.ssid);
    if !network.secure {
        return Ok(format!(
            r#"<?xml version="1.0"?>
<WLANProfile xmlns="http://www.microsoft.com/networking/WLAN/profile/v1">
  <name>{name}</name>
  <SSIDConfig><SSID><name>{name}</name></SSID></SSIDConfig>
  <connectionType>ESS</connectionType>
  <connectionMode>manual</connectionMode>
  <MSM><security><authEncryption><authentication>open</authentication><encryption>none</encryption><useOneX>false</useOneX></authEncryption></security></MSM>
</WLANProfile>"#
        ));
    }

    let password = password.ok_or_else(|| {
        PlatformError::InvalidParameter("connectWifi requires password for secure network".into())
    })?;
    if password.len() < 8 || password.len() > 63 {
        return Err(PlatformError::InvalidParameter(
            "connectWifi WPA/WPA2 password must be 8..63 characters".into(),
        ));
    }

    let authentication = if network.auth == DOT11_AUTH_ALGO_RSNA_PSK {
        "WPA2PSK"
    } else if network.auth == DOT11_AUTH_ALGO_WPA_PSK {
        "WPAPSK"
    } else {
        return Err(PlatformError::NotSupported(format!(
            "connectWifi only supports open/WPA/WPA2-PSK networks on Windows; auth={:?}",
            network.auth
        )));
    };
    let encryption = if network.cipher == DOT11_CIPHER_ALGO_CCMP {
        "AES"
    } else if network.cipher == DOT11_CIPHER_ALGO_TKIP {
        "TKIP"
    } else {
        return Err(PlatformError::NotSupported(format!(
            "connectWifi only supports AES/TKIP Wi-Fi encryption on Windows; cipher={:?}",
            network.cipher
        )));
    };
    let key = escape_xml(password);
    Ok(format!(
        r#"<?xml version="1.0"?>
<WLANProfile xmlns="http://www.microsoft.com/networking/WLAN/profile/v1">
  <name>{name}</name>
  <SSIDConfig><SSID><name>{name}</name></SSID></SSIDConfig>
  <connectionType>ESS</connectionType>
  <connectionMode>manual</connectionMode>
  <MSM><security><authEncryption><authentication>{authentication}</authentication><encryption>{encryption}</encryption><useOneX>false</useOneX></authEncryption><sharedKey><keyType>passPhrase</keyType><protected>false</protected><keyMaterial>{key}</keyMaterial></sharedKey></security></MSM>
</WLANProfile>"#
    ))
}

fn connect_wifi_request(request: &WifiConnectRequest) -> Result<(), PlatformError> {
    let client = WlanClient::open()?;
    let target_ssid = request.ssid.trim();
    let target_dot11 = dot11_from_ssid(target_ssid)?;
    let interfaces = wlan_interfaces(&client)?;
    for interface in interfaces {
        wait_for_scan(&client, &interface.InterfaceGuid, Some(&target_dot11));
        let networks = available_networks(&client, &interface.InterfaceGuid)?;
        let Some(network) = networks.into_iter().find(|item| item.ssid == target_ssid) else {
            continue;
        };

        let profile_storage = if request.password.is_none()
            && let Some(profile) = network.profile_name.clone()
        {
            to_wide_nul(&profile)
        } else {
            let xml = profile_xml(&network, request.password.as_deref())?;
            let xml_wide = to_wide_nul(&xml);
            let mut reason = 0u32;
            let code = unsafe {
                WlanSetProfile(
                    client.handle,
                    &interface.InterfaceGuid,
                    0,
                    PCWSTR(xml_wide.as_ptr()),
                    PCWSTR::null(),
                    true,
                    None,
                    &mut reason,
                )
            };
            if code != WLAN_SUCCESS {
                return Err(PlatformError::Platform(format!(
                    "WlanSetProfile failed with Win32 code {code}, reason {reason}"
                )));
            }
            to_wide_nul(&network.ssid)
        };

        let mut ssid = target_dot11;
        let params = WLAN_CONNECTION_PARAMETERS {
            wlanConnectionMode: wlan_connection_mode_profile,
            strProfile: PCWSTR(profile_storage.as_ptr()),
            pDot11Ssid: &mut ssid,
            pDesiredBssidList: null_mut(),
            dot11BssType: dot11_BSS_type_infrastructure,
            dwFlags: 0,
        };
        connect_and_wait(&client, &interface.InterfaceGuid, &params)?;
        return Ok(());
    }

    Err(PlatformError::Platform(format!(
        "Wi-Fi network `{target_ssid}` not found"
    )))
}

fn connectivity_level(profile: &ConnectionProfile) -> NetworkConnectivityLevel {
    profile
        .GetNetworkConnectivityLevel()
        .unwrap_or(NetworkConnectivityLevel::None)
}

fn profile_is_connected(profile: &ConnectionProfile) -> bool {
    !matches!(
        connectivity_level(profile),
        NetworkConnectivityLevel::None | NetworkConnectivityLevel::LocalAccess
    )
}

fn is_wlan_profile(profile: &ConnectionProfile) -> bool {
    profile.IsWlanConnectionProfile().unwrap_or(false)
}

fn current_wlan_profile() -> Option<ConnectionProfile> {
    let profile = NetworkInformation::GetInternetConnectionProfile().ok()?;
    if is_wlan_profile(&profile) && profile_is_connected(&profile) {
        Some(profile)
    } else {
        None
    }
}

fn has_wlan_interface() -> bool {
    WlanClient::open()
        .and_then(|client| wlan_interfaces(&client))
        .map(|interfaces| !interfaces.is_empty())
        .unwrap_or(false)
}

fn signal_strength(profile: &ConnectionProfile) -> u8 {
    profile
        .GetSignalBars()
        .ok()
        .map(|bars| bars.min(5).saturating_mul(20))
        .unwrap_or(0)
}

fn is_secure(profile: &ConnectionProfile) -> bool {
    let Some(auth) = profile
        .NetworkSecuritySettings()
        .ok()
        .and_then(|settings| settings.NetworkAuthenticationType().ok())
    else {
        return true;
    };
    !matches!(
        auth,
        NetworkAuthenticationType::None | NetworkAuthenticationType::Open80211
    )
}

fn connected_wifi_info() -> WindowsWifiInfo {
    let Some(profile) = current_wlan_profile() else {
        return WindowsWifiInfo {
            ssid: String::new(),
            bssid: None,
            secure: false,
            signal_strength: 0,
            frequency: None,
            connected: false,
            state: "disconnected",
        };
    };

    let ssid = profile
        .WlanConnectionProfileDetails()
        .ok()
        .and_then(|details| details.GetConnectedSsid().ok())
        .map(|ssid| ssid.to_string_lossy())
        .unwrap_or_default();
    let connected = !ssid.is_empty();

    WindowsWifiInfo {
        ssid,
        bssid: None,
        secure: is_secure(&profile),
        signal_strength: signal_strength(&profile),
        frequency: None,
        connected,
        state: if connected {
            "connected"
        } else {
            "disconnected"
        },
    }
}

fn connected_wifi_json() -> Result<String, PlatformError> {
    serde_json::to_string(&connected_wifi_info())
        .map_err(|err| PlatformError::Platform(format!("serialize Wi-Fi info: {err}")))
}

fn invoke_success(callback_id: u64, payload: impl Into<String>) -> Result<(), PlatformError> {
    let _ = invoke_callback(callback_id, Ok(payload.into()));
    Ok(())
}

impl Wifi for Platform {
    fn start_wifi(&self, callback_id: u64) -> Result<(), PlatformError> {
        invoke_success(callback_id, "{}")
    }

    fn stop_wifi(&self, callback_id: u64) -> Result<(), PlatformError> {
        invoke_success(callback_id, "{}")
    }

    fn connect_wifi(&self, request: WifiConnectRequest) -> Result<(), PlatformError> {
        connect_wifi_request(&request)?;
        invoke_success(request.callback_id, "{}")
    }

    fn get_wifi_list(&self, callback_id: u64) -> Result<(), PlatformError> {
        invoke_success(callback_id, wifi_list_json()?)
    }

    fn get_connected_wifi(&self, request: WifiGetConnectedRequest) -> Result<(), PlatformError> {
        invoke_success(request.callback_id, connected_wifi_json()?)
    }

    fn is_wifi_enabled(&self) -> Result<bool, PlatformError> {
        Ok(has_wlan_interface())
    }

    fn add_wifi_state_listener(&self, callback_id: u64) -> Result<(), PlatformError> {
        {
            let listeners = wifi_listeners()
                .lock()
                .map_err(|_| PlatformError::Platform("Wi-Fi listener registry poisoned".into()))?;
            if listeners.contains_key(&callback_id) {
                return Ok(());
            }
        }

        if let Ok(payload) = connected_wifi_json() {
            let _ = invoke_callback(callback_id, Ok(payload));
        }

        let handler = NetworkStatusChangedEventHandler::new(move |_| {
            match connected_wifi_json() {
                Ok(payload) => {
                    let _ = invoke_callback(callback_id, Ok(payload));
                }
                Err(err) => {
                    log::warn!("Windows Wi-Fi status callback failed: {err}");
                    let _ = invoke_callback(callback_id, Err(CALLBACK_ERR_INTERNAL));
                }
            }
            Ok(())
        });
        let token = NetworkInformation::NetworkStatusChanged(&handler)
            .map_err(|err| map_windows_error("register Wi-Fi NetworkStatusChanged", err))?;

        let mut listeners = wifi_listeners()
            .lock()
            .map_err(|_| PlatformError::Platform("Wi-Fi listener registry poisoned".into()))?;
        listeners.insert(callback_id, token);
        Ok(())
    }

    fn remove_wifi_state_listener(&self, callback_id: u64) -> Result<(), PlatformError> {
        let token = {
            let mut listeners = wifi_listeners()
                .lock()
                .map_err(|_| PlatformError::Platform("Wi-Fi listener registry poisoned".into()))?;
            listeners.remove(&callback_id)
        };
        if let Some(token) = token {
            NetworkInformation::RemoveNetworkStatusChanged(token)
                .map_err(|err| map_windows_error("remove Wi-Fi NetworkStatusChanged", err))?;
        }
        Ok(())
    }
}
