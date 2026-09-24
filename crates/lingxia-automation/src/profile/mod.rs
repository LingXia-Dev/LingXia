//! `ProfileDriver` — checkpoint and roll back the isolated data profile of a
//! host automation run.
//!
//! Only a run started with an isolated profile (`lxdev test --isolate`)
//! carries a profile scope, and every call also checks that the app still
//! runs on that very profile. Rollback therefore cannot reach an app's real
//! data. Each call closes the app, copies or swaps the closed profile, and
//! reopens it at its initial page, so a driver bound to the previous
//! instance must be selected again afterwards.

use crate::error::{E_PROFILE_NOT_ISOLATED, coded};
use crate::resolve::upgrade_authorized;
use lxapp::LxApp;
use lxapp::data_profile::{self, KeepKeys, RunProfile};
use rong::{
    FromJSObject, HostError, IntoJSObject, JSContext, JSResult, function::Optional, js_class,
    js_method,
};
use std::sync::{Arc, Weak};

/// Marks a host automation context with the isolated profile its run owns.
#[derive(Clone)]
pub(crate) struct ProfileRunScope {
    appid: String,
    profile: RunProfile,
    active: Arc<dyn Fn() -> bool + Send + Sync>,
}

pub(crate) fn attach_run_scope(
    ctx: &JSContext,
    appid: String,
    profile: RunProfile,
    active: impl Fn() -> bool + Send + Sync + 'static,
) {
    ctx.set_state(ProfileRunScope {
        appid,
        profile,
        active: Arc::new(active),
    });
}

fn not_isolated(message: impl Into<String>) -> rong::RongJSError {
    coded(E_PROFILE_NOT_ISOLATED, message).into()
}

/// The scope this call may act in: an active isolated run whose profile the
/// selected app currently runs on.
fn scope_for(ctx: &JSContext, app: &LxApp) -> JSResult<ProfileRunScope> {
    let scope = ctx.get_state::<ProfileRunScope>().cloned().ok_or_else(|| {
        not_isolated("profile rollback needs an isolated run (lxdev test --isolate)")
    })?;
    if !(scope.active)() {
        return Err(not_isolated("this automation run has ended"));
    }
    if scope.appid != app.appid {
        return Err(not_isolated(format!(
            "lxapp {} is not isolated in this run; only {} is",
            app.appid, scope.appid
        )));
    }
    if !scope.profile.is_active_for(&scope.appid) {
        return Err(not_isolated(format!(
            "lxapp {} no longer runs on this run's profile",
            scope.appid
        )));
    }
    Ok(scope)
}

#[derive(FromJSObject)]
struct RestoreOptions {
    /// `lx.getStorage()` key globs whose current values survive the rollback.
    keep: Option<Vec<String>>,
}

#[derive(Debug, Clone, IntoJSObject)]
struct RestoreResult {
    /// Kept keys whose current values were carried into the restored data.
    kept: Vec<String>,
}

#[js_class(clone)]
pub(crate) struct JSProfileDriver {
    lxapp: Weak<LxApp>,
}

impl JSProfileDriver {
    /// Authorization is checked per call, so reading `.profile` never throws.
    pub(crate) fn new(lxapp: Weak<LxApp>) -> Self {
        Self { lxapp }
    }
}

#[js_class(rename = "ProfileDriver")]
impl JSProfileDriver {
    #[js_method(constructor)]
    fn _ctor() -> JSResult<()> {
        Err(HostError::new(
            rong::error::E_ILLEGAL_CONSTRUCTOR,
            "Use lx.automation().lxapp().profile",
        )
        .into())
    }

    /// Close the app, copy its profile into a new checkpoint, reopen it.
    /// Resolves the checkpoint id.
    #[js_method]
    async fn checkpoint(&self, ctx: JSContext) -> JSResult<String> {
        let app = upgrade_authorized(&ctx, &self.lxapp)?;
        let scope = scope_for(&ctx, &app)?;
        drop(app);
        data_profile::checkpoint(&scope.appid, &scope.profile)
            .await
            .map_err(|err| crate::auto_err(err.to_string()))
    }

    /// Close the app, replace its profile with checkpoint `id`, reopen it.
    /// With `keep`, storage keys matching those globs keep their current
    /// values (or stay absent) across the rollback; the app is closed for
    /// the whole merge, so it never sees the checkpoint's values of them.
    #[js_method]
    async fn restore(
        &self,
        ctx: JSContext,
        id: String,
        // `Option` inside: an explicit `undefined` or `null` means no options.
        options: Optional<Option<RestoreOptions>>,
    ) -> JSResult<RestoreResult> {
        let app = upgrade_authorized(&ctx, &self.lxapp)?;
        let scope = scope_for(&ctx, &app)?;
        drop(app);
        let keep = KeepKeys::new(
            options
                .0
                .flatten()
                .and_then(|options| options.keep)
                .unwrap_or_default(),
        )
        .map_err(|err| crate::auto_err(err.to_string()))?;
        let kept = data_profile::restore_keeping(&scope.appid, &scope.profile, &id, &keep)
            .await
            .map_err(|err| crate::auto_err(err.to_string()))?;
        Ok(RestoreResult { kept })
    }

    /// Discard checkpoint `id`. The app keeps running.
    #[js_method]
    async fn drop(&self, ctx: JSContext, id: String) -> JSResult<()> {
        let app = upgrade_authorized(&ctx, &self.lxapp)?;
        let scope = scope_for(&ctx, &app)?;
        scope
            .profile
            .drop_checkpoint(&id)
            .map_err(|err| crate::auto_err(err.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::RestoreOptions;
    use rong::{JSEngine, JSFunc, RongJS, Source, function::Optional};

    /// `restore(id)`, `restore(id, undefined)` and `restore(id, null)` all
    /// mean "no options".
    #[test]
    fn restore_options_accept_undefined_and_null() {
        let runtime = RongJS::runtime();
        let ctx = runtime.context();
        let restore = JSFunc::new(
            &ctx,
            |_id: String, options: Optional<Option<RestoreOptions>>| -> String {
                match options.0.flatten() {
                    None => "none".into(),
                    Some(options) => options.keep.unwrap_or_default().join(","),
                }
            },
        )
        .unwrap();
        ctx.global().set("restore", restore).unwrap();
        let out: String = ctx
            .eval(Source::from_bytes(
                "JSON.stringify([restore('p'), restore('p', undefined), restore('p', null), \
                 restore('p', { keep: ['a', 'b*'] })])",
            ))
            .unwrap();
        assert_eq!(out, r#"["none","none","none","a,b*"]"#);
    }
}
