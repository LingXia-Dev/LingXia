//! GPU pages have no WebView: their automation goes through a per-page
//! backend the GPU page binding registers, answering the same query payload.
use lxapp::{LxApp, automation as auto};
use serde_json::Value;
use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex, OnceLock, Weak};

pub type GpuFuture<T> = Pin<Box<dyn Future<Output = Result<T, String>> + Send>>;

/// A locator action on a GPU page, addressed like the WebView path (`css` + `index`).
#[derive(Clone, Debug, PartialEq)]
pub enum GpuAction {
    Click { force: bool },
    Fill { text: String, force: bool },
    Type { text: String },
    ScrollIntoView,
}

/// What a live GPU page offers automation; implemented by the GPU page binding.
pub trait GpuPageAutomation: Send + Sync {
    /// The DOM query payload (`lingxia_ui::semantics::query::query`) for `css`.
    fn query(
        &self,
        css: &str,
        index: Option<usize>,
        all: bool,
        max_text: Option<usize>,
    ) -> GpuFuture<Value>;
    /// Resolve `css`/`index` and run the action through the page's input paths.
    fn act(&self, css: &str, index: Option<usize>, action: GpuAction) -> GpuFuture<()>;
    /// A PNG of the presented frame.
    fn screenshot(&self) -> GpuFuture<Vec<u8>>;
}

fn registry() -> &'static Mutex<HashMap<String, Weak<dyn GpuPageAutomation>>> {
    static PAGES: OnceLock<Mutex<HashMap<String, Weak<dyn GpuPageAutomation>>>> = OnceLock::new();
    PAGES.get_or_init(Default::default)
}

/// Register a GPU page by its page instance id; dropping the backend unregisters it.
pub fn register(instance_id: &str, page: Weak<dyn GpuPageAutomation>) {
    let mut pages = registry().lock().unwrap_or_else(|e| e.into_inner());
    pages.retain(|_, page| page.strong_count() > 0);
    pages.insert(instance_id.to_owned(), page);
}

fn backend(
    app: &Arc<LxApp>,
    page: Option<&str>,
) -> Result<Option<Arc<dyn GpuPageAutomation>>, String> {
    let (instance, _) = match auto::resolve_page(app, page) {
        Ok(found) => found,
        // Let the WebView path report its own resolution errors.
        Err(_) => return Ok(None),
    };
    if instance.renderer() != lxapp::PageRenderer::Gpu {
        return Ok(None);
    }
    let pages = registry().lock().unwrap_or_else(|e| e.into_inner());
    match pages
        .get(&instance.instance_id_string())
        .and_then(Weak::upgrade)
    {
        Some(page) => Ok(Some(page)),
        None => Err("E_PAGE_NOT_READY: the GPU page has not presented".into()),
    }
}

pub(crate) async fn page_query(
    app: &Arc<LxApp>,
    page: Option<&str>,
    css: &str,
    index: Option<usize>,
    all: bool,
    max_text: Option<usize>,
) -> Result<Value, String> {
    match backend(app, page)? {
        Some(gpu) => gpu.query(css, index, all, max_text).await,
        None => auto::page_query(app, page, css, index, all, max_text).await,
    }
}

pub(crate) async fn page_act(
    app: &Arc<LxApp>,
    page: Option<&str>,
    css: &str,
    index: Option<usize>,
    action: GpuAction,
) -> Result<(), String> {
    if let Some(gpu) = backend(app, page)? {
        return gpu.act(css, index, action).await;
    }
    match action {
        GpuAction::Click { force } => auto::page_click_with(app, page, css, index, force).await,
        GpuAction::Fill { text, force } => {
            auto::page_fill_with(app, page, css, index, &text, force).await
        }
        GpuAction::Type { text } => auto::page_type(app, page, css, index, &text).await,
        GpuAction::ScrollIntoView => auto::page_scroll_to(app, page, css).await,
    }
}

pub(crate) async fn page_screenshot(
    app: &Arc<LxApp>,
    page: Option<&str>,
) -> Result<Vec<u8>, String> {
    match backend(app, page)? {
        Some(gpu) => gpu.screenshot().await,
        None => auto::page_screenshot(app, page).await,
    }
}
