use crate::lxapp::LxApp;

use super::app::LxAppSvc;
use super::page::PageSvc;

use rong::{JSContext, JSContextService, JSResult, error::HostError};

use std::cell::{Cell, RefCell};
use std::collections::{HashMap, HashSet};
use std::rc::Rc;
use std::sync::{Arc, Weak};

/// AppService state owned by exactly one JSContext.
pub(crate) struct LxAppRuntimeCtx {
    appid: String,
    active: Cell<bool>,
    pub(crate) app: Weak<LxApp>,
    pub(crate) page_svc_map: Rc<RefCell<HashMap<String, PageSvc>>>,
    pub(crate) app_svc: RefCell<Option<LxAppSvc>>,
    /// Tracks which plugin logic.js files have been loaded
    pub(crate) loaded_plugins: Rc<RefCell<HashSet<String>>>,
}

impl LxAppRuntimeCtx {
    fn deactivate(&self) {
        if self.active.replace(false) {
            self.page_svc_map.borrow_mut().clear();
            self.app_svc.borrow_mut().take();
            self.loaded_plugins.borrow_mut().clear();
        }
    }
}

impl JSContextService for LxAppRuntimeCtx {
    fn on_shutdown(&self) {
        self.deactivate();
    }
}

pub(crate) fn register_app_ctx(
    ctx: &JSContext,
    lxapp: &Arc<LxApp>,
) -> Rc<RefCell<HashMap<String, PageSvc>>> {
    let page_svc_map = Rc::new(RefCell::new(HashMap::new()));
    ctx.set_service(LxAppRuntimeCtx {
        appid: lxapp.appid.clone(),
        active: Cell::new(true),
        app: Arc::downgrade(lxapp),
        page_svc_map: page_svc_map.clone(),
        app_svc: RefCell::new(None),
        loaded_plugins: Rc::new(RefCell::new(HashSet::new())),
    });
    page_svc_map
}

pub(crate) fn remove_app_ctx(ctx: &JSContext) {
    if let Some(app_ctx) = ctx.get_service::<LxAppRuntimeCtx>() {
        app_ctx.deactivate();
    }
}

fn with_lxapp_ctx<F, R>(ctx: &JSContext, f: F) -> JSResult<R>
where
    F: FnOnce(&LxAppRuntimeCtx) -> JSResult<R>,
{
    let app_ctx = ctx.get_service::<LxAppRuntimeCtx>().ok_or_else(|| {
        HostError::new(
            rong::error::E_INTERNAL,
            "LxApp runtime context not set in JSContext",
        )
    })?;
    if !app_ctx.active.get() {
        return Err(HostError::new(
            rong::error::E_INTERNAL,
            format!("LxApp runtime context is inactive for {}", app_ctx.appid),
        )
        .into());
    }
    f(app_ctx)
}

fn with_lxapp<F, R>(ctx: &JSContext, f: F) -> JSResult<R>
where
    F: FnOnce(&Arc<LxApp>) -> JSResult<R>,
{
    with_lxapp_ctx(ctx, |app_ctx| {
        let app = app_ctx
            .app
            .upgrade()
            .ok_or_else(|| HostError::new(rong::error::E_INTERNAL, "LxApp has been dropped"))?;
        f(&app)
    })
}

pub(crate) fn with_app_svc<F, R>(ctx: &JSContext, f: F) -> JSResult<R>
where
    F: FnOnce(&LxAppSvc) -> JSResult<R>,
{
    with_lxapp_ctx(ctx, |app_ctx| {
        let app_svc = app_ctx.app_svc.borrow();
        if let Some(ref svc) = *app_svc {
            f(svc)
        } else {
            Err(HostError::new(
                rong::error::E_INTERNAL,
                "LxAppSvc not loaded in LxApp runtime context",
            )
            .into())
        }
    })
}

pub(crate) fn set_app_svc_for_ctx(ctx: &JSContext, app_svc: LxAppSvc) -> JSResult<()> {
    with_lxapp_ctx(ctx, |app_ctx| {
        app_ctx.app_svc.replace(Some(app_svc));
        Ok(())
    })
}

pub(crate) fn with_page_svc_map<F, R>(ctx: &JSContext, f: F) -> JSResult<R>
where
    F: FnOnce(&Rc<RefCell<HashMap<String, PageSvc>>>) -> JSResult<R>,
{
    with_lxapp_ctx(ctx, |app_ctx| f(&app_ctx.page_svc_map))
}

/// Check if a plugin's logic.js has been loaded, and mark it as loaded if not.
/// Returns true if the plugin was NOT loaded before (i.e., needs loading now).
pub(crate) fn mark_plugin_loaded_if_new(ctx: &JSContext, plugin_name: &str) -> JSResult<bool> {
    with_lxapp_ctx(ctx, |app_ctx| {
        let mut loaded = app_ctx.loaded_plugins.borrow_mut();
        if loaded.contains(plugin_name) {
            Ok(false)
        } else {
            loaded.insert(plugin_name.to_string());
            Ok(true)
        }
    })
}

pub(crate) fn unmark_plugin_loaded(ctx: &JSContext, plugin_name: &str) -> JSResult<()> {
    with_lxapp_ctx(ctx, |app_ctx| {
        app_ctx.loaded_plugins.borrow_mut().remove(plugin_name);
        Ok(())
    })
}

/// LxApp extension: derive current LxApp from JSContext.
impl LxApp {
    pub fn from_ctx(ctx: &JSContext) -> JSResult<Arc<LxApp>> {
        with_lxapp(ctx, |app| Ok(app.clone()))
    }
}
