//! GPU pages have no WebView: their automation goes through a per-page backend
//! the GPU page binding registers, answering the same query payload, so every
//! front-end (`lx.automation`, `lxdev`) reaches them through this lower half.
use super::resolve_page;
use crate::lxapp::LxApp;
use serde_json::Value;
use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex, OnceLock, Weak};

pub type GpuFuture<T> = Pin<Box<dyn Future<Output = Result<T, String>> + Send>>;

/// A locator action on a GPU page, addressed like the WebView path (`css` + `index`).
#[derive(Clone, Debug, PartialEq)]
pub enum GpuAction {
    Click {
        force: bool,
    },
    Fill {
        text: String,
        force: bool,
    },
    Type {
        text: String,
    },
    ScrollIntoView,
    /// A key by its DOM name; an empty selector presses on what has focus.
    Press {
        key: String,
    },
    /// Choose a `<select>` option by value, else by label.
    SelectOption {
        value: String,
    },
    /// Select the word at the match's center (text, editors); a click elsewhere.
    DoubleClick,
    /// Take keyboard focus from the match when it holds it.
    Blur,
    /// A semantic action by name, as assistive tech performs it.
    Action {
        name: String,
    },
}

/// What a live GPU page offers automation; implemented by the GPU page binding.
pub trait GpuPageAutomation: Send + Sync {
    /// The DOM query payload for `css`.
    fn query(
        &self,
        css: &str,
        index: Option<usize>,
        all: bool,
        max_text: Option<usize>,
    ) -> GpuFuture<Value>;
    /// Resolve `css`/`index` and run the action through the page's input paths.
    fn act(&self, css: &str, index: Option<usize>, action: GpuAction) -> GpuFuture<()>;
    /// A PNG of the presented frame. A `<webview>` whose content has not drawn yet fails it
    /// with `E_WEBVIEW_NOT_READY`, unless `allow_not_ready` takes its placeholder.
    fn screenshot(&self, allow_not_ready: bool) -> GpuFuture<Vec<u8>>;
    /// The accessibility tree of the presented frame.
    fn tree(&self) -> GpuFuture<Value> {
        Box::pin(async { Err("E_UNSUPPORTED: this GPU page binding has no tree".to_owned()) })
    }
    /// Start page action `name` as `view.actions[name](args)` would; answers its call id.
    fn action(&self, _name: &str, _args: Option<Value>) -> GpuFuture<u64> {
        Box::pin(async { Err("E_UNSUPPORTED: this GPU page binding has no actions".to_owned()) })
    }
    /// A started call's value once it settled and its frame is committed; `None` while pending.
    fn result(&self, _call: u64) -> GpuFuture<Option<Value>> {
        Box::pin(async { Err("E_UNSUPPORTED: this GPU page binding has no actions".to_owned()) })
    }
    /// Presentation counters: `ui.frame` and `ui.reveal` (see `page_frame`).
    fn frame(&self) -> GpuFuture<Value> {
        Box::pin(async {
            Err("E_UNSUPPORTED: this GPU page binding has no frame counters".to_owned())
        })
    }
    /// [`Self::frame`] with frame pacing over the last `window_ms` (default 5 s).
    fn metrics(&self, _window_ms: Option<u64>) -> GpuFuture<Value> {
        self.frame()
    }
    /// Draw the renderer's frame-statistics HUD over the page, or stop.
    fn hud(&self, _on: bool) -> GpuFuture<Value> {
        Box::pin(async { Err("E_UNSUPPORTED: this GPU page binding has no HUD".to_owned()) })
    }
    /// Pin the page's theme and still its motion for a test, or clear (`None`).
    /// Answers as [`Self::environment_facts`].
    fn environment(&self, _pin: Option<TestEnvironment>) -> GpuFuture<Value> {
        Box::pin(async {
            Err("E_UNSUPPORTED: this GPU page binding has no test environment".to_owned())
        })
    }
    /// The pin in force with the presented frame's `textScale` and `language`.
    fn environment_facts(&self) -> GpuFuture<Value> {
        Box::pin(async {
            Err("E_UNSUPPORTED: this GPU page binding has no test environment".to_owned())
        })
    }
    /// Hold the `index`th match of `css` hovered as a resting pointer would, or lift the pin
    /// (`hover: false`, selector ignored).
    fn pin_state(&self, _css: &str, _index: Option<usize>, _hover: bool) -> GpuFuture<Value> {
        Box::pin(async { Err("E_UNSUPPORTED: this GPU page binding has no hover pin".to_owned()) })
    }
}

/// A test run's override of a GPU page's environment.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TestEnvironment {
    /// `Some(true)` dark, `Some(false)` light, `None` the device's theme.
    pub dark: Option<bool>,
    /// Reduced motion on and the caret held steady.
    pub still: bool,
    /// The system text size as a factor of the default; `None` the device's.
    pub text_scale: Option<f32>,
    /// `Some(true)` increased contrast, `Some(false)` standard, `None` the device's.
    pub high_contrast: Option<bool>,
}

type Registry = Mutex<HashMap<String, Weak<dyn GpuPageAutomation>>>;

fn registry() -> &'static Registry {
    static PAGES: OnceLock<Registry> = OnceLock::new();
    PAGES.get_or_init(Default::default)
}

/// Register a GPU page by its page instance id; dropping the backend unregisters it.
pub fn register(instance_id: &str, page: Weak<dyn GpuPageAutomation>) {
    let mut pages = registry().lock().unwrap_or_else(|e| e.into_inner());
    pages.retain(|_, page| page.strong_count() > 0);
    pages.insert(instance_id.to_owned(), page);
}

/// Why the GPU page registered as `instance_id` will never be ready: its first render failed.
/// `None` for a page that drew, is still loading, or is not a GPU page.
pub(super) async fn first_frame_failure(instance_id: &str) -> Option<String> {
    let page = registry()
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .get(instance_id)
        .and_then(Weak::upgrade)?;
    // Any query on a page that failed before its first frame answers with the failure.
    page.query("[data-testid=\"lx-none\"]", None, false, Some(0))
        .await
        .err()
        .filter(|error| error.starts_with("E_PAGE_FAILED:"))
}

/// The backend of a GPU page, or `None` for a WebView page (or one the
/// WebView path should report on).
pub(super) fn backend(
    app: &Arc<LxApp>,
    page: Option<&str>,
) -> Result<Option<Arc<dyn GpuPageAutomation>>, String> {
    Ok(instance_backend(app, page)?.map(|(_, page)| page))
}

/// A GPU page's automation backend and the page instance id it is registered under.
pub(super) type InstanceBackend = (String, Arc<dyn GpuPageAutomation>);

/// [`backend`] with the page instance id it is registered under.
pub(super) fn instance_backend(
    app: &Arc<LxApp>,
    page: Option<&str>,
) -> Result<Option<InstanceBackend>, String> {
    let Ok((instance, _)) = resolve_page(app, page) else {
        return Ok(None);
    };
    if instance.renderer() != crate::PageRenderer::Gpu {
        return Ok(None);
    }
    let id = instance.instance_id_string();
    let pages = registry().lock().unwrap_or_else(|e| e.into_inner());
    match pages.get(&id).and_then(Weak::upgrade) {
        Some(page) => Ok(Some((id, page))),
        None => Err("E_PAGE_NOT_READY: the GPU page has not presented".into()),
    }
}
