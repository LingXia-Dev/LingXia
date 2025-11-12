use std::fmt;
use std::hash::Hash;

// Unified AppServiceEvent for app-level events (lifecycle + update)
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum AppServiceEvent {
    // Lifecycle
    OnLaunch,
    OnShow,
    OnHide,
    // Update
    UpdateReady,
    UpdateFailed,
}

impl AppServiceEvent {
    pub fn as_str(&self) -> &'static str {
        match self {
            AppServiceEvent::OnLaunch => "onLaunch",
            AppServiceEvent::OnShow => "onShow",
            AppServiceEvent::OnHide => "onHide",
            AppServiceEvent::UpdateReady => "UpdateReady",
            AppServiceEvent::UpdateFailed => "UpdateFailed",
        }
    }

    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "onLaunch" => Some(AppServiceEvent::OnLaunch),
            "onShow" => Some(AppServiceEvent::OnShow),
            "onHide" => Some(AppServiceEvent::OnHide),
            "UpdateReady" => Some(AppServiceEvent::UpdateReady),
            "UpdateFailed" => Some(AppServiceEvent::UpdateFailed),
            _ => None,
        }
    }
}

impl fmt::Display for AppServiceEvent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

// App-level events
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

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum LxAppUpdateEvent {
    UpdateReady,
    UpdateFailed,
}

impl LxAppUpdateEvent {
    pub fn as_str(&self) -> &'static str {
        match self {
            LxAppUpdateEvent::UpdateReady => "UpdateReady",
            LxAppUpdateEvent::UpdateFailed => "UpdateFailed",
        }
    }
}

impl fmt::Display for LxAppUpdateEvent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum LxAppEvent {
    Lifecycle(LxAppLifecycleEvent),
    Update(LxAppUpdateEvent),
}

impl LxAppEvent {
    pub fn as_str(&self) -> &'static str {
        match self {
            LxAppEvent::Lifecycle(l) => l.as_str(),
            LxAppEvent::Update(u) => u.as_str(),
        }
    }
}

impl fmt::Display for LxAppEvent {
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
}

impl PageServiceEvent {
    pub fn as_str(&self) -> &'static str {
        match self {
            PageServiceEvent::OnLoad => "onLoad",
            PageServiceEvent::OnShow => "onShow",
            PageServiceEvent::OnReady => "onReady",
            PageServiceEvent::OnHide => "onHide",
            PageServiceEvent::OnUnload => "onUnload",
        }
    }

    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "onLoad" => Some(PageServiceEvent::OnLoad),
            "onShow" => Some(PageServiceEvent::OnShow),
            "onReady" => Some(PageServiceEvent::OnReady),
            "onHide" => Some(PageServiceEvent::OnHide),
            "onUnload" => Some(PageServiceEvent::OnUnload),
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
            PageLifecycleEvent::Unknown => "unknown",
        }
    }
}

impl From<PageLifecycleEvent> for String {
    fn from(event: PageLifecycleEvent) -> Self {
        event.as_str().to_string()
    }
}
