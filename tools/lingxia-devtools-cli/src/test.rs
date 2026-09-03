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
    #[arg(long, default_value_t = 60, value_parser = clap::value_parser!(u64).range(1..=3600))]
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
    let machine = options.json || options.pretty;
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
    if let Some(grep) = &options.grep {
        args.insert("grep".to_string(), grep.clone());
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
            "{} run {} started (timeout {}s)",
            "test".cyan(),
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

    let outcome = poll_until_terminal(
        info,
        &run_id,
        &output_dir,
        machine,
        &interrupts,
        Duration::from_secs(options.timeout_secs),
        &args,
    )?;
    if outcome.partial && !machine {
        print_partial_summary(&outcome.streamed);
    }
    report(&outcome, &bundle, &run_id, &output_dir, &options);

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
    interrupts: &AtomicUsize,
    run_timeout: Duration,
    args: &HashMap<String, String>,
) -> Result<Outcome> {
    let run_started_at = chrono::Utc::now().to_rfc3339();
    let run_started = std::time::Instant::now();
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

        let poll: TestPollResponse = execute_typed(
            &info.ws_url,
            methods::session::test::POLL,
            &TestPollArgs {
                run_id: run_id.to_string(),
                after_seq,
            },
        )?;

        for event in &poll.events {
            after_seq = after_seq.max(event.seq);
            last_event_at = std::time::Instant::now();
            match &event.payload {
                TestEventPayload::Console { level, message } => {
                    if !machine {
                        print_console(level, message);
                    }
                    console.push((level.clone(), message.clone()));
                }
                TestEventPayload::Artifact {
                    name,
                    mime_type,
                    base64,
                } => {
                    let (path, bytes) = write_artifact(output_dir, name, base64)?;
                    if !machine {
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
                    covers,
                } => {
                    case_budget = timeout_ms
                        .map(Duration::from_millis)
                        .unwrap_or(DEFAULT_CASE_TIMEOUT);
                    streamed.push(StreamedCase {
                        name: name.clone(),
                        full_name: full_name.clone(),
                        status: None,
                        duration_ms: 0,
                        covers: covers.clone(),
                        steps: Vec::new(),
                    });
                }
                TestEventPayload::CaseFinished {
                    name,
                    full_name,
                    status,
                    duration_ms,
                    error,
                } => {
                    if let Some(current) = streamed.last_mut() {
                        current.status = Some(*status);
                        current.duration_ms = *duration_ms;
                    }
                    if !machine {
                        print_case_finished(name, full_name, *status, *duration_ms, error.as_ref());
                    }
                }
                TestEventPayload::StepStarted { name, path } => {
                    if !machine {
                        eprintln!("{} {path}", "▸".dimmed());
                    }
                    if let Some(current) = streamed.last_mut() {
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
                    ..
                } => {
                    if let Some(current) = streamed.last_mut() {
                        current.steps.push(json!({
                            "name": name,
                            "path": path,
                            "status": status,
                            "duration_ms": duration_ms,
                        }));
                    }
                }
            }
        }
        let events_drained = after_seq.saturating_add(1) >= poll.next_seq;
        if poll.state.is_terminal() && events_drained {
            return Ok(Outcome {
                state: poll.state,
                result: poll.result,
                console,
                artifacts,
                partial: false,
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
            write_partial_report(
                output_dir,
                run_id,
                args,
                &run_started_at,
                run_started.elapsed().as_millis() as u64,
                &streamed,
            )?;
            if !machine {
                eprintln!(
                    "{} no test event for {:.1}s; wrote partial report.json",
                    "✗ timed out".red().bold(),
                    last_event_at.elapsed().as_secs_f64()
                );
                print_partial_summary(&streamed);
            }
            return Ok(Outcome {
                state: TestRunState::TimedOut,
                result: None,
                console,
                artifacts,
                partial: true,
                streamed,
            });
        }
        if let Some(deadline) = cancel_deadline
            && std::time::Instant::now() > deadline
        {
            bail!("run {run_id} did not reach a terminal state after cancel");
        }
        if std::time::Instant::now() > poll_deadline {
            bail!("run {run_id} did not reach a terminal state within its deadline");
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
) {
    let machine = options.json || options.pretty;
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
        let framework_report = outcome
            .result
            .as_ref()
            .and_then(|result| result.report.as_ref())
            .map(|report| report_value(report, bundle));
        let envelope = json!({
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
            "{} passed, {} failed, {} skipped ({} cases, {:.1}s framework)",
            framework_report.passed,
            framework_report.failed,
            framework_report.skipped,
            framework_report.total,
            framework_report.duration_ms as f64 / 1000.0
        );
        for case in &framework_report.cases {
            if !matches!(case.status, TestCaseStatus::Failed) {
                continue;
            }
            let Some(error) = &case.error else {
                continue;
            };
            eprintln!("{}: {}", case.full_name.red(), error.message);
            if let Some(stack) = &error.stack {
                let (mapped, _) = bundle.remap_stack(stack);
                for line in mapped.lines() {
                    eprintln!("    {line}");
                }
            }
            print_error_causes(&error.causes, bundle, 1);
        }
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
    let others = artifacts.len().saturating_sub(3);
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
    json!({
        "total": report.total,
        "passed": report.passed,
        "failed": report.failed,
        "skipped": report.skipped,
        "duration_ms": report.duration_ms,
        "cases": report.cases.iter().map(|case| {
            json!({
                "name": case.name,
                "full_name": case.full_name,
                "status": case.status,
                "duration_ms": case.duration_ms,
                "error": case.error.as_ref().map(|error| {
                    let (stack, primary) = match &error.stack {
                        Some(stack) => {
                            let (mapped, primary) = bundle.remap_stack(stack);
                            (Some(mapped), primary)
                        }
                        None => (None, None),
                    };
                    mapped_error_value(error, &stack, primary.as_ref(), bundle)
                }),
            })
        }).collect::<Vec<_>>(),
    })
}

fn mapped_error_value(
    error: &TestRunError,
    stack: &Option<String>,
    primary: Option<&MappedPosition>,
    bundle: &TestBundle,
) -> serde_json::Value {
    json!({
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
    })
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
        TestCaseStatus::Passed => {
            eprintln!("{} {display_name} ({seconds:.2}s)", "✓".green())
        }
        TestCaseStatus::Skipped => {
            eprintln!("{} {display_name}", "-".yellow())
        }
        TestCaseStatus::Failed => {
            eprintln!("{} {display_name} ({seconds:.2}s)", "✗".red());
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
    println!("{tag} {message}");
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
            let status = match case.status {
                Some(TestCaseStatus::Passed) => "passed",
                Some(TestCaseStatus::Failed) => "failed",
                Some(TestCaseStatus::Skipped) => "skipped",
                None => "timeout",
            };
            json!({
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
            })
        })
        .collect::<Vec<_>>();
    let count = |wanted: &str| {
        cases
            .iter()
            .filter(|case| case.get("status").and_then(|value| value.as_str()) == Some(wanted))
            .count()
    };
    let envelope = json!({
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
        "filtered": args.contains_key("grep"),
        "run_id": run_id,
        "total": cases.len(),
        "passed": count("passed"),
        "failed": count("failed"),
        "skipped": count("skipped"),
        "xfail": 0,
        "xpass": 0,
        "timeout": count("timeout"),
        "duration_ms": duration_ms,
        "cases": cases,
    });
    let path = output_dir.join("report.json");
    std::fs::write(&path, serde_json::to_vec_pretty(&envelope)?)
        .with_context(|| format!("failed to write {}", path.display()))?;
    Ok(())
}

fn print_partial_summary(streamed: &[StreamedCase]) {
    eprintln!(
        "{} partial: {} case event(s) streamed, no HTML on this path",
        "test".cyan(),
        streamed.len()
    );
    for case in streamed {
        let status = case
            .status
            .map(|status| match status {
                TestCaseStatus::Passed => "passed",
                TestCaseStatus::Failed => "failed",
                TestCaseStatus::Skipped => "skipped",
            })
            .unwrap_or("running");
        eprintln!("  {status} {}", case.full_name);
        for step in &case.steps {
            if let Some(path) = step.get("path").and_then(|value| value.as_str()) {
                eprintln!("    step {path}");
            }
        }
    }
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
    fn partial_report_is_json_without_html() {
        let dir = tempfile::tempdir().unwrap();
        write_partial_report(
            dir.path(),
            "run-1",
            &HashMap::new(),
            "2026-01-01T00:00:00Z",
            1234,
            &[StreamedCase {
                name: "home".into(),
                full_name: "home".into(),
                status: Some(TestCaseStatus::Passed),
                duration_ms: 10,
                covers: vec!["lx.app".into()],
                steps: vec![json!({ "name": "greet", "path": "greet", "status": "passed" })],
            }],
        )
        .unwrap();
        let text = std::fs::read_to_string(dir.path().join("report.json")).unwrap();
        let value: serde_json::Value = serde_json::from_str(&text).unwrap();
        assert_eq!(value["partial"], serde_json::json!(true));
        assert_eq!(value["total"], serde_json::json!(1));
        assert_eq!(value["passed"], serde_json::json!(1));
        assert_eq!(value["cases"][0]["covers"][0], serde_json::json!("lx.app"));
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
