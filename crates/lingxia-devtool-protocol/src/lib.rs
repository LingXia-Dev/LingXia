#[cfg(feature = "broker")]
pub mod broker;
pub mod session_test;

use serde::{Deserialize, Serialize};

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

    pub mod lxapp_device {
        /// List the device presets the runner offers (id, name, group, size).
        pub const LIST: &str = "lxapp.device.list";
        /// Report the currently selected device and orientation.
        pub const GET: &str = "lxapp.device.get";
        /// Switch the simulated device (by preset id) and/or orientation.
        pub const SET: &str = "lxapp.device.set";
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
pub enum DevtoolsPeerRole {
    Devtool,
    Client,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DevtoolsLogLevel {
    Verbose,
    Debug,
    Info,
    Warn,
    Error,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DevtoolsLogSource {
    Native,
    WebViewConsole,
    LxAppServiceConsole,
    BrowserConsole,
    /// Isolated host automation output mirrored into the session log (`path`
    /// carries the run id). `lxdev test` live output flows through poll.
    Automation,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DevtoolsLogMessage {
    pub timestamp_ms: u64,
    #[serde(alias = "tag")]
    pub source: DevtoolsLogSource,
    pub level: DevtoolsLogLevel,
    pub appid: Option<String>,
    pub path: Option<String>,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum DevtoolsWireMessage {
    Hello {
        role: DevtoolsPeerRole,
        /// Session auth token echoed from the `?token=` query of the websocket
        /// URL. The server also authenticates that upgrade-query token directly
        /// so cached peers from before this field remain compatible.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        token: Option<String>,
    },
    LogBatch {
        logs: Vec<DevtoolsLogMessage>,
    },
    Command {
        command_id: String,
        handler: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        args: Option<serde_json::Value>,
    },
    Result {
        command_id: String,
        ok: bool,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        data: Option<serde_json::Value>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        error: Option<String>,
    },
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
    fn hello_without_token_deserializes_as_none() {
        let hello: DevtoolsWireMessage =
            serde_json::from_str(r#"{"type":"hello","role":"client"}"#).unwrap();
        let DevtoolsWireMessage::Hello { role, token } = hello else {
            panic!("expected hello");
        };
        assert_eq!(role, DevtoolsPeerRole::Client);
        assert_eq!(token, None);
    }
}
