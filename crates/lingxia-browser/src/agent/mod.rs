//! Agent-grade page primitives: snapshots with element refs, auto-waiting
//! actions with typed failures, complete form input, readable text,
//! annotated screenshots and console capture.
//!
//! The page side is `agent.js`, installed lazily into each document; this
//! module owns waiting (actionability is polled until it holds or the
//! deadline passes), the choice between trusted browser input and DOM
//! synthesis, and everything that needs the host (files, screenshots).

mod console;

use crate::automation::{
    browser_evaluate_javascript, browser_tab_webview, prepare_browser_tab_for_input,
};
use crate::types::BrowserAutomationError;
use lingxia_webview::{PointerButton, PointerInput, ScreenshotRegion, WebViewInputError};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};
use std::collections::HashMap;
use std::path::Path;
use std::sync::{LazyLock, Mutex, OnceLock};
use std::time::{Duration, Instant};

use console::forget_tab as forget_console;
pub use console::{ConsoleEntry, ConsoleList, clear as console_clear};

const AGENT_SOURCE: &str = include_str!("agent.js");
const VERSION_PLACEHOLDER: &str = "__LX_AGENT_VERSION__";

/// The agent's version is a hash of its source, so a page still holding an
/// agent from another build gets this one installed over it.
static AGENT: LazyLock<(String, String)> = LazyLock::new(|| {
    // FNV-1a: stable across builds and platforms, unlike `DefaultHasher`.
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in AGENT_SOURCE.bytes() {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x0100_0000_01b3);
    }
    let version = format!("{hash:016x}");
    let source = AGENT_SOURCE.replace(VERSION_PLACEHOLDER, &version);
    (version, source)
});

const POLL_INTERVAL: Duration = Duration::from_millis(50);
/// Upper bound on files handed to one file input: they travel base64-encoded
/// inside one script evaluation.
const MAX_INPUT_FILES_BYTES: u64 = 20 * 1024 * 1024;
/// Taller full-page captures are cut here (bitmap and CDP limits).
const MAX_FULL_PAGE_HEIGHT: f64 = 16384.0;

type InputFilePolicy = dyn Fn(&Path) -> bool + Send + Sync;
static INPUT_FILE_POLICY: OnceLock<Box<InputFilePolicy>> = OnceLock::new();

/// Decide which host files `set_input_files` may read; without one every
/// absolute path is allowed. A host that lets an AI agent drive its browser
/// sets this: page text can talk an agent into uploading any file it names.
pub fn set_input_file_policy(policy: impl Fn(&Path) -> bool + Send + Sync + 'static) -> bool {
    INPUT_FILE_POLICY.set(Box::new(policy)).is_ok()
}

/// An element: a snapshot `ref` or a CSS `selector` (with `index` among its
/// matches). Exactly one of `ref` and `selector`.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ElementTarget {
    #[serde(rename = "ref", default, skip_serializing_if = "Option::is_none")]
    pub reference: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub selector: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub index: Option<usize>,
}

impl ElementTarget {
    fn by_ref(reference: &str) -> Self {
        Self {
            reference: Some(reference.to_string()),
            ..Self::default()
        }
    }
}

/// A typed failure. `kind` is one of [`kinds`]; `details` carries what the
/// caller needs to recover (`target`, `covered_by`, `reason`, `available`…).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BrowserActionError {
    pub kind: String,
    pub message: String,
    #[serde(flatten)]
    pub details: Map<String, Value>,
}

impl std::fmt::Display for BrowserActionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.kind, self.message)
    }
}

pub mod kinds {
    pub const NOT_FOUND: &str = "not_found";
    pub const STALE_REF: &str = "stale_ref";
    pub const NOT_VISIBLE: &str = "not_visible";
    pub const DISABLED: &str = "disabled";
    pub const COVERED: &str = "covered";
    pub const NOT_EDITABLE: &str = "not_editable";
    pub const TIMEOUT: &str = "timeout";
    pub const UNSUPPORTED: &str = "unsupported";
    pub const USAGE: &str = "usage";
    pub const FAILED: &str = "failed";
    pub const PERMISSION: &str = "permission";
}

fn action_error(kind: &str, message: impl Into<String>, details: Value) -> BrowserAutomationError {
    BrowserAutomationError::Action(BrowserActionError {
        kind: kind.to_string(),
        message: message.into(),
        details: match details {
            Value::Object(map) => map,
            _ => Map::new(),
        },
    })
}

fn usage(message: impl Into<String>) -> BrowserAutomationError {
    action_error(kinds::USAGE, message, Value::Null)
}

// ---------------------------------------------------------------------------
// Calling into the page

/// Refs each document issued, so a ref from a document that has since been
/// replaced is reported as `navigated` rather than unknown.
#[derive(Default)]
struct IssuedRefs {
    /// (generation, highest ref number issued), newest last.
    documents: Vec<(String, u64)>,
}

fn tab_key(tab_id: &str) -> String {
    crate::tabs::normalize_runtime_tab_id(tab_id).unwrap_or_else(|| tab_id.to_string())
}

static ISSUED: LazyLock<Mutex<HashMap<String, IssuedRefs>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

fn note_generation(tab_id: &str, generation: &str, refs_issued: u64) {
    let mut issued = ISSUED.lock().unwrap_or_else(|e| e.into_inner());
    let entry = issued.entry(tab_key(tab_id)).or_default();
    match entry.documents.last_mut() {
        Some((g, n)) if g == generation => *n = (*n).max(refs_issued),
        _ => {
            entry.documents.push((generation.to_string(), refs_issued));
            if entry.documents.len() > 8 {
                entry.documents.remove(0);
            }
        }
    }
}

/// Where a new document's refs start: past every ref this tab has issued.
fn next_ref_number(tab_id: &str) -> u64 {
    let issued = ISSUED.lock().unwrap_or_else(|e| e.into_inner());
    issued
        .get(&tab_key(tab_id))
        .and_then(|refs| refs.documents.iter().map(|(_, max)| *max).max())
        .unwrap_or(0)
        + 1
}

fn ref_number(reference: &str) -> Option<u64> {
    reference.strip_prefix('e')?.parse().ok()
}

fn issued_by_earlier_document(tab_id: &str, generation: &str, reference: &str) -> bool {
    let Some(n) = ref_number(reference) else {
        return false;
    };
    let issued = ISSUED.lock().unwrap_or_else(|e| e.into_inner());
    issued.get(&tab_key(tab_id)).is_some_and(|refs| {
        refs.documents
            .iter()
            .any(|(g, max)| g != generation && *max >= n)
    })
}

pub(crate) fn forget_tab(tab_id: &str) {
    ISSUED
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .remove(&tab_key(tab_id));
    forget_console(tab_id);
}

/// Run one agent op in the tab's current document, installing the agent
/// first when the document has none (or an older one).
async fn agent_call(tab_id: &str, op: &str, args: Value) -> Result<Value, BrowserAutomationError> {
    let op_json = serde_json::to_string(op).unwrap_or_default();
    let args_json = serde_json::to_string(&args).unwrap_or_else(|_| "{}".to_string());
    let (version, source) = &*AGENT;
    let probe = format!(
        "((a) => (a && a.version === '{version}') ? a.call({op_json}, {args_json}) : {{ __lx_agent_missing: true }})(window.__LingXiaAgent)"
    );
    let mut value = browser_evaluate_javascript(tab_id, &probe).await?;
    if value.get("__lx_agent_missing").and_then(Value::as_bool) == Some(true) {
        let ref_start = next_ref_number(tab_id);
        let install = format!(
            "(() => {{\nwindow.__LingXiaAgentRefStart = {ref_start};\n{source}\nreturn window.__LingXiaAgent.call({op_json}, {args_json});\n}})()"
        );
        value = browser_evaluate_javascript(tab_id, &install).await?;
    }
    interpret(tab_id, value)
}

fn interpret(tab_id: &str, mut value: Value) -> Result<Value, BrowserAutomationError> {
    let generation = value
        .get("generation")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    let refs_issued = value
        .get("refs_issued")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    if !generation.is_empty() {
        note_generation(tab_id, &generation, refs_issued);
    }
    let Some(object) = value.as_object_mut() else {
        return Err(action_error(
            kinds::FAILED,
            "the page agent returned no result",
            Value::Null,
        ));
    };
    object.remove("refs_issued");
    let page_dropped = object
        .remove("console_dropped")
        .and_then(|v| v.as_u64())
        .unwrap_or(0);
    let console_url = object.remove("console_url");
    let entries = match object.remove("console") {
        Some(Value::Array(entries)) => entries,
        _ => Vec::new(),
    };
    if !generation.is_empty() {
        console::record_page(
            tab_id,
            &generation,
            &entries,
            console_url.as_ref().and_then(Value::as_str),
            page_dropped,
        );
    }
    if object.remove("ok").and_then(|ok| ok.as_bool()) == Some(true) {
        return Ok(value);
    }
    let error = object.remove("error").unwrap_or(Value::Null);
    let mut error: BrowserActionError =
        serde_json::from_value(error).unwrap_or(BrowserActionError {
            kind: kinds::FAILED.to_string(),
            message: "the page agent failed".to_string(),
            details: Map::new(),
        });
    if error.kind == kinds::STALE_REF
        && error.details.get("reason").and_then(Value::as_str) == Some("unknown")
    {
        let reference = error
            .details
            .get("ref")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string();
        if issued_by_earlier_document(tab_id, &generation, &reference) {
            error
                .details
                .insert("reason".to_string(), json!("navigated"));
            error.message = format!(
                "ref {reference} belongs to a previous document; the page navigated, take a new snapshot"
            );
        }
    }
    if error.kind == "script" {
        error.kind = kinds::FAILED.to_string();
    }
    Err(BrowserAutomationError::Action(error))
}

// ---------------------------------------------------------------------------
// Snapshot

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SnapshotOptions {
    /// Snapshot only this subtree.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub root: Option<ElementTarget>,
    /// `text` (default), `json`, or `both`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub format: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_nodes: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_chars: Option<u32>,
    /// `last`: report only what changed since the previous snapshot of the
    /// same scope in this document (the full snapshot when there is none).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub since: Option<String>,
    /// With `since`, include the full snapshot as well.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub full: bool,
}

pub async fn snapshot(
    tab_id: &str,
    options: &SnapshotOptions,
) -> Result<Value, BrowserAutomationError> {
    if let Some(format) = options.format.as_deref()
        && !matches!(format, "text" | "json" | "both")
    {
        return Err(usage("format must be text, json, or both"));
    }
    if let Some(since) = options.since.as_deref()
        && since != "last"
    {
        return Err(usage("since must be \"last\""));
    }
    let args = serde_json::to_value(options).unwrap_or_default();
    agent_call(tab_id, "snapshot", args).await
}

// ---------------------------------------------------------------------------
// Actionability

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Check {
    Click,
    Hover,
    Drag,
    Fill,
    Type,
    Press,
    Select,
    ScrollTo,
}

impl Check {
    fn as_str(self) -> &'static str {
        match self {
            Check::Click => "click",
            Check::Hover => "hover",
            Check::Drag => "drag",
            Check::Fill => "fill",
            Check::Type => "type",
            Check::Press => "press",
            Check::Select => "select",
            Check::ScrollTo => "scroll_to",
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
struct Probe {
    #[serde(rename = "ref")]
    reference: String,
    rect: Rect,
    point: Point,
    #[serde(default)]
    generation: String,
    /// HTML5 drag source (`draggable`), which needs drag events rather
    /// than pointer moves.
    #[serde(default)]
    draggable: bool,
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq)]
struct Rect {
    x: f64,
    y: f64,
    width: f64,
    height: f64,
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq)]
struct Point {
    x: f64,
    y: f64,
}

/// Failures that waiting cannot fix.
fn is_final(error: &BrowserActionError) -> bool {
    matches!(
        error.kind.as_str(),
        kinds::STALE_REF | kinds::USAGE | kinds::UNSUPPORTED | kinds::FAILED
    )
}

/// Wait until the target is attached, visible, enabled, (for text input)
/// editable, (for pointer input) not covered, and stable — the same box on
/// two consecutive probes. Scrolls it into view on the way. At the deadline
/// the last blocking reason is returned, marked `timed_out`.
async fn wait_actionable(
    tab_id: &str,
    target: &ElementTarget,
    check: Check,
    timeout: Duration,
) -> Result<Probe, BrowserAutomationError> {
    let started = Instant::now();
    let mut previous: Option<Rect> = None;
    let mut last_error: Option<BrowserActionError>;
    loop {
        let args = json!({ "target": target, "action": check.as_str() });
        match agent_call(tab_id, "probe", args).await {
            Ok(value) => {
                let probe: Probe = serde_json::from_value(value).map_err(|err| {
                    action_error(
                        kinds::FAILED,
                        format!("bad probe result: {err}"),
                        Value::Null,
                    )
                })?;
                if previous == Some(probe.rect) {
                    return Ok(probe);
                }
                previous = Some(probe.rect);
                last_error = None;
            }
            Err(BrowserAutomationError::Action(error)) => {
                if is_final(&error) {
                    return Err(BrowserAutomationError::Action(error));
                }
                previous = None;
                last_error = Some(error);
            }
            Err(other) => return Err(other),
        }
        let elapsed = started.elapsed();
        if elapsed >= timeout && previous.is_some() {
            // Actionable at the deadline: one immediate re-probe settles
            // stability rather than failing a ready element.
            if let Ok(value) = agent_call(
                tab_id,
                "probe",
                json!({ "target": target, "action": check.as_str() }),
            )
            .await
                && let Ok(probe) = serde_json::from_value::<Probe>(value)
                && previous == Some(probe.rect)
            {
                return Ok(probe);
            }
        }
        if elapsed >= timeout {
            let timeout_ms = timeout.as_millis() as u64;
            return Err(match last_error {
                Some(mut error) => {
                    error.details.insert("timed_out".to_string(), json!(true));
                    error
                        .details
                        .insert("timeout_ms".to_string(), json!(timeout_ms));
                    error.message = format!("{} (waited {timeout_ms}ms)", error.message);
                    BrowserAutomationError::Action(error)
                }
                None => action_error(
                    kinds::TIMEOUT,
                    format!("element did not stop moving within {timeout_ms}ms"),
                    json!({ "timeout_ms": timeout_ms }),
                ),
            });
        }
        // The stability re-check only needs a frame; a blocked probe backs off.
        let pause = if previous.is_some() {
            Duration::from_millis(30)
        } else {
            POLL_INTERVAL
        };
        tokio::time::sleep(pause.min(timeout.saturating_sub(elapsed))).await;
    }
}

/// Wait for the target to exist (no visibility requirement), for inputs a
/// page keeps hidden behind custom chrome, like file inputs.
async fn wait_resolved(
    tab_id: &str,
    op: &str,
    args: Value,
    timeout: Duration,
) -> Result<Value, BrowserAutomationError> {
    let started = Instant::now();
    loop {
        match agent_call(tab_id, op, args.clone()).await {
            Err(BrowserAutomationError::Action(error))
                if error.kind == kinds::NOT_FOUND && started.elapsed() < timeout =>
            {
                tokio::time::sleep(POLL_INTERVAL).await;
            }
            other => return other,
        }
    }
}

// ---------------------------------------------------------------------------
// Actions

/// After an action, optionally report what changed on the page.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum AfterAction {
    #[default]
    None,
    /// Changes since the previous snapshot (a full snapshot after navigation).
    Diff,
    /// A full snapshot.
    Full,
}

#[derive(Debug, Clone, Default)]
pub struct ActionOptions {
    pub timeout: Duration,
    pub after: AfterAction,
}

async fn finish(
    tab_id: &str,
    generation_before: &str,
    mut result: Map<String, Value>,
    after: AfterAction,
) -> Result<Value, BrowserAutomationError> {
    // The document the action ran in; `with_page_change` compares against it.
    result.insert("generation".to_string(), json!(generation_before));
    with_page_change(tab_id, Value::Object(result), after).await
}

/// Add what the action changed on the page to its `result` (which names the
/// document it ran in as `generation`). Callers that also wait for a
/// navigation call this after that wait, so the snapshot is of the page the
/// action led to.
pub async fn with_page_change(
    tab_id: &str,
    mut result: Value,
    after: AfterAction,
) -> Result<Value, BrowserAutomationError> {
    if after == AfterAction::None {
        return Ok(result);
    }
    let before = result
        .get("generation")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    // A navigating document cannot answer; the next one will.
    let mut generation = None;
    for _ in 0..20 {
        let _ = agent_call(tab_id, "settle", json!({ "quiet_ms": 150, "max_ms": 2000 })).await;
        if let Ok(metrics) = agent_call(tab_id, "metrics", json!({})).await {
            generation = metrics
                .get("generation")
                .and_then(Value::as_str)
                .map(str::to_string);
            break;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    let navigated = generation.as_deref() != Some(before.as_str());
    let options = SnapshotOptions {
        since: (after == AfterAction::Diff && !navigated).then(|| "last".to_string()),
        ..SnapshotOptions::default()
    };
    let mut snap = snapshot(tab_id, &options).await?;
    if navigated && let Some(object) = snap.as_object_mut() {
        object.insert("navigated".to_string(), json!(true));
    }
    if let Some(object) = result.as_object_mut() {
        object.insert("snapshot".to_string(), snap);
    }
    Ok(result)
}

fn base_result(probe: &Probe) -> Map<String, Value> {
    let mut result = Map::new();
    result.insert("ref".to_string(), json!(probe.reference));
    result
}

fn trusted_or_fallback(
    result: Result<(), WebViewInputError>,
) -> Result<bool, BrowserAutomationError> {
    match result {
        Ok(()) => Ok(true),
        Err(WebViewInputError::Unsupported(_)) => Ok(false),
        Err(err) => Err(err.into()),
    }
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct ClickOptions {
    #[serde(default)]
    pub button: PointerButton,
    #[serde(default = "one")]
    pub click_count: u32,
}

fn one() -> u32 {
    1
}

pub async fn click(
    tab_id: &str,
    target: &ElementTarget,
    click: ClickOptions,
    options: &ActionOptions,
) -> Result<Value, BrowserAutomationError> {
    prepare_browser_tab_for_input(tab_id).await?;
    let probe = wait_actionable(tab_id, target, Check::Click, options.timeout).await?;
    let webview = browser_tab_webview(tab_id)?;
    // A browser-level right click opens the WebView's native context menu,
    // which then holds the next input; the page gets `contextmenu` instead.
    let trusted = click.button != PointerButton::Right
        && trusted_or_fallback(
            webview
                .trusted_pointer(PointerInput::Click {
                    x: probe.point.x,
                    y: probe.point.y,
                    button: click.button,
                    count: click.click_count.max(1),
                })
                .await,
        )?;
    if !trusted {
        agent_call(
            tab_id,
            "click",
            json!({
                "target": ElementTarget::by_ref(&probe.reference),
                "point": { "x": probe.point.x, "y": probe.point.y },
                "button": click.button,
                "click_count": click.click_count.max(1),
            }),
        )
        .await?;
    }
    let mut result = base_result(&probe);
    result.insert("trusted".to_string(), json!(trusted));
    finish(tab_id, &probe.generation, result, options.after).await
}

pub async fn hover(
    tab_id: &str,
    target: &ElementTarget,
    options: &ActionOptions,
) -> Result<Value, BrowserAutomationError> {
    prepare_browser_tab_for_input(tab_id).await?;
    let probe = wait_actionable(tab_id, target, Check::Hover, options.timeout).await?;
    let webview = browser_tab_webview(tab_id)?;
    let trusted = trusted_or_fallback(
        webview
            .trusted_pointer(PointerInput::Move {
                x: probe.point.x,
                y: probe.point.y,
            })
            .await,
    )?;
    if !trusted {
        agent_call(
            tab_id,
            "hover",
            json!({ "target": ElementTarget::by_ref(&probe.reference) }),
        )
        .await?;
    }
    let mut result = base_result(&probe);
    // DOM-synthesized hover fires the events but cannot apply `:hover` styles.
    result.insert("trusted".to_string(), json!(trusted));
    finish(tab_id, &probe.generation, result, options.after).await
}

pub async fn drag(
    tab_id: &str,
    source: &ElementTarget,
    destination: &ElementTarget,
    options: &ActionOptions,
) -> Result<Value, BrowserAutomationError> {
    prepare_browser_tab_for_input(tab_id).await?;
    let from = wait_actionable(tab_id, source, Check::Drag, options.timeout).await?;
    let to = wait_actionable(tab_id, destination, Check::ScrollTo, options.timeout).await?;
    // Scrolling the destination into view may have moved the source.
    let from = wait_actionable(
        tab_id,
        &ElementTarget::by_ref(&from.reference),
        Check::Drag,
        options.timeout,
    )
    .await?;
    let webview = browser_tab_webview(tab_id)?;
    // Browser-level mouse drags do not start HTML5 drag-and-drop in an
    // embedded WebView, so a draggable source gets drag events instead.
    let trusted = if from.draggable {
        false
    } else {
        trusted_or_fallback(
            webview
                .trusted_pointer(PointerInput::Drag {
                    from: (from.point.x, from.point.y),
                    to: (to.point.x, to.point.y),
                })
                .await,
        )?
    };
    if !trusted {
        agent_call(
            tab_id,
            "drag",
            json!({
                "target": ElementTarget::by_ref(&from.reference),
                "to": ElementTarget::by_ref(&to.reference),
            }),
        )
        .await?;
    }
    let mut result = base_result(&from);
    result.insert("to".to_string(), json!(to.reference));
    result.insert("trusted".to_string(), json!(trusted));
    finish(tab_id, &from.generation, result, options.after).await
}

pub async fn fill(
    tab_id: &str,
    target: &ElementTarget,
    value: &str,
    options: &ActionOptions,
) -> Result<Value, BrowserAutomationError> {
    prepare_browser_tab_for_input(tab_id).await?;
    let probe = wait_actionable(tab_id, target, Check::Fill, options.timeout).await?;
    let filled = agent_call(
        tab_id,
        "fill",
        json!({ "target": ElementTarget::by_ref(&probe.reference), "value": value }),
    )
    .await?;
    let mut result = base_result(&probe);
    if let Some(value) = filled.get("value") {
        result.insert("value".to_string(), value.clone());
    }
    finish(tab_id, &probe.generation, result, options.after).await
}

/// Type `text` at the end of the target's content (after clearing it with
/// `clear`). Trusted keyboard/IME input where the WebView offers it,
/// otherwise editor commands in the page.
pub async fn type_text(
    tab_id: &str,
    target: &ElementTarget,
    text: &str,
    clear: bool,
    options: &ActionOptions,
) -> Result<Value, BrowserAutomationError> {
    prepare_browser_tab_for_input(tab_id).await?;
    let probe = wait_actionable(tab_id, target, Check::Type, options.timeout).await?;
    let by_ref = ElementTarget::by_ref(&probe.reference);
    agent_call(
        tab_id,
        "focus_for_typing",
        json!({ "target": by_ref, "clear": clear }),
    )
    .await?;
    let webview = browser_tab_webview(tab_id)?;
    let trusted = trusted_or_fallback(webview.trusted_insert_text(text).await)?;
    let mut result = base_result(&probe);
    if !trusted {
        let typed = agent_call(
            tab_id,
            "type",
            json!({ "target": by_ref, "text": text, "clear": false }),
        )
        .await?;
        if let Some(value) = typed.get("value") {
            result.insert("value".to_string(), value.clone());
        }
    }
    result.insert("trusted".to_string(), json!(trusted));
    finish(tab_id, &probe.generation, result, options.after).await
}

/// Press a key on the target (focused first) or, without one, on whatever
/// has focus.
pub async fn press(
    tab_id: &str,
    target: Option<&ElementTarget>,
    key: &str,
    options: &ActionOptions,
) -> Result<Value, BrowserAutomationError> {
    prepare_browser_tab_for_input(tab_id).await?;
    let mut result = Map::new();
    let mut generation = String::new();
    if let Some(target) = target {
        let probe = wait_actionable(tab_id, target, Check::Press, options.timeout).await?;
        agent_call(
            tab_id,
            "focus",
            json!({ "target": ElementTarget::by_ref(&probe.reference) }),
        )
        .await?;
        generation = probe.generation.clone();
        result = base_result(&probe);
    } else if options.after != AfterAction::None {
        generation = agent_call(tab_id, "metrics", json!({}))
            .await?
            .get("generation")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string();
    }
    crate::automation::browser_press(tab_id, key).await?;
    finish(tab_id, &generation, result, options.after).await
}

pub async fn scroll_to(
    tab_id: &str,
    target: &ElementTarget,
    options: &ActionOptions,
) -> Result<Value, BrowserAutomationError> {
    let probe = wait_actionable(tab_id, target, Check::ScrollTo, options.timeout).await?;
    let mut result = base_result(&probe);
    result.insert(
        "rect".to_string(),
        json!({ "x": probe.rect.x, "y": probe.rect.y, "width": probe.rect.width, "height": probe.rect.height }),
    );
    finish(tab_id, &probe.generation, result, options.after).await
}

/// One `<option>` to select: by `value`, visible `label`, or `index`; a
/// bare string matches a value first, then a label.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(untagged)]
pub enum OptionChoice {
    Text(String),
    Value { value: String },
    Label { label: String },
    Index { index: u32 },
}

pub async fn select_option(
    tab_id: &str,
    target: &ElementTarget,
    choices: &[OptionChoice],
    options: &ActionOptions,
) -> Result<Value, BrowserAutomationError> {
    if choices.is_empty() {
        return Err(usage("pass at least one option"));
    }
    prepare_browser_tab_for_input(tab_id).await?;
    let probe = wait_actionable(tab_id, target, Check::Select, options.timeout).await?;
    let selected = agent_call(
        tab_id,
        "select_option",
        json!({ "target": ElementTarget::by_ref(&probe.reference), "options": choices }),
    )
    .await?;
    let mut result = base_result(&probe);
    if let Some(value) = selected.get("selected") {
        result.insert("selected".to_string(), value.clone());
    }
    finish(tab_id, &probe.generation, result, options.after).await
}

#[derive(Debug, Deserialize)]
struct CheckState {
    #[serde(rename = "ref")]
    reference: String,
    checked: bool,
    #[serde(default)]
    radio: bool,
}

async fn check_state(
    tab_id: &str,
    target: &ElementTarget,
) -> Result<CheckState, BrowserAutomationError> {
    let value = agent_call(tab_id, "check_target", json!({ "target": target })).await?;
    serde_json::from_value(value).map_err(|err| {
        action_error(
            kinds::FAILED,
            format!("bad check state: {err}"),
            Value::Null,
        )
    })
}

/// Make a checkbox, radio, or switch `checked` (idempotent). It is clicked
/// like a user would, so a styled control whose real input is hidden is
/// reached through its label.
pub async fn set_checked(
    tab_id: &str,
    target: &ElementTarget,
    checked: bool,
    options: &ActionOptions,
) -> Result<Value, BrowserAutomationError> {
    prepare_browser_tab_for_input(tab_id).await?;
    let started = Instant::now();
    let state = loop {
        match check_state(tab_id, target).await {
            Err(BrowserAutomationError::Action(error))
                if error.kind == kinds::NOT_FOUND && started.elapsed() < options.timeout =>
            {
                tokio::time::sleep(POLL_INTERVAL).await;
            }
            other => break other?,
        }
    };
    let control = ElementTarget::by_ref(&state.reference);
    let mut result = Map::new();
    result.insert("ref".to_string(), json!(state.reference));
    if state.checked == checked {
        result.insert("checked".to_string(), json!(checked));
        result.insert("changed".to_string(), json!(false));
        return Ok(Value::Object(result));
    }
    if state.radio && !checked {
        return Err(usage(
            "a radio button cannot be unchecked; check another one in its group",
        ));
    }
    let remaining = options.timeout.saturating_sub(started.elapsed());
    let click_options = ActionOptions {
        timeout: remaining,
        after: AfterAction::None,
    };
    // A styled control hides its input and draws a label: decide which to
    // click from one look now, not after the input's wait times out.
    let hidden = matches!(
        agent_call(tab_id, "probe", json!({ "target": control, "action": "click" })).await,
        Err(BrowserAutomationError::Action(ref error)) if error.kind == kinds::NOT_VISIBLE
    );
    let label = if hidden {
        agent_call(tab_id, "label_for", json!({ "target": control }))
            .await
            .ok()
            .and_then(|v| v.get("label").and_then(Value::as_str).map(str::to_string))
    } else {
        None
    };
    let clicked = click(
        tab_id,
        &label.map_or_else(|| control.clone(), |label| ElementTarget::by_ref(&label)),
        ClickOptions {
            button: PointerButton::Left,
            click_count: 1,
        },
        &click_options,
    )
    .await?;
    let after = check_state(tab_id, &control).await?;
    if after.checked != checked {
        return Err(action_error(
            kinds::FAILED,
            "clicking did not change the checked state",
            json!({ "ref": state.reference, "checked": after.checked }),
        ));
    }
    result.insert("checked".to_string(), json!(after.checked));
    result.insert("changed".to_string(), json!(true));
    if let Some(trusted) = clicked.get("trusted") {
        result.insert("trusted".to_string(), trusted.clone());
    }
    let generation = agent_call(tab_id, "metrics", json!({}))
        .await?
        .get("generation")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    finish(tab_id, &generation, result, options.after).await
}

/// Put files on an `<input type=file>` without the native chooser. Paths
/// must be absolute host paths; the host decides which ones it may read.
pub async fn set_input_files(
    tab_id: &str,
    target: &ElementTarget,
    paths: &[String],
    options: &ActionOptions,
) -> Result<Value, BrowserAutomationError> {
    use base64::Engine as _;
    let mut files = Vec::with_capacity(paths.len());
    let mut total = 0u64;
    for raw in paths {
        let path = Path::new(raw);
        if !path.is_absolute() {
            return Err(usage(format!("file path must be absolute: {raw}")));
        }
        let metadata = std::fs::metadata(path).map_err(|err| {
            action_error(
                kinds::NOT_FOUND,
                format!("cannot read {raw}: {err}"),
                json!({ "path": raw }),
            )
        })?;
        if !metadata.is_file() {
            return Err(usage(format!("not a file: {raw}")));
        }
        if let Some(policy) = INPUT_FILE_POLICY.get() {
            let resolved = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
            if !policy(&resolved) {
                return Err(action_error(
                    kinds::PERMISSION,
                    format!("the host does not allow uploading {raw}"),
                    json!({ "path": raw }),
                ));
            }
        }
        total += metadata.len();
        if total > MAX_INPUT_FILES_BYTES {
            return Err(usage(format!(
                "files exceed {} MiB in total",
                MAX_INPUT_FILES_BYTES / (1024 * 1024)
            )));
        }
        let bytes = std::fs::read(path).map_err(|err| {
            action_error(
                kinds::FAILED,
                format!("cannot read {raw}: {err}"),
                json!({ "path": raw }),
            )
        })?;
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("file")
            .to_string();
        let last_modified = metadata
            .modified()
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_millis() as u64);
        files.push(json!({
            "name": name,
            "mime": mime_for(path),
            "base64": base64::engine::general_purpose::STANDARD.encode(bytes),
            "last_modified": last_modified,
        }));
    }
    let args = json!({ "target": target, "files": files });
    let value = wait_resolved(tab_id, "set_files", args, options.timeout).await?;
    let mut result = Map::new();
    if let Some(files) = value.get("files") {
        result.insert("files".to_string(), files.clone());
    }
    let generation = value
        .get("generation")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    finish(tab_id, &generation, result, options.after).await
}

fn mime_for(path: &Path) -> &'static str {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .map(str::to_ascii_lowercase)
        .unwrap_or_default();
    match ext.as_str() {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "svg" => "image/svg+xml",
        "heic" => "image/heic",
        "pdf" => "application/pdf",
        "txt" => "text/plain",
        "csv" => "text/csv",
        "json" => "application/json",
        "html" | "htm" => "text/html",
        "zip" => "application/zip",
        "mp4" => "video/mp4",
        "mov" => "video/quicktime",
        "mp3" => "audio/mpeg",
        "wav" => "audio/wav",
        "doc" => "application/msword",
        "docx" => "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
        "xls" => "application/vnd.ms-excel",
        "xlsx" => "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
        _ => "",
    }
}

// ---------------------------------------------------------------------------
// Observation

/// What the page logged: entries after `since` (all when `None`),
/// optionally only `levels`. Collects the current document's new entries
/// first; a page that cannot be reached right now still lists what was
/// collected before.
pub async fn console_list(tab_id: &str, since: Option<u64>, levels: &[String]) -> ConsoleList {
    let _ = agent_call(tab_id, "console", json!({})).await;
    console::list(tab_id, since, levels)
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TextOptions {
    /// Only this element's content.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target: Option<ElementTarget>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_chars: Option<u32>,
    /// Include navigation, header, footer and sidebars too.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub all: bool,
}

/// The page's main readable content as Markdown.
pub async fn text(tab_id: &str, options: &TextOptions) -> Result<Value, BrowserAutomationError> {
    let args = serde_json::to_value(options).unwrap_or_default();
    agent_call(tab_id, "text", args).await
}

#[derive(Debug, Clone, PartialEq)]
pub enum ScreenshotScope {
    Viewport,
    FullPage,
    Element(ElementTarget),
}

pub struct Screenshot {
    pub png: Vec<u8>,
    /// Scope, size, and for annotated captures the ref → box map.
    pub meta: Map<String, Value>,
}

/// A screenshot of the viewport, the full page, or one element. `annotate`
/// draws each visible snapshot ref's box and label over a viewport capture
/// (set-of-marks) and returns the boxes.
pub async fn screenshot(
    tab_id: &str,
    scope: &ScreenshotScope,
    annotate: bool,
    timeout: Duration,
) -> Result<Screenshot, BrowserAutomationError> {
    let webview = browser_tab_webview(tab_id)?;
    let mut meta = Map::new();
    match scope {
        ScreenshotScope::Viewport => {
            meta.insert("scope".to_string(), json!("viewport"));
            if !annotate {
                let png = webview.take_screenshot().await?;
                return Ok(Screenshot { png, meta });
            }
            let marks = agent_call(tab_id, "marks", json!({})).await?;
            let _ = agent_call(tab_id, "frame", json!({})).await;
            let captured = webview.take_screenshot().await;
            let _ = agent_call(tab_id, "clear_marks", json!({})).await;
            let png = captured?;
            for key in ["marks", "truncated", "viewport"] {
                if let Some(value) = marks.get(key) {
                    meta.insert(key.to_string(), value.clone());
                }
            }
            Ok(Screenshot { png, meta })
        }
        ScreenshotScope::FullPage => {
            if annotate {
                return Err(usage(
                    "annotated screenshots cover the viewport; drop full_page",
                ));
            }
            let metrics = agent_call(tab_id, "metrics", json!({})).await?;
            let width = metrics.get("width").and_then(Value::as_f64).unwrap_or(0.0);
            let height = metrics.get("height").and_then(Value::as_f64).unwrap_or(0.0);
            if width <= 0.0 || height <= 0.0 {
                return Err(action_error(
                    kinds::FAILED,
                    "the page has no size",
                    Value::Null,
                ));
            }
            // macOS renders the whole page and scales it to fit instead.
            let clipped = height > MAX_FULL_PAGE_HEIGHT && !cfg!(target_vendor = "apple");
            let png = webview
                .take_screenshot_region(ScreenshotRegion::FullPage {
                    width,
                    height: height.min(MAX_FULL_PAGE_HEIGHT),
                })
                .await?;
            if clipped {
                meta.insert("clipped".to_string(), json!(true));
            }
            meta.insert("scope".to_string(), json!("full_page"));
            meta.insert(
                "page".to_string(),
                json!({ "width": width, "height": height }),
            );
            Ok(Screenshot { png, meta })
        }
        ScreenshotScope::Element(target) => {
            if annotate {
                return Err(usage(
                    "annotated screenshots cover the viewport; drop the element",
                ));
            }
            let probe = wait_actionable(tab_id, target, Check::ScrollTo, timeout).await?;
            let boxed = agent_call(
                tab_id,
                "element_box",
                json!({ "target": ElementTarget::by_ref(&probe.reference) }),
            )
            .await?;
            let metrics = agent_call(tab_id, "metrics", json!({})).await?;
            let get = |v: &Value, k: &str| v.get(k).and_then(Value::as_f64).unwrap_or(0.0);
            let document = boxed.get("document").cloned().unwrap_or_default();
            let region = ScreenshotRegion::Clip {
                x: get(&document, "x"),
                y: get(&document, "y"),
                width: get(&document, "width"),
                height: get(&document, "height"),
                scroll_x: get(&metrics, "scroll_x"),
                scroll_y: get(&metrics, "scroll_y"),
            };
            let png = webview.take_screenshot_region(region).await?;
            meta.insert("scope".to_string(), json!("element"));
            meta.insert("ref".to_string(), json!(probe.reference));
            meta.insert(
                "rect".to_string(),
                boxed.get("rect").cloned().unwrap_or_default(),
            );
            // macOS snapshots the visible view: an element taller or wider
            // than the viewport is cut at its edges.
            if cfg!(target_vendor = "apple") {
                let clipped = get(&document, "height") > get(&metrics, "viewport_height")
                    || get(&document, "width") > get(&metrics, "viewport_width");
                if clipped {
                    meta.insert("clipped".to_string(), json!(true));
                }
            }
            Ok(Screenshot { png, meta })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn agent_version_is_stamped_into_the_script() {
        let (version, source) = &*AGENT;
        assert_eq!(AGENT_SOURCE.matches(VERSION_PLACEHOLDER).count(), 1);
        assert!(source.contains(&format!("const VERSION = '{version}';")));
    }

    #[test]
    fn target_serializes_ref_under_its_wire_name() {
        let value = serde_json::to_value(ElementTarget::by_ref("e3")).unwrap();
        assert_eq!(value, json!({ "ref": "e3" }));
        let parsed: ElementTarget =
            serde_json::from_value(json!({ "selector": "#a", "index": 2 })).unwrap();
        assert_eq!(parsed.selector.as_deref(), Some("#a"));
        assert_eq!(parsed.index, Some(2));
    }

    #[test]
    fn option_choices_parse_by_value_label_or_index() {
        let choices: Vec<OptionChoice> = serde_json::from_value(
            json!([{ "value": "cn" }, { "label": "Japan" }, { "index": 2 }, "Korea"]),
        )
        .unwrap();
        assert_eq!(
            choices,
            vec![
                OptionChoice::Value { value: "cn".into() },
                OptionChoice::Label {
                    label: "Japan".into()
                },
                OptionChoice::Index { index: 2 },
                OptionChoice::Text("Korea".into()),
            ]
        );
    }

    #[test]
    fn unknown_ref_from_a_replaced_document_reads_as_navigated() {
        let tab = "agent-test-tab";
        note_generation(tab, "doc-a", 12);
        let value = json!({
            "ok": false,
            "generation": "doc-b",
            "refs_issued": 0,
            "error": { "kind": "stale_ref", "message": "x", "reason": "unknown", "ref": "e7" }
        });
        let Err(BrowserAutomationError::Action(error)) = interpret(tab, value) else {
            panic!("expected an action error");
        };
        assert_eq!(error.kind, kinds::STALE_REF);
        assert_eq!(error.details["reason"], "navigated");
        forget_tab(tab);
    }

    #[test]
    fn ok_results_drop_internal_fields() {
        let value = interpret(
            "agent-test-ok",
            json!({ "ok": true, "generation": "g", "refs_issued": 3, "text": "- button" }),
        )
        .unwrap();
        assert_eq!(value, json!({ "generation": "g", "text": "- button" }));
        forget_tab("agent-test-ok");
    }

    #[test]
    fn script_failures_surface_as_failed() {
        let value = json!({ "ok": false, "generation": "g", "error": { "kind": "script", "message": "boom" } });
        let Err(BrowserAutomationError::Action(error)) = interpret("agent-test-script", value)
        else {
            panic!("expected an action error");
        };
        assert_eq!(error.kind, kinds::FAILED);
        forget_tab("agent-test-script");
    }

    #[test]
    fn mime_types_follow_the_extension() {
        assert_eq!(mime_for(Path::new("/a/b.PNG")), "image/png");
        assert_eq!(mime_for(Path::new("/a/b.unknownext")), "");
    }
}
