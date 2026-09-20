//! `lxdev test <entry>` — bundle a JS/TS test, run it in the selected live
//! session in an isolated automation runtime, stream console output, download
//! artifacts, and report one terminal summary.

use crate::client::execute_command;
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
use std::time::Duration;

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
Example: lxdev test tests/ --grep home")]
pub struct TestOptions {
    /// Test entry file, or a directory of `*.test.ts` files
    pub entry: PathBuf,

    /// Whole-run budget in seconds
    #[arg(long, default_value_t = 300, value_parser = clap::value_parser!(u64).range(1..=3600))]
    timeout_secs: u64,

    /// Key=value string exposed as test.args (repeatable)
    #[arg(long = "arg", value_name = "KEY=VALUE", value_parser = parse_key_value)]
    args: Vec<(String, String)>,

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
    let started_at = chrono::Utc::now().to_rfc3339();
    warn_package_version(&options.entry, machine);
    let bundle = bundle_test_path(&options.entry)?;
    if !machine {
        eprintln!(
            "{} bundled {} ({})",
            "test".cyan(),
            options.entry.display(),
            human_bytes(bundle.code.len())
        );
    }

    let mut args = options.args.iter().cloned().collect::<HashMap<_, _>>();
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

    let start: TestStartResponse = execute_typed(
        &info.ws_url,
        methods::session::test::START,
        &TestStartArgs {
            source: bundle.code.clone(),
            source_name: Some(bundle.bundle_name.clone()),
            timeout_ms: Some(options.timeout_secs * 1000),
            args: args.clone(),
        },
    )?;
    let run_id = start.run_id;
    if !machine {
        eprintln!(
            "{} {} · run {} started (timeout {}s)",
            "test".cyan(),
            info.target,
            run_id,
            options.timeout_secs
        );
    }

    let output_dir = options
        .output_dir
        .clone()
        .unwrap_or_else(|| PathBuf::from("test-results").join(&run_id));

    // First Ctrl-C requests a cooperative cancel; the second exits immediately.
    let interrupts = Arc::new(AtomicUsize::new(0));
    {
        let interrupts = interrupts.clone();
        ctrlc::set_handler(move || {
            if interrupts.fetch_add(1, Ordering::SeqCst) >= 1 {
                std::process::exit(130);
            }
        })
        .context("failed to install Ctrl-C handler")?;
    }

    std::fs::create_dir_all(&output_dir)?;
    let mut outcome = poll_until_terminal(
        info,
        &run_id,
        &output_dir,
        machine,
        options.verbose,
        options.jsonl,
        &interrupts,
        Duration::from_secs(options.timeout_secs),
        &args,
    )?;
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
            value["meta"] = json!({"started_at": started_at, "duration_ms": framework.duration_ms, "args": args});
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
                &args,
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
        &info.session_id,
    );

    let exit_code = match outcome.state {
        TestRunState::Passed if !outcome.partial => 0,
        TestRunState::Cancelled if interrupts.load(Ordering::SeqCst) > 0 => 130,
        _ => 1,
    };
    std::process::exit(exit_code);
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

#[allow(clippy::too_many_arguments)]
fn poll_until_terminal(
    info: &SessionInfo,
    run_id: &str,
    output_dir: &Path,
    machine: bool,
    verbose: bool,
    jsonl: bool,
    interrupts: &AtomicUsize,
    run_timeout: Duration,
    _args: &HashMap<String, String>,
) -> Result<Outcome> {
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
            let _ = execute_typed::<_, TestCancelResponse>(
                &info.ws_url,
                methods::session::test::CANCEL,
                &TestCancelArgs {
                    run_id: run_id.to_string(),
                    reason: Some("client_interrupt".to_string()),
                },
            );
        }

        let polled: Result<TestPollResponse> = execute_typed(
            &info.ws_url,
            methods::session::test::POLL,
            &TestPollArgs {
                run_id: run_id.to_string(),
                after_seq,
            },
        );
        let poll = match polled {
            Ok(poll) => {
                poll_failures = 0;
                poll
            }
            Err(err) => {
                poll_failures += 1;
                let now = std::time::Instant::now();
                if poll_failures < 3
                    && now < poll_deadline
                    && cancel_deadline.is_none_or(|deadline| now < deadline)
                {
                    if !machine && verbose {
                        eprintln!("Retrying test status request: {err:#}");
                    }
                    std::thread::sleep(POLL_INTERVAL);
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
                            print_case_finished(name, full_name, *status, *duration_ms, None);
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
        if last_event_at.elapsed() > case_budget + WATCHDOG_GRACE {
            let _ = execute_typed::<_, TestCancelResponse>(
                &info.ws_url,
                methods::session::test::CANCEL,
                &TestCancelArgs {
                    run_id: run_id.to_string(),
                    reason: Some("hang_watchdog".to_string()),
                },
            );
            return Ok(interrupted_outcome(
                TestRunState::TimedOut,
                "No test event before the lxdev hang watchdog fired".into(),
                run_started.elapsed(),
                console,
                artifacts,
                streamed,
            ));
        }
        if let Some(deadline) = cancel_deadline
            && std::time::Instant::now() > deadline
        {
            return Ok(interrupted_outcome(
                TestRunState::Cancelled,
                "Cancellation deadline exceeded".into(),
                run_started.elapsed(),
                console,
                artifacts,
                streamed,
            ));
        }
        if std::time::Instant::now() > poll_deadline {
            return Ok(interrupted_outcome(
                TestRunState::TimedOut,
                "Run deadline exceeded".into(),
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
                    shell_quote(&options.entry.to_string_lossy()),
                    shell_quote(id),
                    options.timeout_secs
                );
                for (key, value) in &options.args {
                    command.push_str(&format!(
                        " --arg {}",
                        shell_quote(&format!("{key}={value}"))
                    ));
                }
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
        TestCaseStatus::Skipped => {
            eprintln!("{} {display_name}", "-".yellow())
        }
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
        let outcome = poll_until_terminal(
            &session,
            "r",
            dir.path(),
            true,
            false,
            false,
            &AtomicUsize::new(0),
            Duration::from_secs(1),
            &HashMap::new(),
        )
        .unwrap();
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
