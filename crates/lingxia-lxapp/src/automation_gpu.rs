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
    Click { force: bool },
    Fill { text: String, force: bool },
    Type { text: String },
    ScrollIntoView,
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
    /// A PNG of the presented frame.
    fn screenshot(&self) -> GpuFuture<Vec<u8>>;
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

/// The backend of a GPU page, or `None` for a WebView page (or one the
/// WebView path should report on).
pub(super) fn backend(
    app: &Arc<LxApp>,
    page: Option<&str>,
) -> Result<Option<Arc<dyn GpuPageAutomation>>, String> {
    let Ok((instance, _)) = resolve_page(app, page) else {
        return Ok(None);
    };
    if instance.renderer() != crate::PageRenderer::Gpu {
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
