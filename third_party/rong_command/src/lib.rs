//! Command execution APIs attached to `globalThis.Rong`.

mod child_process;
mod io;
mod shell;
mod sync_process;

use rong::{
    HostError, IntoJSValue, JSArray, JSContext, JSContextService, JSObject, JSResult, JSValue,
};
use std::env;
use std::sync::Arc;
use std::time::Duration;

/// Native authority checked at each process operation and while children run.
///
/// The JavaScript namespace cannot install or replace this object. Embedders
/// should bind it to the exact live session that owns process execution.
trait ProcessAuthority: Send + Sync + 'static {
    fn authorize(&self) -> Result<(), String>;
}

#[derive(Clone)]
struct ProcessAuthorityService(Arc<dyn ProcessAuthority>);

impl JSContextService for ProcessAuthorityService {}

pub(crate) fn process_authority(ctx: &JSContext) -> Option<Arc<dyn ProcessAuthority>> {
    ctx.get_service::<ProcessAuthorityService>()
        .map(|service| Arc::clone(&service.0))
}

pub(crate) fn authorize_process(ctx: &JSContext) -> JSResult<()> {
    authorize_installed_process(process_authority(ctx))
}

fn authorize_installed_process(authority: Option<Arc<dyn ProcessAuthority>>) -> JSResult<()> {
    let authority = authority.ok_or_else(|| {
        HostError::new(
            rong::error::E_PERMISSION_DENIED,
            "process execution requires a sealed live session authority",
        )
    })?;
    authorize_process_with(authority.as_ref())
}

pub(crate) fn authorize_process_with(authority: &dyn ProcessAuthority) -> JSResult<()> {
    authority
        .authorize()
        .map_err(|message| HostError::new(rong::error::E_PERMISSION_DENIED, message).into())
}

pub(crate) async fn wait_for_process_revocation(authority: Arc<dyn ProcessAuthority>) {
    loop {
        if authority.authorize().is_err() {
            return;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
}

fn create_env_object(ctx: &JSContext) -> JSResult<JSObject> {
    let env_obj = JSObject::new(ctx);
    for (key, value) in env::vars() {
        env_obj.set(key.as_str(), value)?;
    }
    Ok(env_obj)
}

fn create_string_array(
    ctx: &JSContext,
    values: impl IntoIterator<Item = String>,
) -> JSResult<JSValue> {
    let array = JSArray::new(ctx)?;
    for value in values {
        array.push(value)?;
    }
    Ok(array.into_js_value(ctx))
}

fn init(ctx: &JSContext) -> JSResult<()> {
    let rong = ctx.host_namespace();
    rong.set("env", create_env_object(ctx)?)?;
    rong.set("argv", create_string_array(ctx, env::args())?)?;
    rong.set("args", create_string_array(ctx, env::args().skip(2))?)?;

    rong_buffer::init(ctx)?;
    rong_encoding::init(ctx)?;
    rong_abort::init(ctx)?;
    rong_stream::init(ctx)?;
    io::init(ctx)?;
    child_process::init(ctx)?;
    sync_process::init(ctx)?;
    shell::init(ctx)?;
    Ok(())
}

/// Initialize the command namespace with a sealed, per-context authority.
///
/// A second installation is rejected so later native modules cannot silently
/// replace the session authority selected by the embedder.
fn init_with_authority(ctx: &JSContext, authority: Arc<dyn ProcessAuthority>) -> JSResult<()> {
    if ctx.get_service::<ProcessAuthorityService>().is_some() {
        return Err(HostError::new(
            rong::error::E_ALREADY_EXISTS,
            "process authority is already sealed for this context",
        )
        .into());
    }
    authority
        .authorize()
        .map_err(|message| HostError::new(rong::error::E_PERMISSION_DENIED, message))?;
    ctx.set_service(ProcessAuthorityService(authority));
    init(ctx)
}

struct CallbackAuthority(Arc<dyn Fn() -> Result<(), String> + Send + Sync + 'static>);

impl ProcessAuthority for CallbackAuthority {
    fn authorize(&self) -> Result<(), String> {
        (self.0)()
    }
}

/// Private Rust ABI used only by lingxia-lxapp's session bootstrap. There is
/// no safe downstream trait or installer that can substitute an always-allow
/// authority.
#[unsafe(export_name = "lingxia_rong_command_init_with_authority_v1")]
pub(crate) extern "Rust" fn init_from_lxapp(
    ctx: &JSContext,
    authorize: Arc<dyn Fn() -> Result<(), String> + Send + Sync + 'static>,
) -> JSResult<()> {
    init_with_authority(ctx, Arc::new(CallbackAuthority(authorize)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicBool, Ordering};

    struct RevocableAuthority(AtomicBool);

    impl ProcessAuthority for RevocableAuthority {
        fn authorize(&self) -> Result<(), String> {
            self.0
                .load(Ordering::SeqCst)
                .then_some(())
                .ok_or_else(|| "revoked".to_string())
        }
    }

    #[tokio::test]
    async fn revocation_waiter_observes_authority_change() {
        let authority = Arc::new(RevocableAuthority(AtomicBool::new(true)));
        let task = tokio::spawn(wait_for_process_revocation(authority.clone()));
        authority.0.store(false, Ordering::SeqCst);
        tokio::time::timeout(Duration::from_secs(1), task)
            .await
            .expect("revocation waiter must finish")
            .expect("waiter task");
    }

    #[test]
    fn allow_and_stale_authorities_are_checked_on_every_operation() {
        let authority = RevocableAuthority(AtomicBool::new(true));
        assert!(authorize_process_with(&authority).is_ok());
        authority.0.store(false, Ordering::SeqCst);
        let error = authorize_process_with(&authority).expect_err("stale authority must fail");
        assert!(error.to_string().contains("revoked"));
    }

    #[test]
    fn missing_authority_fails_closed() {
        let error = authorize_installed_process(None).expect_err("missing authority must fail");
        assert!(error.to_string().contains("sealed live session authority"));
    }

    #[test]
    fn package_exposes_no_safe_authority_installer() {
        let source = include_str!("lib.rs");
        assert!(!source.contains(concat!("pub trait ", "ProcessAuthority")));
        assert!(!source.contains(concat!("pub fn ", "init(ctx")));
        assert!(!source.contains(concat!("pub fn ", "init_with_authority")));
    }
}
