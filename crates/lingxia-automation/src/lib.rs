//! In-process UI/runtime automation JS API for LingXia lxapps.
//!
//! Exposes `lx.automation()` — a factory returning a capability handle whose
//! members (`page`, `nav`, `lxapp`) drive the calling lxapp's own UI and
//! runtime. Gated by the `automation` security privilege; dev sessions grant it
//! by default. This is the product-side, privilege-scoped mapping of the
//! devtool (lxdev) automation surface.

mod host;
mod info;
mod nav;
mod page;
mod resolve;

use lxapp::{LxApp, LxAppSecurityPrivilege, lx};
use rong::{
    Class, FromJSObj, HostError, JSContext, JSFunc, JSObject, JSResult, RongJSError,
    function::Optional, js_class, js_export, js_method,
};
use std::sync::{Arc, Weak};

/// Base-tier privilege: operate on the calling lxapp itself.
const PRIV_AUTOMATION: &str = "automation";
/// Privileged tier: cross-lxapp, browser, and host-window input.
const PRIV_HOST: &str = "host";

/// Build a JS-facing automation error.
pub(crate) fn auto_err(msg: impl AsRef<str>) -> RongJSError {
    HostError::new("E_AUTOMATION", msg.as_ref()).into()
}

fn require_privilege(app: &LxApp, id: &str) -> JSResult<()> {
    // Dev/test hosts grant automation implicitly — including the host tier — so
    // scripts can drive the app without declaring privileges: a `lingxia dev`
    // session, or the Runner (which calls `set_automation_auto_grant`). Product
    // hosts fall through to the manifest gate below; note that gate is
    // self-declared today, pending cloud-side privilege grants for `host`.
    if lxapp::is_dev_session() || lxapp::automation_auto_grant() {
        return Ok(());
    }
    let privilege = LxAppSecurityPrivilege::new(id).map_err(|err| auto_err(err.to_string()))?;
    if app.has_security_privilege(&privilege) {
        Ok(())
    } else {
        Err(auto_err(format!(
            "automation_privilege_required: declare \"{id}\" in lxapp.json security.privileges"
        )))
    }
}

#[derive(FromJSObj, Default, Clone)]
struct JSAutomationOptions {
    host: Option<bool>,
}

/// The capability handle returned by `lx.automation()`. Sub-drivers are
/// created lazily per property access; the drivers themselves are stateless
/// (a `Weak<LxApp>` at most), so no instance caching is needed.
#[js_export]
pub(crate) struct JSAutomation {
    lxapp: Weak<LxApp>,
    host: bool,
}

impl JSAutomation {
    pub(crate) fn new(lxapp: &Arc<LxApp>, host: bool) -> Self {
        Self {
            lxapp: Arc::downgrade(lxapp),
            host,
        }
    }

    fn owner(&self) -> JSResult<Arc<LxApp>> {
        resolve::upgrade(&self.lxapp)
    }

    fn require_host(&self) -> JSResult<()> {
        if self.host {
            Ok(())
        } else {
            Err(auto_err(
                "host tier required: call lx.automation({ host: true })",
            ))
        }
    }
}

#[js_class(rename = "Automation")]
impl JSAutomation {
    #[js_method(constructor)]
    fn _ctor() -> JSResult<()> {
        Err(HostError::new(rong::error::E_ILLEGAL_CONSTRUCTOR, "Use lx.automation()").into())
    }

    #[js_method(getter, enumerable)]
    fn page(&self, ctx: JSContext) -> JSResult<JSObject> {
        let app = self.owner()?;
        Ok(Class::lookup::<page::JSPageDriver>(&ctx)?.instance(page::JSPageDriver::new(&app)))
    }

    #[js_method(getter, enumerable)]
    fn nav(&self, ctx: JSContext) -> JSResult<JSObject> {
        let app = self.owner()?;
        Ok(Class::lookup::<nav::JSNavDriver>(&ctx)?.instance(nav::JSNavDriver::new(&app)))
    }

    /// Base tier: read-only self introspection. Host tier: the cross-app manager.
    #[js_method(getter, enumerable)]
    fn lxapp(&self, ctx: JSContext) -> JSResult<JSObject> {
        if self.host {
            Ok(Class::lookup::<host::JSLxAppManager>(&ctx)?.instance(host::JSLxAppManager::new()))
        } else {
            let app = self.owner()?;
            Ok(Class::lookup::<info::JSSelfInfo>(&ctx)?.instance(info::JSSelfInfo::new(&app)))
        }
    }

    #[js_method(getter, enumerable)]
    fn browser(&self, ctx: JSContext) -> JSResult<JSObject> {
        self.require_host()?;
        Ok(Class::lookup::<host::JSBrowserDriver>(&ctx)?.instance(host::JSBrowserDriver::new()))
    }

    #[js_method(getter, enumerable)]
    fn app(&self, ctx: JSContext) -> JSResult<JSObject> {
        self.require_host()?;
        Ok(Class::lookup::<host::JSAppDriver>(&ctx)?.instance(host::JSAppDriver::new()))
    }
}

/// `lx.automation([{ host }])`. Base tier → `{ page, nav, lxapp(self) }`.
/// Host tier (`{ host: true }`) additionally exposes `{ lxapp(manager),
/// browser, app }`. Permission is checked here, once, per tier.
fn make_automation(ctx: JSContext, options: Optional<JSAutomationOptions>) -> JSResult<JSObject> {
    let lxapp = LxApp::from_ctx(&ctx)?;
    let host = options.as_ref().and_then(|opts| opts.host).unwrap_or(false);

    if host {
        require_privilege(&lxapp, PRIV_HOST)?;
    } else {
        require_privilege(&lxapp, PRIV_AUTOMATION)?;
    }

    Ok(Class::lookup::<JSAutomation>(&ctx)?.instance(JSAutomation::new(&lxapp, host)))
}

struct AutomationExtension;

impl lx::LxLogicExtension for AutomationExtension {
    fn init(&self, ctx: &JSContext) -> JSResult<()> {
        ctx.register_hidden_class::<JSAutomation>()?;
        ctx.register_hidden_class::<page::JSPageDriver>()?;
        ctx.register_hidden_class::<nav::JSNavDriver>()?;
        ctx.register_hidden_class::<info::JSSelfInfo>()?;
        ctx.register_hidden_class::<host::JSLxAppManager>()?;
        ctx.register_hidden_class::<host::JSBrowserDriver>()?;
        ctx.register_hidden_class::<host::JSBrowserCookies>()?;
        ctx.register_hidden_class::<host::JSAppDriver>()?;
        ctx.register_hidden_class::<host::JSAppMouse>()?;
        ctx.register_hidden_class::<host::JSAppKey>()?;
        lx::register_js_api(ctx, "automation", JSFunc::new(ctx, make_automation)?)?;
        Ok(())
    }
}

/// Register the automation runtime. Call once at process bootstrap, before any
/// LxApp JS context is created (same timing as `register_logic_runtime`).
pub fn register_automation_runtime() {
    lx::register_logic_extension(Box::new(AutomationExtension));
}
