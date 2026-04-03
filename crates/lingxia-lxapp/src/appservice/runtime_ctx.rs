use crate::lxapp::LxApp;

use super::app::LxAppSvc;
use super::page::PageSvc;

use rong::{JSContext, JSResult, JSRuntime, JSRuntimeService, error::HostError};

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;
use std::sync::{Arc, Weak};

/// Identity of the LxApp bound to a JSContext.
#[derive(Clone)]
pub(crate) struct LxAppIdentity {
    pub(crate) appid: String,
}

/// Per-LxApp runtime context stored in a runtime-level registry.
///
/// This keeps JSContext lightweight: it only stores an LxAppIdentity
/// service; the actual LxApp, AppSvc and PageSvc map live here.
#[derive(Clone)]
pub(crate) struct LxAppRuntimeCtx {
    pub(crate) app: Weak<LxApp>,
    pub(crate) page_svc_map: Rc<RefCell<HashMap<String, PageSvc>>>,
    pub(crate) app_svc: Option<LxAppSvc>,
    /// Tracks which plugin logic.js files have been loaded
    pub(crate) loaded_plugins: Rc<RefCell<HashSet<String>>>,
}

/// Runtime-level registry that tracks all LxApp runtime contexts for a JSRuntime.
pub(crate) struct LxAppRegistry {
    pub(crate) apps: RefCell<HashMap<String, LxAppRuntimeCtx>>,
}

impl Default for LxAppRegistry {
    fn default() -> Self {
        Self {
            apps: RefCell::new(HashMap::new()),
        }
    }
}

impl JSRuntimeService for LxAppRegistry {}

pub(crate) fn register_app_ctx(
    runtime: &JSRuntime,
    ctx: &JSContext,
    lxapp: &Arc<LxApp>,
) -> Rc<RefCell<HashMap<String, PageSvc>>> {
    // Bind identity of this LxApp to JSContext for later lookups.
    ctx.set_state(LxAppIdentity {
        appid: lxapp.appid.clone(),
    });

    // Create page_svc map and insert runtime context into registry.
    let page_svc_map: Rc<RefCell<HashMap<String, PageSvc>>> = Rc::new(RefCell::new(HashMap::new()));

    let registry = runtime.get_or_init_service::<LxAppRegistry>();
    registry.apps.borrow_mut().insert(
        lxapp.appid.clone(),
        LxAppRuntimeCtx {
            app: Arc::downgrade(lxapp),
            page_svc_map: page_svc_map.clone(),
            app_svc: None,
            loaded_plugins: Rc::new(RefCell::new(HashSet::new())),
        },
    );

    page_svc_map
}

pub(crate) fn remove_app_ctx(runtime: &JSRuntime, appid: &str) {
    let registry = runtime.get_or_init_service::<LxAppRegistry>();
    registry.apps.borrow_mut().remove(appid);
}

fn with_lxapp_ctx<F, R>(ctx: &JSContext, f: F) -> JSResult<R>
where
    F: FnOnce(&LxAppRuntimeCtx) -> JSResult<R>,
{
    let ident = ctx.get_state::<LxAppIdentity>().ok_or_else(|| {
        HostError::new(
            rong::error::E_INTERNAL,
            "LxAppIdentity not set in JSContext",
        )
    })?;

    let registry = ctx.runtime().get_or_init_service::<LxAppRegistry>();
    let apps = registry.apps.borrow();
    let app_ctx = apps.get(&ident.appid).ok_or_else(|| {
        HostError::new(rong::error::E_INTERNAL, "LxApp runtime context not found")
    })?;
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
        if let Some(ref svc) = app_ctx.app_svc {
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
    let ident = ctx.get_state::<LxAppIdentity>().ok_or_else(|| {
        HostError::new(
            rong::error::E_INTERNAL,
            "LxAppIdentity not set in JSContext",
        )
    })?;

    let registry = ctx.runtime().get_or_init_service::<LxAppRegistry>();
    let mut apps = registry.apps.borrow_mut();
    if let Some(entry) = apps.get_mut(&ident.appid) {
        entry.app_svc = Some(app_svc);
        Ok(())
    } else {
        Err(HostError::new(rong::error::E_INTERNAL, "LxApp runtime context not found").into())
    }
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
