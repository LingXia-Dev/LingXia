use lingxia_webview::{DocumentGeneration, DocumentOutboundGate, WebView, WebViewController};
use std::{
    future::Future,
    pin::Pin,
    sync::{Arc, OnceLock},
};

pub type GpuViewCreation =
    Pin<Box<dyn Future<Output = Result<Arc<dyn GpuPageView>, lingxia_error::Error>> + Send>>;

/// Installed by the native SDK before opening lxapps. Cancelled creation must release resources.
/// A created View has already been given its module, so the first connect needs no reload.
pub trait GpuPageViewFactory: Send + Sync {
    fn create(&self, page: super::PageInstance) -> GpuViewCreation;

    /// The app session ended; anything kept for its next page can go now.
    fn app_closed(&self, _appid: &str, _session: u64) {}
}

static GPU_FACTORY: OnceLock<Arc<dyn GpuPageViewFactory>> = OnceLock::new();

pub fn register_gpu_page_view_factory(
    factory: Arc<dyn GpuPageViewFactory>,
) -> Result<(), lingxia_error::Error> {
    GPU_FACTORY
        .set(factory)
        .map_err(|_| lingxia_error::Error::internal("GPU View factory already registered"))
}

pub(crate) fn gpu_factory() -> Option<Arc<dyn GpuPageViewFactory>> {
    GPU_FACTORY.get().cloned()
}

pub(crate) fn gpu_app_closed(appid: &str, session: u64) {
    if let Some(factory) = GPU_FACTORY.get() {
        factory.app_closed(appid, session);
    }
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
    /// The capsule's box over the page, `[x, y, width, height]` in points; `None` when the page
    /// has a navigation bar, which holds the capsule.
    PageChrome {
        capsule: Option<[f64; 4]>,
        revision: u64,
    },
    /// The largest factor the system text size may scale this page's text by.
    TextScaleLimit { max: f64 },
    /// The product asks for more contrast than the system's.
    Contrast { more: bool, revision: u64 },
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

/// A bridge failure as a reply carries it: `{ code, message, reason?, data? }`.
#[derive(Clone, Debug, PartialEq)]
pub struct GpuBridgeError {
    pub code: String,
    pub message: Option<String>,
    pub reason: Option<String>,
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
    /// `error` is set when the channel ended in failure.
    ChannelClose {
        id: String,
        error: Option<GpuBridgeError>,
    },
    /// Answered by [`GpuViewEvent::Reply`] under the same id.
    Request { id: String, request: GpuViewRequest },
}

/// Logic requests a GPU View serves, one variant per family; replies route by id.
#[derive(Clone, Debug, PartialEq)]
pub enum GpuViewRequest {
    Video(GpuVideoRequest),
}

/// `page.video(name)`: one command to the page's player `name`, which is a
/// `useVideo` name or a `<video id>`.
#[derive(Clone, Debug, PartialEq)]
pub struct GpuVideoRequest {
    pub player: String,
    pub command: GpuVideoCommand,
}

#[derive(Clone, Debug, PartialEq)]
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
    /// `error` is set when the channel ended in failure.
    ChannelClose {
        id: String,
        error: Option<GpuBridgeError>,
    },
    /// The answer to a Logic call into the View.
    Reply {
        id: String,
        outcome: Result<serde_json::Value, GpuBridgeError>,
    },
    /// A View event handler threw or its promise rejected; Logic reports it to `App.onError`.
    HandlerFailure(GpuHandlerFailure),
}

/// A failed View event handler, as `App.onError` receives its `view_handler` source.
#[derive(Clone, Debug, PartialEq)]
pub struct GpuHandlerFailure {
    /// The handler's prop, such as `onPress`.
    pub prop: String,
    pub test_id: Option<String>,
    /// The page action the handler called, when that call is what failed.
    pub action: Option<String>,
    /// The thrown error's `name` (`Error` when it was not an `Error`).
    pub name: String,
    pub message: String,
}

/// Native-owned GPU endpoint. Methods must enqueue without blocking the caller.
pub trait GpuPageView: Send + Sync {
    /// Replies, stream and channel traffic and requests; rechecked for this generation at
    /// delivery like state.
    fn deliver(
        &self,
        generation: GpuViewGeneration,
        message: GpuViewMessage,
    ) -> Result<(), lingxia_error::Error>;
    /// Typed Logic state; like messages, rechecked for this generation at delivery.
    fn write_state(
        &self,
        generation: GpuViewGeneration,
        write: GpuStateWrite,
    ) -> Result<(), lingxia_error::Error>;
    /// The View answers with [`GpuViewEvent::Connected`] once it can take this generation.
    fn connect(&self, generation: GpuViewGeneration) -> Result<(), lingxia_error::Error>;
    fn request_leave(&self, reason: super::LeaveReason) -> Result<(), lingxia_error::Error>;
    fn update_environment(&self, update: PageViewEnvironment) -> Result<(), lingxia_error::Error>;
    /// Rebuild View state for a fresh Logic connection.
    fn reload(&self) -> Result<(), lingxia_error::Error>;
    /// Clear local state and stop presenting until the next entry.
    fn park(&self) -> Result<(), lingxia_error::Error>;
    /// Covered or left inside the app, not by the app going to the background: the page's
    /// passing layers (a select's sheet, a menu, a popover) close as a tap outside would.
    fn page_left(&self) -> Result<(), lingxia_error::Error> {
        Ok(())
    }
    /// Whether a hidden page keeps its presented surface; see `surface_retention`.
    fn retain_surface(&self, retained: bool) -> Result<(), lingxia_error::Error>;
    /// A hidden page that keeps its surface drops the render targets a redraw rebuilds.
    fn trim_surface(&self) -> Result<(), lingxia_error::Error> {
        Ok(())
    }
    /// Release the context and native surface; must be idempotent.
    fn dispose(&self);
}

#[derive(Clone)]
pub(crate) enum PageView {
    Web(Arc<WebView>),
    Gpu(Arc<dyn GpuPageView>),
}

impl PageView {
    pub(crate) fn update_environment(
        &self,
        update: PageViewEnvironment,
    ) -> Result<(), lingxia_error::Error> {
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
                    // A web page reads the capsule from its page-chrome publication.
                    PageViewEnvironment::PageChrome { .. } => return Ok(()),
                    // The web page's text size follows the WebView's own setting.
                    PageViewEnvironment::TextScaleLimit { .. } => return Ok(()),
                    // Web pages follow the system's contrast.
                    PageViewEnvironment::Contrast { .. } => return Ok(()),
                };
                view.exec_js(&script).map_err(lingxia_error::Error::from)
            }
        }
    }
    pub(crate) fn webview(&self) -> Option<Arc<WebView>> {
        match self {
            Self::Web(view) => Some(view.clone()),
            Self::Gpu(_) => None,
        }
    }

    pub(crate) fn post_message(&self, message: &str) -> Result<(), lingxia_error::Error> {
        match self {
            Self::Web(view) => view
                .post_message(message)
                .map_err(lingxia_error::Error::from),
            Self::Gpu(_) => Err(lingxia_error::Error::internal(
                "a GPU View takes typed messages only",
            )),
        }
    }

    pub(crate) fn post_message_to_document(
        &self,
        generation: DocumentGeneration,
        gate: Arc<dyn DocumentOutboundGate>,
        message: &str,
    ) -> Result<(), lingxia_error::Error> {
        match self {
            Self::Web(view) => view
                .post_message_to_document(generation, gate, message)
                .map_err(lingxia_error::Error::from),
            Self::Gpu(_) => Err(lingxia_error::Error::internal(
                "browser document transport is unavailable for GPU pages",
            )),
        }
    }

    pub(crate) fn dispose(&self) {
        if let Self::Gpu(view) = self {
            view.dispose();
        }
    }
}
