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

/// Native-owned GPU endpoint. Methods must enqueue without blocking the caller.
pub trait GpuPageView: Send + Sync {
    fn post_message(&self, message: &str) -> Result<(), LxAppError>;
    /// Recheck this generation at worker delivery, not only while enqueueing.
    fn post_message_for_generation(
        &self,
        generation: GpuViewGeneration,
        message: &str,
    ) -> Result<(), LxAppError>;
    fn connect(&self, generation: GpuViewGeneration, nonce: &str) -> Result<(), LxAppError>;
    fn request_leave(&self, reason: super::LeaveReason) -> Result<(), LxAppError>;
    fn update_environment(&self, update: PageViewEnvironment) -> Result<(), LxAppError>;
    /// Rebuild View state for a fresh Logic connection.
    fn reload(&self) -> Result<(), LxAppError>;
    /// Clear local state and stop presenting until the next entry.
    fn park(&self) -> Result<(), LxAppError>;
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
                    PageViewEnvironment::DisplayLanguage { language, revision } => {
                        let language = serde_json::to_string(&language)?;
                        format!(
                            "var f = window.__lingxiaApplyDisplayLanguage; if (f) f({language}, {revision});"
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
            Self::Gpu(view) => view.post_message(message),
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
