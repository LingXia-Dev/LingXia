//! `lxdev network`: the network panel of a dev session. It shows what
//! answers the running lxapp's Logic `fetch` and `Rong.SSE` and records real
//! traffic into scenario files for `lxdev scenario use`.
//!
//! Only hosts built with the automation test runtime (development hosts, the
//! Runner) can do this.

use crate::client::{self, CommandError};
use crate::project::SessionInfo;
use anyhow::{Context, Result, anyhow, bail};
use clap::{Args, Subcommand};
use lingxia_control_protocol::methods::session::companion as companion_method;
use lingxia_control_protocol::methods::session::network as method;
use lingxia_control_protocol::scenario::{DEV_OWNER, companion};
use owo_colors::OwoColorize;
use serde_json::{Value, json};
use std::path::{Path, PathBuf};

#[derive(Args, Clone)]
pub struct NetworkOptions {
    #[command(subcommand)]
    command: NetworkCommand,
}

#[derive(Subcommand, Clone)]
enum NetworkCommand {
    /// Capture real Logic traffic into a scenario file
    #[command(subcommand)]
    Record(RecordCommand),
    /// Show the active scenario and recording
    Status {
        /// Print JSON output
        #[arg(long)]
        json: bool,
    },
}

#[derive(Subcommand, Clone)]
enum RecordCommand {
    /// Start capturing real Logic fetch responses
    Start {
        /// Only record URLs matching this glob (or /regex/flags)
        #[arg(long = "match", value_name = "GLOB")]
        matcher: Option<String>,
        /// Target lxapp (default: the home lxapp, else the current one)
        #[arg(long)]
        appid: Option<String>,
        /// Print JSON output
        #[arg(long)]
        json: bool,
    },
    /// Stop and write what was captured as a scenario file
    Stop {
        /// Scenario file to write
        #[arg(long, value_name = "FILE")]
        out: PathBuf,
        /// Scenario name (default: "recorded <start time>")
        #[arg(long)]
        name: Option<String>,
        /// Replace this value with `***` wherever it appears (repeatable)
        #[arg(long = "redact", value_name = "VALUE")]
        redact: Vec<String>,
        /// Print JSON output
        #[arg(long)]
        json: bool,
    },
}

pub fn execute(info: &SessionInfo, options: NetworkOptions) -> Result<()> {
    let ws = info.ws_url.as_str();
    match options.command {
        NetworkCommand::Status { json } => {
            let session = crate::scenario::Live { ws: ws.to_string() };
            let mut status = crate::scenario::status(&session)?;
            if crate::scenario::companion_support(&session).is_ok()
                && let Ok(result) = crate::scenario::Session::companion(
                    &session,
                    companion_method::SCENARIO_CALLS,
                    Some(json!({ "since": 0 })),
                )
                && let Ok(calls) = serde_json::from_value::<companion::CallsResult>(result)
            {
                let mut all = status["calls"].as_array().cloned().unwrap_or_default();
                all.extend(function_calls(&status, &calls));
                all.sort_by_key(|call| call["time"].as_u64().unwrap_or_default());
                let keep = all.len().saturating_sub(20);
                status["calls"] = Value::Array(all.split_off(keep));
            }
            print(&status, json, print_status)
        }
        NetworkCommand::Record(RecordCommand::Start {
            matcher,
            appid,
            json,
        }) => {
            let status = call(
                ws,
                method::RECORD_START,
                Some(json!({ "match": matcher, "appid": appid })),
            )?;
            print(&status, json, |status| {
                print_warning(status);
                println!(
                    "recording real Logic traffic{}; stop with `lxdev network record stop --out <file.json>`",
                    matcher
                        .as_deref()
                        .map(|m| format!(" matching {m}"))
                        .unwrap_or_default()
                );
            })
        }
        NetworkCommand::Record(RecordCommand::Stop {
            out,
            name,
            redact,
            json,
        }) => {
            // Make sure the file can be written before the recording is
            // taken from the host: stopping it is not undoable.
            let sink = ScenarioSink::prepare(&out)?;
            let mut result = call(ws, method::RECORD_STOP, Some(json!({ "name": name })))?;
            mask_values(&mut result["scenario"], &redact);
            sink.commit_or_print(&result["scenario"], &mut std::io::stdout())?;
            let routes = result["scenario"]["rules"].as_array().map_or(0, Vec::len);
            print(&result, json, |result| {
                println!(
                    "wrote {} ({} rule{} from {} request{})",
                    out.display(),
                    routes,
                    if routes == 1 { "" } else { "s" },
                    result["exchanges"],
                    if result["exchanges"] == 1 { "" } else { "s" },
                );
                if let Some(dropped) = result["dropped"].as_u64().filter(|n| *n > 0) {
                    eprintln!(
                        "{} {dropped} more requests exceeded the recording limit",
                        "warning".yellow().bold()
                    );
                }
                for rule in result["scenario"]["rules"].as_array().into_iter().flatten() {
                    if let Some(note) = rule["note"].as_str() {
                        eprintln!("note: {}: {note}", rule["http"].as_str().unwrap_or(""));
                    }
                }
            })
        }
    }
}

pub(crate) fn call(ws: &str, handler: &str, args: Option<Value>) -> Result<Value> {
    match client::execute_command(ws, handler, args) {
        Ok(value) => Ok(value.unwrap_or(Value::Null)),
        Err(err) => {
            if err
                .downcast_ref::<CommandError>()
                .is_some_and(|command| command.code == "unknown_method")
            {
                bail!(
                    "this host cannot route or record network traffic: it was built without the \
                     automation test runtime (release builds never have it). Use a development \
                     host or the Runner."
                );
            }
            Err(err)
        }
    }
}

/// Where `record stop --out` puts the scenario: a placeholder file created
/// next to `out` before the recording is stopped, renamed over `out` once
/// the scenario is written.
struct ScenarioSink {
    out: PathBuf,
    partial: PathBuf,
}

impl ScenarioSink {
    fn prepare(out: &Path) -> Result<Self> {
        if out.is_dir() {
            bail!("--out {} is a directory; name a .json file", out.display());
        }
        let parent = out
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
            .unwrap_or(Path::new("."));
        std::fs::create_dir_all(parent)
            .with_context(|| format!("cannot create {}", parent.display()))?;
        let file_name = out
            .file_name()
            .ok_or_else(|| anyhow!("--out {} names no file", out.display()))?;
        let mut partial_name = std::ffi::OsString::from(".");
        partial_name.push(file_name);
        partial_name.push(format!(".{}.partial", std::process::id()));
        let partial = parent.join(partial_name);
        std::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(&partial)
            .with_context(|| format!("cannot write next to {}", out.display()))?;
        Ok(Self {
            out: out.to_path_buf(),
            partial,
        })
    }

    fn commit(&self, scenario: &Value) -> Result<()> {
        let mut text = serde_json::to_string_pretty(scenario)?;
        text.push('\n');
        std::fs::write(&self.partial, text)
            .with_context(|| format!("cannot write {}", self.partial.display()))?;
        std::fs::rename(&self.partial, &self.out)
            .with_context(|| format!("cannot write {}", self.out.display()))
    }

    /// Write the scenario; when that fails, print it to `fallback` so the
    /// stopped recording is not lost, and fail naming the path.
    fn commit_or_print(self, scenario: &Value, fallback: &mut impl std::io::Write) -> Result<()> {
        let Err(err) = self.commit(scenario) else {
            return Ok(());
        };
        let _ = std::fs::remove_file(&self.partial);
        let printed = serde_json::to_string_pretty(scenario)
            .ok()
            .and_then(|text| writeln!(fallback, "{text}").ok())
            .is_some();
        let err = err.context(format!(
            "the recording was stopped but {} could not be written",
            self.out.display()
        ));
        if printed {
            Err(err.context("the recorded scenario was printed to stdout instead"))
        } else {
            Err(err)
        }
    }
}

impl Drop for ScenarioSink {
    fn drop(&mut self) {
        // Gone after a rename; left behind when the stop call failed.
        let _ = std::fs::remove_file(&self.partial);
    }
}

/// Replace every occurrence of `values` in the strings of `value`.
pub(crate) fn mask_values(value: &mut Value, values: &[String]) {
    match value {
        Value::String(text) => {
            for secret in values.iter().filter(|secret| !secret.is_empty()) {
                if text.contains(secret.as_str()) {
                    *text = text.replace(secret.as_str(), "***");
                }
            }
        }
        Value::Array(items) => items.iter_mut().for_each(|item| mask_values(item, values)),
        Value::Object(fields) => fields
            .values_mut()
            .for_each(|field| mask_values(field, values)),
        _ => {}
    }
}

pub(crate) fn print(value: &Value, json: bool, human: impl FnOnce(&Value)) -> Result<()> {
    if json {
        println!("{}", serde_json::to_string(value)?);
    } else {
        human(value);
    }
    Ok(())
}

/// Why and when the last dev scenario stopped answering.
pub(crate) fn print_last_cleared(status: &Value) {
    let Some(cleared) = status
        .get("lastCleared")
        .filter(|cleared| cleared.is_object())
    else {
        return;
    };
    let why = match cleared["reason"].as_str() {
        Some("cleared") => "cleared with `lxdev scenario clear`",
        Some("replaced") => "replaced by another `lxdev scenario use`",
        Some("session_ended") => "cleared: the dev session disconnected",
        _ => "cleared",
    };
    println!(
        "  last: '{}' {why} at {}",
        cleared["name"]
            .as_str()
            .or(cleared["source"].as_str())
            .unwrap_or("unnamed"),
        cleared["clearedAt"].as_str().unwrap_or("?"),
    );
}

/// The host's warning about the request, such as an unknown `--appid`.
pub(crate) fn print_warning(status: &Value) {
    if let Some(warning) = status["warning"].as_str() {
        eprintln!("{} {warning}", "warning".yellow().bold());
    }
}

/// The recording, if one runs.
pub(crate) fn print_recording(status: &Value) {
    if let Some(recording) = status
        .get("recording")
        .filter(|recording| recording.is_object())
    {
        println!(
            "recording since {} ({} request{} captured{})",
            recording["startedAt"].as_str().unwrap_or("?"),
            recording["exchanges"],
            if recording["exchanges"] == 1 { "" } else { "s" },
            recording["match"]
                .as_str()
                .map(|matcher| format!(", matching {matcher}"))
                .unwrap_or_default(),
        );
    }
}

/// The dev scenario's Function calls from the companion, shaped like the
/// host's call log, their rule positions turned into rule numbers.
pub(crate) fn function_calls(status: &Value, calls: &companion::CallsResult) -> Vec<Value> {
    let label = status["scenario"]["label"].as_str().unwrap_or("scenario");
    let function_rules: Vec<u64> = status["scenario"]["rules"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|rule| rule["kind"] == "function")
        .filter_map(|rule| rule["index"].as_u64())
        .collect();
    calls
        .calls
        .iter()
        .map(|call| {
            let rule = call
                .rule
                .filter(|_| call.owner.as_deref() == Some(DEV_OWNER))
                .and_then(|position| function_rules.get(position).copied());
            let answered_by = match (rule, call.owner.as_deref()) {
                (Some(index), _) => format!("rule {index} ({label})"),
                (None, Some(owner)) if owner != DEV_OWNER => format!("{owner} scenario"),
                _ => "companion default".to_string(),
            };
            let mut entry = json!({
                "time": call.time,
                "kind": "function",
                "function": call.function,
                "outcome": call.outcome,
                "answeredBy": answered_by,
            });
            if let Some(no_match) = &call.no_match {
                entry["noMatch"] = json!(no_match);
            }
            entry
        })
        .collect()
}

/// One line per call: `GET https://… → 200  rule 1 (wifi:b)`.
pub(crate) fn call_lines(calls: &[Value]) -> Vec<String> {
    calls
        .iter()
        .map(|call| {
            let what = if call["kind"] == "function" {
                format!(
                    "function {} → {}",
                    call["function"].as_str().unwrap_or(""),
                    call["outcome"].as_str().unwrap_or("")
                )
            } else {
                let outcome = match (call["status"].as_u64(), call["error"].as_str()) {
                    (Some(status), _) => status.to_string(),
                    (None, Some(error)) => error.to_string(),
                    (None, None) => "pending".to_string(),
                };
                format!(
                    "{} {} → {outcome}",
                    call["method"].as_str().unwrap_or("GET"),
                    call["url"].as_str().unwrap_or("")
                )
            };
            let mut line = format!(
                "{what}  answered by: {}",
                call["answeredBy"].as_str().unwrap_or("real")
            );
            if let Some(no_match) = call["noMatch"].as_str() {
                line.push_str(&format!("\n    {no_match}"));
            }
            line
        })
        .collect()
}

pub(crate) fn print_status(status: &Value) {
    match status
        .get("scenario")
        .filter(|scenario| scenario.is_object())
    {
        Some(scenario) => {
            println!(
                "{} scenario '{}' ({} rule{}; `lxdev scenario status` for per-rule hits)",
                "ACTIVE".yellow().bold(),
                scenario["label"].as_str().unwrap_or("unnamed"),
                scenario["rules"].as_array().map_or(0, Vec::len),
                if scenario["rules"].as_array().map_or(0, Vec::len) == 1 {
                    ""
                } else {
                    "s"
                },
            );
            if status["suspended"] == true {
                println!("  (standing aside while a test run is active)");
            }
        }
        None => {
            println!("no scenario is active");
            print_last_cleared(status);
        }
    }
    print_recording(status);
    let calls = status["calls"].as_array().cloned().unwrap_or_default();
    if calls.is_empty() {
        if status["active"] == true {
            println!(
                "no calls yet — the app may serve its own cache; reload the page or `lxdev lxapp restart`"
            );
        }
        return;
    }
    println!("recent calls, oldest first:");
    for line in call_lines(&calls) {
        println!("  {line}");
    }
    println!("(an app cache can answer without a request; such calls are not listed)");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn values_are_masked_everywhere_in_a_scenario() {
        let mut scenario = json!({
            "rules": [{ "http": "GET https://h/a?key=s3cret", "json": { "echo": "s3cret-x" } }]
        });
        mask_values(&mut scenario, &["s3cret".to_string(), String::new()]);
        assert_eq!(scenario["rules"][0]["http"], "GET https://h/a?key=***");
        assert_eq!(scenario["rules"][0]["json"]["echo"], "***-x");
    }

    #[test]
    fn calls_say_who_answered_them() {
        let status = json!({ "scenario": { "label": "Checkout:expired", "rules": [
            { "index": 1, "kind": "http" },
            { "index": 2, "kind": "function" },
            { "index": 3, "kind": "function" }
        ] } });
        let calls: companion::CallsResult = serde_json::from_value(json!({ "calls": [
            { "time": 5, "function": "orders.submit", "owner": "dev", "rule": 1, "outcome": "fault" },
            { "time": 6, "function": "orders.status", "outcome": "default", "noMatch": "rule 0 match.args.id: missing" },
            { "time": 7, "function": "orders.status", "owner": "test:r1", "rule": 0, "outcome": "result" }
        ] })).unwrap();
        let functions = function_calls(&status, &calls);
        assert_eq!(functions[0]["answeredBy"], "rule 3 (Checkout:expired)");
        assert_eq!(functions[1]["answeredBy"], "companion default");
        assert_eq!(functions[2]["answeredBy"], "test:r1 scenario");
        let mut all = vec![json!({
            "time": 4, "kind": "fetch", "method": "GET", "url": "https://h/cart",
            "status": 200, "answeredBy": "rule 1 (Checkout:expired)"
        })];
        all.extend(functions);
        let lines = call_lines(&all);
        assert_eq!(
            lines[0],
            "GET https://h/cart → 200  answered by: rule 1 (Checkout:expired)"
        );
        assert_eq!(
            lines[1],
            "function orders.submit → fault  answered by: rule 3 (Checkout:expired)"
        );
        assert_eq!(
            lines[2],
            "function orders.status → default  answered by: companion default\n    rule 0 match.args.id: missing"
        );
    }

    #[test]
    fn a_scenario_sink_checks_the_path_before_the_recording_stops() {
        let dir = tempfile::tempdir().unwrap();
        // A directory, or a parent that is a file, fails up front.
        assert!(ScenarioSink::prepare(dir.path()).is_err());
        let file = dir.path().join("plain");
        std::fs::write(&file, "x").unwrap();
        assert!(ScenarioSink::prepare(&file.join("out.json")).is_err());

        let out = dir.path().join("nested/rec.json");
        let sink = ScenarioSink::prepare(&out).unwrap();
        let mut stdout = Vec::new();
        sink.commit_or_print(&json!({ "rules": [] }), &mut stdout)
            .unwrap();
        assert!(stdout.is_empty());
        let written: Value = serde_json::from_str(&std::fs::read_to_string(&out).unwrap()).unwrap();
        assert_eq!(written, json!({ "rules": [] }));
        // No placeholder is left behind.
        let left: Vec<_> = std::fs::read_dir(dir.path().join("nested"))
            .unwrap()
            .map(|entry| entry.unwrap().file_name())
            .collect();
        assert_eq!(left, vec![std::ffi::OsString::from("rec.json")]);
    }

    #[test]
    fn a_failed_write_prints_the_scenario_and_fails() {
        let dir = tempfile::tempdir().unwrap();
        let out = dir.path().join("gone/rec.json");
        let sink = ScenarioSink::prepare(&out).unwrap();
        // The directory disappears between the check and the write.
        std::fs::remove_dir_all(dir.path().join("gone")).unwrap();
        let mut stdout = Vec::new();
        let err = sink
            .commit_or_print(&json!({ "name": "kept" }), &mut stdout)
            .unwrap_err();
        let printed: Value = serde_json::from_slice(&stdout).unwrap();
        assert_eq!(printed["name"], "kept");
        let message = format!("{err:#}");
        assert!(message.contains("printed to stdout"), "{message}");
        assert!(message.contains("rec.json"), "{message}");
    }
}
