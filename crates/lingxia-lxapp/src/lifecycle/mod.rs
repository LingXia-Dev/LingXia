pub mod key_events;

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

// Page lifecycle events, both for the PageInstance state machine and for the
// Logic handler they are delivered to.
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub enum PageLifecycleEvent {
    OnLoad,
    OnReady,
    OnShow,
    OnHide,
    OnUnload,
    OnPullDownRefresh,
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
        }
    }
}

impl fmt::Display for PageLifecycleEvent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl From<PageLifecycleEvent> for String {
    fn from(event: PageLifecycleEvent) -> Self {
        event.as_str().to_string()
    }
}
