use crate::error::LxAppError;
use crate::page::PageInstanceId;
use crate::page::config::PageConfig;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::time::Duration;

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
    Popup,
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

#[derive(Debug, Clone)]
pub(crate) struct PageInstanceRuntimeRecord {
    pub(crate) owner: PageOwner,
    pub(crate) surface: PresentationKind,
    pub(crate) dispose_ttl: Option<Duration>,
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
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::User => "user",
            Self::Programmatic => "programmatic",
            Self::OwnerClosed => "owner_closed",
            Self::AppClosed => "app_closed",
            Self::Unknown => "unknown",
        }
    }
}

pub(crate) fn transition_page_instance_lifecycle(
    current: PageInstanceLifecycleState,
    event: &PageInstanceEvent,
) -> Result<PageInstanceLifecycleState, LxAppError> {
    use PageInstanceEvent as Event;
    use PageInstanceLifecycleState as State;

    match event {
        Event::Mounted => match current {
            State::Created => Ok(State::Mounted),
            State::Mounted => Ok(State::Mounted),
            State::Visible | State::Hidden => Ok(current),
            State::Disposed => Err(LxAppError::UnsupportedOperation(
                "cannot notify mounted after disposed".to_string(),
            )),
        },
        Event::Visible => match current {
            State::Mounted | State::Visible | State::Hidden => Ok(State::Visible),
            State::Created => Err(LxAppError::UnsupportedOperation(
                "visible before mounted is not allowed".to_string(),
            )),
            State::Disposed => Err(LxAppError::UnsupportedOperation(
                "cannot notify visible after disposed".to_string(),
            )),
        },
        Event::Hidden { .. } => match current {
            State::Mounted | State::Visible | State::Hidden => Ok(State::Hidden),
            State::Created => Err(LxAppError::UnsupportedOperation(
                "hidden before mounted is not allowed".to_string(),
            )),
            State::Disposed => Err(LxAppError::UnsupportedOperation(
                "cannot notify hidden after disposed".to_string(),
            )),
        },
        Event::Resized { .. } => match current {
            State::Disposed => Err(LxAppError::UnsupportedOperation(
                "cannot notify resized after disposed".to_string(),
            )),
            _ => Ok(current),
        },
        Event::Disposed { .. } => match current {
            State::Disposed => Err(LxAppError::UnsupportedOperation(
                "page instance already disposed".to_string(),
            )),
            _ => Ok(State::Disposed),
        },
    }
}
