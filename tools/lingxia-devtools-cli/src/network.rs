//! `lxdev network`: fake or record a running lxapp's Logic network traffic
//! in a dev session, outside any test run.
//!
//! A scenario installed here answers the app's Logic `fetch` and `Rong.SSE`
//! until it is cleared or the dev session ends. Only hosts built with the
//! automation test runtime (development hosts, the Runner) can do this.

use crate::client::{self, CommandError};
use crate::project::SessionInfo;
use anyhow::{Context, Result, anyhow, bail};
use clap::{Args, Subcommand};
use lingxia_control_protocol::methods::session::network as method;
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
    /// Answer the app's Logic requests from a scenario file
    #[command(subcommand)]
    Scenario(ScenarioCommand),
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
enum ScenarioCommand {
    /// Install a scenario until `scenario clear` or the session ends; it
    /// replaces a scenario installed earlier
    Use {
        /// Scenario JSON file
        file: PathBuf,
        /// Target lxapp (default: the home lxapp, else the current one)
        #[arg(long)]
        appid: Option<String>,
        /// Print JSON output
        #[arg(long)]
        json: bool,
    },
    /// Remove the active scenario
    Clear {
        /// Print JSON output
        #[arg(long)]
        json: bool,
    },
    /// Show the active scenario and what it answered
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
        NetworkCommand::Status { json }
        | NetworkCommand::Scenario(ScenarioCommand::Status { json }) => {
            let status = call(ws, method::STATUS, None)?;
            print(&status, json, print_status)
        }
        NetworkCommand::Scenario(ScenarioCommand::Use { file, appid, json }) => {
            let scenario = read_scenario(&file)?;
            let source = file
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_else(|| file.display().to_string());
            let status = call(
                ws,
                method::SCENARIO_USE,
                Some(json!({ "scenario": scenario, "source": source, "appid": appid })),
            )?;
            print(&status, json, |status| {
                print_status(status);
                print_warning(status);
                eprintln!(
                    "{} Logic requests matching it are faked until `lxdev network scenario clear` or the session ends",
                    "warning".yellow().bold()
                );
            })
        }
        NetworkCommand::Scenario(ScenarioCommand::Clear { json }) => {
            let result = call(ws, method::SCENARIO_CLEAR, None)?;
            print(&result, json, |result| {
                if result["cleared"] == true {
                    println!("network scenario cleared; Logic requests reach the network again");
                } else {
                    println!("no network scenario was active");
                }
            })
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
            let routes = result["scenario"]["routes"].as_array().map_or(0, Vec::len);
            print(&result, json, |result| {
                println!(
                    "wrote {} ({} route{} from {} request{})",
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
                for route in result["scenario"]["routes"]
                    .as_array()
                    .into_iter()
                    .flatten()
                {
                    if let Some(note) = route["note"].as_str() {
                        eprintln!(
                            "note: {} {}: {note}",
                            route["method"].as_str().unwrap_or(""),
                            route["url"].as_str().unwrap_or("")
                        );
                    }
                }
            })
        }
    }
}

fn call(ws: &str, handler: &str, args: Option<Value>) -> Result<Value> {
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

fn read_scenario(file: &Path) -> Result<Value> {
    let text = std::fs::read_to_string(file)
        .with_context(|| format!("cannot read scenario {}", file.display()))?;
    serde_json::from_str(&text).map_err(|err| {
        anyhow!(
            "{} is not valid JSON (line {}, column {}): {err}",
            file.display(),
            err.line(),
            err.column()
        )
    })
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

fn print(value: &Value, json: bool, human: impl FnOnce(&Value)) -> Result<()> {
    if json {
        println!("{}", serde_json::to_string(value)?);
    } else {
        human(value);
    }
    Ok(())
}

/// The host's warning about the request, such as an unknown `--appid`.
fn print_warning(status: &Value) {
    if let Some(warning) = status["warning"].as_str() {
        eprintln!("{} {warning}", "warning".yellow().bold());
    }
}

fn print_status(status: &Value) {
    match status
        .get("scenario")
        .filter(|scenario| scenario.is_object())
    {
        Some(scenario) => {
            let name = scenario["name"].as_str().unwrap_or("unnamed");
            let source = scenario["source"]
                .as_str()
                .map(|source| format!(" ({source})"))
                .unwrap_or_default();
            println!(
                "{} network scenario '{name}'{source} for {} since {}",
                "ACTIVE".yellow().bold(),
                scenario["appid"].as_str().unwrap_or("?"),
                scenario["installedAt"].as_str().unwrap_or("?"),
            );
            let routes = scenario["routes"].as_array().map_or(0, Vec::len);
            println!(
                "  {routes} route{}, {} request{} answered",
                if routes == 1 { "" } else { "s" },
                scenario["requests"],
                if scenario["requests"] == 1 { "" } else { "s" },
            );
            for request in scenario["lastRequests"].as_array().into_iter().flatten() {
                println!(
                    "  {} {} {} → {}{}",
                    request["time"].as_str().unwrap_or(""),
                    request["method"].as_str().unwrap_or(""),
                    request["url"].as_str().unwrap_or(""),
                    request["action"].as_str().unwrap_or(""),
                    request["status"]
                        .as_u64()
                        .map(|status| format!(" {status}"))
                        .unwrap_or_default(),
                );
            }
            if status["suspended"] == true {
                println!("  (standing aside while a test run is active)");
            }
        }
        None => println!("no network scenario is active"),
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn values_are_masked_everywhere_in_a_scenario() {
        let mut scenario = json!({
            "routes": [{ "url": "https://h/a?key=s3cret", "json": { "echo": "s3cret-x" } }]
        });
        mask_values(&mut scenario, &["s3cret".to_string(), String::new()]);
        assert_eq!(scenario["routes"][0]["url"], "https://h/a?key=***");
        assert_eq!(scenario["routes"][0]["json"]["echo"], "***-x");
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
        sink.commit_or_print(&json!({ "routes": [] }), &mut stdout)
            .unwrap();
        assert!(stdout.is_empty());
        let written: Value = serde_json::from_str(&std::fs::read_to_string(&out).unwrap()).unwrap();
        assert_eq!(written, json!({ "routes": [] }));
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

    #[test]
    fn a_bad_scenario_file_names_where_it_broke() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("bad.json");
        std::fs::write(&file, "{\n  \"routes\": [,]\n}").unwrap();
        let err = read_scenario(&file).unwrap_err().to_string();
        assert!(err.contains("line 2"), "{err}");
    }
}
