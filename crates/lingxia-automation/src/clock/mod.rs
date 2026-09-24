//! `ClockDriver` — a test clock for an lxapp's Logic context.
//!
//! Every Logic context of an automation-enabled host evaluates a dormant
//! controller (`fake_clock.js`). A host automation run installs it through
//! the driver: from then on the app's Logic `Date`, timers and
//! `performance.now` read fake time, and timers fire only when the test
//! advances the clock. The View WebView, native work and real network I/O
//! keep real time.
//!
//! Each install holds a lease owned by its run. The run's finalization drops
//! the lease, and the Logic side checks it once a second on a real timer and
//! uninstalls itself when it is gone, so a run that ends any way at all
//! (cancelled, timed out, disconnected) cannot leave the app on fake time. An
//! app that reopens gets a new Logic context, which starts on real time.

use crate::auto_err;
use crate::error::{E_CLOCK_INSTALLED, E_CLOCK_NOT_INSTALLED, coded};
use crate::resolve::upgrade_authorized;
use lxapp::LxApp;
use rong::{
    FromJSObject, HostError, IntoJSObject, JSContext, JSFunc, JSResult, JSValue, RongExecutor,
    Source, function::Optional, js_class, js_method,
};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock, Weak};
use std::time::Duration;

const FAKE_CLOCK: &str = include_str!("fake_clock.js");
const CONTROLLER: &str = "globalThis[Symbol.for('lingxia.automation.clock')]";

/// Upper bound on one `tick`: 400 days.
pub(crate) const MAX_TICK_MS: f64 = 400.0 * 24.0 * 3_600_000.0;
/// Default and upper bound of `runAll({ maxTimers })`.
pub(crate) const DEFAULT_RUN_ALL_TIMERS: u32 = 1_000;
pub(crate) const MAX_RUN_ALL_TIMERS: u32 = 100_000;
/// How long one clock call may run in Logic, settling included.
const CALL_TIMEOUT: Duration = Duration::from_secs(5);
const ADVANCE_TIMEOUT: Duration = Duration::from_secs(60);

// ------------------------------- leases -------------------------------

#[derive(Debug, Clone, PartialEq, Eq)]
struct Lease {
    run_id: String,
    appid: String,
}

fn leases() -> &'static Mutex<HashMap<u64, Lease>> {
    static LEASES: OnceLock<Mutex<HashMap<u64, Lease>>> = OnceLock::new();
    LEASES.get_or_init(|| Mutex::new(HashMap::new()))
}

fn with_leases<T>(f: impl FnOnce(&mut HashMap<u64, Lease>) -> T) -> T {
    f(&mut leases().lock().unwrap_or_else(|err| err.into_inner()))
}

fn next_token() -> u64 {
    static NEXT: AtomicU64 = AtomicU64::new(1);
    NEXT.fetch_add(1, Ordering::Relaxed)
}

fn is_leased(token: u64) -> bool {
    with_leases(|leases| leases.contains_key(&token))
}

/// Drop every lease a run holds. Its clocks uninstall themselves within a
/// second. Called on every terminal transition of the run.
pub(crate) fn clear_run(run_id: &str) {
    with_leases(|leases| leases.retain(|_, lease| lease.run_id != run_id));
}

/// Keep only `keep` among the run's leases for `appid`.
fn release_app(run_id: &str, appid: &str, keep: Option<u64>) {
    with_leases(|leases| {
        leases.retain(|token, lease| {
            Some(*token) == keep || lease.run_id != run_id || lease.appid != appid
        })
    });
}

/// The run's lease for `appid`, newest first.
fn lease_for(run_id: &str, appid: &str) -> Option<u64> {
    with_leases(|leases| {
        leases
            .iter()
            .filter(|(_, lease)| lease.run_id == run_id && lease.appid == appid)
            .map(|(token, _)| *token)
            .max()
    })
}

// ------------------------------ run scope ------------------------------

/// Marks a host automation context with the run that owns its clocks.
#[derive(Clone)]
pub(crate) struct ClockRunScope {
    run_id: String,
    active: Arc<dyn Fn() -> bool + Send + Sync>,
}

pub(crate) fn attach_run_scope(
    ctx: &JSContext,
    run_id: String,
    active: impl Fn() -> bool + Send + Sync + 'static,
) {
    ctx.set_state(ClockRunScope {
        run_id,
        active: Arc::new(active),
    });
}

fn run_scope(ctx: &JSContext) -> JSResult<ClockRunScope> {
    let scope = ctx.get_state::<ClockRunScope>().cloned().ok_or_else(|| {
        auto_err("the test clock is available only inside a host automation run (lxdev test)")
    })?;
    if !(scope.active)() {
        return Err(auto_err("this automation run has ended"));
    }
    Ok(scope)
}

// ------------------------------ Logic side ------------------------------

/// One real event-loop turn for the controller to settle on: a round trip
/// through the host executor, which is where native work a timer callback
/// started (fetch, storage) completes.
///
/// Not a real `setTimeout(fn, 0)`: Rong's timer registry can drop a
/// one-shot timer whose tick is still queued when its spawn task finishes,
/// and a zero delay makes that race likely enough to stall a settle loop.
async fn host_turn() -> JSResult<()> {
    RongExecutor::global()
        .spawn(async {})
        .await
        .map_err(|err| auto_err(format!("test clock turn failed: {err}")))
}

/// Evaluate the dormant controller in a Logic context. Its internal timers
/// use the natives captured here, before any install.
pub(crate) fn install_logic_clock(ctx: &JSContext) -> JSResult<()> {
    let global = ctx.global();
    let set_timeout = global.get::<_, JSValue>("setTimeout")?;
    let set_interval = global.get::<_, JSValue>("setInterval")?;
    let clear_interval = global.get::<_, JSValue>("clearInterval")?;
    if !(set_timeout.is_function() && set_interval.is_function() && clear_interval.is_function()) {
        return Ok(());
    }
    let leased = JSFunc::new(ctx, |token: f64| -> bool { is_leased(token as u64) })?;
    let turn = JSFunc::new(ctx, host_turn)?;
    let installer = ctx.eval::<JSFunc>(Source::from_bytes(FAKE_CLOCK))?;
    installer.call::<_, ()>(
        None,
        (leased, turn, set_timeout, set_interval, clear_interval),
    )
}

// ----------------------------- driver side -----------------------------

/// A time for `install({ now })` and `setSystemTime`: epoch milliseconds or
/// a string `Date.parse` reads. A host `Date` arrives as its epoch value.
fn time_arg(value: JSValue) -> JSResult<Value> {
    if value.is_undefined() || value.is_null() {
        return Ok(Value::Null);
    }
    if value.is_number() {
        let ms: f64 = value.to_rust()?;
        return finite_time(ms);
    }
    if value.is_string() {
        let text: String = value.to_rust()?;
        return Ok(Value::String(text));
    }
    if let Some(object) = value.into_object()
        && let Ok(get_time) = object.get::<_, JSFunc>("getTime")
    {
        let ms: f64 = get_time.call(Some(object), ())?;
        return finite_time(ms);
    }
    Err(auto_err(
        "clock time must be epoch milliseconds, a date string, or a Date",
    ))
}

fn finite_time(ms: f64) -> JSResult<Value> {
    serde_json::Number::from_f64(ms)
        .map(Value::Number)
        .ok_or_else(|| auto_err(format!("clock time must be a finite number, got {ms}")))
}

/// `tick(ms)`: a finite, non-negative duration.
pub(crate) fn tick_ms(ms: f64) -> Result<f64, String> {
    if ms.is_finite() && (0.0..=MAX_TICK_MS).contains(&ms) {
        Ok(ms)
    } else {
        Err(format!(
            "clock.tick(ms) needs a finite duration in 0..={MAX_TICK_MS} ms, got {ms}"
        ))
    }
}

/// `runAll({ maxTimers })`.
pub(crate) fn run_all_limit(max: Option<f64>) -> Result<u32, String> {
    match max {
        None => Ok(DEFAULT_RUN_ALL_TIMERS),
        Some(max) if max >= 1.0 && max.fract() == 0.0 && max <= f64::from(MAX_RUN_ALL_TIMERS) => {
            Ok(max as u32)
        }
        Some(max) => Err(format!(
            "clock.runAll maxTimers must be an integer in 1..={MAX_RUN_ALL_TIMERS}, got {max}"
        )),
    }
}

/// A controller answer as the driver resolves or rejects it.
#[derive(Debug, PartialEq)]
pub(crate) enum Outcome {
    Ok(Value),
    NotInstalled,
    Installed,
    InvalidTime,
    Busy,
    Limit { fired: u64, pending: u64 },
    Other(String),
}

pub(crate) fn outcome(value: Value) -> Outcome {
    match value.get("error").and_then(Value::as_str) {
        None => Outcome::Ok(value),
        Some("not_installed") => Outcome::NotInstalled,
        Some("installed") => Outcome::Installed,
        Some("invalid_time") => Outcome::InvalidTime,
        Some("busy") => Outcome::Busy,
        Some("limit") => Outcome::Limit {
            fired: value.get("fired").and_then(Value::as_u64).unwrap_or(0),
            pending: value.get("pending").and_then(Value::as_u64).unwrap_or(0),
        },
        Some(other) => Outcome::Other(other.to_string()),
    }
}

fn not_installed() -> rong::RongJSError {
    coded(
        E_CLOCK_NOT_INSTALLED,
        "the test clock is not installed in this lxapp's Logic (call clock.install(); \
         a reopened app starts on real time)",
    )
    .into()
}

fn reject(outcome: Outcome, op: &str) -> rong::RongJSError {
    match outcome {
        Outcome::NotInstalled => not_installed(),
        Outcome::Installed => coded(
            E_CLOCK_INSTALLED,
            "the test clock is already installed in this lxapp's Logic; uninstall it first",
        )
        .into(),
        Outcome::InvalidTime => auto_err("clock time is not a valid date"),
        Outcome::Busy => auto_err(format!(
            "clock.{op} called while the clock is still advancing; await the previous call"
        )),
        Outcome::Limit { fired, pending } => auto_err(if op == "tick" {
            format!(
                "clock.tick fired {fired} timers, its limit for one call, before reaching the \
                 target time; {pending} are still pending: a short setInterval or a timer that \
                 re-arms itself fires too often for one tick; tick in smaller steps"
            )
        } else {
            format!(
                "clock.{op} fired {fired} timers and {pending} are still pending: a setInterval \
                 or a timer that re-arms itself never lets the queue empty; advance with \
                 tick(ms) instead"
            )
        }),
        Outcome::Other(error) => auto_err(format!("clock.{op} failed: {error}")),
        Outcome::Ok(value) => auto_err(format!("unexpected clock answer: {value}")),
    }
}

/// Evaluate one controller call in the app's Logic.
async fn call(app: &Arc<LxApp>, expression: String, timeout: Duration) -> JSResult<Value> {
    let script = format!(
        "(() => {{ const clock = {CONTROLLER}; \
         if (!clock) throw new Error('the test clock is not available in this Logic context'); \
         return {expression}; }})()"
    );
    tokio::time::timeout(timeout, app.eval_logic(script))
        .await
        .map_err(|_| {
            crate::error::coded(
                crate::error::E_EVAL_TIMEOUT,
                "the test clock call did not settle in time",
            )
        })?
        .map_err(|err| auto_err(format!("test clock: {err}")))
}

#[derive(FromJSObject)]
struct InstallOptions {
    now: Option<JSValue>,
}

#[derive(FromJSObject)]
struct RunAllOptions {
    #[js_name = "maxTimers"]
    max_timers: Option<f64>,
}

#[derive(Debug, Clone, IntoJSObject)]
struct ClockAdvance {
    /// Fake epoch milliseconds after the call.
    now: f64,
    /// Timers fired by this call.
    fired: f64,
    /// Timers still scheduled on the fake clock.
    pending: f64,
}

#[derive(Debug, Clone, IntoJSObject)]
struct ClockUninstall {
    uninstalled: bool,
    /// Timers still scheduled when the clock was removed; they never fire.
    dropped: f64,
}

fn number(value: &Value, key: &str) -> f64 {
    value.get(key).and_then(Value::as_f64).unwrap_or(0.0)
}

fn advance(value: Value) -> ClockAdvance {
    ClockAdvance {
        now: number(&value, "now"),
        fired: number(&value, "fired"),
        pending: number(&value, "pending"),
    }
}

#[js_class(clone)]
pub(crate) struct JSClockDriver {
    lxapp: Weak<LxApp>,
}

impl JSClockDriver {
    /// Authorization is checked per call, so reading `.clock` never throws.
    pub(crate) fn new(lxapp: Weak<LxApp>) -> Self {
        Self { lxapp }
    }

    fn target(&self, ctx: &JSContext) -> JSResult<(Arc<LxApp>, ClockRunScope)> {
        let app = upgrade_authorized(ctx, &self.lxapp)?;
        let scope = run_scope(ctx)?;
        Ok((app, scope))
    }

    /// The token of this run's clock for the app. A clock the app lost by
    /// reopening still has a lease until uninstall; Logic then answers
    /// `not_installed`.
    fn token(app: &LxApp, scope: &ClockRunScope) -> JSResult<u64> {
        lease_for(&scope.run_id, &app.appid).ok_or_else(not_installed)
    }
}

#[js_class(rename = "ClockDriver")]
impl JSClockDriver {
    #[js_method(constructor)]
    fn _ctor() -> JSResult<()> {
        Err(HostError::new(
            rong::error::E_ILLEGAL_CONSTRUCTOR,
            "Use lx.automation().lxapp().clock",
        )
        .into())
    }

    /// Put the app's Logic on fake time, starting at `now` (default: the
    /// real current time). Resolves the fake epoch milliseconds.
    #[js_method]
    async fn install(
        &self,
        ctx: JSContext,
        // `Option` inside: an explicit `undefined` or `null` means no options.
        options: Optional<Option<InstallOptions>>,
    ) -> JSResult<f64> {
        let (app, scope) = self.target(&ctx)?;
        let now = match options.0.flatten().and_then(|options| options.now) {
            Some(now) => time_arg(now)?,
            None => Value::Null,
        };
        let token = next_token();
        with_leases(|leases| {
            leases.insert(
                token,
                Lease {
                    run_id: scope.run_id.clone(),
                    appid: app.appid.clone(),
                },
            )
        });
        let result = call(&app, format!("clock.install({token}, {now})"), CALL_TIMEOUT).await;
        match result.map(outcome) {
            Ok(Outcome::Ok(value)) => {
                // Earlier leases of this run for the app belonged to clocks a
                // reopen already discarded.
                release_app(&scope.run_id, &app.appid, Some(token));
                Ok(number(&value, "now"))
            }
            Ok(other) => {
                with_leases(|leases| leases.remove(&token));
                Err(reject(other, "install"))
            }
            Err(err) => {
                with_leases(|leases| leases.remove(&token));
                Err(err)
            }
        }
    }

    /// Advance fake time by `ms`, firing every timer due on the way in time
    /// order and letting the Logic context settle after each.
    #[js_method]
    async fn tick(&self, ctx: JSContext, ms: f64) -> JSResult<ClockAdvance> {
        let (app, scope) = self.target(&ctx)?;
        let ms = tick_ms(ms).map_err(auto_err)?;
        let token = Self::token(&app, &scope)?;
        let value = call(&app, format!("clock.tick({token}, {ms})"), ADVANCE_TIMEOUT).await?;
        match outcome(value) {
            Outcome::Ok(value) => Ok(advance(value)),
            other => Err(reject(other, "tick")),
        }
    }

    /// Fire timers, including ones they schedule, until none is left.
    /// Rejects after `maxTimers` firings (default 1000): an interval never
    /// lets the queue empty.
    #[js_method(rename = "runAll")]
    async fn run_all(
        &self,
        ctx: JSContext,
        // `Option` inside: an explicit `undefined` or `null` means no options.
        options: Optional<Option<RunAllOptions>>,
    ) -> JSResult<ClockAdvance> {
        let (app, scope) = self.target(&ctx)?;
        let limit = run_all_limit(options.0.flatten().and_then(|options| options.max_timers))
            .map_err(auto_err)?;
        let token = Self::token(&app, &scope)?;
        let value = call(
            &app,
            format!("clock.runAll({token}, {limit})"),
            ADVANCE_TIMEOUT,
        )
        .await?;
        match outcome(value) {
            Outcome::Ok(value) => Ok(advance(value)),
            other => Err(reject(other, "runAll")),
        }
    }

    /// Set what `Date` reads without firing timers; timer due times and
    /// `performance.now` are unaffected.
    #[js_method(rename = "setSystemTime")]
    async fn set_system_time(&self, ctx: JSContext, time: JSValue) -> JSResult<f64> {
        let (app, scope) = self.target(&ctx)?;
        let time = time_arg(time)?;
        if time.is_null() {
            return Err(auto_err("clock.setSystemTime needs a time"));
        }
        let token = Self::token(&app, &scope)?;
        let value = call(
            &app,
            format!("clock.setSystemTime({token}, {time})"),
            CALL_TIMEOUT,
        )
        .await?;
        match outcome(value) {
            Outcome::Ok(value) => Ok(number(&value, "now")),
            other => Err(reject(other, "setSystemTime")),
        }
    }

    /// Return the app's Logic to real time. Timers still pending on the fake
    /// clock are dropped. Resolves `{ uninstalled: false }` when no clock is
    /// installed (never installed, or the app reopened since).
    #[js_method]
    async fn uninstall(&self, ctx: JSContext) -> JSResult<ClockUninstall> {
        let (app, scope) = self.target(&ctx)?;
        let value = call(&app, "clock.uninstall()".to_string(), CALL_TIMEOUT).await;
        release_app(&scope.run_id, &app.appid, None);
        let value = value?;
        Ok(ClockUninstall {
            uninstalled: value
                .get("uninstalled")
                .and_then(Value::as_bool)
                .unwrap_or(false),
            dropped: number(&value, "dropped"),
        })
    }
}

#[cfg(test)]
mod tests;
