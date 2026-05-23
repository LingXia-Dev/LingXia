use serde::{Deserialize, Serialize};

pub mod handlers {
    pub const ECHO: &str = "echo";

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
    }

    pub mod lxapp {
        pub const LIST: &str = "lxapp.list";
        pub const CURRENT: &str = "lxapp.current";
        pub const INFO: &str = "lxapp.info";
        pub const PAGES: &str = "lxapp.pages";
        pub const EVAL: &str = "lxapp.eval";
        pub const OPEN: &str = "lxapp.open";
        pub const CLOSE: &str = "lxapp.close";
        pub const RESTART: &str = "lxapp.restart";
        pub const UNINSTALL: &str = "lxapp.uninstall";
    }

    pub mod app {
        /// Capture a PNG of the host app's window. Accepts an optional
        /// `window_id` (returned by [`WINDOWS`]) so multi-window desktop
        /// apps can pick a specific surface; mobile platforms ignore it
        /// since they have a single foreground window. Returns a JSON
        /// envelope `{format, size_bytes, data_base64}`.
        pub const SCREENSHOT: &str = "app.screenshot";

        /// Enumerate the host app's top-level windows. Returns a JSON
        /// array of `{id, title, focused, main, visible, width, height}`.
        pub const WINDOWS: &str = "app.windows";
    }

    pub mod lxapp_page {
        pub const CURRENT: &str = "lxapp.page.current";
        pub const LIST: &str = "lxapp.page.list";
        pub const INFO: &str = "lxapp.page.info";
        pub const EVAL: &str = "lxapp.page.eval";
        pub const QUERY: &str = "lxapp.page.query";
        pub const CLICK: &str = "lxapp.page.click";
        pub const TYPE: &str = "lxapp.page.type";
        pub const FILL: &str = "lxapp.page.fill";
        pub const PRESS: &str = "lxapp.page.press";
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
