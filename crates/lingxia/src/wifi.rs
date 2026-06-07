//! Wi-Fi control and scanning APIs for native Rust code.
//!
//! The platform implementation is callback-based, but this module exposes plain
//! async functions for one-shot operations. Wi-Fi state listeners are represented
//! by an RAII [`Subscription`]; dropping the handle unregisters the listener.
//!
//! # Example
//!
//! ```ignore
//! use lingxia::wifi;
//!
//! wifi::start().await?;
//! let networks = wifi::scan().await?;
//! let _sub = wifi::subscribe(|info| log::info!("wifi: {}", info.ssid))?;
//! ```

use std::sync::Arc;

use lingxia_messaging::CallbackResult;
use lingxia_platform::Platform;
use lingxia_platform::PlatformError;
use lingxia_platform::traits::wifi::{Wifi, WifiConnectRequest, WifiGetConnectedRequest, WifiInfo};
use serde::Deserialize;

/// Connection request for [`connect`].
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectRequest {
    pub ssid: String,
    #[serde(default)]
    pub password: Option<String>,
}

impl ConnectRequest {
    /// Creates a request to connect to `ssid`.
    pub fn new(ssid: impl Into<String>) -> Self {
        Self {
            ssid: ssid.into(),
            password: None,
        }
    }

    /// Sets the network password.
    pub fn password(mut self, password: impl Into<String>) -> Self {
        self.password = Some(password.into());
        self
    }
}

struct CallbackGuard {
    id: u64,
    active: bool,
}

impl CallbackGuard {
    fn new(id: u64) -> Self {
        Self { id, active: true }
    }

    fn disarm(&mut self) {
        self.active = false;
    }
}

impl Drop for CallbackGuard {
    fn drop(&mut self) {
        if self.active {
            lingxia_messaging::remove_callback(self.id);
        }
    }
}

async fn native_callback<F>(init: F) -> crate::Result<String>
where
    F: FnOnce(u64) -> Result<(), PlatformError>,
{
    let (callback_id, rx) = lingxia_messaging::get_callback();
    let mut guard = CallbackGuard::new(callback_id);

    if let Err(err) = init(callback_id) {
        guard.disarm();
        lingxia_messaging::remove_callback(callback_id);
        return Err(crate::Error::from(err));
    }

    let result = match rx.await {
        Ok(result) => {
            guard.disarm();
            result
                .into_result()
                .map_err(PlatformError::BusinessError)
                .map_err(crate::Error::from)
        }
        Err(_) => Err(crate::Error::from(PlatformError::CallbackDropped)),
    };
    result
}

fn parse_json<T>(operation: &str, raw: String) -> crate::Result<T>
where
    T: serde::de::DeserializeOwned,
{
    serde_json::from_str(&raw).map_err(|err| {
        crate::Error::platform(format!(
            "{operation}: failed to parse response: {err} (raw: {raw})"
        ))
    })
}

fn parse_wifi_info_with_defaults(
    raw: &str,
    default_signal_strength: u8,
    default_secure: bool,
) -> crate::Result<WifiInfo> {
    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct RawWifiInfo {
        #[serde(alias = "SSID")]
        ssid: String,
        #[serde(default, alias = "BSSID")]
        bssid: Option<String>,
        secure: Option<bool>,
        signal_strength: Option<u8>,
        #[serde(default)]
        frequency: Option<u32>,
    }

    let raw: RawWifiInfo = serde_json::from_str(raw).map_err(|err| {
        crate::Error::platform(format!(
            "wifi info: failed to parse response: {err} (raw: {raw})"
        ))
    })?;
    Ok(WifiInfo {
        ssid: raw.ssid,
        bssid: raw.bssid,
        secure: raw.secure.unwrap_or(default_secure),
        signal_strength: raw
            .signal_strength
            .unwrap_or(default_signal_strength)
            .min(100),
        frequency: raw.frequency,
    })
}

fn parse_wifi_info(raw: &str) -> crate::Result<WifiInfo> {
    parse_wifi_info_with_defaults(raw, 0, false)
}

fn parse_wifi_list(raw: String) -> crate::Result<Vec<WifiInfo>> {
    let values: Vec<serde_json::Value> = parse_json("wifi list", raw)?;
    values
        .into_iter()
        .map(|value| parse_wifi_info(&value.to_string()))
        .collect()
}

fn parse_connected_wifi(raw: String) -> crate::Result<WifiInfo> {
    parse_wifi_info_with_defaults(&raw, 100, true)
}

/// Initializes the platform Wi-Fi module.
pub async fn start() -> crate::Result<()> {
    let runtime = crate::runtime::platform()?;
    native_callback(|callback_id| runtime.start_wifi(callback_id))
        .await
        .map(|_| ())
}

/// Tears down the platform Wi-Fi module.
pub async fn stop() -> crate::Result<()> {
    let runtime = crate::runtime::platform()?;
    native_callback(|callback_id| runtime.stop_wifi(callback_id))
        .await
        .map(|_| ())
}

/// Scans for nearby Wi-Fi access points.
pub async fn scan() -> crate::Result<Vec<WifiInfo>> {
    let runtime = crate::runtime::platform()?;
    let raw = native_callback(|callback_id| runtime.get_wifi_list(callback_id)).await?;
    parse_wifi_list(raw)
}

/// Returns the currently connected access point.
pub async fn connected() -> crate::Result<WifiInfo> {
    let runtime = crate::runtime::platform()?;
    let raw = native_callback(|callback_id| {
        runtime.get_connected_wifi(WifiGetConnectedRequest { callback_id })
    })
    .await?;
    parse_connected_wifi(raw)
}

/// Connects to the access point described by `req`.
pub async fn connect(req: ConnectRequest) -> crate::Result<()> {
    let runtime = crate::runtime::platform()?;
    native_callback(|callback_id| {
        let request = WifiConnectRequest {
            ssid: req.ssid,
            password: req.password,
            callback_id,
        };
        runtime.connect_wifi(request)
    })
    .await
    .map(|_| ())
}

/// RAII handle for a registered Wi-Fi state subscription.
///
/// Dropping the handle unregisters the platform listener and removes its native
/// callback.
#[must_use = "drop the subscription to stop receiving events"]
pub struct Subscription {
    callback_id: u64,
    runtime: Arc<Platform>,
}

impl Subscription {
    /// Numeric callback id assigned by the native callback registry.
    pub fn id(&self) -> u64 {
        self.callback_id
    }
}

impl Drop for Subscription {
    fn drop(&mut self) {
        if let Err(err) = self.runtime.remove_wifi_state_listener(self.callback_id) {
            log::warn!(
                "wifi::Subscription({}) drop: remove listener failed: {err}",
                self.callback_id
            );
        }
        lingxia_messaging::remove_callback(self.callback_id);
    }
}

/// Subscribes to Wi-Fi connection-state changes. `handler` runs with the
/// decoded [`WifiInfo`] each time the platform reports a transition. Hold the
/// returned [`Subscription`] for as long as you want the callback active;
/// dropping it unregisters automatically.
pub fn subscribe<F>(handler: F) -> crate::Result<Subscription>
where
    F: Fn(WifiInfo) + Send + Sync + 'static,
{
    let runtime = crate::runtime::platform()?;
    let callback_id = lingxia_messaging::register_handler(move |result| match result {
        CallbackResult::Success(json) => match parse_wifi_info(&json) {
            Ok(info) => handler(info),
            Err(err) => log::warn!("wifi state payload decode failed: {err}"),
        },
        CallbackResult::Error(code) => {
            log::warn!("wifi state listener reported error code: {code}")
        }
    });
    if let Err(err) = runtime.add_wifi_state_listener(callback_id) {
        lingxia_messaging::remove_callback(callback_id);
        return Err(crate::Error::from(err));
    }
    Ok(Subscription {
        callback_id,
        runtime,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_platform_wifi_info() {
        let info = parse_wifi_info(
            r#"{"ssid":"Office","bssid":"aa:bb:cc:dd:ee:ff","secure":true,"signalStrength":80,"frequency":2412}"#,
        )
        .unwrap();
        assert_eq!(info.ssid, "Office");
        assert_eq!(info.bssid.as_deref(), Some("aa:bb:cc:dd:ee:ff"));
        assert!(info.secure);
        assert_eq!(info.signal_strength, 80);
        assert_eq!(info.frequency, Some(2412));
    }

    #[test]
    fn accepts_js_compatible_wifi_field_names() {
        let info = parse_wifi_info(
            r#"{"SSID":"Cafe","BSSID":"11:22:33:44:55:66","secure":false,"signalStrength":120}"#,
        )
        .unwrap();
        assert_eq!(info.ssid, "Cafe");
        assert_eq!(info.bssid.as_deref(), Some("11:22:33:44:55:66"));
        assert!(!info.secure);
        assert_eq!(info.signal_strength, 100);
    }

    #[test]
    fn connected_wifi_defaults_only_when_fields_are_missing() {
        let missing = parse_connected_wifi(r#"{"ssid":"Home"}"#.to_string()).unwrap();
        assert!(missing.secure);
        assert_eq!(missing.signal_strength, 100);

        let explicit = parse_connected_wifi(
            r#"{"ssid":"Open","secure":false,"signalStrength":0}"#.to_string(),
        )
        .unwrap();
        assert!(!explicit.secure);
        assert_eq!(explicit.signal_strength, 0);
    }
}
