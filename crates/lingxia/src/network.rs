//! Network status and change subscriptions.
//!
//! Thin wrapper over `lingxia_platform::traits::network::Network` that
//! returns typed structs instead of JSON strings and pairs a subscription
//! with an RAII handle so callers can't forget to remove the platform
//! listener and the [`lingxia_messaging`] callback.
//!
//! # Example
//!
//! ```ignore
//! use lingxia::network::{self, NetworkKind};
//!
//! let info = network::current(&app).await?;
//! if info.kind != NetworkKind::Wifi {
//!     // ...
//! }
//!
//! let _subscription = network::subscribe(&app, |info| {
//!     log::info!("network changed: {:?}", info);
//! })?;
//! ```

use std::sync::Arc;

use lingxia_messaging::CallbackResult;
use lingxia_platform::Platform;
use lingxia_platform::traits::network::Network;
use serde::{Deserialize, Serialize};

use crate::LxApp;

/// Connection state plus IP addresses, as observed by the platform.
///
/// Matches the JSON schema emitted by `Network::get_network_info` /
/// `add_network_change_listener` payloads. Field defaults make
/// deserialization tolerant of platforms that omit keys (e.g. older
/// Android JNI shims).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct NetworkInfo {
    #[serde(default)]
    pub is_connected: bool,
    #[serde(default, rename = "networkType")]
    pub kind: NetworkKind,
    #[serde(default)]
    pub ipv4: Vec<String>,
    #[serde(default)]
    pub ipv6: Vec<String>,
}

/// Connection technology. The platform reports a free-form lowercase
/// string (`"wifi"`, `"4g"`, …); this enum gives callers a typed match.
/// Any value the platform reports outside the known set — including the
/// legacy `"cellular"` — folds into [`NetworkKind::Unknown`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum NetworkKind {
    #[serde(rename = "none")]
    None,
    #[serde(rename = "wifi")]
    Wifi,
    #[serde(rename = "ethernet")]
    Ethernet,
    #[serde(rename = "2g")]
    Cellular2G,
    #[serde(rename = "3g")]
    Cellular3G,
    #[serde(rename = "4g")]
    Cellular4G,
    #[serde(rename = "5g")]
    Cellular5G,
    #[serde(other)]
    Unknown,
}

impl Default for NetworkKind {
    fn default() -> Self {
        NetworkKind::Unknown
    }
}

impl NetworkKind {
    /// True for any `2G`/`3G`/`4G`/`5G` link.
    pub fn is_cellular(self) -> bool {
        matches!(
            self,
            NetworkKind::Cellular2G
                | NetworkKind::Cellular3G
                | NetworkKind::Cellular4G
                | NetworkKind::Cellular5G
        )
    }

    /// True only on Wi-Fi.
    pub fn is_wifi(self) -> bool {
        matches!(self, NetworkKind::Wifi)
    }
}

/// Fetch the current network state from the host platform.
pub async fn current(app: &Arc<LxApp>) -> crate::Result<NetworkInfo> {
    let raw = app
        .runtime
        .get_network_info()
        .await
        .map_err(|err| crate::Error::platform(format!("get_network_info: {err}")))?;
    serde_json::from_str(&raw)
        .map_err(|err| crate::Error::platform(format!("parse network info: {err} (raw: {raw})")))
}

/// RAII handle for a registered network-change subscription. Dropping it
/// removes both the platform listener and the underlying
/// [`lingxia_messaging`] callback — pair both halves together so callers
/// can't leak one and not the other.
#[must_use = "drop the subscription to stop receiving events"]
pub struct Subscription {
    callback_id: u64,
    runtime: Arc<Platform>,
}

impl Subscription {
    /// Numeric callback id assigned by [`lingxia_messaging`]. Exposed for
    /// diagnostics / logging only — manually calling `remove_callback`
    /// would break the RAII contract.
    pub fn id(&self) -> u64 {
        self.callback_id
    }
}

impl Drop for Subscription {
    fn drop(&mut self) {
        if let Err(err) = self
            .runtime
            .remove_network_change_listener(self.callback_id)
        {
            log::warn!(
                "network::Subscription({}) drop: remove listener failed: {err}",
                self.callback_id
            );
        }
        lingxia_messaging::remove_callback(self.callback_id);
    }
}

/// Subscribe to network state changes. `handler` is invoked with the
/// decoded [`NetworkInfo`] on every platform-reported transition. Hold
/// the returned [`Subscription`] for the lifetime you want the callback
/// active; dropping it unregisters automatically.
pub fn subscribe<F>(app: &Arc<LxApp>, handler: F) -> crate::Result<Subscription>
where
    F: Fn(NetworkInfo) + Send + Sync + 'static,
{
    let callback_id = lingxia_messaging::register_handler(move |result| match result {
        CallbackResult::Success(json) => match serde_json::from_str::<NetworkInfo>(&json) {
            Ok(info) => handler(info),
            Err(err) => log::warn!("network change payload decode failed: {err}"),
        },
        CallbackResult::Error(code) => {
            log::warn!("network change listener reported error code: {code}")
        }
    });
    if let Err(err) = app.runtime.add_network_change_listener(callback_id) {
        lingxia_messaging::remove_callback(callback_id);
        return Err(crate::Error::platform(format!(
            "add_network_change_listener: {err}"
        )));
    }
    Ok(Subscription {
        callback_id,
        runtime: app.runtime.clone(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_wifi_info() {
        let raw = r#"{"isConnected":true,"networkType":"wifi","ipv4":["192.168.1.10"],"ipv6":[]}"#;
        let info: NetworkInfo = serde_json::from_str(raw).unwrap();
        assert!(info.is_connected);
        assert_eq!(info.kind, NetworkKind::Wifi);
        assert!(info.kind.is_wifi());
        assert!(!info.kind.is_cellular());
    }

    #[test]
    fn parses_cellular_levels() {
        for (raw_kind, expected) in [
            ("2g", NetworkKind::Cellular2G),
            ("3g", NetworkKind::Cellular3G),
            ("4g", NetworkKind::Cellular4G),
            ("5g", NetworkKind::Cellular5G),
        ] {
            let raw =
                format!(r#"{{"isConnected":true,"networkType":"{raw_kind}","ipv4":[],"ipv6":[]}}"#);
            let info: NetworkInfo = serde_json::from_str(&raw).unwrap();
            assert_eq!(info.kind, expected);
            assert!(info.kind.is_cellular());
        }
    }

    #[test]
    fn unknown_kinds_fold_into_unknown_variant() {
        // Legacy "cellular" and any unrecognized string both end up here.
        for raw_kind in ["cellular", "lte", "weirdfuturenet"] {
            let raw =
                format!(r#"{{"isConnected":true,"networkType":"{raw_kind}","ipv4":[],"ipv6":[]}}"#);
            let info: NetworkInfo = serde_json::from_str(&raw).unwrap();
            assert_eq!(info.kind, NetworkKind::Unknown);
        }
    }

    #[test]
    fn tolerates_missing_fields() {
        let raw = r#"{"isConnected":false}"#;
        let info: NetworkInfo = serde_json::from_str(raw).unwrap();
        assert!(!info.is_connected);
        assert_eq!(info.kind, NetworkKind::Unknown);
        assert!(info.ipv4.is_empty());
        assert!(info.ipv6.is_empty());
    }
}
