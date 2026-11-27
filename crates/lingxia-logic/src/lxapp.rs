use ::lxapp::{AppSessionClass, LxApp};
use rong::{IntoJSObject, JSContext, JSResult};

/// This lxapp's identity and the role it runs in.
#[derive(Debug, Clone, IntoJSObject)]
struct AppInfo {
    /// This lxapp's id.
    #[js_name = "appId"]
    app_id: String,
    /// Its display name.
    name: String,
    /// Its version, from `lxapp.json`.
    version: String,
    /// The channel it was installed from.
    #[js_name = "channel"]
    #[ts_type = "LxAppReleaseType"]
    release_type: String,
    /// What this lxapp may call. `control`: the product's own lxapp, admitted to
    /// Control-only members. `control_surface`: a host-bundled settings lxapp the
    /// Control app opens, admitted only to its own members. `guest`: any other.
    #[ts_type = "'control' | 'control_surface' | 'guest'"]
    role: &'static str,
}

/// Identify the running lxapp: its id, display name, version, channel, and role.
fn get_app_info(ctx: JSContext) -> JSResult<AppInfo> {
    let lxapp = LxApp::from_ctx(&ctx)?;
    let info = lxapp.get_lxapp_info();
    Ok(AppInfo {
        app_id: lxapp.appid.clone(),
        name: info.app_name,
        version: info.version,
        release_type: info.release_type,
        role: match lxapp.app_session_class() {
            AppSessionClass::ControlApp => "control",
            AppSessionClass::ControlSurface => "control_surface",
            AppSessionClass::StandardApp => "guest",
        },
    })
}

pub(crate) fn init(ctx: &JSContext) -> JSResult<()> {
    register_api(ctx)?;
    register_app_api(ctx)?;
    // Framework-internal, for `@lingxia/logic`'s `packageText`; not part of `lx`.
    ctx.global()
        .set("__lingxiaAppText", rong::JSFunc::new(ctx, app_text)?)?;
    Ok(())
}

/// The lxapp's override of a package word in `language`'s catalogue, or null.
fn app_text(
    ctx: JSContext,
    key: String,
    language: String,
    args: String,
) -> JSResult<Option<String>> {
    let lxapp = LxApp::from_ctx(&ctx)?;
    let args = serde_json::from_str(&args).map_err(|error| {
        crate::i18n::js_invalid_parameter_error(format!("packageText args: {error}"))
    })?;
    Ok(lxapp.message_in(&key, &language, &args))
}

rong::js_api! {
    fn register_api(ctx) {
        namespace Lx = ctx.global().get::<_, rong::JSObject>("lx")?;
        const app: "LxAppApi" = crate::namespace::app(ctx)?;
    }
}

/// The lxapp's message `key` from its `i18n/` catalogues in its language, with ICU `args`;
/// tags keep their text. Read it when the text is shown: a later language switch does not change it.
fn message(
    ctx: JSContext,
    key: String,
    args: rong::function::Optional<rong::JSObject>,
) -> JSResult<String> {
    let lxapp = LxApp::from_ctx(&ctx)?;
    let args = match args.0 {
        None => serde_json::Value::Null,
        Some(object) => {
            let json = object.to_json_string().map_err(|error| {
                crate::i18n::js_invalid_parameter_error(format!(
                    "lx.app.t args must be a plain object: {error}"
                ))
            })?;
            serde_json::from_str(&json).map_err(|error| {
                crate::i18n::js_invalid_parameter_error(format!("lx.app.t args: {error}"))
            })?
        }
    };
    lxapp
        .message(&key, &args)
        .map_err(|error| crate::i18n::host_error_of(error).into())
}

rong::js_api! {
    fn register_app_api(ctx) {
        namespace LxAppApi = crate::namespace::app(ctx)?;
        const info: "Readonly<AppInfo>" = get_app_info(ctx.clone())?;
        fn t(
            ts_params = "...message: LxAppMessageCall"
        ) = message;
        fn errorText(
            ts_params = "error: unknown, options?: ErrorTextOptions",
            ts_return = "string | null"
        ) = error_text;
    }
}

/// Codes a person can act on get their own word; the rest read as a plain failure. The same table
/// as the View's `errorText`.
fn error_word(code: &str) -> Option<&'static str> {
    Some(match code {
        "E_NETWORK" | "VIDEO_NETWORK" | "WEBVIEW_NETWORK" => "network",
        "E_TIMEOUT" => "timeout",
        "E_PERMISSION_DENIED" | "VIDEO_NOT_GRANTED" | "WEBVIEW_NOT_GRANTED" => "permission",
        "E_NOT_SUPPORTED"
        | "E_CAPABILITY_MISSING"
        | "VIDEO_UNSUPPORTED"
        | "VIDEO_NO_PLAYER"
        | "VIDEO_DECODERS"
        | "WEBVIEW_UNSUPPORTED" => "unsupported",
        "E_NOT_FOUND" | "VIDEO_NOT_FOUND" => "notFound",
        "E_QUOTA" => "limit",
        _ => return None,
    })
}

#[derive(rong::FromJSObject, Default)]
#[ts_skip]
pub(crate) struct ErrorTextOptions {
    pub(crate) fallback: Option<String>,
}

/// A thrown `Error`, or an object carrying a string `code` such as an event's failure.
pub(crate) fn is_failure(value: &rong::JSValue) -> bool {
    value.is_error()
        || value
            .clone()
            .into_object()
            .is_some_and(|object| object.get_opt::<_, String>("code").ok().flatten().is_some())
}

/// A failure as text for the person, by its `code`, in the lxapp's language: what a View's
/// `errorText` says for it. `fallback` when no code-specific word applies; `null` for a cancel.
/// The error's `message` is English for the log and is never returned.
fn error_text(
    ctx: JSContext,
    error: rong::JSValue,
    options: rong::function::Optional<ErrorTextOptions>,
) -> JSResult<Option<String>> {
    error_words(&ctx, error, options.0.and_then(|options| options.fallback))
}

/// `errorText`'s words for `error`; `None` for a cancel.
pub(crate) fn error_words(
    ctx: &JSContext,
    error: rong::JSValue,
    fallback: Option<String>,
) -> JSResult<Option<String>> {
    let code = error
        .into_object()
        .and_then(|object| object.get_opt::<_, String>("code").ok().flatten());
    if code.as_deref() == Some("E_ABORT") {
        return Ok(None);
    }
    let specific = code.as_deref().and_then(error_word);
    if specific.is_none()
        && let Some(fallback) = fallback
    {
        return Ok(Some(fallback));
    }
    let word = specific.unwrap_or("failed");
    let lxapp = LxApp::from_ctx(ctx)?;
    let language = lxapp.content_language(&::lxapp::display_language());
    let key = format!("@view.error.{word}");
    if let Some(own) = lxapp.message_in(&key, &language, &serde_json::Value::Null) {
        return Ok(Some(own));
    }
    Ok(Some(
        crate::view_error_word(word, &language)
            .or_else(|| crate::view_error_word(word, "en"))
            .unwrap_or_default()
            .to_string(),
    ))
}

#[cfg(test)]
mod tests {
    #[test]
    fn a_failure_is_an_error_or_carries_a_code() {
        use rong::{JSEngine, Source};
        let runtime = <rong::RongJS as JSEngine>::runtime();
        let ctx = runtime.context();
        let failure = |js: &str| super::is_failure(&ctx.eval(Source::from_bytes(js)).unwrap());
        assert!(failure("new Error('x')"));
        assert!(failure("({ code: 'E_NETWORK' })"));
        assert!(!failure("({ title: 'Saved' })"));
        assert!(!failure("'Saved'"));
        assert!(!failure("({ code: 1 })"));
    }

    #[test]
    fn error_words_match_the_view() {
        let view = include_str!("../../../packages/lingxia-view/src/runtime/words.js");
        for (code, word) in [
            ("E_NETWORK", "network"),
            ("WEBVIEW_NOT_GRANTED", "permission"),
            ("E_CAPABILITY_MISSING", "unsupported"),
            ("VIDEO_NOT_FOUND", "notFound"),
            ("E_QUOTA", "limit"),
        ] {
            assert_eq!(super::error_word(code), Some(word));
            assert!(
                view.contains(&format!("{code}: '{word}'")),
                "{code} differs from the View"
            );
        }
        assert_eq!(super::error_word("E_INTERNAL"), None);
        assert!(crate::view_error_word("network", "zh-Hans").is_some());
    }
}
