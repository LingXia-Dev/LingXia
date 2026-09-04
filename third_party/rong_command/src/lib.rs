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
pub trait ProcessAuthority: Send + Sync + 'static {
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
    if let Some(authority) = process_authority(ctx) {
        authorize_process_with(authority.as_ref())?;
    }
    Ok(())
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

pub fn init(ctx: &JSContext) -> JSResult<()> {
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
pub fn init_with_authority(ctx: &JSContext, authority: Arc<dyn ProcessAuthority>) -> JSResult<()> {
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
}
