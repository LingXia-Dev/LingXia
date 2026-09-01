use serde::{Deserialize, Serialize};

/// Values the launcher carries so the product's own executable knows it was
/// typed rather than launched, and where to reach the app it belongs to.
///
/// A contract between two sides that share no code: the app writes the
/// launcher, a separate process reads it back.
pub mod invocation {
    /// Inserted into argv by the product launcher and consumed before clap or
    /// a provider sees it. Unlike an environment marker, it is part of the
    /// invocation itself and cannot disappear when an agent sanitizes env.
    pub const CLI_ARGUMENT: &str = "--cli";

    /// The control endpoint. A Windows pipe is a kernel name derived from the
    /// app id, which a client cannot read before the runtime is up, so the
    /// launcher carries it.
    pub const ENDPOINT: &str = "LINGXIA_CONTROL_ENDPOINT";

    /// Normalize a product name to the unquoted command written by its launcher.
    pub fn command_name(product_name: &str) -> String {
        let mut name = String::new();
        for character in product_name.chars() {
            if character.is_ascii_alphanumeric() {
                name.push(character.to_ascii_lowercase());
            } else if !name.is_empty() && !name.ends_with('-') {
                name.push('-');
            }
        }
        let name = name.trim_end_matches('-');
        if name.is_empty() {
            "app".to_string()
        } else {
            name.to_string()
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn command_names_are_typable_without_quoting() {
            assert_eq!(command_name("LingXia Demo"), "lingxia-demo");
            assert_eq!(command_name("My_App"), "my-app");
            assert_eq!(command_name("My  Term!!"), "my-term");
            assert_eq!(command_name("!!!"), "app");
        }
    }
}

pub mod methods {
    pub const ECHO: &str = "echo";

    /// The automation interface talking about itself. Both are answered
    /// without a capability declaration: they reveal only whether anyone is
    /// listening, which a connect already reveals, and the only power they
    /// offer is taking automation *away*.
    pub mod control {
        pub const STATUS: &str = "control.status";
        pub const DISABLE: &str = "control.disable";
    }

    pub mod session {
        /// Prepare an optional supervised session participant. The response is
        /// [`crate::dev_session::DevSessionPrepareResult`].
        pub const PREPARE: &str = "session.prepare";

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
        pub const UA_SHOW: &str = "browser.ua.show";
        pub const UA_SET: &str = "browser.ua.set";
        pub const UA_RESET: &str = "browser.ua.reset";
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
        /// Args: `{id?, landscape?, appearance?, capsule?}`.
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

    /// Automating the machine rather than the app, unlocked by
    /// `capabilities.computerUse`.
    ///
    /// A host runs these itself rather than the client running them in its own
    /// process, even though the work is local either way. macOS attributes
    /// Accessibility and Screen Recording to the responsible process, so a
    /// client invoked from a terminal would borrow that terminal's grants —
    /// different answers from different terminals, and a product name the user
    /// never sees in System Settings. Routed here, the grant belongs to the
    /// product, which is the only thing a user can meaningfully allow or revoke.
    pub mod desktop {
        pub const DOCTOR: &str = "desktop.doctor";
        pub const PERMISSIONS: &str = "desktop.permissions";
        pub const REQUEST_PERMISSIONS: &str = "desktop.permissions.request";
        pub const DISPLAYS: &str = "desktop.displays";
        pub const WINDOWS: &str = "desktop.windows";
        pub const SCREENSHOT: &str = "desktop.screenshot";
        pub const PIXEL: &str = "desktop.pixel";
        pub const WAIT_WINDOW: &str = "desktop.wait.window";
        pub const WAIT_PIXEL: &str = "desktop.wait.pixel";

        pub mod window {
            pub const STATUS: &str = "desktop.window.status";
            pub const FOCUS: &str = "desktop.window.focus";
            pub const ACTIVATE: &str = "desktop.window.activate";
            pub const RAISE: &str = "desktop.window.raise";
            pub const MOVE: &str = "desktop.window.move";
            pub const MOVE_DISPLAY: &str = "desktop.window.move_display";
            pub const RESIZE: &str = "desktop.window.resize";
            pub const MINIMIZE: &str = "desktop.window.minimize";
            pub const RESTORE: &str = "desktop.window.restore";
            pub const MAXIMIZE: &str = "desktop.window.maximize";
            pub const CLOSE: &str = "desktop.window.close";
            pub const SET_ALWAYS_ON_TOP: &str = "desktop.window.always_on_top";
        }

        pub mod pointer {
            pub const MOVE: &str = "desktop.pointer.move";
            pub const DOWN: &str = "desktop.pointer.down";
            pub const UP: &str = "desktop.pointer.up";
            pub const CLICK: &str = "desktop.pointer.click";
            pub const SCROLL: &str = "desktop.pointer.scroll";
            pub const DRAG: &str = "desktop.pointer.drag";
        }

        pub mod key {
            pub const TYPE: &str = "desktop.key.type";
            pub const DOWN: &str = "desktop.key.down";
            pub const UP: &str = "desktop.key.up";
            pub const PRESS: &str = "desktop.key.press";
        }

        pub mod ax {
            pub const TREE: &str = "desktop.ax.tree";
            pub const HIT_TEST: &str = "desktop.ax.hit_test";
            pub const QUERY: &str = "desktop.ax.query";
            pub const INVOKE: &str = "desktop.ax.invoke";
            pub const FOCUS: &str = "desktop.ax.focus";
            pub const SET_VALUE: &str = "desktop.ax.set_value";
            pub const SELECT: &str = "desktop.ax.select";
            pub const EXPAND: &str = "desktop.ax.expand";
            pub const COLLAPSE: &str = "desktop.ax.collapse";
            pub const SCROLL_INTO_VIEW: &str = "desktop.ax.scroll_into_view";
            pub const WAIT: &str = "desktop.ax.wait";
        }

        pub mod clipboard {
            pub const GET: &str = "desktop.clipboard.get";
            pub const SET: &str = "desktop.clipboard.set";
            pub const CLEAR: &str = "desktop.clipboard.clear";
            pub const PASTE: &str = "desktop.clipboard.paste";
        }

        pub mod app {
            pub const LAUNCH: &str = "desktop.app.launch";
            pub const QUIT: &str = "desktop.app.quit";
        }

        pub mod process {
            pub const LIST: &str = "desktop.process.list";
            pub const KILL: &str = "desktop.process.kill";
        }

        /// Every method in this namespace. A host matches on these one by one,
        /// so two constants sharing a string would quietly route two commands
        /// to whichever arm came first.
        pub const ALL: &[&str] = &[
            DOCTOR,
            PERMISSIONS,
            REQUEST_PERMISSIONS,
            DISPLAYS,
            WINDOWS,
            SCREENSHOT,
            PIXEL,
            WAIT_WINDOW,
            WAIT_PIXEL,
            window::STATUS,
            window::FOCUS,
            window::ACTIVATE,
            window::RAISE,
            window::MOVE,
            window::MOVE_DISPLAY,
            window::RESIZE,
            window::MINIMIZE,
            window::RESTORE,
            window::MAXIMIZE,
            window::CLOSE,
            window::SET_ALWAYS_ON_TOP,
            pointer::MOVE,
            pointer::DOWN,
            pointer::UP,
            pointer::CLICK,
            pointer::SCROLL,
            pointer::DRAG,
            key::TYPE,
            key::DOWN,
            key::UP,
            key::PRESS,
            ax::TREE,
            ax::HIT_TEST,
            ax::QUERY,
            ax::INVOKE,
            ax::FOCUS,
            ax::SET_VALUE,
            ax::SELECT,
            ax::EXPAND,
            ax::COLLAPSE,
            ax::SCROLL_INTO_VIEW,
            ax::WAIT,
            clipboard::GET,
            clipboard::SET,
            clipboard::CLEAR,
            clipboard::PASTE,
            app::LAUNCH,
            app::QUIT,
            process::LIST,
            process::KILL,
        ];
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ControlRequest {
    pub id: String,
    pub method: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub params: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ControlResponse {
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub result: Option<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<ControlError>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ControlError {
    pub code: String,
    pub message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
}

impl ControlResponse {
    pub fn success(id: impl Into<String>, result: Option<serde_json::Value>) -> Self {
        Self {
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
        Self {
            id: id.into(),
            result: None,
            error: Some(ControlError {
                code: code.into(),
                message: message.into(),
                data: None,
            }),
        }
    }
}

/// The newline-delimited product-control wire. Dev sessions reuse the same
/// request and response payloads inside their larger websocket envelope.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ControlMessage {
    Request(ControlRequest),
    Response(ControlResponse),
}

pub mod dev_session {
    #[cfg(feature = "broker")]
    pub mod broker;
    pub mod session_test;

    use super::{ControlRequest, ControlResponse};
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

    /// Extract the session auth token from a dev websocket URL's `?token=` query.
    pub fn token_from_ws_url(ws_url: &str) -> Option<String> {
        let (_, query) = ws_url.split_once('?')?;
        query.split('&').find_map(|pair| {
            let (key, value) = pair.split_once('=')?;
            (key == "token" && !value.is_empty()).then(|| value.to_string())
        })
    }

    /// Append a `?token=` query to a dev websocket URL.
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
        /// Open producer-defined identifier used for filtering and routing.
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
    pub struct DevSessionPrepareResult {
        pub active: bool,
        #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
        pub runtime_env: BTreeMap<String, String>,
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
        Request(ControlRequest),
        Response(ControlResponse),
    }

    impl DevSessionMessage {
        pub fn success(id: impl Into<String>, result: Option<serde_json::Value>) -> Self {
            Self::Response(ControlResponse::success(id, result))
        }

        pub fn error(
            id: impl Into<String>,
            code: impl Into<String>,
            message: impl Into<String>,
        ) -> Self {
            Self::Response(ControlResponse::error(id, code, message))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use dev_session::*;
    use std::collections::BTreeMap;

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
        let response = ControlResponse::error("7", "unknown_method", "not supported");
        let encoded = serde_json::to_value(ControlMessage::Response(response.clone())).unwrap();
        assert_eq!(encoded["type"], "response");
        assert_eq!(encoded["id"], "7");
        assert_eq!(encoded["error"]["code"], "unknown_method");
        assert!(encoded.get("result").is_none());

        let dev_encoded = serde_json::to_value(DevSessionMessage::Response(response)).unwrap();
        assert_eq!(dev_encoded, encoded);
    }

    #[test]
    fn product_and_dev_request_wires_are_identical() {
        let request = ControlRequest {
            id: "9".to_string(),
            method: methods::ECHO.to_string(),
            params: Some(serde_json::json!({"a": 1})),
        };
        let product = serde_json::to_value(ControlMessage::Request(request.clone())).unwrap();
        let dev = serde_json::to_value(DevSessionMessage::Request(request)).unwrap();
        assert_eq!(product, dev);
        assert_eq!(
            product,
            serde_json::json!({"type":"request","id":"9","method":"echo","params":{"a":1}})
        );
    }
}

#[cfg(test)]
mod desktop_method_tests {
    use super::methods::desktop;

    #[test]
    fn method_names_are_unique_and_namespaced() {
        let mut seen = std::collections::HashSet::new();
        for name in desktop::ALL {
            assert!(
                name.starts_with("desktop."),
                "{name} is dispatched by prefix and would never be reached"
            );
            assert!(seen.insert(*name), "{name} is declared twice");
        }
    }
}
