#[cfg(feature = "broker")]
pub mod broker;
pub mod session_test;

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub const DEV_SESSION_PROTOCOL_VERSION: u32 = 2;

pub mod capabilities {
    pub const REQUESTS: &str = "requests";
    pub const LOG_EVENTS: &str = "events.log";
}

pub mod event_kinds {
    pub const LOG: &str = "log";
}

/// Extract the session auth token from a dev websocket URL's `?token=` query
/// parameter. The tokened URL is the single credential artifact: the server
/// prints it, and every peer parses the token back out to present in `Hello`.
pub fn token_from_ws_url(ws_url: &str) -> Option<String> {
    let (_, query) = ws_url.split_once('?')?;
    query.split('&').find_map(|pair| {
        let (key, value) = pair.split_once('=')?;
        (key == "token" && !value.is_empty()).then(|| value.to_string())
    })
}

/// Append a `?token=` query to a dev websocket URL. Authority-only URLs
/// (`ws://host:port`) get an explicit `/` first so naive authority parsers
/// (`split('/')`) never see the query glued to the port.
pub fn ws_url_with_token(ws_url: &str, token: &str) -> String {
    if ws_url.contains('?') {
        return format!("{ws_url}&token={token}");
    }
    let has_path = ws_url
        .split_once("://")
        .is_some_and(|(_, rest)| rest.contains('/'));
    let separator = if has_path { "?" } else { "/?" };
    format!("{ws_url}{separator}token={token}")
}

pub mod handlers {
    pub const ECHO: &str = "echo";

    pub mod session {
        /// Request the owning `lingxia dev` process to stop this session.
        /// Handled by the dev server, not forwarded to the runtime.
        pub const SHUTDOWN: &str = "session.shutdown";

        /// Session test runner (`lxdev test`). Runtime-owned: the dev server
        /// forwards these to the host, which executes the bundled test in an
        /// isolated `lingxia-automation` runtime.
        pub mod test {
            pub const START: &str = "session.test.start";
            pub const POLL: &str = "session.test.poll";
            pub const CANCEL: &str = "session.test.cancel";
        }
    }

    pub mod browser {
        pub const OPEN: &str = "browser.open";
        pub const TABS: &str = "browser.tabs";
        pub const CURRENT: &str = "browser.current";
        pub const ACTIVATE: &str = "browser.activate";
        pub const CLOSE: &str = "browser.close";
        pub const RELOAD: &str = "browser.reload";
        pub const BACK: &str = "browser.back";
        pub const FORWARD: &str = "browser.forward";
        pub const EVAL: &str = "browser.eval";
        pub const QUERY: &str = "browser.query";
        pub const WAIT: &str = "browser.wait";
        pub const WAIT_URL: &str = "browser.wait_url";
        pub const WAIT_NAVIGATION: &str = "browser.wait_navigation";
        pub const CLICK: &str = "browser.click";
        pub const TYPE: &str = "browser.type";
        pub const FILL: &str = "browser.fill";
        pub const PRESS: &str = "browser.press";
        pub const SCROLL: &str = "browser.scroll";
        pub const SCROLL_TO: &str = "browser.scroll_to";
        pub const COOKIES_LIST: &str = "browser.cookies.list";
        pub const COOKIES_SET: &str = "browser.cookies.set";
        pub const COOKIES_DELETE: &str = "browser.cookies.delete";
        pub const COOKIES_CLEAR: &str = "browser.cookies.clear";
        pub const SCREENSHOT: &str = "browser.screenshot";
        // Network capture (Windows/WebView2 CDP only).
        pub const NETWORK_ENABLE: &str = "browser.network.enable";
        pub const NETWORK_DISABLE: &str = "browser.network.disable";
        pub const NETWORK_LIST: &str = "browser.network.list";
        pub const NETWORK_CLEAR: &str = "browser.network.clear";
    }

    pub mod lxapp {
        pub const LIST: &str = "lxapp.list";
        pub const CURRENT: &str = "lxapp.current";
        /// Report page screenshot/input support and the runtime tier.
        pub const DOCTOR: &str = "lxapp.doctor";
        pub const INFO: &str = "lxapp.info";
        pub const PAGES: &str = "lxapp.pages";
        pub const EVAL: &str = "lxapp.eval";
        pub const OPEN: &str = "lxapp.open";
        pub const CLOSE: &str = "lxapp.close";
        pub const RESTART: &str = "lxapp.restart";
        pub const UNINSTALL: &str = "lxapp.uninstall";
        /// Rebuild the lxapp front-end bundle. Handled by the `lingxia dev`
        /// orchestrator (which owns the project + build pipeline), not the
        /// runtime — so it works even with no app attached.
        pub const BUILD: &str = "lxapp.build";
    }

    pub mod lxapp_nav {
        pub const TO: &str = "lxapp.nav.to";
        pub const REDIRECT: &str = "lxapp.nav.redirect";
        pub const SWITCH_TAB: &str = "lxapp.nav.switch_tab";
        pub const RELAUNCH: &str = "lxapp.nav.relaunch";
        pub const BACK: &str = "lxapp.nav.back";
    }

    pub mod app {
        /// Report host-window automation capabilities and coordinate units.
        pub const DOCTOR: &str = "app.doctor";

        /// Capture a PNG of the host app's window. Accepts an optional
        /// `window_id` (returned by [`WINDOWS`]) so multi-window desktop
        /// apps can pick a specific surface; mobile platforms ignore it
        /// since they have a single foreground window. Returns the unified
        /// screenshot envelope `{target, kind, coordinate_space, format,
        /// width, height, size_bytes, image:{mime, encoding, data}}`.
        pub const SCREENSHOT: &str = "app.screenshot";

        /// Enumerate the host app's top-level windows. Returns a JSON
        /// array of `{id, title, focused, main, visible, width, height}`.
        pub const WINDOWS: &str = "app.windows";

        /// Dispatch mouse input to a host app window. Accepts
        /// `{window_id?, action}` where action is a tagged object such as
        /// `{kind:"click", x, y, button?}`.
        pub const MOUSE: &str = "app.mouse";

        /// Dispatch keyboard input to a host app window's focused control.
        /// Accepts `{window_id?, action}` where action is a tagged object
        /// such as `{kind:"type", text}` or `{kind:"press", key, modifiers?}`.
        pub const KEYBOARD: &str = "app.keyboard";
    }

    pub mod runner {
        /// List the device presets the runner can simulate.
        pub const PRESETS: &str = "runner.presets";
        /// Report the simulated environment (device, orientation, appearance).
        pub const GET: &str = "runner.get";
        /// Update the simulated environment; only provided fields change.
        /// Args: `{id?, landscape?, appearance?}`.
        pub const SET: &str = "runner.set";
    }

    pub mod lxapp_page {
        pub const CURRENT: &str = "lxapp.page.current";
        pub const LIST: &str = "lxapp.page.list";
        pub const INFO: &str = "lxapp.page.info";
        pub const WAIT: &str = "lxapp.page.wait";
        pub const EVAL: &str = "lxapp.page.eval";
        pub const QUERY: &str = "lxapp.page.query";
        pub const CLICK: &str = "lxapp.page.click";
        pub const TYPE: &str = "lxapp.page.type";
        pub const FILL: &str = "lxapp.page.fill";
        pub const PRESS: &str = "lxapp.page.press";
        pub const SCROLL: &str = "lxapp.page.scroll";
        pub const SCROLL_TO: &str = "lxapp.page.scroll_to";
        pub const BACK: &str = "lxapp.page.back";
        pub const SCREENSHOT: &str = "lxapp.page.screenshot";
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DevSessionRole {
    Runtime,
    Controller,
    Companion,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DevSessionLogLevel {
    Verbose,
    Debug,
    Info,
    Warn,
    Error,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DevSessionLog {
    pub level: DevSessionLogLevel,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub appid: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target: Option<String>,
    pub message: String,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub attributes: BTreeMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DevSessionEvent {
    pub timestamp_ms: u64,
    /// Open producer-defined namespace used for filtering and routing.
    pub origin: String,
    pub kind: String,
    #[serde(default)]
    pub data: serde_json::Value,
}

impl DevSessionEvent {
    pub fn log(
        timestamp_ms: u64,
        origin: impl Into<String>,
        log: DevSessionLog,
    ) -> Result<Self, serde_json::Error> {
        Ok(Self {
            timestamp_ms,
            origin: origin.into(),
            kind: event_kinds::LOG.to_string(),
            data: serde_json::to_value(log)?,
        })
    }

    pub fn as_log(&self) -> Result<Option<DevSessionLog>, serde_json::Error> {
        if self.kind != event_kinds::LOG {
            return Ok(None);
        }
        serde_json::from_value(self.data.clone()).map(Some)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DevSessionError {
    pub code: String,
    pub message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum DevSessionMessage {
    Hello {
        version: u32,
        role: DevSessionRole,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        capabilities: Vec<String>,
    },
    EventBatch {
        events: Vec<DevSessionEvent>,
    },
    Request {
        id: String,
        method: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        params: Option<serde_json::Value>,
    },
    Response {
        id: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        result: Option<serde_json::Value>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        error: Option<DevSessionError>,
    },
}

impl DevSessionMessage {
    pub fn success(id: impl Into<String>, result: Option<serde_json::Value>) -> Self {
        Self::Response {
            id: id.into(),
            result,
            error: None,
        }
    }

    pub fn error(
        id: impl Into<String>,
        code: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Self::Response {
            id: id.into(),
            result: None,
            error: Some(DevSessionError {
                code: code.into(),
                message: message.into(),
                data: None,
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn token_appends_with_path_separator_on_authority_only_urls() {
        let url = ws_url_with_token("ws://192.168.1.20:39142", "abc");
        assert_eq!(url, "ws://192.168.1.20:39142/?token=abc");
        assert_eq!(token_from_ws_url(&url).as_deref(), Some("abc"));
    }

    #[test]
    fn token_round_trips_with_existing_path_and_query() {
        assert_eq!(
            ws_url_with_token("ws://h:1/x", "t"),
            "ws://h:1/x?token=t".to_string()
        );
        assert_eq!(
            ws_url_with_token("ws://h:1/x?a=b", "t"),
            "ws://h:1/x?a=b&token=t".to_string()
        );
        assert_eq!(
            token_from_ws_url("ws://h:1/x?a=b&token=t").as_deref(),
            Some("t")
        );
        assert_eq!(token_from_ws_url("ws://h:1/x?a=b"), None);
        assert_eq!(token_from_ws_url("ws://h:1"), None);
    }

    #[test]
    fn hello_declares_version_role_and_capabilities() {
        let hello: DevSessionMessage = serde_json::from_str(
            r#"{"type":"hello","version":2,"role":"controller","capabilities":["requests"]}"#,
        )
        .unwrap();
        let DevSessionMessage::Hello {
            version,
            role,
            capabilities,
        } = hello
        else {
            panic!("expected hello");
        };
        assert_eq!(version, DEV_SESSION_PROTOCOL_VERSION);
        assert_eq!(role, DevSessionRole::Controller);
        assert_eq!(capabilities, [capabilities::REQUESTS]);
    }

    #[test]
    fn log_event_round_trips_open_origin_and_attributes() {
        let mut attributes = BTreeMap::new();
        attributes.insert("request.id".to_string(), serde_json::json!("req-1"));
        let event = DevSessionEvent::log(
            42,
            "runtime.console",
            DevSessionLog {
                level: DevSessionLogLevel::Info,
                appid: Some("com.example".to_string()),
                path: Some("pages/home".to_string()),
                target: Some("console".to_string()),
                message: "ready".to_string(),
                attributes,
            },
        )
        .unwrap();
        let encoded = serde_json::to_string(&event).unwrap();
        let decoded: DevSessionEvent = serde_json::from_str(&encoded).unwrap();
        assert_eq!(decoded.origin, "runtime.console");
        let log = decoded.as_log().unwrap().unwrap();
        assert_eq!(log.message, "ready");
        assert_eq!(log.attributes["request.id"], "req-1");
    }

    #[test]
    fn response_error_is_structured() {
        let message = DevSessionMessage::error("7", "unknown_method", "not supported");
        let encoded = serde_json::to_value(message).unwrap();
        assert_eq!(encoded["type"], "response");
        assert_eq!(encoded["id"], "7");
        assert_eq!(encoded["error"]["code"], "unknown_method");
        assert!(encoded.get("result").is_none());
    }
}
