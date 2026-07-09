//! `NavDriver` — page-stack navigation + runtime stack reads, for the calling
//! lxapp itself. Action verbs (`to` / `redirect` / `switchTab` / `relaunch`)
//! take a configured page name (+ optional `query`); reads are `current` /
//! `stack`; `back` pops. Semantics come from the shared `lxapp::automation`
//! lower half (tab-bar guard included), matching `lxdev lxapp nav`.

use crate::auto_err;
use crate::resolve::{js_object_to_json, upgrade};
use lxapp::{LxApp, NavigationType, automation as auto};
use rong::{
    FromJSObj, HostError, IntoJSObj, JSContext, JSObject, JSResult, function::Optional, js_class,
    js_export, js_method,
};
use std::sync::{Arc, Weak};

#[js_export]
pub(crate) struct JSNavDriver {
    lxapp: Weak<LxApp>,
}

impl JSNavDriver {
    pub(crate) fn new(lxapp: &Arc<LxApp>) -> Self {
        Self {
            lxapp: Arc::downgrade(lxapp),
        }
    }
}

#[derive(FromJSObj)]
struct JSBackOptions {
    delta: Option<u32>,
}

#[derive(FromJSObj)]
struct JSNavOptions {
    page: String,
    query: Option<JSObject>,
}

#[derive(FromJSObj, Default)]
struct JSPageRef {
    page: Option<String>,
}

#[derive(Debug, Clone, IntoJSObj)]
struct JSPageInfo {
    path: String,
    name: Option<String>,
    current: bool,
    #[rename = "inStack"]
    in_stack: bool,
    ready: bool,
}

impl From<auto::PageStatus> for JSPageInfo {
    fn from(status: auto::PageStatus) -> Self {
        Self {
            path: status.path,
            name: status.name,
            current: status.current,
            in_stack: status.in_stack,
            ready: status.ready,
        }
    }
}

impl JSNavDriver {
    async fn navigate(&self, options: &JSNavOptions, kind: NavigationType) -> JSResult<JSPageInfo> {
        let app = upgrade(&self.lxapp)?;
        let query = options.query.as_ref().map(js_object_to_json).transpose()?;
        let (page, name) = auto::navigate(&app, &options.page, query.as_ref(), kind, false)
            .await
            .map_err(auto_err)?;
        Ok(auto::page_status(&app, &page, name.as_deref()).into())
    }
}

#[js_class(rename = "NavDriver")]
impl JSNavDriver {
    #[js_method(constructor)]
    fn _ctor() -> JSResult<()> {
        Err(HostError::new(rong::error::E_ILLEGAL_CONSTRUCTOR, "Use lx.automation()").into())
    }

    #[js_method]
    async fn to(&self, _ctx: JSContext, options: JSNavOptions) -> JSResult<JSPageInfo> {
        self.navigate(&options, NavigationType::Forward).await
    }

    #[js_method]
    async fn redirect(&self, _ctx: JSContext, options: JSNavOptions) -> JSResult<JSPageInfo> {
        self.navigate(&options, NavigationType::Replace).await
    }

    #[js_method(rename = "switchTab")]
    async fn switch_tab(&self, _ctx: JSContext, options: JSNavOptions) -> JSResult<JSPageInfo> {
        self.navigate(&options, NavigationType::SwitchTab).await
    }

    #[js_method]
    async fn relaunch(&self, _ctx: JSContext, options: JSNavOptions) -> JSResult<JSPageInfo> {
        self.navigate(&options, NavigationType::Launch).await
    }

    #[js_method]
    async fn back(&self, _ctx: JSContext, options: JSBackOptions) -> JSResult<JSPageInfo> {
        let app = upgrade(&self.lxapp)?;
        let (page, name) = auto::navigate_back(&app, options.delta.unwrap_or(1), false)
            .await
            .map_err(auto_err)?;
        Ok(auto::page_status(&app, &page, name.as_deref()).into())
    }

    #[js_method]
    async fn current(&self, _ctx: JSContext) -> JSResult<JSPageInfo> {
        let app = upgrade(&self.lxapp)?;
        let (page, name) = auto::resolve_page(&app, None).map_err(auto_err)?;
        Ok(auto::page_status(&app, &page, name.as_deref()).into())
    }

    /// Status of a configured page by name (`lxdev lxapp page info --page`);
    /// omit `page` for the current page.
    #[js_method]
    async fn info(&self, _ctx: JSContext, options: Optional<JSPageRef>) -> JSResult<JSPageInfo> {
        let app = upgrade(&self.lxapp)?;
        let options = options.0.unwrap_or_default();
        let (page, name) = auto::resolve_page(&app, options.page.as_deref()).map_err(auto_err)?;
        Ok(auto::page_status(&app, &page, name.as_deref()).into())
    }

    #[js_method]
    async fn stack(&self, _ctx: JSContext) -> JSResult<Vec<JSPageInfo>> {
        let app = upgrade(&self.lxapp)?;
        let info = app.runtime_info();
        let current = info.current_page.clone();
        let stack = info
            .page_stack
            .iter()
            .map(|path| JSPageInfo {
                name: auto::page_name_for_path(&app, path),
                current: current
                    .as_deref()
                    .is_some_and(|c| auto::page_paths_match(c, path)),
                in_stack: true,
                ready: app.get_page(path).and_then(|p| p.webview()).is_some(),
                path: path.clone(),
            })
            .collect();
        Ok(stack)
    }
}
