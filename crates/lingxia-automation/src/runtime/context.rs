//! Fresh-context assembly for one host automation run.
//!
//! The context deliberately exposes only `lx.automation()`, the
//! `__LINGXIA_AUTOMATION_HOST__`, `console`, timers, and the Web APIs `fetch`
//! needs — no appid-scoped `lx.*`, filesystem, environment, process, or
//! module loading.

use super::protocol::*;
use super::run::RunShared;
use base64::Engine as _;
use base64::engine::general_purpose::STANDARD as BASE64;
use lingxia_log::{LogBuilder, LogLevel, LogTag};
use rong::function::Rest;
use rong::{
    Class, FromJSObject, HostError, JSContext, JSFunc, JSObject, JSResult, JSValue, RongJSError,
    js_class, js_method,
};
use std::collections::HashMap;
use std::sync::{Arc, Weak};

/// Modules the automation profile registers: timers plus the surface `fetch` pulls
/// in. `console` is deliberately absent — the run needs its own sink.
const AUTOMATION_MODULES: [&str; 10] = [
    "timer",
    "event",
    "exception",
    "abort",
    "encoding",
    "url",
    "buffer",
    "stream",
    "http",
    "compression",
];

pub(crate) fn init_automation_context(
    ctx: &JSContext,
    shared: &Arc<RunShared>,
    args: &HashMap<String, String>,
    control: &HashMap<String, String>,
) -> JSResult<()> {
    rong_modules::init(ctx, AUTOMATION_MODULES)?;

    // A bare `lx` namespace carrying only the automation factory.
    ctx.global().set("lx", JSObject::new(ctx))?;
    init_automation(ctx, shared)?;
    let run = Arc::downgrade(shared);
    crate::network::attach_run_scope(ctx, shared.run_id.clone(), move || {
        run.upgrade().is_some_and(|run| !run.state().is_terminal())
    });
    let run = Arc::downgrade(shared);
    crate::clock::attach_run_scope(ctx, shared.run_id.clone(), move || {
        run.upgrade().is_some_and(|run| !run.state().is_terminal())
    });
    if let Some(profile) = shared.profile() {
        let run = Arc::downgrade(shared);
        crate::profile::attach_run_scope(
            ctx,
            profile.appid.clone(),
            profile.profile.clone(),
            move || run.upgrade().is_some_and(|run| !run.state().is_terminal()),
        );
    }

    ctx.register_hidden_class::<AutomationConsole>()?;
    ctx.global().set(
        "console",
        Class::lookup::<AutomationConsole>(ctx)?.instance(AutomationConsole {
            shared: Arc::downgrade(shared),
        }),
    )?;
    ctx.global().set(
        "__LINGXIA_AUTOMATION_HOST__",
        make_host(ctx, shared, args, control)?,
    )?;
    Ok(())
}

#[cfg(not(test))]
fn init_automation(ctx: &JSContext, _shared: &RunShared) -> JSResult<()> {
    crate::init_automation_context(ctx)?;
    crate::attach_host_automation_authority(ctx);
    Ok(())
}

// Unit-test binaries do not link a native LingXia host's Swift bridge. The
// runtime/report integration test does not drive automation; Runner smoke tests
// cover the real host authority and driver registration path.
#[cfg(test)]
fn init_automation(ctx: &JSContext, _shared: &RunShared) -> JSResult<()> {
    crate::attach_host_automation_authority(ctx);
    Ok(())
}

/// Map a script failure to the structured result error, preserving the
/// generated stack for client-side source-map remapping.
pub(crate) fn map_js_error(ctx: &JSContext, error: RongJSError) -> AutomationRunError {
    if let Some(thrown) = error.thrown_value(ctx) {
        if thrown.is_string() {
            if let Ok(message) = thrown.into_value().try_into() {
                return AutomationRunError {
                    name: "Error".to_string(),
                    message,
                    stack: None,
                    causes: Vec::new(),
                };
            }
        } else if let Some(object) = thrown.into_object() {
            let name = object
                .get::<_, String>("name")
                .unwrap_or_else(|_| "Error".to_string());
            let message = object.get::<_, String>("message").unwrap_or_default();
            let stack = object.get::<_, String>("stack").ok();
            return AutomationRunError {
                name,
                message,
                stack,
                causes: Vec::new(),
            };
        }
    }
    AutomationRunError {
        name: "Error".to_string(),
        message: error.to_string(),
        stack: None,
        causes: Vec::new(),
    }
}

fn format_console_args(args: Rest<JSValue>) -> String {
    let mut parts = Vec::with_capacity(args.0.len());
    for value in args.0 {
        parts.push(rong_console::inspect_value(value));
    }
    parts.join(" ")
}

/// Everything the run's JS context can reach holds the run weakly. The
/// context outlives the run until the engine collects it, and a strong
/// reference would keep the run's report, events and attachments with it;
/// the manager's retention decides how long a run lives.
#[js_class(clone)]
struct AutomationConsole {
    shared: Weak<RunShared>,
}

impl AutomationConsole {
    fn emit(&self, level: &str, log_level: LogLevel, args: Rest<JSValue>) {
        let Some(shared) = self.shared.upgrade() else {
            return;
        };
        let message = format_console_args(args);
        // Mirror into the session dev log for later diagnosis; live output
        // flows to the client through poll events.
        LogBuilder::new(LogTag::Automation, &message)
            .with_path(shared.run_id.clone())
            .with_level(log_level);
        shared.push_console(level, message);
    }
}

#[js_class(rename = "Console")]
impl AutomationConsole {
    #[js_method(constructor)]
    fn _ctor() -> JSResult<()> {
        Err(HostError::new(rong::error::E_ILLEGAL_CONSTRUCTOR, "console is provided").into())
    }

    #[js_method]
    fn log(&self, args: Rest<JSValue>) {
        self.emit("info", LogLevel::Info, args);
    }

    #[js_method]
    fn info(&self, args: Rest<JSValue>) {
        self.emit("info", LogLevel::Info, args);
    }

    #[js_method]
    fn warn(&self, args: Rest<JSValue>) {
        self.emit("warn", LogLevel::Warn, args);
    }

    #[js_method]
    fn error(&self, args: Rest<JSValue>) {
        self.emit("error", LogLevel::Error, args);
    }

    #[js_method]
    fn debug(&self, args: Rest<JSValue>) {
        self.emit("debug", LogLevel::Debug, args);
    }

    #[js_method]
    fn trace(&self, args: Rest<JSValue>) {
        self.emit("debug", LogLevel::Verbose, args);
    }
}

#[derive(FromJSObject)]
struct AttachOptions {
    #[js_name = "mimeType"]
    mime_type: String,
    base64: String,
}

fn make_host(
    ctx: &JSContext,
    shared: &Arc<RunShared>,
    args: &HashMap<String, String>,
    control: &HashMap<String, String>,
) -> JSResult<JSObject> {
    let host = JSObject::new(ctx);
    let string_map = |map: &HashMap<String, String>| -> JSResult<JSObject> {
        let object = JSObject::new(ctx);
        for (key, value) in map {
            object.set(key.as_str(), value.as_str())?;
        }
        Ok(object)
    };
    host.set("args", string_map(args)?)?;
    // Absent when the caller sent none: a framework then reads its controls
    // from `args`, as callers that predate the split still send them.
    if !control.is_empty() {
        host.set("control", string_map(control)?)?;
    }

    let attach_shared = Arc::downgrade(shared);
    host.set(
        "attach",
        JSFunc::new(ctx, move |name: String, options: AttachOptions| {
            let run = live_run(&attach_shared)?;
            attach(&run, name, options)
        })?,
    )?;

    let event_shared = Arc::downgrade(shared);
    host.set(
        "emit",
        JSFunc::new(ctx, move |event: JSObject| {
            let run = live_run(&event_shared)?;
            emit(&run, event)
        })?,
    )?;

    let logs_shared = Arc::downgrade(shared);
    host.set(
        "logs",
        JSFunc::new(ctx, move || {
            logs_shared
                .upgrade()
                .map(|run| run.log_ring_text())
                .unwrap_or_default()
        })?,
    )?;
    crate::network::attach_host_functions(
        ctx,
        &host,
        &shared.run_id,
        secret_values(args, control),
    )?;
    Ok(host)
}

/// The run a host function belongs to, while it is still kept.
fn live_run(run: &Weak<RunShared>) -> JSResult<Arc<RunShared>> {
    run.upgrade()
        .ok_or_else(|| HostError::new("E_AUTOMATION_ENDED", "the automation run has ended").into())
}

/// Values of the `--secret-arg` keys lxdev declared in `control.secretArgs`.
fn secret_values(args: &HashMap<String, String>, control: &HashMap<String, String>) -> Vec<String> {
    control
        .get("secretArgs")
        .and_then(|keys| serde_json::from_str::<Vec<String>>(keys).ok())
        .unwrap_or_default()
        .iter()
        .filter_map(|key| args.get(key))
        .filter(|value| value.len() >= 4)
        .cloned()
        .collect()
}

fn attach(shared: &RunShared, name: String, options: AttachOptions) -> JSResult<()> {
    const MAX_BASE64_BYTES: usize = MAX_ATTACHMENT_BYTES.div_ceil(3) * 4;
    if options.base64.len() > MAX_BASE64_BYTES {
        return Err(HostError::new(
            "E_AUTOMATION_ATTACH",
            format!("attachment exceeds the {MAX_ATTACHMENT_BYTES}-byte limit"),
        )
        .into());
    }
    let decoded = BASE64
        .decode(options.base64.as_bytes())
        .map_err(|err| HostError::new("E_AUTOMATION_ATTACH", format!("invalid base64: {err}")))?;
    shared
        .push_artifact(&name, &options.mime_type, options.base64, decoded.len())
        .map_err(|err| HostError::new("E_AUTOMATION_ATTACH", err).into())
}

fn emit(shared: &RunShared, event: JSObject) -> JSResult<()> {
    let encoded = event.to_json_string().map_err(|err| {
        HostError::new(
            "E_AUTOMATION_EVENT",
            format!("event must be JSON-compatible: {err}"),
        )
    })?;
    if encoded.len() > MAX_RETAINED_EVENT_BYTES {
        return Err(HostError::new(
            "E_AUTOMATION_EVENT",
            format!("event exceeds the {MAX_RETAINED_EVENT_BYTES}-byte limit"),
        )
        .into());
    }
    let value = serde_json::from_str(&encoded).map_err(|err| {
        HostError::new(
            "E_AUTOMATION_EVENT",
            format!("failed to decode event: {err}"),
        )
    })?;
    shared.push_event(value);
    Ok(())
}
