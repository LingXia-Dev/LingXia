//! `lxdev network`: who answered the running app's calls, and a capture of
//! the traffic that reached the network. A capture is written as a JSON
//! request list for an agent or as HAR for a viewer, chosen by file extension.
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
    /// Capture real network traffic to request JSON or HAR
    #[command(subcommand)]
    Capture(CaptureCommand),
    /// Show the traffic live in Chrome DevTools, for people; runs until
    /// Ctrl-C (agents: use `capture`)
    Devtools,
    /// Show who answered each recent call (route, scenario rule, mock or
    /// real, and the selection layer), the scenario and the recording
    Status {
        /// Print JSON output
        #[arg(long)]
        json: bool,
    },
}

#[derive(Subcommand, Clone)]
enum CaptureCommand {
    /// Start capturing real network traffic of an lxapp
    Start {
        /// Only capture URLs matching this glob (or /regex/flags)
        #[arg(long = "match", value_name = "GLOB")]
        matcher: Option<String>,
        /// Target lxapp id, home, or current (default: home, then current)
        #[arg(long = "app", conflicts_with = "all")]
        appid: Option<String>,
        /// Capture every owner's traffic, host subsystems included
        #[arg(long)]
        all: bool,
        /// Print JSON output
        #[arg(long)]
        json: bool,
    },
    /// Stop and write the capture
    Stop {
        /// `*.har` gets HAR 1.2 for a network viewer; any other name gets the
        /// request JSON for an agent. Give one of each to write both
        #[arg(value_name = "FILE", num_args = 1..=2, required_unless_present = "har")]
        files: Vec<PathBuf>,
        /// Deprecated: pass the `.har` file as FILE
        #[arg(long, value_name = "FILE", hide = true)]
        har: Option<PathBuf>,
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
            let session = crate::mock::Live { ws: ws.to_string() };
            let mut status = crate::mock::status(&session)?;
            if crate::mock::companion_support(&session).is_ok()
                && let Ok(result) = crate::mock::Session::companion(
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
        NetworkCommand::Devtools => crate::network_devtools::run(info),
        NetworkCommand::Capture(CaptureCommand::Start {
            matcher,
            appid,
            all,
            json,
        }) => {
            let status = call(
                ws,
                method::RECORD_START,
                Some(json!({ "match": matcher, "appid": appid, "all": all })),
            )?;
            print(&status, json, |status| {
                print_warning(status);
                println!(
                    "capturing network traffic{}; stop with `lxdev network capture stop <file.json|file.har>`",
                    matcher
                        .as_deref()
                        .map(|m| format!(" matching {m}"))
                        .unwrap_or_default()
                );
            })
        }
        NetworkCommand::Capture(CaptureCommand::Stop {
            files,
            har: har_flag,
            redact,
            json,
        }) => {
            if har_flag.is_some() {
                eprintln!(
                    "{} `--har FILE` is deprecated; pass the `.har` file as FILE",
                    "warning".yellow().bold()
                );
            }
            let CaptureOutputs {
                json: json_file,
                har,
            } = CaptureOutputs::from_args(files, har_flag)?;
            // Make sure the files can be written before the capture is taken
            // from the host: stopping it is not undoable.
            let json_sink = json_file
                .as_deref()
                .map(|path| ScenarioSink::prepare(path, "JSON file"))
                .transpose()?;
            let har_sink = har
                .as_deref()
                .map(|path| ScenarioSink::prepare(path, "HAR file"))
                .transpose()?;
            let mut result = call(ws, method::RECORD_STOP, None)?;
            mask_values(&mut result["requests"], &redact);
            let Some(recorded) = result["requests"].as_array() else {
                bail!("the host returned no `requests`; rebuild it with this lxdev's release");
            };
            if let Some(sink) = json_sink {
                sink.commit_or_print(&result["requests"], &mut std::io::stdout(), "JSON")?;
            }
            if let Some(sink) = har_sink {
                sink.commit_or_print(&requests_to_har(recorded), &mut std::io::stdout(), "HAR")?;
            }
            let dropped = result["dropped"].clone();
            let requests = result["requests"].clone();
            let summary = json!({ "dropped": dropped, "requests": requests });
            print(&summary, json, |summary| {
                if let Some(dropped) = summary["dropped"].as_u64().filter(|n| *n > 0) {
                    eprintln!(
                        "{} {dropped} more requests exceeded the capture limit",
                        "warning".yellow().bold()
                    );
                }
                let requests = summary["requests"].as_array().cloned().unwrap_or_default();
                if !requests.is_empty() {
                    println!("captured requests:");
                    for request in &requests {
                        println!("  {}", request_line(request));
                    }
                }
                if let Some(path) = &json_file {
                    let n = requests.len();
                    println!(
                        "wrote {} ({} request{})",
                        path.display(),
                        n,
                        if n == 1 { "" } else { "s" },
                    );
                }
                if let Some(path) = &har {
                    let n = requests.len();
                    println!(
                        "wrote {} ({} entr{})",
                        path.display(),
                        n,
                        if n == 1 { "y" } else { "ies" },
                    );
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
                    "this host cannot capture network traffic: it was built without the \
                     automation test runtime (release builds never have it). Use a development \
                     host or the Runner."
                );
            }
            Err(err)
        }
    }
}

/// The files `capture stop` writes, sorted by extension.
#[derive(Debug, PartialEq)]
struct CaptureOutputs {
    json: Option<PathBuf>,
    har: Option<PathBuf>,
}

impl CaptureOutputs {
    fn from_args(files: Vec<PathBuf>, har_flag: Option<PathBuf>) -> Result<Self> {
        let mut out = Self {
            json: None,
            har: None,
        };
        let tagged = files
            .into_iter()
            .map(|path| (is_har(&path), path))
            .chain(har_flag.map(|path| (true, path)));
        for (har, path) in tagged {
            let (slot, kind) = if har {
                (&mut out.har, "HAR")
            } else {
                (&mut out.json, "JSON")
            };
            if let Some(first) = slot {
                bail!(
                    "{} and {} are both {kind} outputs; give at most one `.json` and one `.har`",
                    first.display(),
                    path.display()
                );
            }
            *slot = Some(path);
        }
        Ok(out)
    }
}

fn is_har(path: &Path) -> bool {
    path.extension()
        .is_some_and(|ext| ext.eq_ignore_ascii_case("har"))
}

/// Where `capture stop` puts a file: a placeholder created next to the path
/// before the recording is stopped, renamed over the path once written.
struct ScenarioSink {
    out: PathBuf,
    partial: PathBuf,
}

impl ScenarioSink {
    fn prepare(out: &Path, flag: &str) -> Result<Self> {
        if out.is_dir() {
            bail!("{flag} {} is a directory; name a file", out.display());
        }
        let parent = out
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
            .unwrap_or(Path::new("."));
        std::fs::create_dir_all(parent)
            .with_context(|| format!("cannot create {}", parent.display()))?;
        let file_name = out
            .file_name()
            .ok_or_else(|| anyhow!("{flag} {} names no file", out.display()))?;
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

    /// Write `value`; when that fails, print it to `fallback` so the stopped
    /// recording is not lost, and fail naming the path. `label` is the noun
    /// in that error ("scenario", "requests", "HAR").
    fn commit_or_print(
        self,
        value: &Value,
        fallback: &mut impl std::io::Write,
        label: &str,
    ) -> Result<()> {
        let Err(err) = self.commit(value) else {
            return Ok(());
        };
        let _ = std::fs::remove_file(&self.partial);
        let printed = serde_json::to_string_pretty(value)
            .ok()
            .and_then(|text| writeln!(fallback, "{text}").ok())
            .is_some();
        let err = err.context(format!(
            "the recording was stopped but {} could not be written",
            self.out.display()
        ));
        if printed {
            Err(err.context(format!(
                "the recorded {label} was printed to stdout instead"
            )))
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
        Some("cleared") => "cleared with `lxdev mock clear`",
        Some("replaced") => "replaced by another `lxdev mock use`",
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

/// The host's warning about the request, such as an unknown `--app`.
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
            let answered_by = match (rule, call.owner.as_deref(), call.handler.as_deref()) {
                (Some(index), _, _) => format!("rule {index} ({label})"),
                (None, Some(owner), _) if owner != DEV_OWNER => format!("{owner} scenario"),
                // The companion's mock selection chose the handler.
                (None, _, Some(handler)) => format!("function {handler}"),
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

/// `#3 GET <url> 200 · app-id fetch · ttfb 12ms · 30ms`: one observed request.
pub(crate) fn request_line(request: &Value) -> String {
    let outcome = request["outcome"].as_str().unwrap_or("?");
    let answer = match request["status"].as_u64() {
        Some(status) if outcome == "completed" => status.to_string(),
        Some(status) => format!("{status} {outcome}"),
        None => outcome.to_string(),
    };
    let mut line = format!(
        "#{} {} {} {answer} · {} {}",
        request["id"],
        request["method"].as_str().unwrap_or("?"),
        request["url"].as_str().unwrap_or("?"),
        request["owner"].as_str().unwrap_or("(no owner)"),
        request["kind"].as_str().unwrap_or("?"),
    );
    if let Some(from) = request["redirectedFrom"].as_u64() {
        line.push_str(&format!(" · redirected from #{from}"));
    }
    if let Some(ttfb) = request["ttfbMs"].as_f64() {
        line.push_str(&format!(" · ttfb {ttfb}ms"));
    }
    if let Some(duration) = request["durationMs"].as_f64() {
        line.push_str(&format!(" · {duration}ms"));
    }
    if let Some(error) = request["error"].as_str() {
        line.push_str(&format!(" · {error}"));
    }
    line
}

pub(crate) fn print_status(status: &Value) {
    match status
        .get("scenario")
        .filter(|scenario| scenario.is_object())
    {
        Some(scenario) => {
            println!(
                "{} scenario '{}' ({} rule{}; `lxdev mock` for per-rule hits)",
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
    if status["mock"]["apps"]
        .as_array()
        .is_some_and(|apps| !apps.is_empty())
    {
        for line in crate::mock::selection_lines(&status["mock"]) {
            println!("{line}");
        }
    }
    print_recording(status);
    let observed = status["observed"].as_array().cloned().unwrap_or_default();
    if !observed.is_empty() {
        println!("reached the network, oldest first:");
        for request in &observed {
            println!("  {}", request_line(request));
        }
    }
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

/// HAR 1.2 of recorded requests. `timings.wait` is time to the first byte and
/// `timings.receive` is the rest of `durationMs`. DNS, connect and TLS are
/// not in the recording, so those timings are -1. Owner, kind, outcome,
/// redirect and chunk log have no HAR field, so they sit on `_lingxia`.
fn requests_to_har(requests: &[Value]) -> Value {
    let entries: Vec<Value> = requests.iter().map(har_entry).collect();
    let started = entries
        .first()
        .and_then(|entry| entry["startedDateTime"].as_str())
        .unwrap_or("1970-01-01T00:00:00.000Z");
    json!({
        "log": {
            "version": "1.2",
            "creator": {
                "name": "lxdev",
                "version": env!("CARGO_PKG_VERSION"),
            },
            "pages": [{
                "startedDateTime": started,
                "id": "page_1",
                "title": "lxdev network capture",
                "pageTimings": { "onContentLoad": -1, "onLoad": -1 }
            }],
            "entries": entries,
        }
    })
}

fn har_entry(request: &Value) -> Value {
    let url = request["url"].as_str().unwrap_or("");
    let (time, wait, receive) = har_timings(request);
    json!({
        "pageref": "page_1",
        "startedDateTime": started_date_time(request),
        "time": time,
        "request": har_request(request, url),
        "response": har_response(request),
        "cache": {},
        "timings": {
            "blocked": -1,
            "dns": -1,
            "connect": -1,
            "send": -1,
            "wait": wait,
            "receive": receive,
        },
        "_lingxia": lingxia_fields(request),
    })
}

/// `(total, wait, receive)` in milliseconds, at 0.1 ms. A missing TTFB makes
/// the whole duration wait. A missing duration after headers leaves receive
/// unknown (`-1`).
fn har_timings(request: &Value) -> (f64, f64, f64) {
    let total = millis_field(request, "durationMs").map(tenths);
    let ttfb = millis_field(request, "ttfbMs").map(tenths);
    let (time, wait, receive) = match (total, ttfb) {
        (Some(total), Some(ttfb)) => {
            let wait = ttfb.min(total).max(0);
            (wait + (total - wait).max(0), wait, (total - wait).max(0))
        }
        (Some(total), None) => {
            let total = total.max(0);
            (total, total, 0)
        }
        (None, Some(ttfb)) => {
            let wait = ttfb.max(0);
            (wait, wait, -1)
        }
        (None, None) => (0, -1, -1),
    };
    (from_tenths(time), from_tenths(wait), from_tenths(receive))
}

fn tenths(ms: f64) -> i64 {
    (ms * 10.0).round() as i64
}

fn from_tenths(tenths: i64) -> f64 {
    if tenths < 0 {
        -1.0
    } else {
        tenths as f64 / 10.0
    }
}

fn millis_field(request: &Value, key: &str) -> Option<f64> {
    request.get(key).and_then(Value::as_f64)
}

fn started_date_time(request: &Value) -> String {
    if let Some(text) = request["startedAt"].as_str() {
        return text.to_string();
    }
    let ms = request["time"].as_i64().or_else(|| {
        request["time"]
            .as_u64()
            .and_then(|ms| i64::try_from(ms).ok())
    });
    ms.and_then(chrono::DateTime::from_timestamp_millis)
        .map(|time| time.to_rfc3339_opts(chrono::SecondsFormat::Millis, true))
        .unwrap_or_else(|| "1970-01-01T00:00:00.000Z".to_string())
}

fn har_request(request: &Value, url: &str) -> Value {
    let headers = &request["requestHeaders"];
    let mut out = json!({
        "method": request["method"].as_str().unwrap_or("GET"),
        "url": url,
        "httpVersion": request["httpVersion"].as_str().unwrap_or(""),
        "cookies": [],
        "headers": har_headers(headers),
        "queryString": query_string(url),
        "headersSize": -1,
        "bodySize": byte_size(&request["requestBytes"]),
    });
    let mut notes = Vec::new();
    if let Some(body) = request.get("requestBody").filter(|body| !body.is_null()) {
        match body_text(body, header_value(headers, "content-type")) {
            BodyText::Text(text) => {
                out["postData"] = json!({
                    "mimeType": mime_type(headers),
                    "text": text,
                });
            }
            BodyText::Base64(data) => {
                out["postData"] = json!({
                    "mimeType": mime_type(headers),
                    "text": data,
                    "encoding": "base64",
                });
            }
            BodyText::Omitted(note) => notes.push(note),
        }
    }
    if request.get("requestBodyTruncated") == Some(&Value::Bool(true)) {
        notes.push("request body truncated".to_string());
    }
    if !notes.is_empty() {
        out["comment"] = json!(notes.join("; "));
    }
    out
}

fn har_response(request: &Value) -> Value {
    let headers = &request["responseHeaders"];
    let status = request["status"].as_u64().unwrap_or(0);
    let mut content = json!({
        "mimeType": mime_type(headers),
        "size": request["responseBytes"].as_u64().unwrap_or(0),
    });
    let mut notes = Vec::new();
    if let Some(note) = request["bodyNote"].as_str() {
        notes.push(note.to_string());
    }
    if let Some(body) = request.get("responseBody").filter(|body| !body.is_null()) {
        match body_text(body, header_value(headers, "content-type")) {
            BodyText::Text(text) => content["text"] = json!(text),
            BodyText::Base64(data) => {
                content["text"] = json!(data);
                content["encoding"] = json!("base64");
            }
            BodyText::Omitted(note) => notes.push(note),
        }
    } else if let Some(text) = stream_text(request) {
        content["text"] = json!(text);
        notes.push("stream rebuilt from its redacted frames".to_string());
    }
    if let Some(encoding) = request["responseDecoded"].as_str() {
        if let (Some(size), Some(wire)) = (
            request["responseDecodedBytes"].as_u64(),
            request["responseBytes"].as_u64(),
        ) {
            content["size"] = json!(size);
            if size >= wire {
                content["compression"] = json!(size - wire);
            }
        }
        notes.push(format!("decoded from {encoding}"));
    }
    if request.get("responseBodyTruncated") == Some(&Value::Bool(true)) {
        notes.push("response body truncated".to_string());
    }
    if !notes.is_empty() {
        content["comment"] = json!(notes.join("; "));
    }
    json!({
        "status": status,
        "statusText": status_text(request, status),
        "httpVersion": request["httpVersion"].as_str().unwrap_or(""),
        "cookies": [],
        "headers": har_headers(headers),
        "content": content,
        "redirectURL": header_value(headers, "location").unwrap_or(""),
        "headersSize": -1,
        "bodySize": byte_size(&request["responseBytes"]),
    })
}

fn lingxia_fields(request: &Value) -> Value {
    let mut out = json!({
        "id": request["id"],
        "owner": request["owner"],
        "kind": request["kind"],
        "outcome": request["outcome"],
    });
    if let Some(from) = request.get("redirectedFrom").filter(|from| !from.is_null()) {
        out["redirectedFrom"] = from.clone();
    }
    if let Some(log) = request.get("chunkLog").filter(|log| log.is_array()) {
        out["chunkLog"] = log.clone();
    }
    if let Some(error) = request.get("error").filter(|error| error.is_string()) {
        out["error"] = error.clone();
    }
    if request.get("incomplete") == Some(&Value::Bool(true)) {
        out["incomplete"] = json!(true);
    }
    out
}

fn status_text(request: &Value, status: u64) -> String {
    if status == 0 {
        if let Some(error) = request["error"].as_str() {
            return error.to_string();
        }
        if let Some(outcome) = request["outcome"]
            .as_str()
            .filter(|outcome| *outcome != "completed")
        {
            return outcome.to_string();
        }
    }
    String::new()
}

fn byte_size(value: &Value) -> i64 {
    value
        .as_u64()
        .and_then(|n| i64::try_from(n).ok())
        .unwrap_or(-1)
}

fn har_headers(headers: &Value) -> Value {
    let Some(pairs) = headers.as_array() else {
        return json!([]);
    };
    Value::Array(
        pairs
            .iter()
            .filter_map(|pair| {
                let pair = pair.as_array()?;
                Some(json!({
                    "name": pair.first()?.as_str()?,
                    "value": pair.get(1)?.as_str()?,
                }))
            })
            .collect(),
    )
}

fn header_value<'a>(headers: &'a Value, name: &str) -> Option<&'a str> {
    headers.as_array()?.iter().find_map(|pair| {
        let pair = pair.as_array()?;
        pair.first()?
            .as_str()?
            .eq_ignore_ascii_case(name)
            .then(|| pair.get(1)?.as_str())
            .flatten()
    })
}

fn mime_type(headers: &Value) -> &str {
    header_value(headers, "content-type").unwrap_or("")
}

enum BodyText {
    Text(String),
    Base64(String),
    Omitted(String),
}

/// The host wraps a non-JSON body as `{ "base64" }` or `{ "omitted" }`. A
/// JSON content type is the body's own object, even when it has those keys.
fn body_text(body: &Value, content_type: Option<&str>) -> BodyText {
    let json_type = content_type.is_some_and(|kind| kind.to_ascii_lowercase().contains("json"));
    match body {
        Value::String(text) => BodyText::Text(text.clone()),
        Value::Object(map) if !json_type && map.len() == 1 => {
            if let Some(data) = map.get("base64").and_then(Value::as_str) {
                return BodyText::Base64(data.to_string());
            }
            if let Some(note) = map.get("omitted").and_then(Value::as_str) {
                return BodyText::Omitted(note.to_string());
            }
            BodyText::Text(serde_json::to_string(body).unwrap_or_default())
        }
        other => BodyText::Text(serde_json::to_string(other).unwrap_or_default()),
    }
}

/// A stream's body is reported as its frames (`events` for SSE, `lines` for
/// NDJSON), each redacted; this is their wire text again.
fn stream_text(request: &Value) -> Option<String> {
    let frame = |value: &Value| match value {
        Value::String(text) => text.clone(),
        other => serde_json::to_string(other).unwrap_or_default(),
    };
    if let Some(events) = request["events"].as_array() {
        let mut text = String::new();
        for event in events {
            if let Some(name) = event["event"].as_str() {
                text.push_str(&format!("event: {name}\n"));
            }
            if let Some(id) = event["id"].as_str() {
                text.push_str(&format!("id: {id}\n"));
            }
            for line in frame(&event["data"]).split('\n') {
                text.push_str(&format!("data: {line}\n"));
            }
            text.push('\n');
        }
        return Some(text);
    }
    let lines = request["lines"].as_array()?;
    Some(lines.iter().map(|line| frame(line) + "\n").collect())
}

fn query_string(url: &str) -> Value {
    let Some((_, query)) = url.split_once('?') else {
        return json!([]);
    };
    let query = query.split('#').next().unwrap_or(query);
    Value::Array(
        query
            .split('&')
            .filter(|pair| !pair.is_empty())
            .map(|pair| {
                let (name, value) = pair.split_once('=').unwrap_or((pair, ""));
                json!({
                    "name": percent_decode(name),
                    "value": percent_decode(value),
                })
            })
            .collect(),
    )
}

fn percent_decode(input: &str) -> String {
    let bytes = input.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%'
            && let (Some(high), Some(low)) = (
                bytes.get(index + 1).copied().and_then(hex_val),
                bytes.get(index + 2).copied().and_then(hex_val),
            )
        {
            out.push((high << 4) | low);
            index += 3;
            continue;
        }
        out.push(bytes[index]);
        index += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn hex_val(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
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
            { "seq": 1, "time": 5, "function": "orders.submit", "owner": "dev", "rule": 1, "outcome": "fault" },
            { "seq": 2, "time": 6, "function": "orders.status", "outcome": "default", "noMatch": "rule 0 match.args.id: missing" },
            { "seq": 3, "time": 7, "function": "orders.status", "owner": "test:r1", "rule": 0, "outcome": "result" },
            { "seq": 4, "time": 8, "function": "orders.list", "outcome": "default", "handler": "mock" },
            { "seq": 5, "time": 9, "function": "orders.list", "outcome": "default", "handler": "real" }
        ], "droppedThrough": 0 })).unwrap();
        let functions = function_calls(&status, &calls);
        assert_eq!(functions[0]["answeredBy"], "rule 3 (Checkout:expired)");
        assert_eq!(functions[1]["answeredBy"], "companion default");
        assert_eq!(functions[2]["answeredBy"], "test:r1 scenario");
        assert_eq!(functions[3]["answeredBy"], "function mock");
        assert_eq!(functions[4]["answeredBy"], "function real");
        let mut all = vec![
            json!({
                "time": 4, "kind": "fetch", "method": "GET", "url": "https://h/cart",
                "status": 200, "answeredBy": "rule 1 (Checkout:expired)"
            }),
            json!({
                "time": 5, "kind": "fetch", "method": "GET", "url": "https://h/devices",
                "status": 200, "answeredBy": "mock (GET **/devices) · live target"
            }),
        ];
        all.extend(functions);
        let lines = call_lines(&all);
        assert_eq!(
            lines[0],
            "GET https://h/cart → 200  answered by: rule 1 (Checkout:expired)"
        );
        assert_eq!(
            lines[1],
            "GET https://h/devices → 200  answered by: mock (GET **/devices) · live target"
        );
        assert_eq!(
            lines[2],
            "function orders.submit → fault  answered by: rule 3 (Checkout:expired)"
        );
        assert_eq!(
            lines[3],
            "function orders.status → default  answered by: companion default\n    rule 0 match.args.id: missing"
        );
    }

    #[test]
    fn capture_outputs_are_sorted_by_extension() {
        fn p(path: &str) -> PathBuf {
            PathBuf::from(path)
        }
        let outputs = |files: &[&str], flag: Option<&str>| {
            CaptureOutputs::from_args(files.iter().map(|f| p(f)).collect(), flag.map(p))
        };
        assert_eq!(
            outputs(&["r.HAR", "r.json"], None).unwrap(),
            CaptureOutputs {
                json: Some(p("r.json")),
                har: Some(p("r.HAR")),
            }
        );
        // Anything that is not `.har` is the request JSON.
        assert_eq!(
            outputs(&["requests"], None).unwrap(),
            CaptureOutputs {
                json: Some(p("requests")),
                har: None,
            }
        );
        // The deprecated flag is HAR whatever its extension.
        assert_eq!(
            outputs(&["r.json"], Some("r.txt")).unwrap(),
            CaptureOutputs {
                json: Some(p("r.json")),
                har: Some(p("r.txt")),
            }
        );
        assert!(outputs(&["a.json", "b.json"], None).is_err());
        assert!(outputs(&["a.har", "b.har"], None).is_err());
        assert!(outputs(&["a.har"], Some("b.har")).is_err());
    }

    #[test]
    fn a_scenario_sink_checks_the_path_before_the_recording_stops() {
        let dir = tempfile::tempdir().unwrap();
        // A directory, or a parent that is a file, fails up front.
        assert!(ScenarioSink::prepare(dir.path(), "--out").is_err());
        let file = dir.path().join("plain");
        std::fs::write(&file, "x").unwrap();
        assert!(ScenarioSink::prepare(&file.join("out.json"), "--out").is_err());

        let out = dir.path().join("nested/rec.json");
        let sink = ScenarioSink::prepare(&out, "--out").unwrap();
        let mut stdout = Vec::new();
        sink.commit_or_print(&json!({ "rules": [] }), &mut stdout, "scenario")
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
        let sink = ScenarioSink::prepare(&out, "--out").unwrap();
        // The directory disappears between the check and the write.
        std::fs::remove_dir_all(dir.path().join("gone")).unwrap();
        let mut stdout = Vec::new();
        let err = sink
            .commit_or_print(&json!({ "name": "kept" }), &mut stdout, "scenario")
            .unwrap_err();
        let printed: Value = serde_json::from_slice(&stdout).unwrap();
        assert_eq!(printed["name"], "kept");
        let message = format!("{err:#}");
        assert!(message.contains("printed to stdout"), "{message}");
        assert!(message.contains("rec.json"), "{message}");
    }

    #[test]
    fn a_recording_becomes_a_har_a_viewer_can_open() {
        let har = requests_to_har(&[json!({
            "id": 40,
            "owner": "lingxia-showcase",
            "kind": "fetch",
            "method": "POST",
            "url": "http://127.0.0.1:8789/upload?q=a%20b&token=***",
            "startedAt": "2026-10-16T06:16:01.000Z",
            "status": 415,
            "httpVersion": "HTTP/1.1",
            "ttfbMs": 1.2,
            "durationMs": 5.0,
            "outcome": "completed",
            "requestBytes": 11,
            "responseBytes": 4,
            "requestHeaders": [["content-type", "application/json"], ["authorization", "Bearer ***"]],
            "responseHeaders": [["content-type", "text/plain"]],
            "requestBody": { "user": "ada" },
            "responseBody": "nope",
        })]);
        assert_eq!(har["log"]["version"], "1.2");
        assert_eq!(har["log"]["creator"]["name"], "lxdev");
        assert_eq!(har["log"]["creator"]["version"], env!("CARGO_PKG_VERSION"));
        assert_eq!(har["log"]["pages"][0]["id"], "page_1");
        assert_eq!(
            har["log"]["pages"][0]["startedDateTime"],
            "2026-10-16T06:16:01.000Z"
        );
        let entry = &har["log"]["entries"][0];
        assert_eq!(entry["pageref"], "page_1");
        assert_eq!(entry["startedDateTime"], "2026-10-16T06:16:01.000Z");
        assert_eq!(entry["time"], 5.0);
        assert_eq!(entry["timings"]["wait"], 1.2);
        assert_eq!(entry["timings"]["receive"], 3.8);
        assert_eq!(entry["timings"]["dns"], -1);
        assert_eq!(entry["timings"]["connect"], -1);
        assert!(entry["timings"].get("ssl").is_none());
        assert!(entry["cache"].is_object());
        assert_eq!(entry["request"]["method"], "POST");
        assert_eq!(entry["request"]["headers"][1]["name"], "authorization");
        assert_eq!(entry["request"]["headers"][1]["value"], "Bearer ***");
        assert_eq!(entry["request"]["queryString"][0]["value"], "a b");
        assert_eq!(entry["request"]["queryString"][1]["value"], "***");
        assert_eq!(entry["request"]["postData"]["mimeType"], "application/json");
        assert_eq!(entry["request"]["postData"]["text"], r#"{"user":"ada"}"#);
        assert_eq!(entry["request"]["bodySize"], 11);
        assert_eq!(entry["response"]["status"], 415);
        assert_eq!(entry["response"]["content"]["text"], "nope");
        assert_eq!(entry["response"]["content"]["size"], 4);
        assert_eq!(entry["response"]["bodySize"], 4);
        assert_eq!(entry["response"]["redirectURL"], "");
        let lingxia = &entry["_lingxia"];
        assert_eq!(lingxia["id"], 40);
        assert_eq!(lingxia["owner"], "lingxia-showcase");
        assert_eq!(lingxia["kind"], "fetch");
        assert_eq!(lingxia["outcome"], "completed");
        assert!(lingxia.get("redirectedFrom").is_none());
        assert!(lingxia.get("chunkLog").is_none());
    }

    #[test]
    fn har_keeps_owner_redirects_and_chunk_times() {
        let har = requests_to_har(&[
            json!({
                "id": 5,
                "owner": "lingxia-showcase",
                "kind": "fetch",
                "method": "GET",
                "url": "https://api.test/go",
                "time": 1000,
                "status": 302,
                "ttfbMs": 2.0,
                "durationMs": 2.0,
                "outcome": "completed",
                "responseHeaders": [["Location", "https://api.test/next?token=***"]],
                "responseBytes": 0,
            }),
            json!({
                "id": 6,
                "owner": null,
                "kind": "sse",
                "method": "GET",
                "url": "https://api.test/next?token=***",
                "startedAt": "2026-10-16T06:16:02.000Z",
                "status": 200,
                "ttfbMs": 2.0,
                "durationMs": 202.0,
                "outcome": "completed",
                "redirectedFrom": 5,
                "chunkLog": [[2, 10], [102, 10], [202, 4]],
                "responseHeaders": [["content-type", "text/event-stream"]],
                "responseBytes": 24,
                "events": [
                    { "event": "tick", "data": "1", "id": "e1" },
                    { "data": { "token": "***" } },
                    { "data": "a\nb" },
                ],
            }),
            json!({
                "id": 7,
                "owner": "lingxia-showcase",
                "kind": "fetch",
                "method": "GET",
                "url": "https://api.test/feed",
                "startedAt": "2026-10-16T06:16:03.000Z",
                "status": 200,
                "responseHeaders": [["content-type", "application/x-ndjson"]],
                "responseBytes": 20,
                "lines": [{ "n": 1 }, "plain"],
            }),
        ]);
        let first = &har["log"]["entries"][0];
        assert_eq!(first["startedDateTime"], "1970-01-01T00:00:01.000Z");
        assert_eq!(
            har["log"]["pages"][0]["startedDateTime"],
            "1970-01-01T00:00:01.000Z"
        );
        assert_eq!(
            first["response"]["redirectURL"],
            "https://api.test/next?token=***"
        );
        assert_eq!(first["timings"]["receive"], 0.0);
        let second = &har["log"]["entries"][1];
        assert_eq!(second["_lingxia"]["owner"], Value::Null);
        assert_eq!(second["_lingxia"]["redirectedFrom"], 5);
        assert_eq!(
            second["_lingxia"]["chunkLog"],
            json!([[2, 10], [102, 10], [202, 4]])
        );
        assert_eq!(second["timings"]["wait"], 2.0);
        assert_eq!(second["timings"]["receive"], 200.0);
        assert_eq!(
            second["response"]["content"]["text"],
            "event: tick\nid: e1\ndata: 1\n\ndata: {\"token\":\"***\"}\n\ndata: a\ndata: b\n\n"
        );
        assert_eq!(
            second["response"]["content"]["comment"],
            "stream rebuilt from its redacted frames"
        );
        assert_eq!(second["response"]["content"]["size"], 24);
        let ndjson = &har["log"]["entries"][2]["response"]["content"];
        assert_eq!(ndjson["text"], "{\"n\":1}\nplain\n");
        assert!(first["response"]["content"].get("text").is_none());
    }

    #[test]
    fn har_timings_cover_a_block_and_an_open_response() {
        let har = requests_to_har(&[
            json!({
                "id": 7,
                "owner": "lingxia-showcase",
                "kind": "fetch",
                "method": "GET",
                "url": "http://evil.test/",
                "startedAt": "2026-10-16T06:16:03.000Z",
                "outcome": "blocked",
                "error": "domain 'evil.test' is not allowed",
                "durationMs": 0.4,
            }),
            json!({
                "id": 8,
                "owner": "host:update",
                "kind": "download",
                "method": "GET",
                "url": "https://cdn.test/app.bin",
                "startedAt": "2026-10-16T06:16:04.000Z",
                "status": 200,
                "ttfbMs": 12.0,
                "outcome": "inFlight",
                "incomplete": true,
                "responseHeaders": [["content-type", "application/octet-stream"]],
                "responseBody": { "base64": "aGVsbG8=" },
                "responseBytes": 5,
            }),
            json!({
                "id": 9,
                "owner": "lingxia-showcase",
                "kind": "upload",
                "method": "POST",
                "url": "https://api.test/file",
                "startedAt": "2026-10-16T06:16:05.000Z",
                "status": 200,
                "ttfbMs": 3.0,
                "durationMs": 3.0,
                "outcome": "completed",
                "requestHeaders": [["content-type", "multipart/form-data; boundary=x"]],
                "requestBody": { "omitted": "multipart body of 40 bytes" },
                "responseHeaders": [["content-type", "application/json"]],
                "responseBody": { "base64": "not-a-wrapper" },
                "responseDecoded": "gzip",
                "responseBytes": 9,
                "responseBodyTruncated": true,
            }),
            json!({
                "id": 10,
                "owner": "lingxia-showcase",
                "kind": "fetch",
                "method": "GET",
                "url": "https://api.test/big",
                "startedAt": "2026-10-16T06:16:06.000Z",
                "status": 200,
                "outcome": "completed",
                "responseHeaders": [["content-type", "application/json"]],
                "responseBody": { "items": [] },
                "responseDecoded": "gzip",
                "responseDecodedBytes": 40,
                "responseBytes": 12,
            }),
            json!({
                "id": 11,
                "owner": "lingxia-showcase",
                "kind": "download",
                "method": "GET",
                "url": "https://cdn.test/huge.bin",
                "startedAt": "2026-10-16T06:16:07.000Z",
                "status": 200,
                "outcome": "completed",
                "bodyNote": "the recording's body budget was spent",
                "responseBytes": 900,
            }),
        ]);
        let blocked = &har["log"]["entries"][0];
        assert_eq!(blocked["response"]["status"], 0);
        assert_eq!(
            blocked["response"]["statusText"],
            "domain 'evil.test' is not allowed"
        );
        assert_eq!(blocked["timings"]["wait"], 0.4);
        assert_eq!(blocked["timings"]["receive"], 0.0);
        assert_eq!(blocked["time"], 0.4);
        assert_eq!(
            blocked["_lingxia"]["error"],
            "domain 'evil.test' is not allowed"
        );

        let open = &har["log"]["entries"][1];
        assert_eq!(open["timings"]["wait"], 12.0);
        assert_eq!(open["timings"]["receive"], -1.0);
        assert_eq!(open["time"], 12.0);
        assert_eq!(open["response"]["content"]["encoding"], "base64");
        assert_eq!(open["response"]["content"]["text"], "aGVsbG8=");
        assert_eq!(open["response"]["content"]["size"], 5);
        assert_eq!(open["_lingxia"]["incomplete"], true);
        assert_eq!(open["_lingxia"]["outcome"], "inFlight");
        assert_eq!(open["_lingxia"]["owner"], "host:update");
        assert_eq!(open["response"]["status"], 200);
        assert_eq!(open["response"]["statusText"], "");

        let upload = &har["log"]["entries"][2];
        assert_eq!(upload["request"]["comment"], "multipart body of 40 bytes");
        assert!(upload["request"].get("postData").is_none());
        // A JSON content type is the body, not the binary wrapper.
        let text = upload["response"]["content"]["text"].as_str().unwrap();
        assert_eq!(text, r#"{"base64":"not-a-wrapper"}"#);
        // Cut before it was fully decoded: the decoded size is unknown.
        assert_eq!(upload["response"]["content"]["size"], 9);
        assert!(upload["response"]["content"].get("compression").is_none());
        assert_eq!(
            upload["response"]["content"]["comment"],
            "decoded from gzip; response body truncated"
        );

        let compressed = &har["log"]["entries"][3]["response"]["content"];
        assert_eq!(compressed["size"], 40);
        assert_eq!(compressed["compression"], 28);
        assert_eq!(compressed["text"], r#"{"items":[]}"#);

        let budget = &har["log"]["entries"][4]["response"]["content"];
        assert_eq!(budget["size"], 900);
        assert_eq!(budget["comment"], "the recording's body budget was spent");
        assert!(budget.get("text").is_none());
    }

    #[test]
    fn an_empty_recording_is_still_a_har() {
        let har = requests_to_har(&[]);
        assert_eq!(har["log"]["entries"], json!([]));
        assert_eq!(
            har["log"]["pages"][0]["startedDateTime"],
            "1970-01-01T00:00:00.000Z"
        );
    }
}
