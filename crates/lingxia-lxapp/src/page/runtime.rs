use crate::page::PageInstanceId;
use crate::page::config::PageConfig;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SceneId(pub String);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PageDefinition {
    pub name: Option<String>,
    pub path: String,
    pub config: PageConfig,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PageTarget {
    Name(String),
    Path(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PageQueryInput {
    Raw(String),
    Params(BTreeMap<String, String>),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PageOwner {
    Scene(SceneId),
    Page(PageInstanceId),
    Host,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PresentationKind {
    Window,
    Panel,
    Overlay,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResolvedPage {
    pub appid: String,
    pub path: String,
    pub query: String,
    pub definition: PageDefinition,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CreatePageInstanceRequest {
    pub owner: PageOwner,
    pub appid: String,
    pub target: PageTarget,
    pub query: Option<PageQueryInput>,
    pub surface: PresentationKind,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CreatedPageInstance {
    pub page_instance_id: PageInstanceId,
    pub appid: String,
    pub resolved_path: String,
    pub query: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CloseReason {
    User,
    Programmatic,
    OwnerClosed,
    AppClosed,
    /// SDK reclaimed a long-hidden surface webview for resource reasons.
    /// Behaves like a normal close for the page instance, but distinguishes
    /// "you didn't ask for this" cleanup so consumers can react (e.g.
    /// re-open on next use).
    Reclaimed,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum PageInstanceEvent {
    Mounted,
    Visible,
    Hidden { reason: CloseReason },
    Disposed { reason: CloseReason },
    Resized { width: f64, height: f64 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PageInstanceLifecycleState {
    Created,
    Mounted,
    Visible,
    Hidden,
    Disposed,
}

impl PageInstanceLifecycleState {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Created => "created",
            Self::Mounted => "mounted",
            Self::Visible => "visible",
            Self::Hidden => "hidden",
            Self::Disposed => "disposed",
        }
    }
}

/// Runtime metadata for one live page instance.
#[derive(Debug, Clone, Serialize)]
pub struct PageInstanceRuntimeInfo {
    pub instance_id: String,
    pub name: Option<String>,
    pub path: String,
    pub query: serde_json::Value,
    pub owner: PageOwner,
    pub presentation: PresentationKind,
    pub lifecycle: String,
    pub stack_index: Option<usize>,
    pub current: bool,
    pub state: crate::page::PageAutomationState,
}

#[derive(Debug, Clone)]
pub(crate) struct PageInstanceRuntimeRecord {
    pub(crate) owner: PageOwner,
    pub(crate) surface: PresentationKind,
    pub(crate) page: ResolvedPage,
    pub(crate) lifecycle: PageInstanceLifecycleState,
}

impl PageQueryInput {
    pub(crate) fn to_query_string(&self) -> String {
        match self {
            Self::Raw(raw) => raw.trim_start_matches('?').to_string(),
            Self::Params(params) => params
                .iter()
                .map(|(k, v)| format!("{}={}", urlencoding::encode(k), urlencoding::encode(v)))
                .collect::<Vec<_>>()
                .join("&"),
        }
    }
}

impl CloseReason {
    pub(crate) fn from_reason_str(reason: &str) -> Self {
        match reason.trim().to_ascii_lowercase().as_str() {
            "user" => Self::User,
            "programmatic" => Self::Programmatic,
            "owner_closed" => Self::OwnerClosed,
            "app_closed" => Self::AppClosed,
            "reclaimed" => Self::Reclaimed,
            _ => Self::Unknown,
        }
    }

    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::User => "user",
            Self::Programmatic => "programmatic",
            Self::OwnerClosed => "owner_closed",
            Self::AppClosed => "app_closed",
            Self::Reclaimed => "reclaimed",
            Self::Unknown => "unknown",
        }
    }
}

pub(crate) fn transition_page_instance_lifecycle(
    current: PageInstanceLifecycleState,
    event: &PageInstanceEvent,
) -> Result<PageInstanceLifecycleState, lingxia_error::Error> {
    use PageInstanceEvent as Event;
    use PageInstanceLifecycleState as State;

    match event {
        Event::Mounted => match current {
            State::Created => Ok(State::Mounted),
            State::Mounted => Ok(State::Mounted),
            State::Visible | State::Hidden => Ok(current),
            State::Disposed => Err(lingxia_error::Error::not_supported(
                "cannot notify mounted after disposed",
            )),
        },
        Event::Visible => match current {
            State::Mounted | State::Visible | State::Hidden => Ok(State::Visible),
            State::Created => Err(lingxia_error::Error::not_supported(
                "visible before mounted is not allowed",
            )),
            State::Disposed => Err(lingxia_error::Error::not_supported(
                "cannot notify visible after disposed",
            )),
        },
        Event::Hidden { .. } => match current {
            State::Mounted | State::Visible | State::Hidden => Ok(State::Hidden),
            State::Created => Err(lingxia_error::Error::not_supported(
                "hidden before mounted is not allowed",
            )),
            State::Disposed => Err(lingxia_error::Error::not_supported(
                "cannot notify hidden after disposed",
            )),
        },
        Event::Resized { .. } => match current {
            State::Disposed => Err(lingxia_error::Error::not_supported(
                "cannot notify resized after disposed",
            )),
            _ => Ok(current),
        },
        Event::Disposed { .. } => match current {
            State::Disposed => Err(lingxia_error::Error::not_supported(
                "page instance already disposed",
            )),
            _ => Ok(State::Disposed),
        },
    }
}
