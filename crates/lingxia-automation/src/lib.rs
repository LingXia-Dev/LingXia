//! In-process UI/runtime automation JS API for LingXia lxapps.
//!
//! Exposes `lx.automation()` — a stable root whose `lxapp([appid])` selector
//! returns one lxapp driver. Host-only managers and surfaces enforce their
//! privilege when used; callers do not select an internal privilege tier.

mod desktop;
mod host;
mod info;
mod input;
mod nav;
mod page;
mod resolve;
#[cfg(feature = "runtime")]
pub mod runtime;

use lxapp::{LxApp, LxAppSecurityPrivilege, lx};
use rong::{
    Class, HostError, JSContext, JSFunc, JSObject, JSResult, JSValue, RongJSError,
    function::Optional, js_class, js_method,
};
use std::sync::{Arc, Weak};

/// Operate on the calling lxapp itself.
const PRIV_AUTOMATION: &str = "automation";
/// Cross-lxapp, browser, and host-window input.
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
            "{id}_privilege_required: declare \"{id}\" in lxapp.json security.privileges"
        )))
    }
}

/// Sealed marker for an isolated context created by `AutomationRuntime`.
#[derive(Debug, Clone)]
struct HostAutomationAuthority;

#[cfg(feature = "runtime")]
fn attach_host_automation_authority(ctx: &JSContext) {
    ctx.set_state(HostAutomationAuthority);
}

pub(crate) fn host_automation_authority(ctx: &JSContext) -> Option<&HostAutomationAuthority> {
    ctx.get_state::<HostAutomationAuthority>()
}

/// The stable root returned by `lx.automation()`. It carries no authority by
/// itself; each selector checks the privilege for the capability it returns.
#[js_class(clone)]
pub(crate) struct JSAutomation {
    lxapp: Weak<LxApp>,
    /// True when created from trusted host authority: no owner lxapp exists.
    host_runtime: bool,
}

impl JSAutomation {
    pub(crate) fn new(lxapp: &Arc<LxApp>) -> Self {
        Self {
            lxapp: Arc::downgrade(lxapp),
            host_runtime: false,
        }
    }

    fn host_runtime() -> Self {
        Self {
            lxapp: Weak::new(),
            host_runtime: true,
        }
    }

    fn owner(&self) -> JSResult<Arc<LxApp>> {
        if self.host_runtime {
            return Err(auto_err("a host automation run has no calling lxapp"));
        }
        resolve::upgrade(&self.lxapp)
    }

    fn require_host(&self) -> JSResult<()> {
        if self.host_runtime {
            return Ok(());
        }
        let owner = self.owner()?;
        require_privilege(&owner, PRIV_HOST)
    }
}

#[js_class(rename = "Automation")]
impl JSAutomation {
    #[js_method(constructor)]
    fn _ctor() -> JSResult<()> {
        Err(HostError::new(rong::error::E_ILLEGAL_CONSTRUCTOR, "Use lx.automation()").into())
    }

    /// Select the calling/current lxapp, or an explicit app id. Explicit
    /// cross-app selection requires the host privilege.
    #[js_method]
    fn lxapp(&self, ctx: JSContext, appid: Optional<String>) -> JSResult<JSObject> {
        let app = match appid.0 {
            Some(appid) => {
                self.require_host()?;
                resolve::resolve_lxapp_by_id(&appid)?
            }
            None if self.host_runtime => resolve::resolve_lxapp_by_id("current")?,
            None => {
                let owner = self.owner()?;
                require_privilege(&owner, PRIV_AUTOMATION)?;
                owner
            }
        };
        Ok(Class::lookup::<info::JSLxAppDriver>(&ctx)?.instance(info::JSLxAppDriver::new(&app)))
    }

    /// Cross-lxapp lifecycle manager. Selection for page/nav/eval goes through
    /// [`JSAutomation::lxapp`].
    #[js_method(getter, enumerable)]
    fn lxapps(&self, ctx: JSContext) -> JSResult<JSObject> {
        self.require_host()?;
        Ok(Class::lookup::<host::JSLxAppManager>(&ctx)?.instance(host::JSLxAppManager::new()))
    }

    #[js_method(getter, enumerable)]
    fn browser(&self, ctx: JSContext) -> JSResult<JSObject> {
        self.require_host()?;
        Ok(Class::lookup::<host::JSBrowserDriver>(&ctx)?.instance(host::JSBrowserDriver::new()))
    }

    /// Session-less local-OS desktop automation (`lxdev desktop`). Beyond the
    /// app sandbox — it drives the whole OS — so it is available only to a
    /// trusted host automation runtime or an explicitly enabled dev host.
    #[js_method(getter, enumerable)]
    fn desktop(&self, ctx: JSContext) -> JSResult<JSObject> {
        self.require_host()?;
        if !(self.host_runtime || lxapp::is_dev_session() || lxapp::automation_auto_grant()) {
            return Err(auto_err(
                "desktop tier requires a trusted host automation runtime or dev host",
            ));
        }
        Ok(Class::lookup::<desktop::JSDesktopDriver>(&ctx)?
            .instance(desktop::JSDesktopDriver::new()))
    }

    #[js_method(getter, enumerable)]
    fn device(&self, ctx: JSContext) -> JSResult<JSObject> {
        self.require_host()?;
        Ok(Class::lookup::<host::JSDeviceDriver>(&ctx)?.instance(host::JSDeviceDriver::new()))
    }
}

/// `lx.automation()` has one shape in Logic and host automation contexts. The
/// factory itself grants nothing; selectors enforce their own privileges.
fn make_automation(ctx: JSContext, options: Optional<JSValue>) -> JSResult<JSObject> {
    if options.0.is_some() {
        return Err(auto_err("lx.automation() takes no options"));
    }

    if let Ok(lxapp) = LxApp::from_ctx(&ctx) {
        return Ok(Class::lookup::<JSAutomation>(&ctx)?.instance(JSAutomation::new(&lxapp)));
    }

    if host_automation_authority(&ctx).is_some() {
        return Ok(Class::lookup::<JSAutomation>(&ctx)?.instance(JSAutomation::host_runtime()));
    }

    Err(auto_err(
        "lx.automation() requires an lxapp logic context or trusted host automation context",
    ))
}

/// Register the automation classes and the `lx.automation` factory on a
/// context. Used by the lxapp logic extension below and by isolated host
/// automation programs, whose contexts are not lxapp logic contexts.
pub fn init_automation_context(ctx: &JSContext) -> JSResult<()> {
    ctx.register_hidden_class::<JSAutomation>()?;
    ctx.register_hidden_class::<page::JSPageDriver>()?;
    ctx.register_hidden_class::<input::JSPagePointer>()?;
    ctx.register_hidden_class::<input::JSPageKey>()?;
    ctx.register_hidden_class::<nav::JSNavDriver>()?;
    ctx.register_hidden_class::<info::JSLxAppDriver>()?;
    ctx.register_hidden_class::<host::JSLxAppManager>()?;
    ctx.register_hidden_class::<host::JSDeviceDriver>()?;
    ctx.register_hidden_class::<host::JSBrowserDriver>()?;
    ctx.register_hidden_class::<host::JSBrowserCookies>()?;
    ctx.register_hidden_class::<desktop::JSDesktopDriver>()?;
    ctx.register_hidden_class::<desktop::JSDesktopWindow>()?;
    ctx.register_hidden_class::<desktop::JSDesktopPointer>()?;
    ctx.register_hidden_class::<desktop::JSDesktopKey>()?;
    ctx.register_hidden_class::<desktop::JSDesktopClipboard>()?;
    ctx.register_hidden_class::<desktop::JSDesktopAx>()?;
    ctx.register_hidden_class::<desktop::JSDesktopWait>()?;
    ctx.register_hidden_class::<desktop::JSDesktopApp>()?;
    ctx.register_hidden_class::<desktop::JSDesktopProcess>()?;
    lx::register_js_api(ctx, "automation", JSFunc::new(ctx, make_automation)?)?;
    Ok(())
}

struct AutomationExtension;

impl lx::LxLogicExtension for AutomationExtension {
    fn init(&self, ctx: &JSContext) -> JSResult<()> {
        init_automation_context(ctx)
    }
}

/// Register the automation runtime. Call once at process bootstrap, before any
/// LxApp JS context is created (same timing as `register_logic_runtime`).
pub fn register_automation_runtime() {
    lx::register_logic_extension(Box::new(AutomationExtension));
}
