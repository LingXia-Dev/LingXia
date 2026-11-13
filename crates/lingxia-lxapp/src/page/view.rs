use crate::LxAppError;
use lingxia_webview::{DocumentGeneration, DocumentOutboundGate, WebView, WebViewController};
use std::{
    future::Future,
    pin::Pin,
    sync::{Arc, OnceLock},
};

pub type GpuViewCreation =
    Pin<Box<dyn Future<Output = Result<Arc<dyn GpuPageView>, LxAppError>> + Send>>;

/// Installed by the native SDK before opening lxapps. Cancelled creation must release resources.
/// A created View has already been given its module, so the first connect needs no reload.
pub trait GpuPageViewFactory: Send + Sync {
    fn create(&self, page: super::PageInstance) -> GpuViewCreation;
}

static GPU_FACTORY: OnceLock<Arc<dyn GpuPageViewFactory>> = OnceLock::new();

pub fn register_gpu_page_view_factory(
    factory: Arc<dyn GpuPageViewFactory>,
) -> Result<(), LxAppError> {
    GPU_FACTORY
        .set(factory)
        .map_err(|_| LxAppError::Runtime("GPU View factory already registered".into()))
}

pub(crate) fn gpu_factory() -> Option<Arc<dyn GpuPageViewFactory>> {
    GPU_FACTORY.get().cloned()
}

#[derive(Clone, Debug, PartialEq)]
pub enum PageViewEnvironment {
    DisplayLanguage {
        language: String,
        /// `language` narrowed to the languages the lxapp is written in.
        content: String,
        revision: u64,
    },
    SurfaceContext {
        context: serde_json::Value,
        revision: u64,
    },
}

/// Runtime-issued connection identity; the native worker must preserve it through queues.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GpuViewGeneration(pub(crate) u64);

impl GpuViewGeneration {
    pub fn get(self) -> u64 {
        self.0
    }

    /// Hosts never mint generations; tests of native bindings do.
    #[cfg(feature = "test-utils")]
    pub fn new_for_test(value: u64) -> Self {
        Self(value)
    }
}

/// One JSON Patch operation on GPU page data; `path` is a JSON Pointer.
#[derive(Clone, Debug, PartialEq, serde::Deserialize)]
#[serde(tag = "op", rename_all = "lowercase")]
pub enum GpuPatchOp {
    Add {
        path: String,
        value: serde_json::Value,
    },
    Replace {
        path: String,
        value: serde_json::Value,
    },
    Remove {
        path: String,
    },
}

/// Logic state for a GPU View, delivered in process and in write order.
#[derive(Debug)]
pub enum GpuStateWrite {
    /// The whole page data, an object.
    Snapshot { rev: u64, state: serde_json::Value },
    /// `ack`: a `setData` callback waits for [`GpuStateSignal::Acked`] of this revision.
    Patch {
        rev: u64,
        ops: Vec<GpuPatchOp>,
        ack: bool,
    },
}

/// What a GPU View reports about its state writes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GpuStateSignal {
    /// The revision is in a frame committed for display, or will never be.
    Acked(u64),
    /// A patch did not apply; the View drops writes until a snapshot arrives.
    Resync,
}

/// A bridge failure as a reply carries it: `code` is a string or a business code number.
#[derive(Clone, Debug, PartialEq)]
pub struct GpuBridgeError {
    pub code: serde_json::Value,
    pub message: Option<String>,
    pub data: Option<serde_json::Value>,
}

/// What Logic sends a GPU View besides state. Payloads are owned trees, moved, never serialized.
#[derive(Debug)]
pub enum GpuViewMessage {
    Reply {
        id: String,
        outcome: Result<serde_json::Value, GpuBridgeError>,
    },
    StreamEvent {
        id: String,
        seq: u64,
        payload: serde_json::Value,
    },
    ChannelAck {
        id: String,
        outcome: Result<(), GpuBridgeError>,
    },
    ChannelData {
        id: String,
        seq: u64,
        payload: serde_json::Value,
    },
    ChannelClose {
        id: String,
        code: Option<String>,
        reason: Option<String>,
    },
    /// Answered by [`GpuViewEvent::Reply`] under the same id.
    Request { id: String, request: GpuViewRequest },
}

/// Logic requests a GPU View serves, one variant per family. A new family is a variant here,
/// an arm in [`GpuViewRequest::from_call`] and a handler in the View; replies route by id.
#[derive(Clone, Debug, PartialEq)]
pub enum GpuViewRequest {
    Ui(GpuUiRequest),
    Video(GpuVideoRequest),
}

/// `lx.createVideoContext(name)`: one command to the page's player `name`, which is a
/// `useVideo` name or a `<video id>`.
#[derive(Clone, Debug, PartialEq, serde::Deserialize)]
pub struct GpuVideoRequest {
    pub player: String,
    #[serde(flatten)]
    pub command: GpuVideoCommand,
}

#[derive(Clone, Debug, PartialEq, serde::Deserialize)]
#[serde(tag = "op", rename_all = "camelCase")]
pub enum GpuVideoCommand {
    Play,
    Pause,
    Stop,
    Seek { seconds: f64 },
    RequestFullScreen,
    ExitFullScreen,
    SetStreamSource,
    SetQuality { quality: String },
    SetPlaybackRate { rate: f64 },
    SetMuted { muted: bool },
    SetVolume { volume: f64 },
}

/// The feedback overlays a desktop GPU View draws for `lx.showToast` and its siblings.
#[derive(Clone, Debug, PartialEq)]
pub enum GpuUiRequest {
    ShowToast(GpuToast),
    HideToast,
    ShowModal(GpuModal),
    ShowActionSheet(GpuActionSheet),
}

#[derive(Clone, Debug, Default, PartialEq, serde::Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct GpuToast {
    pub title: String,
    pub icon: Option<String>,
    pub duration: Option<f64>,
    pub mask: bool,
    pub position: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq, serde::Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct GpuModal {
    pub title: String,
    pub content: String,
    pub show_cancel: Option<bool>,
    pub cancel_text: Option<String>,
    pub confirm_text: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq, serde::Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct GpuActionSheet {
    pub item_list: Vec<String>,
    pub cancel_text: Option<String>,
}

impl GpuViewRequest {
    /// The request a `call_view(method, params)` makes of a GPU View: `Err((code, message))`
    /// when no family serves `method` or its params do not fit.
    pub(crate) fn from_call(
        method: &str,
        params: Option<serde_json::Value>,
    ) -> Result<Self, (&'static str, String)> {
        let params = params
            .filter(|params| !params.is_null())
            .unwrap_or_else(|| serde_json::json!({}));
        fn fit<T: serde::de::DeserializeOwned>(
            method: &str,
            params: serde_json::Value,
        ) -> Result<T, (&'static str, String)> {
            serde_json::from_value(params)
                .map_err(|error| ("E_INVALID_ARGUMENT", format!("{method}: {error}")))
        }
        Ok(match method {
            "ui.showToast" => Self::Ui(GpuUiRequest::ShowToast(fit(method, params)?)),
            "ui.hideToast" => Self::Ui(GpuUiRequest::HideToast),
            "ui.showModal" => Self::Ui(GpuUiRequest::ShowModal(fit(method, params)?)),
            "ui.showActionSheet" => Self::Ui(GpuUiRequest::ShowActionSheet(fit(method, params)?)),
            "video.command" => Self::Video(fit(method, params)?),
            _ => {
                return Err((
                    "E_UNSUPPORTED",
                    format!("{method} is not served by a GPU View"),
                ));
            }
        })
    }
}

/// What a GPU View sends Logic. Payloads are owned trees, moved, never serialized.
#[derive(Debug)]
pub enum GpuViewEvent {
    /// The View took this connection; Logic may now load the page and write its state.
    Connected,
    /// A page action or stream, or a page-owned `host.navigation.*` control.
    Call {
        id: String,
        method: String,
        params: Option<serde_json::Value>,
        /// When the View queued it, for the debug-build bridge probes; 0 when unknown.
        sent_us: u64,
    },
    Cancel {
        id: String,
    },
    ChannelOpen {
        id: String,
        topic: String,
        params: Option<serde_json::Value>,
    },
    ChannelData {
        id: String,
        payload: serde_json::Value,
    },
    ChannelClose {
        id: String,
        code: Option<String>,
        reason: Option<String>,
    },
    /// The answer to a Logic call into the View.
    Reply {
        id: String,
        outcome: Result<serde_json::Value, GpuBridgeError>,
    },
}

/// Native-owned GPU endpoint. Methods must enqueue without blocking the caller.
pub trait GpuPageView: Send + Sync {
    /// Replies, stream and channel traffic and requests; rechecked for this generation at
    /// delivery like state.
    fn deliver(
        &self,
        generation: GpuViewGeneration,
        message: GpuViewMessage,
    ) -> Result<(), LxAppError>;
    /// Typed Logic state; like messages, rechecked for this generation at delivery.
    fn write_state(
        &self,
        generation: GpuViewGeneration,
        write: GpuStateWrite,
    ) -> Result<(), LxAppError>;
    /// The View answers with [`GpuViewEvent::Connected`] once it can take this generation.
    fn connect(&self, generation: GpuViewGeneration) -> Result<(), LxAppError>;
    fn request_leave(&self, reason: super::LeaveReason) -> Result<(), LxAppError>;
    fn update_environment(&self, update: PageViewEnvironment) -> Result<(), LxAppError>;
    /// Rebuild View state for a fresh Logic connection.
    fn reload(&self) -> Result<(), LxAppError>;
    /// Clear local state and stop presenting until the next entry.
    fn park(&self) -> Result<(), LxAppError>;
    /// Whether a hidden page keeps its presented surface; see `surface_retention`.
    fn retain_surface(&self, retained: bool) -> Result<(), LxAppError>;
    /// Release the context and native surface; must be idempotent.
    fn dispose(&self);
}

#[derive(Clone)]
pub(crate) enum PageView {
    Web(Arc<WebView>),
    Gpu(Arc<dyn GpuPageView>),
}

impl PageView {
    pub(crate) fn update_environment(&self, update: PageViewEnvironment) -> Result<(), LxAppError> {
        match self {
            Self::Gpu(view) => view.update_environment(update),
            Self::Web(view) => {
                let script = match update {
                    PageViewEnvironment::DisplayLanguage {
                        language,
                        content,
                        revision,
                    } => {
                        let language = serde_json::to_string(&language)?;
                        let content = serde_json::to_string(&content)?;
                        format!(
                            "var f = window.__lingxiaApplyDisplayLanguage; if (f) f({language}, {revision}, {content});"
                        )
                    }
                    PageViewEnvironment::SurfaceContext { context, revision } => {
                        crate::lxapp::view_surface_context_script_for(
                            &context.to_string(),
                            revision,
                        )
                    }
                };
                view.exec_js(&script).map_err(LxAppError::from)
            }
        }
    }
    pub(crate) fn webview(&self) -> Option<Arc<WebView>> {
        match self {
            Self::Web(view) => Some(view.clone()),
            Self::Gpu(_) => None,
        }
    }

    pub(crate) fn post_message(&self, message: &str) -> Result<(), LxAppError> {
        match self {
            Self::Web(view) => view.post_message(message).map_err(LxAppError::from),
            Self::Gpu(_) => Err(LxAppError::Bridge(
                "a GPU View takes typed messages only".into(),
            )),
        }
    }

    pub(crate) fn post_message_to_document(
        &self,
        generation: DocumentGeneration,
        gate: Arc<dyn DocumentOutboundGate>,
        message: &str,
    ) -> Result<(), LxAppError> {
        match self {
            Self::Web(view) => view
                .post_message_to_document(generation, gate, message)
                .map_err(LxAppError::from),
            Self::Gpu(_) => Err(LxAppError::Bridge(
                "browser document transport is unavailable for GPU pages".into(),
            )),
        }
    }

    pub(crate) fn dispose(&self) {
        if let Self::Gpu(view) = self {
            view.dispose();
        }
    }
}

#[cfg(test)]
mod gpu_request_tests {
    use super::*;

    #[test]
    fn view_calls_become_typed_requests_and_unknown_methods_are_refused() {
        let toast = GpuViewRequest::from_call(
            "ui.showToast",
            Some(serde_json::json!({"title":"Saved","icon":"success","duration":1500.0,"image":null})),
        )
        .unwrap();
        assert_eq!(
            toast,
            GpuViewRequest::Ui(GpuUiRequest::ShowToast(GpuToast {
                title: "Saved".into(),
                icon: Some("success".into()),
                duration: Some(1500.0),
                ..Default::default()
            }))
        );
        assert_eq!(
            GpuViewRequest::from_call("ui.hideToast", None).unwrap(),
            GpuViewRequest::Ui(GpuUiRequest::HideToast)
        );
        assert_eq!(
            GpuViewRequest::from_call("ui.unknown", None).unwrap_err().0,
            "E_UNSUPPORTED"
        );
        assert_eq!(
            GpuViewRequest::from_call(
                "ui.showActionSheet",
                Some(serde_json::json!({"itemList": 3}))
            )
            .unwrap_err()
            .0,
            "E_INVALID_ARGUMENT"
        );
    }

    #[test]
    fn a_video_command_names_its_player_and_op() {
        assert_eq!(
            GpuViewRequest::from_call(
                "video.command",
                Some(serde_json::json!({"player": "hero", "op": "setQuality", "quality": "720p"}))
            )
            .unwrap(),
            GpuViewRequest::Video(GpuVideoRequest {
                player: "hero".into(),
                command: GpuVideoCommand::SetQuality {
                    quality: "720p".into()
                },
            })
        );
        assert_eq!(
            GpuViewRequest::from_call(
                "video.command",
                Some(serde_json::json!({"player": "hero", "op": "seek"}))
            )
            .unwrap_err()
            .0,
            "E_INVALID_ARGUMENT"
        );
    }
}
