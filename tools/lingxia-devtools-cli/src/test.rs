//! `lxdev test <entry>` — bundle a JS/TS test, run it in the selected live
//! session in an isolated automation runtime, stream console output, download
//! artifacts, and report one terminal summary.

use crate::client::{CommandTimeout, execute_command, execute_command_until, max_poll_wait};
use crate::project::SessionInfo;
use crate::test_bundle::{MappedPosition, TestBundle, bundle_test_path, find_project_root};
use anyhow::{Context, Result, anyhow, bail};
use base64::Engine as _;
use base64::engine::general_purpose::STANDARD as BASE64;
use clap::Args;
use lingxia_control_protocol::{dev_session::session_test::*, methods};
use owo_colors::OwoColorize;
use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::json;
use std::collections::HashMap;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

const POLL_INTERVAL: Duration = Duration::from_millis(250);
/// After a cancel is sent, wait this long for the terminal state.
const CANCEL_GRACE: Duration = Duration::from_secs(10);
const WATCHDOG_GRACE: Duration = Duration::from_secs(5);
const DEFAULT_CASE_TIMEOUT: Duration = Duration::from_secs(30);
const MAX_ARTIFACT_BYTES: usize = 16 * 1024 * 1024;
const MAX_ARTIFACT_BASE64_BYTES: usize = MAX_ARTIFACT_BYTES.div_ceil(3) * 4;

pub const NO_SESSION_HINT: &str = "No live dev session found. Start one with `lingxia dev --background`, then re-run `lxdev test`.";

#[derive(Args)]
#[command(after_long_help = "Pass a file or a directory of *.test.ts files.\n\
Import spec from @lingxia/test (or test from @rongjs/test).\n\
Example: lxdev test tests/ --grep home\n\
Recover a session held by an abandoned run: lxdev test --cancel-active")]
pub struct TestOptions {
    /// Test entry file, or a directory of `*.test.ts` files. Omit it with
    /// `--cancel-active` to only cancel the session's active run.
    #[arg(required_unless_present = "cancel_active")]
    pub entry: Option<PathBuf>,

    /// Whole-run budget in seconds
    #[arg(long, default_value_t = 300, value_parser = parse_timeout_secs)]
    timeout_secs: u64,

    /// Key=value string exposed as test.args (repeatable). Keys that look
    /// like credentials (password, secret, token, api key) are written to
    /// reports as `***`.
    #[arg(long = "arg", value_name = "KEY=VALUE", value_parser = parse_key_value)]
    args: Vec<(String, String)>,

    /// Like `--arg`, but the value is always written to reports as `***`
    #[arg(long = "secret-arg", value_name = "KEY=VALUE", value_parser = parse_key_value)]
    secret_args: Vec<(String, String)>,

    /// Run only specs whose title or id matches this regex
    #[arg(long, value_name = "PATTERN")]
    pub grep: Option<String>,

    /// Fail if any spec.only is registered
    #[arg(long)]
    pub forbid_only: bool,

    /// Retry failed specs (requires a spec.reset hook for their file)
    #[arg(long, default_value_t = 0, value_parser = clap::value_parser!(u8).range(0..=10))]
    retries: u8,

    /// Partition stable spec ids across independent sessions (INDEX/TOTAL)
    #[arg(long)]
    shard: Option<String>,

    /// Rerun failed ids from an earlier report.json
    #[arg(long, value_name = "REPORT")]
    last_failed: Option<PathBuf>,

    /// Cancel an automation run left active by an earlier `lxdev test`, then
    /// start. Only needed when a previous run's client died mid-run. Without
    /// an entry, only cancel the active run.
    #[arg(long)]
    cancel_active: bool,

    /// Allow a selection with no matching specs
    #[arg(long)]
    pass_with_no_tests: bool,

    /// Select one exact stable spec id
    #[arg(long)]
    id: Option<String>,

    /// Include steps, console messages and individual artifact transfers
    #[arg(long)]
    verbose: bool,

    /// Stream versioned JSONL events and a final result
    #[arg(long, conflicts_with_all = ["json", "pretty"])]
    jsonl: bool,

    /// Directory receiving attached artifacts
    /// (default: test-results/<run-id>)
    #[arg(long, value_name = "PATH")]
    output_dir: Option<PathBuf>,

    /// Emit one final compact JSON object instead of live output
    #[arg(long, conflicts_with = "pretty")]
    json: bool,

    /// Emit one final pretty JSON object instead of live output
    #[arg(long, conflicts_with = "json")]
    pretty: bool,
}

/// The ceiling is the automation runtime's own run budget
/// (`MAX_TIMEOUT_MS`), not a CLI preference, so say what to do instead of
/// only naming the range: a suite too big for one budget is what `--shard`
/// is for.
const MAX_TIMEOUT_SECS: u64 = 3600;

fn parse_timeout_secs(raw: &str) -> Result<u64, String> {
    let secs: u64 = raw
        .parse()
        .map_err(|_| format!("`{raw}` is not a whole number of seconds"))?;
    if secs == 0 {
        return Err("a run needs at least 1 second".to_string());
    }
    if secs > MAX_TIMEOUT_SECS {
        return Err(format!(
            "{secs}s exceeds the {MAX_TIMEOUT_SECS}s run budget the automation runtime allows. \
             Split the suite across sessions with `--shard INDEX/TOTAL`, or run fewer files per \
             invocation."
        ));
    }
    Ok(secs)
}

fn parse_key_value(raw: &str) -> Result<(String, String), String> {
    match raw.split_once('=') {
        Some((key, value)) if !key.is_empty() => Ok((key.to_string(), value.to_string())),
        _ => Err(format!("expected KEY=VALUE, got {raw:?}")),
    }
}

fn execute_typed<A, R>(ws_url: &str, handler: &str, args: &A) -> Result<R>
where
    A: Serialize,
    R: DeserializeOwned,
{
    let args = serde_json::to_value(args).context("failed to encode devtool command args")?;
    let response = execute_command(ws_url, handler, Some(args))?
        .ok_or_else(|| anyhow!("{handler} returned no data"))?;
    serde_json::from_value(response).with_context(|| format!("invalid {handler} response"))
}

/// Owns process exit: the run state is the exit code, not an `Err`.
pub fn execute(info: &SessionInfo, options: TestOptions) -> Result<()> {
    execute_inner(info, options).map_err(|err| {
        if looks_unreachable(&err) {
            anyhow!(NO_SESSION_HINT)
        } else {
            err
        }
    })
}

pub fn looks_unreachable(err: &anyhow::Error) -> bool {
    let text = format!("{err:#}").to_lowercase();
    text.contains("no live dev session")
        || text.contains("websocket")
        || text.contains("connection refused")
        || text.contains("failed to connect")
        || text.contains("os error 10061")
        || text.contains("10054")
        || text.contains("broken pipe")
}

fn execute_inner(info: &SessionInfo, options: TestOptions) -> Result<()> {
    let machine = options.json || options.pretty || options.jsonl;
    let Some(entry) = options.entry.clone() else {
        return cancel_active_only(info, &options);
    };
    let started_at = chrono::Utc::now().to_rfc3339();
    // A bad output path must fail before the run exists: afterwards the
    // Runner would keep it active with nobody polling.
    let output_root = options
        .output_dir
        .clone()
        .unwrap_or_else(|| PathBuf::from(DEFAULT_OUTPUT_ROOT));
    ensure_writable_dir(&output_root)?;
    warn_package_version(&entry, machine);
    let bundle = bundle_test_path(&entry)?;
    if !machine {
        eprintln!(
            "{} bundled {} ({})",
            "test".cyan(),
            entry.display(),
            human_bytes(bundle.code.len())
        );
    }

    let mut args = options.args.iter().cloned().collect::<HashMap<_, _>>();
    let secret_keys = secret_keys(&options);
    args.extend(options.secret_args.iter().cloned());
    if !options.secret_args.is_empty() {
        let mut keys = options
            .secret_args
            .iter()
            .map(|(key, _)| key.clone())
            .collect::<Vec<_>>();
        keys.sort();
        keys.dedup();
        args.insert(SECRET_ARGS_KEY.into(), serde_json::to_string(&keys)?);
    }
    args.entry("platform".into())
        .or_insert_with(|| info.target.clone());
    if let Some(grep) = &options.grep {
        args.insert("grep".to_string(), grep.clone());
    }
    args.insert("retries".into(), options.retries.to_string());
    if let Some(shard) = &options.shard {
        args.insert("shard".into(), shard.clone());
    }
    if let Some(path) = &options.last_failed {
        let previous: serde_json::Value = serde_json::from_slice(&std::fs::read(path)?)?;
        let ids = previous["cases"]
            .as_array()
            .ok_or_else(|| anyhow!("{} has no cases", path.display()))?
            .iter()
            .filter(|c| matches!(c["status"].as_str(), Some("failed" | "timeout" | "xpass")))
            .filter_map(|c| c["id"].as_str())
            .collect::<Vec<_>>();
        args.insert("ids".into(), serde_json::to_string(&ids)?);
    }
    if options.pass_with_no_tests {
        args.insert("passWithNoTests".into(), "1".into());
    }
    if let Some(id) = &options.id {
        args.insert("id".into(), id.clone());
    }
    if options.forbid_only {
        args.insert("forbidOnly".to_string(), "1".to_string());
    }

    let start_args = TestStartArgs {
        source: bundle.code.clone(),
        source_name: Some(bundle.bundle_name.clone()),
        timeout_ms: Some(options.timeout_secs * 1000),
        args: args.clone(),
    };
    let start: TestStartResponse =
        start_run(&info.ws_url, &start_args, options.cancel_active, machine)?;
    let run_id = start.run_id;
    // From here every early exit — `?`, a lost session, a local IO error —
    // must not leave the Runner holding this run.
    let active_run = ActiveRun::new(&info.ws_url, &run_id);
    if !machine {
        eprintln!(
            "{} {} · run {} started (timeout {}s)",
            "test".cyan(),
            info.target,
            run_id,
            options.timeout_secs
        );
    }

    let output_dir = if options.output_dir.is_some() {
        output_root.clone()
    } else {
        output_root.join(&run_id)
    };

    // First Ctrl-C requests a cooperative cancel; the second exits
    // immediately, still telling the Runner to drop the run.
    let interrupts = Arc::new(AtomicUsize::new(0));
    {
        let interrupts = interrupts.clone();
        let ws_url = info.ws_url.clone();
        let run_id = run_id.clone();
        ctrlc::set_handler(move || {
            if interrupts.fetch_add(1, Ordering::SeqCst) >= 1 {
                send_cancel(&ws_url, &run_id, "client_interrupt");
                std::process::exit(130);
            }
        })
        .context("failed to install Ctrl-C handler")?;
    }

    std::fs::create_dir_all(&output_dir)
        .with_context(|| format!("failed to create {}", output_dir.display()))?;
    let polled = poll_until_terminal(
        info,
        &active_run,
        &output_dir,
        machine,
        options.verbose,
        options.jsonl,
        &interrupts,
        Duration::from_secs(options.timeout_secs),
    );
    // Cancel (once) unless the host already finished the run.
    drop(active_run);
    let mut outcome = polled?;
    if !options.pass_with_no_tests
        && let Some(result) = outcome.result.as_mut()
        && result.report.as_ref().is_some_and(|r| r.total == 0)
    {
        outcome.state = TestRunState::Failed;
        result.error = Some(TestRunError {
            name: "NoTestsMatched".into(),
            message:
                "No tests matched this selection; use --pass-with-no-tests only when intentional"
                    .into(),
            stack: None,
            causes: vec![],
            detail: Default::default(),
        });
    }
    if !outcome
        .artifacts
        .iter()
        .any(|(name, _, _)| name == "report.json")
    {
        if let Some(framework) = outcome.result.as_ref().and_then(|r| r.report.as_ref()) {
            let mut value = report_value(framework, &bundle);
            value["schema_version"] = json!(1);
            value["partial"] = json!(outcome.partial);
            value["meta"] = json!({"started_at": started_at, "duration_ms": framework.duration_ms, "args": redacted_args(&args, &secret_keys)});
            value["framework"] = json!({"name":"test framework", "version":"unknown"});
            if let Some(cases) = value["cases"].as_array_mut() {
                for case in cases {
                    if case.get("id").is_none() {
                        case["id"] = case["full_name"].clone();
                    }
                    if case.get("title").is_none() {
                        case["title"] = case["name"].clone();
                    }
                    for field in ["steps", "assertions", "attachments", "covers"] {
                        if case.get(field).is_none() {
                            case[field] = json!([]);
                        }
                    }
                }
            }
            std::fs::write(
                output_dir.join("report.json"),
                serde_json::to_vec_pretty(&value)?,
            )?;
        } else {
            outcome.partial = true;
            write_partial_report(
                &output_dir,
                &run_id,
                &redacted_args(&args, &secret_keys),
                &started_at,
                outcome
                    .result
                    .as_ref()
                    .map(|r| r.duration_ms)
                    .unwrap_or_default(),
                &outcome.streamed,
            )?;
        }
    }
    if outcome.partial
        || !outcome
            .artifacts
            .iter()
            .any(|(name, _, _)| name == "report.html")
    {
        complete_client_reports(&output_dir, &run_id, &outcome)?;
    }
    scrub_secret_values(&output_dir, &secret_values(&args, &secret_keys));
    for name in ["report.json", "report.html", "junit.xml"] {
        let path = output_dir.join(name);
        if path.exists() && !outcome.artifacts.iter().any(|(n, _, _)| n == name) {
            let bytes = std::fs::metadata(&path)?.len() as usize;
            outcome.artifacts.push((name.into(), path, bytes));
        }
    }
    report(
        &outcome,
        &bundle,
        &run_id,
        &output_dir,
        &options,
        &entry,
        &info.session_id,
    );

    let exit_code = match outcome.state {
        TestRunState::Passed if !outcome.partial => 0,
        TestRunState::Cancelled if interrupts.load(Ordering::SeqCst) > 0 => 130,
        _ => 1,
    };
    std::process::exit(exit_code);
}

/// The host refuses a second run while one is active, which is what keeps a
/// dead run's actions from landing in a live fixture. But a client that dies
/// mid-run leaves that run active with nobody polling it, and it then holds the
/// session for the rest of its own budget — an hour, for a long suite. Recover
/// deliberately rather than silently: name the stuck run, and cancel it only
/// when asked to.
fn start_run(
    ws_url: &str,
    args: &TestStartArgs,
    cancel_active: bool,
    machine: bool,
) -> Result<TestStartResponse> {
    let first = execute_typed::<_, TestStartResponse>(ws_url, methods::session::test::START, args);
    let Err(error) = first else {
        return first;
    };
    let message = error.to_string();
    let Some(active) = active_run_id(&message) else {
        return Err(error);
    };
    if !cancel_active {
        return Err(anyhow!(
            "{message}\n\
             A run stays active until it ends or its budget expires, so an earlier \
             `lxdev test` whose client died still holds this session. Re-run with \
             `--cancel-active` to cancel run {active} and start, or restart the dev session."
        ));
    }
    if !machine {
        eprintln!("{} cancelling abandoned run {active}…", "test".cyan());
    }
    send_cancel(ws_url, &active, "superseded_by_new_run");
    // The host retires a cancelled run once it reaches a terminal state, so the
    // next start has to wait for that rather than race it.
    let deadline = std::time::Instant::now() + CANCEL_GRACE;
    loop {
        match execute_typed::<_, TestStartResponse>(ws_url, methods::session::test::START, args) {
            Ok(started) => return Ok(started),
            Err(retry) => {
                if active_run_id(&retry.to_string()).is_none()
                    || std::time::Instant::now() >= deadline
                {
                    return Err(retry);
                }
                std::thread::sleep(POLL_INTERVAL);
            }
        }
    }
}

/// `automation_run_in_progress: run <id> is active`
fn active_run_id(message: &str) -> Option<String> {
    let rest = message.strip_prefix("automation_run_in_progress: run ")?;
    let id = rest.split_whitespace().next()?;
    (!id.is_empty()).then(|| id.to_string())
}

const DEFAULT_OUTPUT_ROOT: &str = "test-results";

/// Create `dir` and prove a file can be written in it.
fn ensure_writable_dir(dir: &Path) -> Result<()> {
    std::fs::create_dir_all(dir)
        .with_context(|| format!("output directory {} cannot be created", dir.display()))?;
    let probe = dir.join(format!(".lxdev-write-probe-{}", std::process::id()));
    std::fs::write(&probe, b"")
        .with_context(|| format!("output directory {} is not writable", dir.display()))?;
    let _ = std::fs::remove_file(&probe);
    Ok(())
}

/// Best effort: the session may already be gone, and nothing else can be done
/// about a run we are abandoning anyway.
fn send_cancel(ws_url: &str, run_id: &str, reason: &str) -> Option<TestCancelResponse> {
    execute_typed::<_, TestCancelResponse>(
        ws_url,
        methods::session::test::CANCEL,
        &TestCancelArgs {
            run_id: run_id.to_string(),
            reason: Some(reason.to_string()),
        },
    )
    .ok()
}

/// A started run the client still owes the Runner an ending for. Dropping it
/// without [`ActiveRun::settled`] cancels the run, so every way out of the
/// poll loop — including `?` and panics — releases the session lock.
struct ActiveRun {
    ws_url: String,
    run_id: String,
    settled: std::cell::Cell<bool>,
}

impl ActiveRun {
    fn new(ws_url: &str, run_id: &str) -> Self {
        Self {
            ws_url: ws_url.to_string(),
            run_id: run_id.to_string(),
            settled: std::cell::Cell::new(false),
        }
    }

    fn run_id(&self) -> &str {
        &self.run_id
    }

    /// The host reported a terminal state; there is nothing left to cancel.
    fn settled(&self) {
        self.settled.set(true);
    }

    /// Cancel now with a specific reason; the drop will not cancel again.
    fn cancel(&self, reason: &str) {
        if !self.settled.replace(true) {
            send_cancel(&self.ws_url, &self.run_id, reason);
        }
    }
}

impl Drop for ActiveRun {
    fn drop(&mut self) {
        self.cancel("client_gave_up");
    }
}

/// `lxdev test --cancel-active` with no entry. The protocol has no "which run
/// is active" query, so ask by starting an empty probe: the host refuses it
/// with the active run's id, or — when nothing was active — the probe itself
/// starts and is retired at once.
fn cancel_active_only(info: &SessionInfo, options: &TestOptions) -> Result<()> {
    let machine = options.json || options.pretty || options.jsonl;
    let probe = TestStartArgs {
        source: "/* lxdev test --cancel-active probe */ void 0;".to_string(),
        source_name: Some("lxdev-cancel-active-probe".to_string()),
        timeout_ms: Some(1000),
        args: HashMap::new(),
    };
    let (cancelled, state) = match execute_typed::<_, TestStartResponse>(
        &info.ws_url,
        methods::session::test::START,
        &probe,
    ) {
        Ok(started) => {
            send_cancel(&info.ws_url, &started.run_id, "cancel_active_probe");
            wait_until_terminal(&info.ws_url, &started.run_id, CANCEL_GRACE);
            (None, None)
        }
        Err(error) => {
            let Some(active) = active_run_id(&error.to_string()) else {
                return Err(error);
            };
            send_cancel(&info.ws_url, &active, "cancel_active");
            let state = wait_until_terminal(&info.ws_url, &active, CANCEL_GRACE);
            (Some(active), state)
        }
    };
    if machine {
        let value = json!({
            "schema_version": 1,
            "kind": "cancel_active",
            "cancelled_run_id": cancelled,
            "state": state.map(TestRunState::as_str),
        });
        println!("{value}");
        return Ok(());
    }
    match (&cancelled, state) {
        (None, _) => eprintln!("{} no automation run was active", "test".cyan()),
        (Some(run_id), Some(state)) => {
            eprintln!(
                "{} cancelled run {run_id} ({})",
                "test".cyan(),
                state.as_str()
            )
        }
        (Some(run_id), None) => eprintln!(
            "{} cancel sent to run {run_id}; it has not stopped yet. It is released once its \
             current step returns or its budget expires.",
            "test".yellow()
        ),
    }
    Ok(())
}

/// Poll until the run reports a terminal state, or `grace` passes.
fn wait_until_terminal(ws_url: &str, run_id: &str, grace: Duration) -> Option<TestRunState> {
    let deadline = Instant::now() + grace;
    loop {
        let now = Instant::now();
        let polled = execute_poll(
            ws_url,
            &TestPollArgs {
                run_id: run_id.to_string(),
                after_seq: u64::MAX,
            },
            poll_wait(now, deadline),
            &|| false,
        );
        match polled {
            Ok(poll) if poll.state.is_terminal() => return Some(poll.state),
            // The run is gone (retired or unknown): nothing holds the lock.
            Err(err) if format!("{err:#}").contains("unknown automation run") => {
                return Some(TestRunState::Cancelled);
            }
            _ => {}
        }
        if Instant::now() >= deadline {
            return None;
        }
        std::thread::sleep(POLL_INTERVAL);
    }
}

/// Reserved arg naming the `--secret-arg` keys, so `@lingxia/test` redacts
/// them in the reports it renders.
const SECRET_ARGS_KEY: &str = "secretArgs";
const REDACTED: &str = "***";
/// Shorter values are too likely to collide with ordinary report text.
const MIN_SCRUB_LENGTH: usize = 4;

/// Mirrors `@lingxia/test`: /pass(word)?|secret|token|api[_-]?key|credential/i.
fn looks_secret(key: &str) -> bool {
    let key = key.to_ascii_lowercase();
    [
        "pass",
        "secret",
        "token",
        "apikey",
        "api_key",
        "api-key",
        "credential",
    ]
    .iter()
    .any(|needle| key.contains(needle))
}

fn secret_keys(options: &TestOptions) -> std::collections::HashSet<String> {
    options
        .args
        .iter()
        .map(|(key, _)| key)
        .filter(|key| looks_secret(key))
        .chain(options.secret_args.iter().map(|(key, _)| key))
        .cloned()
        .collect()
}

fn is_secret(key: &str, secret_keys: &std::collections::HashSet<String>) -> bool {
    key != SECRET_ARGS_KEY && (secret_keys.contains(key) || looks_secret(key))
}

fn redacted_args(
    args: &HashMap<String, String>,
    secret_keys: &std::collections::HashSet<String>,
) -> HashMap<String, String> {
    args.iter()
        .map(|(key, value)| {
            let value = if is_secret(key, secret_keys) {
                REDACTED.to_string()
            } else {
                value.clone()
            };
            (key.clone(), value)
        })
        .collect()
}

fn secret_values(
    args: &HashMap<String, String>,
    secret_keys: &std::collections::HashSet<String>,
) -> Vec<String> {
    let mut values = args
        .iter()
        .filter(|(key, value)| is_secret(key, secret_keys) && value.len() >= MIN_SCRUB_LENGTH)
        .map(|(_, value)| value.clone())
        .collect::<Vec<_>>();
    values.sort_by_key(|value| std::cmp::Reverse(value.len()));
    values
}

/// The runtime already redacts; this also covers an older `@lingxia/test`
/// and anything else that echoed a secret into a report or the journal.
fn scrub_secret_values(output_dir: &Path, values: &[String]) {
    if values.is_empty() {
        return;
    }
    let forms = values
        .iter()
        .flat_map(|value| {
            let json = serde_json::to_string(value).unwrap_or_default();
            let json = json
                .get(1..json.len().saturating_sub(1))
                .unwrap_or_default()
                .to_string();
            [value.clone(), json, escape_markup(value)]
        })
        .filter(|form| form.len() >= MIN_SCRUB_LENGTH)
        .collect::<Vec<_>>();
    for name in ["report.json", "report.html", "junit.xml", "events.jsonl"] {
        let path = output_dir.join(name);
        let Ok(text) = std::fs::read_to_string(&path) else {
            continue;
        };
        let mut scrubbed = text.clone();
        for form in &forms {
            if scrubbed.contains(form.as_str()) {
                scrubbed = scrubbed.replace(form.as_str(), REDACTED);
            }
        }
        if scrubbed != text {
            let _ = std::fs::write(&path, scrubbed);
        }
    }
}

fn warn_package_version(entry: &Path, machine: bool) {
    let root = find_project_root(entry);
    let package_json = root
        .join("node_modules")
        .join("@lingxia")
        .join("test")
        .join("package.json");
    let Ok(text) = std::fs::read_to_string(&package_json) else {
        return;
    };
    let Ok(value) = serde_json::from_str::<serde_json::Value>(&text) else {
        return;
    };
    let Some(version) = value.get("version").and_then(|item| item.as_str()) else {
        return;
    };
    if version == env!("CARGO_PKG_VERSION") {
        return;
    }
    if !machine {
        eprintln!(
            "{} @lingxia/test@{version} does not match lxdev {}; use matching package and CLI versions.",
            "warning".yellow(),
            env!("CARGO_PKG_VERSION")
        );
    }
}

struct StreamedCase {
    record: serde_json::Value,
    name: String,
    full_name: String,
    status: Option<TestCaseStatus>,
    duration_ms: u64,
    covers: Vec<String>,
    steps: Vec<serde_json::Value>,
}

struct Outcome {
    state: TestRunState,
    result: Option<TestRunResult>,
    console: Vec<(String, String)>,
    artifacts: Vec<(String, PathBuf, usize)>,
    partial: bool,
    streamed: Vec<StreamedCase>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PollDeadlineStop {
    Cancelled,
    RunDeadline,
    Watchdog,
}

/// A quiet poll is not a reason to stop. These are: the user already asked to
/// cancel and the grace has passed, the run budget is over, or the hang
/// watchdog has seen no event. Cancel grace wins, then the run budget, then
/// the watchdog.
fn poll_deadline_stop(
    now: Instant,
    cancel_deadline: Option<Instant>,
    poll_deadline: Instant,
    watchdog_at: Instant,
) -> Option<PollDeadlineStop> {
    if cancel_deadline.is_some_and(|deadline| now > deadline) {
        return Some(PollDeadlineStop::Cancelled);
    }
    if now > poll_deadline {
        return Some(PollDeadlineStop::RunDeadline);
    }
    if now > watchdog_at {
        return Some(PollDeadlineStop::Watchdog);
    }
    None
}

fn soonest_poll_deadline(
    cancel_deadline: Option<Instant>,
    poll_deadline: Instant,
    watchdog_at: Instant,
) -> Instant {
    let until = poll_deadline.min(watchdog_at);
    cancel_deadline.map_or(until, |deadline| until.min(deadline))
}

/// One status read waits until the next deadline, capped so it returns before
/// the dev server abandons the forward and a second connection piles up
/// behind that lock.
fn poll_wait(now: Instant, until: Instant) -> Duration {
    until
        .saturating_duration_since(now)
        .min(max_poll_wait())
        .max(Duration::from_millis(1))
}

fn execute_poll(
    ws_url: &str,
    args: &TestPollArgs,
    timeout: Duration,
    stop: &dyn Fn() -> bool,
) -> Result<TestPollResponse> {
    let encoded = serde_json::to_value(args).context("failed to encode devtool command args")?;
    let response = execute_command_until(
        ws_url,
        methods::session::test::POLL,
        Some(encoded),
        Some(timeout),
        stop,
    )?
    .ok_or_else(|| anyhow!("session.test.poll returned no data"))?;
    serde_json::from_value(response).context("invalid session.test.poll response")
}

fn finish_poll_deadline(
    stop: PollDeadlineStop,
    run: &ActiveRun,
    elapsed: Duration,
    console: Vec<(String, String)>,
    artifacts: Vec<(String, PathBuf, usize)>,
    streamed: Vec<StreamedCase>,
) -> Outcome {
    let (state, message, reason) = match stop {
        PollDeadlineStop::Cancelled => (
            TestRunState::Cancelled,
            "Cancellation deadline exceeded",
            None,
        ),
        PollDeadlineStop::RunDeadline => (
            TestRunState::TimedOut,
            "Run deadline exceeded",
            Some("run_deadline"),
        ),
        PollDeadlineStop::Watchdog => (
            TestRunState::TimedOut,
            "No test event before the lxdev hang watchdog fired",
            Some("hang_watchdog"),
        ),
    };
    // Leaving a run active here is how the next `lxdev test` gets stranded
    // on automation_run_in_progress. Cancel was already sent for the grace path.
    if let Some(reason) = reason {
        run.cancel(reason);
    }
    interrupted_outcome(
        state,
        message.to_string(),
        elapsed,
        console,
        artifacts,
        streamed,
    )
}

#[allow(clippy::too_many_arguments)]
fn poll_until_terminal(
    info: &SessionInfo,
    run: &ActiveRun,
    output_dir: &Path,
    machine: bool,
    verbose: bool,
    jsonl: bool,
    interrupts: &AtomicUsize,
    run_timeout: Duration,
) -> Result<Outcome> {
    let run_id = run.run_id();
    let run_started = std::time::Instant::now();
    let mut journal = std::fs::File::create(output_dir.join("events.jsonl"))?;
    let mut total = None;
    let mut poll_failures = 0u8;
    let mut after_seq = 0u64;
    let mut console = Vec::new();
    let mut artifacts = Vec::new();
    let mut streamed = Vec::new();
    let mut cancel_sent = false;
    let mut cancel_deadline: Option<std::time::Instant> = None;
    let mut last_event_at = std::time::Instant::now();
    let mut case_budget = run_timeout;
    // The runtime enforces the deadline; this client-side bound only guards
    // against a vanished session.
    let poll_deadline = std::time::Instant::now() + run_timeout + Duration::from_secs(30);

    loop {
        if interrupts.load(Ordering::SeqCst) > 0 && !cancel_sent {
            cancel_sent = true;
            cancel_deadline = Some(std::time::Instant::now() + CANCEL_GRACE);
            if !machine {
                eprintln!("{} cancelling run {run_id}…", "test".cyan());
            }
            run.cancel("client_interrupt");
        }

        let watchdog_at = last_event_at + case_budget + WATCHDOG_GRACE;
        let now = Instant::now();
        if let Some(stop) = poll_deadline_stop(now, cancel_deadline, poll_deadline, watchdog_at) {
            return Ok(finish_poll_deadline(
                stop,
                run,
                run_started.elapsed(),
                console,
                artifacts,
                streamed,
            ));
        }

        let polled = execute_poll(
            &info.ws_url,
            &TestPollArgs {
                run_id: run_id.to_string(),
                after_seq,
            },
            poll_wait(
                now,
                soonest_poll_deadline(cancel_deadline, poll_deadline, watchdog_at),
            ),
            &|| interrupts.load(Ordering::SeqCst) > 0 && !cancel_sent,
        );
        let poll = match polled {
            Ok(poll) => {
                poll_failures = 0;
                poll
            }
            Err(err) => {
                /* A poll that times out has not told us the session is gone:
                 * the runtime answers it between spec steps, so a spec doing
                 * real network work keeps it quiet for as long as that work
                 * takes. Only a transport error counts toward giving up. A
                 * quiet poll that has reached a deadline stops as that
                 * deadline — timeout or cancellation — and cancels the run,
                 * so the next `lxdev test` is not stranded on the lock. */
                let quiet = err.downcast_ref::<CommandTimeout>().is_some();
                if !quiet {
                    poll_failures += 1;
                }
                let now = Instant::now();
                let watchdog_at = last_event_at + case_budget + WATCHDOG_GRACE;
                if let Some(stop) =
                    poll_deadline_stop(now, cancel_deadline, poll_deadline, watchdog_at)
                {
                    return Ok(finish_poll_deadline(
                        stop,
                        run,
                        run_started.elapsed(),
                        console,
                        artifacts,
                        streamed,
                    ));
                }
                if quiet || poll_failures < 3 {
                    if !quiet {
                        if !machine && verbose {
                            eprintln!("Retrying test status request: {err:#}");
                        }
                        std::thread::sleep(POLL_INTERVAL);
                    }
                    continue;
                }
                return Ok(interrupted_outcome(
                    TestRunState::InternalError,
                    format!("Lost test session: {err:#}"),
                    run_started.elapsed(),
                    console,
                    artifacts,
                    streamed,
                ));
            }
        };

        for event in &poll.events {
            let mut logged = serde_json::to_value(event)?;
            logged["schema_version"] = json!(1);
            logged["run_id"] = json!(run_id);
            writeln!(journal, "{logged}")?;
            journal.flush()?;
            if jsonl {
                println!("{logged}");
            }
            after_seq = after_seq.max(event.seq);
            last_event_at = std::time::Instant::now();
            match &event.payload {
                TestEventPayload::RunStarted {
                    total: count,
                    cases,
                } => {
                    total = Some(*count);
                    for record in cases {
                        streamed.push(StreamedCase {
                            name: record["name"].as_str().unwrap_or_default().into(),
                            full_name: record["full_name"].as_str().unwrap_or_default().into(),
                            status: None,
                            duration_ms: 0,
                            covers: vec![],
                            steps: vec![],
                            record: record.clone(),
                        });
                    }
                    if !machine {
                        eprintln!("Running {count} tests");
                    }
                }
                TestEventPayload::Diagnostic { phase, message } => {
                    if !machine {
                        eprintln!("warning ({phase}): {message}");
                    }
                }
                TestEventPayload::Console { level, message } => {
                    if !machine && (verbose || level == "error" || level == "warn") {
                        print_console(level, message);
                    }
                    console.push((level.clone(), message.clone()));
                }
                TestEventPayload::Artifact {
                    name,
                    mime_type,
                    base64,
                } => {
                    let (path, bytes) = match write_artifact(output_dir, name, base64) {
                        Ok(artifact) => artifact,
                        Err(err) => {
                            return Ok(interrupted_outcome(
                                TestRunState::InternalError,
                                format!("Cannot save artifact {name}: {err:#}"),
                                run_started.elapsed(),
                                console,
                                artifacts,
                                streamed,
                            ));
                        }
                    };
                    if !machine && verbose {
                        eprintln!(
                            "{} artifact {} → {} ({mime_type}, {})",
                            "test".cyan(),
                            name,
                            path.display(),
                            human_bytes(bytes)
                        );
                    }
                    artifacts.push((name.clone(), path, bytes));
                }
                TestEventPayload::CaseStarted {
                    name,
                    full_name,
                    timeout_ms,
                    watchdog_timeout_ms,
                    covers,
                    id,
                    file,
                    line,
                } => {
                    case_budget = watchdog_timeout_ms
                        .map(Duration::from_millis)
                        .unwrap_or_else(|| {
                            timeout_ms
                                .map(Duration::from_millis)
                                .unwrap_or(DEFAULT_CASE_TIMEOUT)
                                + Duration::from_secs(27)
                        });
                    let index = streamed.iter().position(|case| {
                        id.as_ref()
                            .map(|id| case.record["id"].as_str() == Some(id))
                            .unwrap_or(case.full_name == *full_name)
                    });
                    let record = json!({ "id": id.as_deref().unwrap_or(full_name), "file": file, "line": line, "started": true });
                    let case = StreamedCase {
                        record,
                        name: name.clone(),
                        full_name: full_name.clone(),
                        status: None,
                        duration_ms: 0,
                        covers: covers.clone(),
                        steps: Vec::new(),
                    };
                    if let Some(index) = index {
                        streamed[index] = case;
                    } else {
                        streamed.push(case);
                    }
                    if !machine {
                        eprintln!(
                            "→ [{}/{}] {full_name}",
                            streamed.iter().filter(|c| c.status.is_some()).count() + 1,
                            total.map(|n| n.to_string()).unwrap_or_else(|| "?".into())
                        );
                    }
                }
                TestEventPayload::CaseFinished {
                    name,
                    full_name,
                    status,
                    duration_ms,
                    error,
                    record,
                } => {
                    if let Some(current) = streamed
                        .iter_mut()
                        .find(|c| c.status.is_none() && c.full_name == *full_name)
                    {
                        current.status = Some(*status);
                        current.duration_ms = *duration_ms;
                        if let Some(record) = record {
                            current.record = record.clone();
                        } else if let Some(error) = error {
                            current.record["error"] = serde_json::to_value(error)?;
                        }
                    }
                    if !machine {
                        if record.as_ref().is_some_and(|r| r["flaky"] == true) {
                            eprintln!(
                                "{} {full_name} ({:.2}s)",
                                "⚠ flaky".yellow(),
                                *duration_ms as f64 / 1000.0
                            );
                        } else {
                            let reason = record
                                .as_ref()
                                .and_then(|r| r["reason"].as_str())
                                .filter(|_| *status == TestCaseStatus::Skipped);
                            print_case_finished(
                                name,
                                full_name,
                                *status,
                                *duration_ms,
                                None,
                                reason,
                            );
                        }
                    }
                }

                TestEventPayload::StepStarted { name, path } => {
                    if !machine && verbose {
                        eprintln!("{} {path}", "▸".dimmed());
                    }
                    if let Some(current) = streamed
                        .iter_mut()
                        .find(|c| c.status.is_none() && c.record["started"] == true)
                    {
                        current.steps.push(json!({
                            "name": name,
                            "path": path,
                            "status": "running",
                        }));
                    }
                }
                TestEventPayload::StepFinished {
                    name,
                    path,
                    status,
                    duration_ms,
                    error,
                } => {
                    if !machine && verbose {
                        eprintln!("  {status} {path} ({:.2}s)", *duration_ms as f64 / 1000.0);
                    }
                    if let Some(current) = streamed
                        .iter_mut()
                        .find(|c| c.status.is_none() && c.record["started"] == true)
                    {
                        let value = json!({"name": name, "path": path, "status": status, "duration_ms": duration_ms, "error": error});
                        if let Some(step) = current
                            .steps
                            .iter_mut()
                            .rev()
                            .find(|s| s["path"] == *path && s["status"] == "running")
                        {
                            *step = value;
                        } else {
                            current.steps.push(value);
                        }
                    }
                }
            }
        }
        let events_drained = after_seq.saturating_add(1) >= poll.next_seq;
        if poll.state.is_terminal() && events_drained {
            run.settled();
            return Ok(Outcome {
                state: poll.state,
                result: poll.result.clone(),
                console,
                artifacts,
                partial: poll
                    .result
                    .as_ref()
                    .and_then(|r| r.report.as_ref())
                    .is_none_or(|r| r.detail.get("partial") == Some(&json!(true))),
                streamed,
            });
        }
        let watchdog_at = last_event_at + case_budget + WATCHDOG_GRACE;
        if let Some(stop) =
            poll_deadline_stop(Instant::now(), cancel_deadline, poll_deadline, watchdog_at)
        {
            return Ok(finish_poll_deadline(
                stop,
                run,
                run_started.elapsed(),
                console,
                artifacts,
                streamed,
            ));
        }
        std::thread::sleep(POLL_INTERVAL);
    }
}

fn report(
    outcome: &Outcome,
    bundle: &TestBundle,
    run_id: &str,
    output_dir: &Path,
    options: &TestOptions,
    entry: &Path,
    session_id: &str,
) {
    let machine = options.json || options.pretty || options.jsonl;
    let duration_ms = outcome
        .result
        .as_ref()
        .map(|result| result.duration_ms)
        .unwrap_or_default();
    let mapped_error = outcome.result.as_ref().and_then(|result| {
        result.error.as_ref().map(|error| {
            let (stack, primary) = match &error.stack {
                Some(stack) => {
                    let (mapped, primary) = bundle.remap_stack(stack);
                    (Some(mapped), primary)
                }
                None => (None, None),
            };
            (error, stack, primary)
        })
    });

    if machine {
        let error_json = mapped_error.as_ref().map(|(error, stack, primary)| {
            mapped_error_value(error, stack, primary.as_ref(), bundle)
        });
        let framework_report = std::fs::read(output_dir.join("report.json"))
            .ok()
            .and_then(|bytes| serde_json::from_slice::<serde_json::Value>(&bytes).ok())
            .or_else(|| {
                outcome
                    .result
                    .as_ref()
                    .and_then(|r| r.report.as_ref())
                    .map(|r| report_value(r, bundle))
            });
        let envelope = json!({
            "schema_version": 1,
            "kind": "result",
            "partial": outcome.partial,
            "run_id": run_id,
            "state": outcome.state.as_str(),
            "duration_ms": duration_ms,
            "error": error_json,
            "report": framework_report,
            "console": outcome
                .console
                .iter()
                .map(|(level, message)| json!({ "level": level, "message": message }))
                .collect::<Vec<_>>(),
            "artifacts": outcome
                .artifacts
                .iter()
                .map(|(name, path, bytes)| {
                    json!({ "name": name, "path": path.display().to_string(), "bytes": bytes })
                })
                .collect::<Vec<_>>(),
            "output_dir": output_dir.display().to_string(),
        });
        let encoded = if options.pretty {
            serde_json::to_string_pretty(&envelope)
        } else {
            serde_json::to_string(&envelope)
        };
        println!("{}", encoded.unwrap_or_else(|_| envelope.to_string()));
        return;
    }

    let seconds = duration_ms as f64 / 1000.0;
    match outcome.state {
        TestRunState::Passed => eprintln!("{} in {seconds:.1}s", "✓ passed".green().bold()),
        TestRunState::Failed => eprintln!("{} in {seconds:.1}s", "✗ failed".red().bold()),
        TestRunState::TimedOut => {
            eprintln!("{} after {seconds:.1}s", "✗ timed out".red().bold())
        }
        TestRunState::Cancelled => {
            eprintln!("{} after {seconds:.1}s", "✗ cancelled".yellow().bold())
        }
        other => eprintln!("{} {} in {seconds:.1}s", "✗".red().bold(), other.as_str()),
    }
    if let Some((error, stack, _)) = &mapped_error {
        eprintln!("{}: {}", error.name.red(), error.message);
        if let Some(stack) = stack {
            for line in stack.lines() {
                eprintln!("    {line}");
            }
        }
        print_error_causes(&error.causes, bundle, 1);
    }
    if let Some(framework_report) = outcome
        .result
        .as_ref()
        .and_then(|result| result.report.as_ref())
    {
        eprintln!(
            "{} passed, {} failed, {} skipped, {} timeout, {} xfail, {} xpass ({} cases, {:.1}s)",
            framework_report.passed,
            framework_report.failed,
            framework_report.skipped,
            framework_report.timeout,
            framework_report.xfail,
            framework_report.xpass,
            framework_report.total,
            framework_report.duration_ms as f64 / 1000.0
        );
        let flaky = framework_report
            .cases
            .iter()
            .filter(|c| c.detail.get("flaky") == Some(&json!(true)))
            .count();
        if flaky > 0 {
            eprintln!(
                "{}",
                format!("{flaky} flaky: passed only after retry").yellow()
            );
        }
        for case in &framework_report.cases {
            if !case.status.is_failure() {
                continue;
            }
            let Some(error) = &case.error else {
                continue;
            };
            eprintln!(
                "\n{} [{}]: {}",
                case.full_name.red(),
                case.status.as_str(),
                error.message
            );
            for field in ["code", "phase", "step", "location", "expected", "actual"] {
                if let Some(value) = error.detail.get(field).and_then(|v| v.as_str()) {
                    match field {
                        "expected" => eprintln!("  expected: {}", value.green()),
                        "actual" => eprintln!("    actual: {}", value.red()),
                        _ => eprintln!("  {field}: {value}"),
                    }
                }
            }
            if let Some(location) = error
                .detail
                .get("location")
                .and_then(|v| v.as_str())
                .and_then(parse_source_location)
            {
                print_code_frame(&location, bundle);
            } else if let (Some(source), Some(line)) = (
                case.detail.get("file").and_then(|v| v.as_str()),
                case.detail.get("line").and_then(|v| v.as_u64()),
            ) {
                print_code_frame(
                    &MappedPosition {
                        source: source.into(),
                        line: line as u32,
                        column: 1,
                    },
                    bundle,
                );
            }
            if let Some(id) = case.detail.get("id").and_then(|v| v.as_str()) {
                let mut command = format!(
                    "lxdev --session {} test {} --id {} --timeout-secs {}",
                    shell_quote(session_id),
                    shell_quote(&entry.to_string_lossy()),
                    shell_quote(id),
                    options.timeout_secs
                );
                command.push_str(&rerun_args(options));
                eprintln!("  Rerun: {command}");
            }
            if options.verbose
                && let Some(stack) = &error.stack
            {
                let (mapped, _) = bundle.remap_stack(stack);
                for line in mapped.lines() {
                    eprintln!("    {line}");
                }
            }
            print_error_causes(&error.causes, bundle, 1);
        }
    }
    if outcome.partial {
        eprintln!(
            "Incomplete run; remaining cases were not executed. See report.json for preserved results."
        );
    }
    print_artifact_index(output_dir, &outcome.artifacts);
}

/// The report is the deliverable, so name it last and name it absolutely —
/// a run-scoped directory is otherwise hard to find in scrollback.
fn print_artifact_index(output_dir: &Path, artifacts: &[(String, PathBuf, usize)]) {
    let mut named = artifacts
        .iter()
        .filter(|(name, _, _)| matches!(name.as_str(), "report.html" | "report.json" | "junit.xml"))
        .collect::<Vec<_>>();
    if named.is_empty() {
        return;
    }
    named.sort_by_key(|(name, _, _)| match name.as_str() {
        "report.html" => 0,
        "report.json" => 1,
        _ => 2,
    });
    let absolute = |path: &Path| {
        std::fs::canonicalize(path).unwrap_or_else(|_| {
            std::env::current_dir()
                .map(|cwd| cwd.join(path))
                .unwrap_or_else(|_| path.to_path_buf())
        })
    };
    for (name, path, _) in named {
        eprintln!(
            "{} {}",
            format!("{name:>11}").cyan(),
            absolute(path).display()
        );
    }
    let others = artifacts
        .iter()
        .filter(|(name, _, _)| {
            !matches!(name.as_str(), "report.html" | "report.json" | "junit.xml")
        })
        .count();
    if others > 0 {
        eprintln!(
            "{} {} more artifact(s) under {}",
            "artifacts".cyan(),
            others,
            absolute(output_dir).display()
        );
    }
}

fn report_value(report: &TestReport, bundle: &TestBundle) -> serde_json::Value {
    let mut value = serde_json::to_value(report).unwrap_or_default();
    if let Some(cases) = value["cases"].as_array_mut() {
        for (case, source) in cases.iter_mut().zip(&report.cases) {
            if let Some(error) = &source.error {
                let (stack, primary) = error
                    .stack
                    .as_ref()
                    .map(|s| bundle.remap_stack(s))
                    .unwrap_or_default();
                case["error"] = mapped_error_value(error, &Some(stack), primary.as_ref(), bundle);
            }
        }
    }
    value
}

fn mapped_error_value(
    error: &TestRunError,
    stack: &Option<String>,
    primary: Option<&MappedPosition>,
    bundle: &TestBundle,
) -> serde_json::Value {
    let mut value = json!({
        "name": error.name,
        "message": error.message,
        "stack": stack,
        "source": primary.map(|position| position.source.clone()),
        "line": primary.map(|position| position.line),
        "column": primary.map(|position| position.column),
        "causes": error.causes.iter().map(|cause| {
            let (stack, primary) = match &cause.stack {
                Some(stack) => {
                    let (mapped, primary) = bundle.remap_stack(stack);
                    (Some(mapped), primary)
                }
                None => (None, None),
            };
            mapped_error_value(cause, &stack, primary.as_ref(), bundle)
        }).collect::<Vec<_>>(),
    });
    if let Some(object) = value.as_object_mut() {
        object.extend(error.detail.clone());
    }
    value
}

fn print_error_causes(causes: &[TestRunError], bundle: &TestBundle, depth: usize) {
    let indent = "  ".repeat(depth);
    for cause in causes {
        eprintln!("{indent}caused by {}: {}", cause.name.red(), cause.message);
        if let Some(stack) = &cause.stack {
            let (mapped, _) = bundle.remap_stack(stack);
            for line in mapped.lines() {
                eprintln!("{indent}  {line}");
            }
        }
        print_error_causes(&cause.causes, bundle, depth + 1);
    }
}

fn print_case_finished(
    name: &str,
    full_name: &str,
    status: TestCaseStatus,
    duration_ms: u64,
    error: Option<&TestRunError>,
    reason: Option<&str>,
) {
    let display_name = if full_name.is_empty() {
        name
    } else {
        full_name
    };
    let seconds = duration_ms as f64 / 1000.0;
    match status {
        TestCaseStatus::Passed | TestCaseStatus::Xfail => {
            eprintln!(
                "{} {display_name} ({seconds:.2}s)",
                format!("✓ {}", status.as_str()).green()
            )
        }
        TestCaseStatus::Skipped => match reason {
            Some(reason) => eprintln!(
                "{} {display_name} {}",
                "- skipped".yellow(),
                format!("({reason})").dimmed()
            ),
            None => eprintln!("{} {display_name}", "-".yellow()),
        },
        TestCaseStatus::Failed | TestCaseStatus::Timeout | TestCaseStatus::Xpass => {
            eprintln!(
                "{} {display_name} ({seconds:.2}s)",
                format!("✗ {}", status.as_str()).red()
            );
            if let Some(error) = error {
                eprintln!("  {}: {}", error.name.red(), error.message);
            }
        }
    }
}

/// `--arg`/`--secret-arg` flags for a rerun hint. Terminal scrollback is a
/// report too, so secret values stay masked; the caller re-supplies them.
fn rerun_args(options: &TestOptions) -> String {
    let explicit = options
        .secret_args
        .iter()
        .map(|(key, _)| key.as_str())
        .collect::<std::collections::HashSet<_>>();
    let mut out = String::new();
    for (key, value) in &options.args {
        let value = if looks_secret(key) || explicit.contains(key.as_str()) {
            REDACTED
        } else {
            value.as_str()
        };
        out.push_str(&format!(
            " --arg {}",
            shell_quote(&format!("{key}={value}"))
        ));
    }
    for (key, _) in &options.secret_args {
        out.push_str(&format!(
            " --secret-arg {}",
            shell_quote(&format!("{key}={REDACTED}"))
        ));
    }
    out
}

fn print_console(level: &str, message: &str) {
    let tag = match level {
        "error" => format!("[{}]", "error".red()),
        "warn" => format!("[{}]", "warn".yellow()),
        "debug" => format!("[{}]", "debug".dimmed()),
        _ => format!("[{}]", level.dimmed()),
    };
    eprintln!("{tag} {message}");
}

/// A hung run never reaches the in-runtime reporter, so the client writes the
/// report itself. It keeps `@lingxia/test`'s `report.json` shape so the same
/// consumers parse both paths; only `partial` tells them apart.
fn write_partial_report(
    output_dir: &Path,
    run_id: &str,
    args: &HashMap<String, String>,
    started_at: &str,
    duration_ms: u64,
    streamed: &[StreamedCase],
) -> Result<()> {
    std::fs::create_dir_all(output_dir)
        .with_context(|| format!("failed to create {}", output_dir.display()))?;
    let cases = streamed
        .iter()
        .map(|case| {
            // A case with no finish event was still running when the watchdog
            // fired; the report grades it as the timeout it is.
            let status = case.status.map(TestCaseStatus::as_str).unwrap_or(
                if case.record["started"] == true {
                    "timeout"
                } else {
                    "skipped"
                },
            );
            let mut value = json!({
                "id": case.full_name,
                "title": case.name,
                "name": case.name,
                "full_name": case.full_name,
                "suite": "lxdev (partial run)",
                "status": status,
                "duration_ms": case.duration_ms,
                "covers": case.covers,
                "steps": case.steps,
                "assertions": [],
                "attachments": [],
                "timeout_ms": 0,
                "error": (status == "timeout").then(|| json!({
                    "name": "TimeoutError",
                    "message": "no test event before the lxdev hang watchdog fired",
                })),
            });
            if let (Some(target), Some(record)) = (value.as_object_mut(), case.record.as_object()) {
                target.extend(record.clone());
            }
            value
        })
        .collect::<Vec<_>>();
    let count = |wanted: &str| {
        cases
            .iter()
            .filter(|case| case.get("status").and_then(|value| value.as_str()) == Some(wanted))
            .count()
    };
    let envelope = json!({
        "schema_version": 1,
        "framework": { "name": "lxdev", "version": env!("CARGO_PKG_VERSION") },
        "meta": {
            "started_at": started_at,
            "duration_ms": duration_ms,
            "args": args,
            "platform": args.get("platform"),
            "framework": args.get("framework"),
            "run_id": run_id,
        },
        "partial": true,
        "filtered": (["grep", "id", "ids", "shard"].iter().any(|key| args.contains_key(*key))),
        "run_id": run_id,
        "total": cases.len(),
        "passed": count("passed"),
        "failed": count("failed"),
        "skipped": count("skipped"),
        "xfail": count("xfail"),
        "xpass": count("xpass"),
        "timeout": count("timeout"),
        "duration_ms": duration_ms,
        "cases": cases,
    });
    let path = output_dir.join("report.json");
    std::fs::write(&path, serde_json::to_vec_pretty(&envelope)?)
        .with_context(|| format!("failed to write {}", path.display()))?;
    Ok(())
}

/// The runtime already validated the name; re-validate before touching the
/// filesystem so an older or hostile host cannot escape the output directory.
fn write_artifact(output_dir: &Path, name: &str, base64: &str) -> Result<(PathBuf, usize)> {
    let normalized = name.replace('\\', "/");
    if normalized.starts_with('/') || normalized.contains(':') {
        bail!("artifact name {name:?} must be a relative path");
    }
    for segment in normalized.split('/') {
        if segment.is_empty() || segment == "." || segment == ".." {
            bail!("artifact name {name:?} contains an invalid path segment");
        }
    }
    if base64.len() > MAX_ARTIFACT_BASE64_BYTES {
        bail!("artifact {name:?} exceeds the {MAX_ARTIFACT_BYTES}-byte limit");
    }
    let path = output_dir.join(&normalized);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }
    let bytes = decode_artifact(name, base64, MAX_ARTIFACT_BYTES)?;
    let len = bytes.len();
    std::fs::write(&path, bytes).with_context(|| format!("failed to write {}", path.display()))?;
    Ok((path, len))
}

fn decode_artifact(name: &str, base64: &str, max_bytes: usize) -> Result<Vec<u8>> {
    let bytes = BASE64
        .decode(base64.as_bytes())
        .with_context(|| format!("artifact {name:?} carries invalid base64"))?;
    if bytes.len() > max_bytes {
        bail!("artifact {name:?} exceeds the {max_bytes}-byte limit");
    }
    Ok(bytes)
}

fn human_bytes(len: usize) -> String {
    if len >= 1024 * 1024 {
        format!("{:.1} MiB", len as f64 / (1024.0 * 1024.0))
    } else if len >= 1024 {
        format!("{:.1} KiB", len as f64 / 1024.0)
    } else {
        format!("{len} B")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recognises_an_active_run_and_ignores_other_failures() {
        assert_eq!(
            active_run_id("automation_run_in_progress: run 6c72fda1-58ed-4f9c is active")
                .as_deref(),
            Some("6c72fda1-58ed-4f9c")
        );
        assert!(active_run_id("automation_runtime_unhealthy: restart the host").is_none());
        assert!(active_run_id("").is_none());
    }

    #[test]
    fn a_quiet_poll_stops_as_the_deadline_it_reached() {
        let now = Instant::now();
        let later = now + Duration::from_secs(30);
        assert_eq!(
            poll_deadline_stop(now, Some(now - Duration::from_millis(1)), later, later),
            Some(PollDeadlineStop::Cancelled)
        );
        assert_eq!(
            poll_deadline_stop(now, None, now - Duration::from_millis(1), later),
            Some(PollDeadlineStop::RunDeadline)
        );
        assert_eq!(
            poll_deadline_stop(now, None, later, now - Duration::from_millis(1)),
            Some(PollDeadlineStop::Watchdog)
        );
        assert!(poll_deadline_stop(now, None, later, later).is_none());
    }

    #[test]
    fn a_poll_waits_until_the_next_deadline_on_one_socket() {
        let now = Instant::now();
        let soon = now + Duration::from_secs(20);
        let later = now + Duration::from_secs(600);
        assert_eq!(
            soonest_poll_deadline(None, later, soon).saturating_duration_since(now),
            Duration::from_secs(20)
        );
        assert_eq!(poll_wait(now, later), max_poll_wait());
        assert!(poll_wait(now, soon) < max_poll_wait());
    }

    #[test]
    fn a_budget_over_the_runtime_ceiling_names_the_way_out() {
        let error = parse_timeout_secs("5400").unwrap_err();

        // The old message only printed the range, which left the caller of a
        // long suite with nowhere to go.
        assert!(error.contains("--shard"), "{error}");
        assert_eq!(parse_timeout_secs("3600").unwrap(), 3600);
        assert!(parse_timeout_secs("0").is_err());
    }

    #[test]
    fn artifact_size_uses_decoded_bytes() {
        let output = tempfile::tempdir().unwrap();
        let (path, len) = write_artifact(output.path(), "nested/a.txt", "aGk=").unwrap();

        assert_eq!(len, 2);
        assert_eq!(std::fs::read(path).unwrap(), b"hi");
    }

    #[test]
    fn artifact_path_cannot_escape_output_directory() {
        let output = tempfile::tempdir().unwrap();

        assert!(write_artifact(output.path(), "../a.txt", "aGk=").is_err());
        assert!(write_artifact(output.path(), "/a.txt", "aGk=").is_err());
    }

    #[test]
    fn artifact_decoded_size_is_revalidated() {
        assert!(decode_artifact("a.bin", "AAAA", 2).is_err());
    }

    #[test]
    fn no_session_hint_points_at_background_dev() {
        assert!(NO_SESSION_HINT.contains("lingxia dev --background"));
        assert!(looks_unreachable(&anyhow!(
            "No live dev session found. Run `lingxia dev` first."
        )));
        assert!(looks_unreachable(&anyhow!("WebSocket handshake failed")));
        assert!(!looks_unreachable(&anyhow!("duplicate spec id")));
    }

    #[test]
    fn partial_report_preserves_json_fields() {
        let dir = tempfile::tempdir().unwrap();
        write_partial_report(
            dir.path(),
            "run-1",
            &HashMap::new(),
            "2026-01-01T00:00:00Z",
            1234,
            &[StreamedCase {
                record: json!({}),
                name: "home".into(),
                full_name: "home".into(),
                status: Some(TestCaseStatus::Passed),
                duration_ms: 10,
                covers: vec!["lx.host".into()],
                steps: vec![json!({ "name": "greet", "path": "greet", "status": "passed" })],
            }],
        )
        .unwrap();
        let text = std::fs::read_to_string(dir.path().join("report.json")).unwrap();
        let value: serde_json::Value = serde_json::from_str(&text).unwrap();
        assert_eq!(value["partial"], serde_json::json!(true));
        assert_eq!(value["total"], serde_json::json!(1));
        assert_eq!(value["passed"], serde_json::json!(1));
        assert_eq!(value["cases"][0]["covers"][0], serde_json::json!("lx.host"));
        // Same shape as the in-runtime reporter so one parser reads both paths.
        for key in [
            "meta",
            "filtered",
            "failed",
            "skipped",
            "timeout",
            "duration_ms",
        ] {
            assert!(value.get(key).is_some(), "partial report is missing {key}");
        }
        assert!(!dir.path().join("report.html").exists());
    }
}

fn interrupted_outcome(
    state: TestRunState,
    message: String,
    elapsed: Duration,
    console: Vec<(String, String)>,
    artifacts: Vec<(String, PathBuf, usize)>,
    streamed: Vec<StreamedCase>,
) -> Outcome {
    Outcome {
        state,
        result: Some(TestRunResult {
            duration_ms: elapsed.as_millis() as u64,
            error: Some(TestRunError {
                name: "TestRunInterrupted".into(),
                message,
                stack: None,
                causes: vec![],
                detail: Default::default(),
            }),
            report: None,
        }),
        console,
        artifacts,
        streamed,
        partial: true,
    }
}

fn complete_client_reports(output: &Path, run_id: &str, outcome: &Outcome) -> Result<()> {
    let path = output.join("report.json");
    let mut report: serde_json::Value = serde_json::from_slice(&std::fs::read(&path)?)?;
    report["schema_version"] = json!(1);
    report["partial"] = json!(outcome.partial);
    report["state"] = json!(outcome.state.as_str());
    report["run_id"] = json!(run_id);
    report["error"] = serde_json::to_value(outcome.result.as_ref().and_then(|r| r.error.as_ref()))?;
    let message = report["error"]["message"]
        .as_str()
        .unwrap_or(if outcome.partial {
            "The test run did not finish"
        } else {
            "Run completed"
        })
        .to_owned();
    if let Some(cases) = report["cases"].as_array_mut() {
        for (case, streamed) in cases.iter_mut().zip(&outcome.streamed) {
            if outcome.partial && streamed.status.is_none() {
                if streamed.record["started"] == true {
                    let status = match outcome.state {
                        TestRunState::Cancelled => "skipped",
                        TestRunState::TimedOut => "timeout",
                        _ => "failed",
                    };
                    case["status"] = json!(status);
                    case["reason"] = json!(message);
                    case["error"] = if status == "skipped" {
                        serde_json::Value::Null
                    } else {
                        json!({"name":"TestRunInterrupted","message":message})
                    };
                } else {
                    case["reason"] = json!("Not run: the test run was interrupted");
                }
            }
        }
    }
    let cases = report["cases"].as_array().cloned().unwrap_or_default();
    for status in ["passed", "failed", "skipped", "timeout", "xfail", "xpass"] {
        report[status] = json!(cases.iter().filter(|c| c["status"] == status).count());
    }
    let title = if outcome.partial {
        "Incomplete test run"
    } else {
        "Test report"
    };
    let mut html = format!(
        "<!doctype html><meta charset=\"utf-8\"><title>{title}</title><style>body{{font:16px system-ui;max-width:1000px;margin:40px auto;padding:20px}}td,th{{text-align:left;padding:12px;border-bottom:1px solid #ddd}}pre{{white-space:pre-wrap}}a{{color:#2563eb}}</style><h1>{title}</h1><p>{}</p><p><a href=\"report.json\">JSON report</a> · <a href=\"events.jsonl\">Event journal</a></p><table><tr><th>Test</th><th>Status</th><th>Details</th></tr>",
        escape_markup(&message)
    );
    let failures = cases
        .iter()
        .filter(|c| matches!(c["status"].as_str(), Some("failed" | "timeout" | "xpass")))
        .count();
    let skipped = cases.iter().filter(|c| c["status"] == "skipped").count();
    let has_run_error =
        outcome.partial || outcome.result.as_ref().is_some_and(|r| r.error.is_some());
    let errors = usize::from(has_run_error);
    let mut xml = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?><testsuites><testsuite name=\"lxdev\" tests=\"{}\" errors=\"{errors}\" failures=\"{failures}\" skipped=\"{skipped}\">",
        cases.len() + errors
    );
    if has_run_error {
        xml.push_str(&format!(
            "<testcase name=\"run interrupted\"><error message=\"{}\"/></testcase>",
            escape_markup(&message)
        ));
    }
    for case in &cases {
        let name = escape_markup(case["full_name"].as_str().unwrap_or("unknown"));
        let status = case["status"].as_str().unwrap_or("skipped");
        let detail = escape_markup(
            case["error"]["message"]
                .as_str()
                .or(case["reason"].as_str())
                .unwrap_or(""),
        );
        html.push_str(&format!(
            "<tr><td>{name}</td><td>{status}</td><td><pre>{detail}</pre></td></tr>"
        ));
        xml.push_str(&format!(
            "<testcase name=\"{name}\" time=\"{}\">",
            case["duration_ms"].as_u64().unwrap_or_default() as f64 / 1000.0
        ));
        if matches!(status, "failed" | "timeout" | "xpass") {
            xml.push_str(&format!("<failure message=\"{detail}\"/>"));
        } else if status == "skipped" {
            xml.push_str(&format!("<skipped message=\"{detail}\"/>"));
        }
        xml.push_str("</testcase>");
    }
    html.push_str("</table>");
    xml.push_str("</testsuite></testsuites>");
    std::fs::write(path, serde_json::to_vec_pretty(&report)?)?;
    if !outcome
        .artifacts
        .iter()
        .any(|(name, _, _)| name == "report.html")
    {
        std::fs::write(output.join("report.html"), html)?;
    }
    std::fs::write(output.join("junit.xml"), xml)?;
    Ok(())
}

fn escape_markup(value: &str) -> String {
    value
        .chars()
        .filter(|ch| *ch >= ' ' || matches!(ch, '\n' | '\r' | '\t'))
        .collect::<String>()
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

fn shell_quote(value: &str) -> String {
    #[cfg(not(windows))]
    let escaped = value.replace('\'', "'\\''");
    #[cfg(windows)]
    let escaped = value.replace('\'', "''");
    format!("'{escaped}'")
}

fn print_code_frame(position: &MappedPosition, bundle: &TestBundle) {
    let disk = std::fs::read_to_string(&position.source).ok();
    let Some(source) = bundle.source_content(&position.source).or(disk.as_deref()) else {
        return;
    };
    for (index, line) in source
        .lines()
        .enumerate()
        .skip(position.line.saturating_sub(3) as usize)
        .take(5)
    {
        let number = index + 1;
        let marker = if number == position.line as usize {
            ">"
        } else {
            " "
        };
        eprintln!("  {marker} {number:4} | {line}");
    }
}

#[cfg(test)]
mod recovery_tests {
    use super::*;

    #[test]
    fn interrupted_run_keeps_failures_and_creates_all_reports() {
        let dir = tempfile::tempdir().unwrap();
        let cases = vec![
            StreamedCase {
                name: "completed failure".into(),
                full_name: "completed failure".into(),
                status: Some(TestCaseStatus::Failed),
                duration_ms: 7,
                covers: vec![],
                steps: vec![],
                record: json!({"id":"stable-id", "error":{"name":"AssertionError","message":"a < b", "actual":"a", "expected":"b"},
                "attachments":[{"name":"failure.png", "path":"attachments/failure.png"}]}),
            },
            StreamedCase {
                name: "not started".into(),
                full_name: "not started".into(),
                status: None,
                duration_ms: 0,
                covers: vec![],
                steps: vec![],
                record: json!({"id":"pending"}),
            },
        ];
        let outcome = interrupted_outcome(
            TestRunState::Cancelled,
            "cancelled by user".into(),
            Duration::from_millis(1400),
            vec![],
            vec![],
            cases,
        );
        write_partial_report(
            dir.path(),
            "run",
            &HashMap::new(),
            "2026-01-01T00:00:00Z",
            1400,
            &outcome.streamed,
        )
        .unwrap();
        complete_client_reports(dir.path(), "run", &outcome).unwrap();
        let report: serde_json::Value =
            serde_json::from_slice(&std::fs::read(dir.path().join("report.json")).unwrap())
                .unwrap();
        assert_eq!(report["state"], "cancelled");
        assert_eq!(report["duration_ms"], 1400);
        assert_eq!(report["cases"][0]["error"]["expected"], "b");
        assert_eq!(report["cases"][1]["status"], "skipped");
        let html = std::fs::read_to_string(dir.path().join("report.html")).unwrap();
        assert!(html.contains("a &lt; b"));
        let junit = std::fs::read_to_string(dir.path().join("junit.xml")).unwrap();
        assert!(junit.contains("<error"));
        assert!(junit.contains("<failure"));
        assert!(junit.contains("<skipped"));
    }

    #[test]
    fn poll_recovers_connection_loss_and_retains_terminal_timeout_events() {
        use lingxia_control_protocol::{ControlResponse, dev_session::DevSessionMessage};
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let server = std::thread::spawn(move || {
            // A dropped read must retry the same event cursor, not lose the run.
            let (stream, _) = listener.accept().unwrap();
            let mut dropped = tungstenite::accept(stream).unwrap();
            while let Ok(message) = dropped.read() {
                if let Ok(DevSessionMessage::Request(request)) =
                    serde_json::from_str(message.to_text().unwrap())
                {
                    assert_eq!(request.params.unwrap()["after_seq"], 0);
                    break;
                }
            }
            drop(dropped);
            let (stream, _) = listener.accept().unwrap();
            let mut socket = tungstenite::accept(stream).unwrap();
            loop {
                let msg = socket.read().unwrap();
                let wire: DevSessionMessage = serde_json::from_str(msg.to_text().unwrap()).unwrap();
                if let DevSessionMessage::Request(request) = wire {
                    let response = DevSessionMessage::Response(ControlResponse::success(
                        request.id,
                        Some(json!({
                            "run_id":"r", "state":"timed_out", "next_seq":2,
                            "events":[{"seq":1,"kind":"case_started","name":"hung","full_name":"hung","timeout_ms":100}],
                            "result":{"duration_ms":200,"error":{"name":"TimeoutError","message":"global deadline"}}
                        })),
                    ));
                    socket
                        .send(tungstenite::Message::Text(
                            serde_json::to_string(&response).unwrap().into(),
                        ))
                        .unwrap();
                    break;
                }
            }
        });
        let session: SessionInfo = serde_json::from_value(json!({"session_id":"test","project_root":".","target":"macos", "pid":1,"ws_url":format!("ws://{address}"),"log_file":""})).unwrap();
        let dir = tempfile::tempdir().unwrap();
        let run = ActiveRun::new(&session.ws_url, "r");
        let outcome = poll_until_terminal(
            &session,
            &run,
            dir.path(),
            true,
            false,
            false,
            &AtomicUsize::new(0),
            Duration::from_secs(1),
        )
        .unwrap();
        // The host reported the run over; dropping must not cancel it.
        assert!(run.settled.get());
        drop(run);
        server.join().unwrap();
        assert!(outcome.partial);
        assert_eq!(outcome.state, TestRunState::TimedOut);
        assert_eq!(outcome.result.unwrap().duration_ms, 200);
        let journal = std::fs::read_to_string(dir.path().join("events.jsonl")).unwrap();
        let event: serde_json::Value = serde_json::from_str(journal.trim()).unwrap();
        assert_eq!(event["schema_version"], 1);
        assert_eq!(event["kind"], "case_started");
    }
}

#[cfg(test)]
mod lifecycle_tests {
    use super::*;
    use clap::Parser;
    use lingxia_control_protocol::{ControlResponse, dev_session::DevSessionMessage};

    #[derive(Parser)]
    struct Harness {
        #[command(flatten)]
        options: TestOptions,
    }

    fn options(args: &[&str]) -> TestOptions {
        Harness::try_parse_from(std::iter::once("test").chain(args.iter().copied()))
            .unwrap()
            .options
    }

    /// Accepts one command, records `(method, params)`, answers success.
    fn one_shot_server() -> (String, std::thread::JoinHandle<(String, serde_json::Value)>) {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("ws://{}", listener.local_addr().unwrap());
        let handle = std::thread::spawn(move || {
            let (stream, _) = listener.accept().unwrap();
            let mut socket = tungstenite::accept(stream).unwrap();
            loop {
                let message = socket.read().unwrap();
                let Ok(DevSessionMessage::Request(request)) =
                    serde_json::from_str(message.to_text().unwrap())
                else {
                    continue;
                };
                let seen = (request.method.clone(), request.params.clone().unwrap());
                let response = DevSessionMessage::Response(ControlResponse::success(
                    request.id,
                    Some(json!({"run_id": seen.1["run_id"], "state": "cancelled"})),
                ));
                socket
                    .send(tungstenite::Message::Text(
                        serde_json::to_string(&response).unwrap().into(),
                    ))
                    .unwrap();
                return seen;
            }
        });
        (url, handle)
    }

    #[test]
    fn an_abandoned_run_is_cancelled_when_the_client_gives_up() {
        let (url, server) = one_shot_server();
        drop(ActiveRun::new(&url, "run-7"));
        let (method, params) = server.join().unwrap();
        assert_eq!(method, methods::session::test::CANCEL);
        assert_eq!(params["run_id"], "run-7");
        assert_eq!(params["reason"], "client_gave_up");
    }

    #[test]
    fn an_explicit_cancel_is_sent_once() {
        let (url, server) = one_shot_server();
        let run = ActiveRun::new(&url, "run-8");
        run.cancel("hang_watchdog");
        let (_, params) = server.join().unwrap();
        assert_eq!(params["reason"], "hang_watchdog");
        // Nothing listens any more: a second cancel would fail to connect,
        // but the guard must not even try.
        assert!(run.settled.get());
        drop(run);
    }

    #[test]
    fn an_unwritable_output_dir_fails_before_the_run_starts() {
        let dir = tempfile::tempdir().unwrap();
        let blocker = dir.path().join("file");
        std::fs::write(&blocker, b"").unwrap();
        let target = blocker.join("out");
        assert!(ensure_writable_dir(&target).is_err());

        // No session is listening: reaching the Runner would fail differently.
        let session: SessionInfo = serde_json::from_value(json!({"session_id":"test","project_root":".","target":"macos","pid":1,"ws_url":"ws://127.0.0.1:9","log_file":""})).unwrap();
        let error = execute_inner(
            &session,
            options(&["missing.test.ts", "--output-dir", target.to_str().unwrap()]),
        )
        .unwrap_err();
        assert!(
            format!("{error:#}").contains("output directory"),
            "{error:#}"
        );
        assert!(!looks_unreachable(&error));

        let writable = dir.path().join("nested/out");
        ensure_writable_dir(&writable).unwrap();
        assert_eq!(std::fs::read_dir(&writable).unwrap().count(), 0);
    }

    #[cfg(unix)]
    #[test]
    fn a_read_only_output_dir_is_refused() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let locked = dir.path().join("locked");
        std::fs::create_dir(&locked).unwrap();
        std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o555)).unwrap();
        let result = ensure_writable_dir(&locked);
        std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o755)).unwrap();
        // Root ignores permission bits; everyone else must be refused.
        if unsafe { libc::geteuid() } != 0 {
            assert!(format!("{:#}", result.unwrap_err()).contains("not writable"));
        }
    }

    #[test]
    fn cancel_active_needs_no_entry() {
        let parsed = options(&["--cancel-active"]);
        assert!(parsed.cancel_active && parsed.entry.is_none());
        assert!(Harness::try_parse_from(["test"]).is_err());
    }

    #[test]
    fn credential_args_are_redacted_in_reports_and_rerun_hints() {
        let parsed = options(&[
            "tests/",
            "--arg",
            "user=alice",
            "--arg",
            "PASSWORD=hunter22",
            "--arg",
            "apiKey=k-123456",
            "--arg",
            "auth_token=t-999999",
            "--secret-arg",
            "pin=4711-9",
        ]);
        let keys = secret_keys(&parsed);
        let mut args = parsed.args.iter().cloned().collect::<HashMap<_, _>>();
        args.extend(parsed.secret_args.iter().cloned());
        let redacted = redacted_args(&args, &keys);
        assert_eq!(redacted["user"], "alice");
        for key in ["PASSWORD", "apiKey", "auth_token", "pin"] {
            assert_eq!(redacted[key], REDACTED, "{key}");
        }
        let hint = rerun_args(&parsed);
        assert!(hint.contains("user=alice"));
        for secret in ["hunter22", "k-123456", "t-999999", "4711-9"] {
            assert!(!hint.contains(secret), "{hint}");
        }
        assert!(hint.contains("--secret-arg 'pin=***'"));

        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("report.json"),
            serde_json::to_vec(&json!({"meta":{"args":args}, "note":"pw hunter22"})).unwrap(),
        )
        .unwrap();
        std::fs::write(dir.path().join("report.html"), "<td>4711-9</td>").unwrap();
        scrub_secret_values(dir.path(), &secret_values(&args, &keys));
        let json = std::fs::read_to_string(dir.path().join("report.json")).unwrap();
        let html = std::fs::read_to_string(dir.path().join("report.html")).unwrap();
        for secret in ["hunter22", "k-123456", "t-999999", "4711-9"] {
            assert!(!json.contains(secret) && !html.contains(secret));
        }
        assert!(json.contains("alice"));
    }
}

fn parse_source_location(location: &str) -> Option<MappedPosition> {
    let mut parts = location.rsplitn(3, ':');
    let column = parts.next()?.parse().ok()?;
    let line = parts.next()?.parse().ok()?;
    let source = parts.next()?.to_owned();
    Some(MappedPosition {
        source,
        line,
        column,
    })
}

#[cfg(test)]
mod legacy_report_tests {
    use super::*;

    #[test]
    fn completed_legacy_reports_do_not_become_incomplete() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("report.json"),
            serde_json::to_vec(&json!({
                "total":1,"passed":1,"failed":0,"skipped":0,"duration_ms":4,
                "cases":[{"name":"legacy","full_name":"legacy","status":"passed","duration_ms":4}]
            }))
            .unwrap(),
        )
        .unwrap();
        let outcome = Outcome {
            state: TestRunState::Passed,
            result: None,
            console: vec![],
            artifacts: vec![],
            partial: false,
            streamed: vec![],
        };
        complete_client_reports(dir.path(), "legacy", &outcome).unwrap();
        let report: serde_json::Value =
            serde_json::from_slice(&std::fs::read(dir.path().join("report.json")).unwrap())
                .unwrap();
        assert_eq!(report["partial"], false);
        assert_eq!(report["passed"], 1);
        let junit = std::fs::read_to_string(dir.path().join("junit.xml")).unwrap();
        assert!(junit.contains("errors=\"0\""));
        assert!(!junit.contains("<error "));
    }
}
