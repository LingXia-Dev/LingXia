//! `lxdev test <entry>` — bundle a JS/TS test, run it in the selected live
//! session in an isolated automation runtime, stream console output, download
//! artifacts, and report one terminal summary.

use crate::client::execute_command;
use crate::project::SessionInfo;
use crate::test_bundle::{MappedPosition, TestBundle, bundle_test_entry};
use anyhow::{Context, Result, anyhow, bail};
use base64::Engine as _;
use base64::engine::general_purpose::STANDARD as BASE64;
use clap::Args;
use lingxia_devtool_protocol::{handlers, session_test::*};
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
const MAX_ARTIFACT_BYTES: usize = 16 * 1024 * 1024;
const MAX_ARTIFACT_BASE64_BYTES: usize = MAX_ARTIFACT_BYTES.div_ceil(3) * 4;

#[derive(Args)]
#[command(
    after_long_help = "The entry must import test APIs from @rongjs/test.\n\
Example: lxdev test tests/home.test.ts --arg locale=en"
)]
pub struct TestOptions {
    /// Test entry (.js/.ts and .mjs/.mts ESM variants)
    entry: PathBuf,

    /// Overall registration and case timeout in seconds
    #[arg(long, default_value_t = 60, value_parser = clap::value_parser!(u64).range(1..=3600))]
    timeout: u64,

    /// Key=value string exposed as test.args (repeatable)
    #[arg(long = "arg", value_name = "KEY=VALUE", value_parser = parse_key_value)]
    args: Vec<(String, String)>,

    /// Directory receiving attached artifacts
    /// (default: test-results/lxdev/<run-id>)
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
    let machine = options.json || options.pretty;
    let bundle = bundle_test_entry(&options.entry)?;
    if !machine {
        eprintln!(
            "{} bundled {} ({})",
            "test".cyan(),
            options.entry.display(),
            human_bytes(bundle.code.len())
        );
    }

    let start: TestStartResponse = execute_typed(
        &info.ws_url,
        handlers::session::test::START,
        &TestStartArgs {
            source: bundle.code.clone(),
            source_name: Some(bundle.bundle_name.clone()),
            timeout_ms: Some(options.timeout * 1000),
            args: options.args.iter().cloned().collect::<HashMap<_, _>>(),
        },
    )?;
    let run_id = start.run_id;
    if !machine {
        eprintln!(
            "{} run {} started (timeout {}s)",
            "test".cyan(),
            run_id,
            options.timeout
        );
    }

    let output_dir = options
        .output_dir
        .clone()
        .unwrap_or_else(|| PathBuf::from("test-results/lxdev").join(&run_id));

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
        Duration::from_secs(options.timeout),
    )?;
    report(&outcome, &bundle, &run_id, &output_dir, &options);

    let exit_code = match outcome.state {
        TestRunState::Passed => 0,
        TestRunState::Cancelled if interrupts.load(Ordering::SeqCst) > 0 => 130,
        _ => 1,
    };
    std::process::exit(exit_code);
}

struct Outcome {
    state: TestRunState,
    result: Option<TestRunResult>,
    console: Vec<(String, String)>,
    artifacts: Vec<(String, PathBuf, usize)>,
}

#[allow(clippy::too_many_arguments)]
fn poll_until_terminal(
    info: &SessionInfo,
    run_id: &str,
    output_dir: &Path,
    machine: bool,
    interrupts: &AtomicUsize,
    run_timeout: Duration,
) -> Result<Outcome> {
    let mut after_seq = 0u64;
    let mut console = Vec::new();
    let mut artifacts = Vec::new();
    let mut cancel_sent = false;
    let mut cancel_deadline: Option<std::time::Instant> = None;
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
                handlers::session::test::CANCEL,
                &TestCancelArgs {
                    run_id: run_id.to_string(),
                    reason: Some("client_interrupt".to_string()),
                },
            );
        }

        let poll: TestPollResponse = execute_typed(
            &info.ws_url,
            handlers::session::test::POLL,
            &TestPollArgs {
                run_id: run_id.to_string(),
                after_seq,
            },
        )?;

        for event in &poll.events {
            after_seq = after_seq.max(event.seq);
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
                TestEventPayload::CaseStarted { .. } => {}
                TestEventPayload::CaseFinished {
                    name,
                    full_name,
                    status,
                    duration_ms,
                    error,
                } => {
                    if !machine {
                        print_case_finished(name, full_name, *status, *duration_ms, error.as_ref());
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
}
