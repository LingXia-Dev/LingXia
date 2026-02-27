use serde::Serialize;
use std::fmt;
use std::hash::Hash;

// Unified AppServiceEvent for app-level events (lifecycle)
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum AppServiceEvent {
    // Lifecycle
    OnLaunch,
    OnShow,
    OnHide,
    OnUserCaptureScreen,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AppServiceEventSource {
    Host,
    Lxapp,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AppServiceEventReason {
    Foreground,
    Background,
    Screenshot,
    Open,
    Close,
    SwitchBack,
    SwitchAway,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize)]
pub struct AppServiceEventArgs {
    pub source: AppServiceEventSource,
    pub reason: AppServiceEventReason,
}

impl AppServiceEventArgs {
    pub fn to_json_string(self) -> String {
        serde_json::to_string(&self).unwrap_or_else(|_| "{}".to_string())
    }
}

impl AppServiceEvent {
    pub fn as_str(&self) -> &'static str {
        match self {
            AppServiceEvent::OnLaunch => "onLaunch",
            AppServiceEvent::OnShow => "onShow",
            AppServiceEvent::OnHide => "onHide",
            AppServiceEvent::OnUserCaptureScreen => "onUserCaptureScreen",
        }
    }

    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "onLaunch" => Some(AppServiceEvent::OnLaunch),
            "onShow" => Some(AppServiceEvent::OnShow),
            "onHide" => Some(AppServiceEvent::OnHide),
            "onUserCaptureScreen" => Some(AppServiceEvent::OnUserCaptureScreen),
            _ => None,
        }
    }
}

impl fmt::Display for AppServiceEvent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

// App-level lifecycle events used by app lifecycle helpers.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum LxAppLifecycleEvent {
    OnLaunch,
    OnShow,
    OnHide,
}

impl LxAppLifecycleEvent {
    pub fn as_str(&self) -> &'static str {
        match self {
            LxAppLifecycleEvent::OnLaunch => "onLaunch",
            LxAppLifecycleEvent::OnShow => "onShow",
            LxAppLifecycleEvent::OnHide => "onHide",
        }
    }

    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "onLaunch" => Some(LxAppLifecycleEvent::OnLaunch),
            "onShow" => Some(LxAppLifecycleEvent::OnShow),
            "onHide" => Some(LxAppLifecycleEvent::OnHide),
            _ => None,
        }
    }
}

impl fmt::Display for LxAppLifecycleEvent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

// Page-level service events
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PageServiceEvent {
    OnLoad,
    OnShow,
    OnReady,
    OnHide,
    OnUnload,
    OnPullDownRefresh,
}

impl PageServiceEvent {
    pub fn as_str(&self) -> &'static str {
        match self {
            PageServiceEvent::OnLoad => "onLoad",
            PageServiceEvent::OnShow => "onShow",
            PageServiceEvent::OnReady => "onReady",
            PageServiceEvent::OnHide => "onHide",
            PageServiceEvent::OnUnload => "onUnload",
            PageServiceEvent::OnPullDownRefresh => "onPullDownRefresh",
        }
    }

    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "onLoad" => Some(PageServiceEvent::OnLoad),
            "onShow" => Some(PageServiceEvent::OnShow),
            "onReady" => Some(PageServiceEvent::OnReady),
            "onHide" => Some(PageServiceEvent::OnHide),
            "onUnload" => Some(PageServiceEvent::OnUnload),
            "onPullDownRefresh" => Some(PageServiceEvent::OnPullDownRefresh),
            _ => None,
        }
    }
}

impl fmt::Display for PageServiceEvent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

// Page lifecycle events (internal state machine and logging)
#[derive(Copy, Clone, PartialEq, Debug)]
pub enum PageLifecycleEvent {
    OnLoad,
    OnReady,
    OnShow,
    OnHide,
    OnUnload,
    OnPullDownRefresh,
    Unknown,
}

impl PageLifecycleEvent {
    pub fn as_str(&self) -> &'static str {
        match self {
            PageLifecycleEvent::OnLoad => "onLoad",
            PageLifecycleEvent::OnReady => "onReady",
            PageLifecycleEvent::OnShow => "onShow",
            PageLifecycleEvent::OnHide => "onHide",
            PageLifecycleEvent::OnUnload => "onUnload",
            PageLifecycleEvent::OnPullDownRefresh => "onPullDownRefresh",
            PageLifecycleEvent::Unknown => "unknown",
        }
    }
}

impl From<PageLifecycleEvent> for String {
    fn from(event: PageLifecycleEvent) -> Self {
        event.as_str().to_string()
    }
}
