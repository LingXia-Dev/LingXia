//! `lxdev scenario`: put the running app into a named product state
//! ("gateway offline", "coupon expired") in a dev session, outside any test
//! run.
//!
//! A scenario file is a list of rules (see
//! [`lingxia_control_protocol::scenario`]). `http` rules answer the app's
//! Logic `fetch` and `Rong.SSE` through the host's
//! `session.network.scenario.*` methods; `function` rules go through the dev
//! server to the session's companion (`session.companion.scenario.*`), which
//! must have declared that it handles them. `use` validates everything
//! before it changes anything, so an invalid file leaves the active scenario
//! answering.

use crate::client::{self, CommandError};
use crate::network;
use crate::project::SessionInfo;
use anyhow::{Context, Result, anyhow, bail};
use clap::{Args, Subcommand};
use lingxia_control_protocol::dev_session::broker::SessionContent;
use lingxia_control_protocol::dev_session::capabilities::SCENARIO_FUNCTION;
use lingxia_control_protocol::methods::session::companion as companion_method;
use lingxia_control_protocol::methods::session::network as method;
use lingxia_control_protocol::scenario::{
    self as format, DEV_OWNER, Resolved, ScenarioFile, companion,
};
use owo_colors::OwoColorize;
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::io::IsTerminal;
use std::path::{Component, Path, PathBuf};
use std::time::{Duration, Instant};

/// Where named scenarios live, relative to a project directory.
pub const SCENARIO_DIR: &str = "tests/scenarios";

/// How long `use` waits for a first request before it hints at app caches.
const FIRST_REQUEST_WAIT: Duration = Duration::from_secs(4);
/// How often `--watch` looks at the file.
const WATCH_INTERVAL: Duration = Duration::from_millis(300);

/// The hint when nothing reached an installed scenario.
pub(crate) const IDLE_HINT: &str = "no request reached this scenario yet — the app may serve its \
     own cache; reload the page or `lxdev lxapp restart`";

#[derive(Args, Clone)]
pub struct ScenarioOptions {
    #[command(subcommand)]
    command: ScenarioCommand,
}

#[derive(Subcommand, Clone)]
enum ScenarioCommand {
    /// List the scenarios (and each `name:variant`) under tests/scenarios/
    List {
        /// Print JSON output
        #[arg(long)]
        json: bool,
    },
    /// Install a scenario by name (`wifi`, `wifi:offline`) or path until
    /// `clear`, another `use`, or the end of the dev session
    Use {
        /// `name` or `name:variant` under tests/scenarios/ (without
        /// `.json`), or a file (`path.json:variant`)
        scenario: String,
        /// Target lxapp (default: the home lxapp, else the current one)
        #[arg(long)]
        appid: Option<String>,
        /// Keep running and reinstall the scenario whenever the file is
        /// saved; an invalid save is reported and the last valid version
        /// keeps answering
        #[arg(long)]
        watch: bool,
        /// Print JSON output
        #[arg(long)]
        json: bool,
    },
    /// Show the active scenario, what each rule answered, and the last one
    /// cleared
    Status {
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
}

impl ScenarioOptions {
    pub fn is_list(&self) -> bool {
        matches!(self.command, ScenarioCommand::List { .. })
    }
}

// ------------------------------- the file -------------------------------

/// A scenario file as written, and parsed.
#[derive(Debug, Clone)]
pub(crate) struct Loaded {
    pub value: Value,
    pub file: ScenarioFile,
}

pub(crate) fn parse_text(path: &Path, text: &str) -> Result<Loaded> {
    let value: Value = serde_json::from_str(text).map_err(|err| {
        anyhow!(
            "{} is not valid JSON (line {}, column {}): {err}",
            path.display(),
            err.line(),
            err.column()
        )
    })?;
    let file = format::parse_file(&value).map_err(|err| anyhow!("{}: {err}", path.display()))?;
    Ok(Loaded { value, file })
}

pub(crate) fn read_file(path: &Path) -> Result<Loaded> {
    let text = std::fs::read_to_string(path)
        .with_context(|| format!("cannot read scenario {}", path.display()))?;
    parse_text(path, &text)
}

// ------------------------------- discovery -------------------------------

/// The scenario directories of a session, most specific first: its content
/// directory, its project root, then the project of the current directory
/// when that lies inside either (a host project's embedded lxapp).
pub(crate) fn search_roots(info: &SessionInfo, cwd: &Path) -> Vec<PathBuf> {
    let mut bases = Vec::new();
    if let Some(SessionContent::Host { path } | SessionContent::LxApp { path }) = &info.content {
        bases.push(PathBuf::from(path));
    }
    bases.push(PathBuf::from(&info.project_root));
    let local = crate::test_bundle::find_project_root(cwd);
    let local = local.canonicalize().unwrap_or(local);
    if bases
        .iter()
        .any(|base| local.starts_with(base.canonicalize().unwrap_or_else(|_| base.clone())))
    {
        bases.push(local);
    }
    dedup_roots(bases)
}

/// Without a session: the project of the current directory.
pub(crate) fn local_roots(cwd: &Path) -> Vec<PathBuf> {
    dedup_roots(vec![crate::test_bundle::find_project_root(cwd)])
}

fn dedup_roots(bases: Vec<PathBuf>) -> Vec<PathBuf> {
    let mut roots: Vec<PathBuf> = Vec::new();
    for base in bases {
        let root = base.join(SCENARIO_DIR);
        let key = root.canonicalize().unwrap_or_else(|_| root.clone());
        if !roots
            .iter()
            .any(|seen| seen.canonicalize().unwrap_or_else(|_| seen.clone()) == key)
        {
            roots.push(root);
        }
    }
    roots
}

/// One scenario file found under a root.
#[derive(Debug)]
pub(crate) struct Found {
    /// Relative path without `.json`, `/`-separated.
    pub name: String,
    pub path: PathBuf,
    pub file: Result<ScenarioFile, String>,
}

impl Found {
    /// What `use` takes for this file: `name` when it has shared rules,
    /// and `name:variant` per variant.
    pub(crate) fn usable(&self) -> Vec<String> {
        let Ok(file) = &self.file else {
            return vec![self.name.clone()];
        };
        file.usable_without_variant()
            .then(|| self.name.clone())
            .into_iter()
            .chain(
                file.variant_names()
                    .map(|variant| format!("{}:{variant}", self.name)),
            )
            .collect()
    }
}

/// Every `*.json` under `roots`, by name; an earlier root shadows a later
/// one.
pub(crate) fn discover(roots: &[PathBuf]) -> Vec<Found> {
    let mut found: BTreeMap<String, Found> = BTreeMap::new();
    for root in roots {
        let mut files = Vec::new();
        walk(root, &mut files);
        for path in files {
            let Some(name) = scenario_name(root, &path) else {
                continue;
            };
            found.entry(name.clone()).or_insert_with(|| Found {
                file: read_file(&path)
                    .map(|loaded| loaded.file)
                    .map_err(|err| format!("{err:#}")),
                name,
                path,
            });
        }
    }
    found.into_values().collect()
}

fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    let mut entries: Vec<_> = entries.flatten().collect();
    entries.sort_by_key(|entry| entry.file_name());
    for entry in entries {
        let path = entry.path();
        if entry.file_name().to_string_lossy().starts_with('.') {
            continue;
        }
        if path.is_dir() {
            walk(&path, out);
        } else if path.extension().is_some_and(|ext| ext == "json") {
            out.push(path);
        }
    }
}

fn scenario_name(root: &Path, path: &Path) -> Option<String> {
    let relative = path.strip_prefix(root).ok()?.with_extension("");
    let parts: Vec<String> = relative
        .components()
        .map(|part| match part {
            Component::Normal(part) => Some(part.to_string_lossy().into_owned()),
            _ => None,
        })
        .collect::<Option<_>>()?;
    Some(parts.join("/"))
}

/// What a `use` argument names.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Target {
    pub path: PathBuf,
    /// The name (or path, as given) the host logs it by.
    pub source: String,
    pub variant: Option<String>,
}

/// A scenario argument: a file that exists (optionally `file:variant`),
/// else `name[:variant]` under `roots`.
pub(crate) fn resolve(arg: &str, roots: &[PathBuf], cwd: &Path) -> Result<Target> {
    let as_path = cwd.join(arg);
    if as_path.is_file() {
        return Ok(Target {
            path: as_path,
            source: arg.to_string(),
            variant: None,
        });
    }
    let (base, variant) = format::split_variant(arg);
    let variant = variant.map(str::to_string);
    let as_path = cwd.join(base);
    if variant.is_some() && as_path.is_file() {
        return Ok(Target {
            path: as_path,
            source: base.to_string(),
            variant,
        });
    }
    let name = base.replace('\\', "/");
    let name = name.strip_suffix(".json").unwrap_or(&name);
    let valid = !name.is_empty()
        && Path::new(name)
            .components()
            .all(|part| matches!(part, Component::Normal(_)));
    if valid {
        for root in roots {
            let candidate = root.join(format!("{name}.json"));
            if candidate.is_file() {
                return Ok(Target {
                    path: candidate,
                    source: name.to_string(),
                    variant,
                });
            }
        }
    }
    let names: Vec<String> = discover(roots).iter().flat_map(Found::usable).collect();
    let looked = roots
        .iter()
        .map(|root| root.display().to_string())
        .collect::<Vec<_>>()
        .join(", ");
    let available = if names.is_empty() {
        "none".to_string()
    } else {
        names.join(", ")
    };
    bail!(
        "no scenario '{arg}': it is not a file, nor a name under {looked} (available: {available})"
    )
}

// ------------------------------- the session -------------------------------

/// A failed request, as the dev server answered it.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct CallError {
    pub code: String,
    pub message: String,
    pub data: Option<Value>,
}

/// What `lxdev scenario` needs from a dev session.
pub(crate) trait Session {
    /// A `session.network.*` request to the app host.
    fn host(&self, method: &str, params: Option<Value>) -> Result<Value>;
    /// A `session.companion.*` request to the dev server.
    fn companion(&self, method: &str, params: Option<Value>) -> Result<Value, CallError>;
}

/// The live session over its websocket.
pub(crate) struct Live {
    pub ws: String,
}

impl Session for Live {
    fn host(&self, method: &str, params: Option<Value>) -> Result<Value> {
        network::call(&self.ws, method, params)
    }

    fn companion(&self, method: &str, params: Option<Value>) -> Result<Value, CallError> {
        match client::execute_command(&self.ws, method, params) {
            Ok(value) => Ok(value.unwrap_or(Value::Null)),
            Err(err) => Err(match err.downcast_ref::<CommandError>() {
                Some(command) => CallError {
                    code: command.code.clone(),
                    message: command.message.clone(),
                    data: command.data.clone(),
                },
                None => CallError {
                    code: "transport".into(),
                    message: format!("{err:#}"),
                    data: None,
                },
            }),
        }
    }
}

/// Whether the session's companion answers `function` rules, and if not,
/// why.
pub(crate) fn companion_support(session: &dyn Session) -> Result<(), String> {
    match session.companion(companion_method::CAPABILITIES, None) {
        Ok(caps) => {
            let declared = caps["capabilities"]
                .as_array()
                .is_some_and(|list| list.iter().any(|cap| cap == SCENARIO_FUNCTION));
            if declared {
                Ok(())
            } else if caps["companion"] == true {
                Err(
                    "the dev session's companion does not answer function rules yet (it did not \
                     declare the `scenario.function` capability)"
                        .into(),
                )
            } else {
                Err(
                    "this dev session has no companion (.lingxia/dev-companion.json), so nothing \
                     answers function rules"
                        .into(),
                )
            }
        }
        // A `lingxia dev` from before function rules relays the method to
        // the runtime, which does not know it.
        Err(err) if err.code == "unknown_method" || err.message.contains("unknown") => {
            Err("this `lingxia dev` predates function rules; update LingXia".into())
        }
        Err(err) => Err(err.message),
    }
}

/// The host's `session.network.scenario.use` arguments.
fn host_args(loaded: &Loaded, target: &Target, appid: Option<&str>, dry_run: bool) -> Value {
    json!({
        "scenario": loaded.value,
        "variant": target.variant,
        "source": target.source,
        "appid": appid,
        "dryRun": dry_run,
    })
}

/// Install `loaded` (with `target.variant`) as the dev scenario, replacing
/// the active one. Everything is validated first; `function` rules are
/// installed before `http` rules and rolled back if those fail, so a
/// failure leaves no part of the new scenario installed. Returns the
/// status.
pub(crate) fn install(
    session: &dyn Session,
    loaded: &Loaded,
    target: &Target,
    appid: Option<&str>,
) -> Result<Value> {
    let source = &target.source;
    let resolved = loaded
        .file
        .resolve(target.variant.as_deref())
        .map_err(|err| anyhow!("{source}: {err}"))?;
    session
        .host(
            method::SCENARIO_USE,
            Some(host_args(loaded, target, appid, true)),
        )
        .with_context(|| format!("{source} is not a valid scenario"))?;
    let functions = resolved.function_rules().count();
    let support = companion_support(session);
    if functions > 0 {
        if let Err(reason) = &support {
            bail!("{source}: {}", resolved.functions_unsupported(reason));
        }
        let params = serde_json::to_value(resolved.companion_use(DEV_OWNER, Some(source)))?;
        session
            .companion(companion_method::SCENARIO_USE, Some(params))
            .map_err(|err| anyhow!("{source}: {}", companion_failure(&resolved, &err)))?;
    } else if support.is_ok() {
        // The previous scenario's function rules must not outlive it.
        session
            .companion(
                companion_method::SCENARIO_CLEAR,
                Some(json!({ "owner": DEV_OWNER })),
            )
            .map_err(|err| {
                anyhow!(
                    "cannot clear the companion's previous scenario: {}",
                    err.message
                )
            })?;
    }
    match session.host(
        method::SCENARIO_USE,
        Some(host_args(loaded, target, appid, false)),
    ) {
        Ok(status) => Ok(status),
        Err(err) => {
            if functions > 0 {
                let _ = session.companion(
                    companion_method::SCENARIO_CLEAR,
                    Some(json!({ "owner": DEV_OWNER })),
                );
            }
            Err(err.context(format!(
                "{source} failed to install; nothing of it stays installed"
            )))
        }
    }
}

fn companion_failure(resolved: &Resolved, err: &CallError) -> String {
    if err.code == companion_method::UNSUPPORTED {
        resolved.functions_unsupported(&err.message)
    } else {
        resolved.companion_error(&err.code, &err.message, err.data.as_ref())
    }
}

/// The host's status with the companion's hit counts on `function` rules.
pub(crate) fn status(session: &dyn Session) -> Result<Value> {
    let mut status = session.host(method::STATUS, None)?;
    let companion = companion_support(session)
        .ok()
        .and_then(|()| {
            session
                .companion(companion_method::SCENARIO_STATUS, Some(json!({})))
                .ok()
        })
        .and_then(|value| serde_json::from_value::<companion::StatusResult>(value).ok());
    if let Some(companion) = &companion {
        merge_function_hits(&mut status, companion);
    }
    Ok(status)
}

/// Put the dev owner's per-rule hits on the `function` rules, in order.
pub(crate) fn merge_function_hits(status: &mut Value, companion: &companion::StatusResult) {
    let Some(dev) = companion
        .owners
        .iter()
        .find(|owner| owner.owner == DEV_OWNER)
    else {
        return;
    };
    let Some(rules) = status["scenario"]["rules"].as_array_mut() else {
        return;
    };
    for (rule, hits) in rules
        .iter_mut()
        .filter(|rule| rule["kind"] == "function")
        .zip(&dev.rules)
    {
        rule["hits"] = json!(hits.hits);
    }
    status["scenario"]["companionActive"] = json!(dev.active);
}

/// Requests (and Function calls) that reached the active scenario.
pub(crate) fn reached(status: &Value) -> u64 {
    let requests = status["scenario"]["requests"].as_u64().unwrap_or(0);
    let functions: u64 = status["scenario"]["rules"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|rule| rule["kind"] == "function")
        .filter_map(|rule| rule["hits"].as_u64())
        .sum();
    requests + functions
}

/// Clear the dev scenario on both sides; both are asked even when one
/// fails.
pub(crate) fn clear(session: &dyn Session) -> Result<Value> {
    let host = session.host(method::SCENARIO_CLEAR, None);
    let companion = match companion_support(session) {
        Ok(()) => Some(session.companion(
            companion_method::SCENARIO_CLEAR,
            Some(json!({ "owner": DEV_OWNER })),
        )),
        Err(_) => None,
    };
    let host = host?;
    let companion_cleared = match companion {
        Some(Ok(result)) => result["cleared"] == true,
        Some(Err(err)) => bail!("cannot clear the companion's scenario: {}", err.message),
        None => false,
    };
    Ok(json!({
        "cleared": host["cleared"] == true || companion_cleared,
        "http": host["cleared"],
        "function": companion_cleared,
    }))
}

// -------------------------------- --watch --------------------------------

/// What one look at the watched file found.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum WatchEvent {
    /// The file changed and installed.
    Installed(Value),
    /// The file changed but did not install; the last valid one answers.
    Rejected(String),
}

/// Reinstall `target` whenever its content changes, until `stop`.
/// `initial` is the content installed (or rejected) before watching.
pub(crate) fn watch(
    session: &dyn Session,
    target: &Target,
    appid: Option<&str>,
    initial: Option<String>,
    interval: Duration,
    stop: &dyn Fn() -> bool,
    report: &mut dyn FnMut(WatchEvent),
) {
    let mut last = initial;
    while !stop() {
        std::thread::sleep(interval);
        let Ok(text) = std::fs::read_to_string(&target.path) else {
            continue;
        };
        if last.as_deref() == Some(text.as_str()) {
            continue;
        }
        last = Some(text.clone());
        let installed = parse_text(&target.path, &text)
            .and_then(|loaded| install(session, &loaded, target, appid));
        report(match installed {
            Ok(status) => WatchEvent::Installed(status),
            Err(err) => WatchEvent::Rejected(format!("{err:#}")),
        });
    }
}

// -------------------------------- command --------------------------------

pub fn execute(info: Option<&SessionInfo>, options: ScenarioOptions) -> Result<()> {
    let cwd = std::env::current_dir()?;
    let roots = match info {
        Some(info) => search_roots(info, &cwd),
        None => local_roots(&cwd),
    };
    if let ScenarioCommand::List { json } = options.command {
        return list(&roots, json);
    }
    let info =
        info.ok_or_else(|| anyhow!("No live dev session found. Run `lingxia dev` first."))?;
    let session = Live {
        ws: info.ws_url.clone(),
    };
    match options.command {
        ScenarioCommand::List { .. } => unreachable!("handled above"),
        ScenarioCommand::Use {
            scenario,
            appid,
            watch: watching,
            json,
        } => {
            let target = resolve(&scenario, &roots, &cwd)?;
            let initial = std::fs::read_to_string(&target.path).ok();
            let installed = read_file(&target.path)
                .and_then(|loaded| install(&session, &loaded, &target, appid.as_deref()));
            let status = match installed {
                Ok(status) => status,
                Err(err) if watching => {
                    eprintln!("{} {err:#}", "error".red().bold());
                    Value::Null
                }
                Err(err) => return Err(err),
            };
            if !status.is_null() {
                let status = if json {
                    status
                } else {
                    self::status(&session)?
                };
                let value = json!({ "source": target.source, "variant": target.variant, "path": target.path, "status": status });
                network::print(&value, json, |value| {
                    print_status(&value["status"]);
                    network::print_warning(&value["status"]);
                    eprintln!(
                        "{} the app answers from this scenario until `lxdev scenario clear`, \
                         another `lxdev scenario use`, or the end of the dev session",
                        "warning".yellow().bold()
                    );
                })?;
                if !json && !watching && std::io::stderr().is_terminal() {
                    wait_for_first_request(&session);
                }
            }
            if watching {
                eprintln!(
                    "watching {} — saving it reinstalls the scenario; Ctrl-C stops watching \
                     (the scenario stays installed)",
                    target.path.display()
                );
                watch(
                    &session,
                    &target,
                    appid.as_deref(),
                    initial,
                    WATCH_INTERVAL,
                    &|| false,
                    &mut |event| match event {
                        WatchEvent::Installed(status) => {
                            if json {
                                println!("{}", json!({ "event": "installed", "status": status }));
                            } else {
                                let label =
                                    status["scenario"]["label"].as_str().unwrap_or("scenario");
                                let rules =
                                    status["scenario"]["rules"].as_array().map_or(0, Vec::len);
                                println!("reinstalled '{label}' ({rules} rule{})", plural(rules));
                            }
                        }
                        WatchEvent::Rejected(error) => {
                            if json {
                                println!("{}", json!({ "event": "rejected", "error": error }));
                            } else {
                                eprintln!(
                                    "{} {error}\n  the last valid version keeps answering",
                                    "error".red().bold()
                                );
                            }
                        }
                    },
                );
            }
            Ok(())
        }
        ScenarioCommand::Status { json } => {
            let status = self::status(&session)?;
            network::print(&status, json, print_status)
        }
        ScenarioCommand::Clear { json } => {
            let result = clear(&session)?;
            network::print(&result, json, |result| {
                if result["cleared"] == true {
                    println!("scenario cleared; the app reaches its real backends again");
                } else {
                    println!("no scenario was active");
                }
            })
        }
    }
}

fn plural(n: usize) -> &'static str {
    if n == 1 { "" } else { "s" }
}

/// Wait briefly for a first request; say why none may come.
fn wait_for_first_request(session: &dyn Session) {
    let deadline = Instant::now() + FIRST_REQUEST_WAIT;
    while Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(250));
        match status(session) {
            Ok(status) if status["active"] != true || reached(&status) > 0 => return,
            Ok(_) => {}
            Err(_) => return,
        }
    }
    eprintln!("{} {IDLE_HINT}", "hint".cyan().bold());
}

pub(crate) fn print_status(status: &Value) {
    match status
        .get("scenario")
        .filter(|scenario| scenario.is_object())
    {
        Some(scenario) => {
            let label = scenario["label"].as_str().unwrap_or("unnamed");
            let source = scenario["source"]
                .as_str()
                .filter(|source| *source != label)
                .map(|source| format!(" ({source})"))
                .unwrap_or_default();
            println!(
                "{} scenario '{label}'{source} for {} since {}",
                "ACTIVE".yellow().bold(),
                scenario["appid"].as_str().unwrap_or("?"),
                scenario["installedAt"].as_str().unwrap_or("?"),
            );
            for line in rule_lines(scenario) {
                println!("  {line}");
            }
            let requests = scenario["requests"].as_u64().unwrap_or(0);
            println!(
                "  {requests} request{} reached it",
                plural(requests as usize)
            );
            for request in scenario["lastRequests"].as_array().into_iter().flatten() {
                println!("  {}", request_line(request));
            }
            if status["suspended"] == true {
                println!(
                    "  (standing aside while a test run is active; it answers again when the run ends)"
                );
            } else if reached(status) == 0 {
                println!("  {} {IDLE_HINT}", "hint".cyan().bold());
            }
        }
        None => {
            println!("no scenario is active");
            network::print_last_cleared(status);
        }
    }
    network::print_recording(status);
}

/// `rule 1 GET **/wifi/main answered 2×` per rule.
pub(crate) fn rule_lines(scenario: &Value) -> Vec<String> {
    let rules = scenario["rules"].as_array().cloned().unwrap_or_default();
    let width = rules
        .iter()
        .map(|rule| rule["target"].as_str().unwrap_or("").len())
        .max()
        .unwrap_or(0);
    rules
        .iter()
        .map(|rule| {
            let target = rule["target"].as_str().unwrap_or("");
            let answered = match rule["hits"].as_u64() {
                Some(hits) => format!("answered {hits}×"),
                None => "answered ?× (the companion does not report)".to_string(),
            };
            let expired = if rule["installed"] == false {
                " (times used up)"
            } else {
                ""
            };
            format!(
                "rule {} {target:<width$}  {answered}{expired}",
                rule["index"],
                width = width
            )
        })
        .collect()
}

fn request_line(request: &Value) -> String {
    let answered = match request["rule"].as_u64() {
        Some(rule) => format!(
            "rule {rule} {}{}",
            request["action"].as_str().unwrap_or(""),
            request["status"]
                .as_u64()
                .map(|status| format!(" {status}"))
                .unwrap_or_default()
        ),
        None => "no rule matched → real".to_string(),
    };
    let mut line = format!(
        "{} {} {} → {answered}",
        request["time"].as_str().unwrap_or(""),
        request["method"].as_str().unwrap_or(""),
        request["url"].as_str().unwrap_or(""),
    );
    if let Some(no_match) = request["noMatch"].as_str() {
        line.push_str(&format!("\n    {no_match}"));
    }
    line
}

fn list(roots: &[PathBuf], json: bool) -> Result<()> {
    let found = discover(roots);
    let entries: Vec<Value> = found
        .iter()
        .map(|found| match &found.file {
            Ok(file) => json!({
                "name": found.name,
                "path": found.path,
                "title": file.name,
                "description": file.description,
                "use": found.usable(),
                "variants": file.variants.iter().map(|(name, variant)| json!({
                    "name": name,
                    "description": variant.description,
                    "rules": variant.rules.len(),
                })).collect::<Vec<_>>(),
                "rules": file.rules.len(),
            }),
            Err(err) => json!({ "name": found.name, "path": found.path, "error": err }),
        })
        .collect();
    let value = json!({ "roots": roots, "scenarios": entries });
    network::print(&value, json, |_| {
        if found.is_empty() {
            let looked = roots
                .iter()
                .map(|root| root.display().to_string())
                .collect::<Vec<_>>()
                .join(", ");
            println!("no scenarios in {looked}");
            return;
        }
        for line in list_lines(&found) {
            println!("{line}");
        }
    })
}

/// One line per usable `name` and `name:variant`.
pub(crate) fn list_lines(found: &[Found]) -> Vec<String> {
    let width = found
        .iter()
        .flat_map(Found::usable)
        .map(|name| name.len())
        .max()
        .unwrap_or(0);
    let mut lines = Vec::new();
    for found in found {
        let file = match &found.file {
            Ok(file) => file,
            Err(err) => {
                lines.push(format!(
                    "{:<width$}  {} {err}",
                    found.name,
                    "invalid".red(),
                    width = width
                ));
                continue;
            }
        };
        let about = match (&file.name, &file.description) {
            (Some(name), Some(description)) => format!("{name} — {description}"),
            (Some(text), None) | (None, Some(text)) => text.clone(),
            (None, None) => String::new(),
        };
        if file.usable_without_variant() {
            lines.push(format!(
                "{:<width$}  {} rule{}  {about}",
                found.name,
                file.rules.len(),
                plural(file.rules.len()),
                width = width
            ));
        } else if !about.is_empty() {
            lines.push(format!("{:<width$}  {about}", found.name, width = width));
        }
        for (name, variant) in &file.variants {
            let rules = variant.rules.len() + file.rules.len();
            lines.push(format!(
                "{:<width$}  {rules} rule{}  {}",
                format!("{}:{name}", found.name),
                plural(rules),
                variant.description.as_deref().unwrap_or(""),
                width = width
            ));
        }
    }
    lines
        .iter()
        .map(|line| line.trim_end().to_string())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;

    fn write(path: &Path, text: &str) {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, text).unwrap();
    }

    #[test]
    fn a_bad_scenario_file_names_where_it_broke() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("bad.json");
        std::fs::write(&file, "{\n  \"rules\": [,]\n}").unwrap();
        let err = read_file(&file).unwrap_err().to_string();
        assert!(err.contains("line 2"), "{err}");
        std::fs::write(&file, r#"{ "http": { "routes": [] } }"#).unwrap();
        let err = read_file(&file).unwrap_err().to_string();
        assert!(err.contains("'http' is the old scenario format"), "{err}");
    }

    const WIFI: &str = r#"{
        "name": "Wi-Fi",
        "rules": [{ "http": "GET **/wifi/clients", "json": [] }],
        "variants": {
            "a": { "rules": [{ "http": "GET **/wifi/main", "json": { "ssid": "A" } }] },
            "b": { "description": "the B network", "rules": [{ "http": "GET **/wifi/main", "json": { "ssid": "B" } }] }
        }
    }"#;

    #[test]
    fn names_and_variants_resolve_against_the_roots_in_order() {
        let dir = tempfile::tempdir().unwrap();
        let content = dir.path().join("app/tests/scenarios");
        let project = dir.path().join("tests/scenarios");
        write(
            &content.join("qoe/offline.json"),
            r#"{ "name": "content offline", "rules": [{ "http": "* **", "status": 503 }] }"#,
        );
        write(
            &project.join("qoe/offline.json"),
            r#"{ "name": "project offline", "rules": [{ "http": "* **", "status": 503 }] }"#,
        );
        write(&project.join("wifi.json"), WIFI);
        write(
            &project.join("only-variants.json"),
            r#"{ "variants": { "x": { "rules": [{ "http": "GET y", "status": 200 }] } } }"#,
        );
        write(&project.join("broken.json"), "{");
        write(&project.join(".hidden.json"), "{}");
        write(&project.join("notes.txt"), "x");
        let roots = vec![content.clone(), project.clone()];

        let found = discover(&roots);
        let names: Vec<&str> = found.iter().map(|found| found.name.as_str()).collect();
        assert_eq!(
            names,
            vec!["broken", "only-variants", "qoe/offline", "wifi"]
        );
        // The content directory shadows the project root.
        assert_eq!(
            found[2].file.as_ref().unwrap().name.as_deref(),
            Some("content offline")
        );
        assert!(found[0].file.is_err());
        let usable: Vec<String> = found.iter().flat_map(Found::usable).collect();
        assert_eq!(
            usable,
            [
                "broken",
                "only-variants:x",
                "qoe/offline",
                "wifi",
                "wifi:a",
                "wifi:b"
            ]
        );
        let lines = list_lines(&found);
        assert!(
            lines
                .iter()
                .any(|line| line.starts_with("wifi:b") && line.contains("2 rules  the B network")),
            "{lines:?}"
        );
        assert!(
            lines.contains(&"only-variants:x  1 rule".to_string()),
            "{lines:?}"
        );
        assert!(
            lines.contains(&"wifi             1 rule  Wi-Fi".to_string()),
            "{lines:?}"
        );

        let cwd = dir.path();
        let target = resolve("qoe/offline", &roots, cwd).unwrap();
        assert_eq!(target.path, content.join("qoe/offline.json"));
        assert_eq!(target.source, "qoe/offline");
        assert_eq!(target.variant, None);
        let target = resolve("wifi:b", &roots, cwd).unwrap();
        assert_eq!(target.path, project.join("wifi.json"));
        assert_eq!(
            (target.source.as_str(), target.variant.as_deref()),
            ("wifi", Some("b"))
        );
        // A path wins, and is logged as given; a variant may follow it.
        let target = resolve("tests/scenarios/wifi.json:a", &roots, cwd).unwrap();
        assert_eq!(target.path, cwd.join("tests/scenarios/wifi.json"));
        assert_eq!(target.source, "tests/scenarios/wifi.json");
        assert_eq!(target.variant.as_deref(), Some("a"));

        let err = resolve("qoe/online", &roots, cwd).unwrap_err().to_string();
        assert!(
            err.contains("available: broken, only-variants:x, qoe/offline, wifi, wifi:a, wifi:b"),
            "{err}"
        );
        assert!(resolve("../scenarios/wifi", &roots, &content).is_err());
    }

    #[test]
    fn search_roots_prefer_content_then_project_then_the_local_lxapp() {
        let dir = tempfile::tempdir().unwrap();
        let project = dir.path().canonicalize().unwrap();
        std::fs::write(project.join("lingxia.yaml"), "").unwrap();
        let lxapp = project.join("lxapp");
        write(&lxapp.join("package.json"), "{}");
        let info = SessionInfo {
            session_id: "s".into(),
            project_root: project.display().to_string(),
            content: Some(SessionContent::Host {
                path: project.display().to_string(),
            }),
            target: "macos".into(),
            pid: 1,
            started_at: 0,
            executable: String::new(),
            ws_url: "ws://127.0.0.1:1".into(),
            log_file: String::new(),
            name: None,
            build: None,
            extra: Default::default(),
        };
        assert_eq!(
            search_roots(&info, &lxapp),
            vec![project.join(SCENARIO_DIR), lxapp.join(SCENARIO_DIR)]
        );
        let elsewhere = tempfile::tempdir().unwrap();
        assert_eq!(
            search_roots(&info, elsewhere.path()),
            vec![project.join(SCENARIO_DIR)]
        );
    }

    /// A session that records what it was asked. The host validates with
    /// the shared format only; `companion` is `None` for no companion, or
    /// its capabilities.
    struct Fake {
        log: RefCell<Vec<String>>,
        companion: Option<Vec<&'static str>>,
        host_fails: bool,
        companion_error: Option<CallError>,
        status: Value,
    }

    impl Fake {
        fn new(companion: Option<Vec<&'static str>>) -> Self {
            Self {
                log: RefCell::new(Vec::new()),
                companion,
                host_fails: false,
                companion_error: None,
                status: json!({ "active": true, "scenario": { "rules": [] } }),
            }
        }

        fn log(&self) -> Vec<String> {
            self.log.borrow().clone()
        }
    }

    impl Session for Fake {
        fn host(&self, method: &str, params: Option<Value>) -> Result<Value> {
            let params = params.unwrap_or_default();
            if method == method::SCENARIO_USE {
                let dry = params["dryRun"] == true;
                self.log
                    .borrow_mut()
                    .push(format!("host use{}", if dry { " (dry run)" } else { "" }));
                format::parse_file(&params["scenario"])
                    .and_then(|file| file.resolve(params["variant"].as_str()))
                    .map_err(|err| anyhow!("invalid scenario: {err}"))?;
                if !dry && self.host_fails {
                    bail!("host refused");
                }
                return Ok(json!({ "active": !dry }));
            }
            self.log.borrow_mut().push(format!("host {method}"));
            if method == method::STATUS {
                return Ok(self.status.clone());
            }
            Ok(json!({ "cleared": true }))
        }

        fn companion(&self, method: &str, params: Option<Value>) -> Result<Value, CallError> {
            if method == companion_method::CAPABILITIES {
                return Ok(json!({
                    "companion": self.companion.is_some(),
                    "capabilities": self.companion.clone().unwrap_or_default(),
                }));
            }
            let owner = params
                .as_ref()
                .map(|p| p["owner"].clone())
                .unwrap_or_default();
            self.log
                .borrow_mut()
                .push(format!("companion {method} {owner}"));
            if method == companion_method::SCENARIO_USE
                && let Some(err) = &self.companion_error
            {
                return Err(err.clone());
            }
            if method == companion_method::SCENARIO_STATUS {
                return Ok(
                    json!({ "owners": [{ "owner": "dev", "active": true, "rules": [{ "hits": 3 }] }] }),
                );
            }
            Ok(json!({ "installed": 1, "cleared": true }))
        }
    }

    fn loaded(text: &str) -> Loaded {
        parse_text(Path::new("s.json"), text).unwrap()
    }

    fn target(variant: Option<&str>) -> Target {
        Target {
            path: PathBuf::from("s.json"),
            source: "checkout".into(),
            variant: variant.map(str::to_string),
        }
    }

    const CHECKOUT: &str = r#"{
        "name": "Checkout",
        "rules": [
            { "http": "GET **/cart", "json": { "items": 1 } },
            { "function": "orders.submit", "fault": "unknown" }
        ]
    }"#;

    #[test]
    fn function_rules_need_a_companion_that_declared_them_and_nothing_installs_otherwise() {
        for (companion, reason) in [
            (None, "this dev session has no companion"),
            (
                Some(vec!["requests"]),
                "did not declare the `scenario.function` capability",
            ),
        ] {
            let session = Fake::new(companion);
            let err = install(&session, &loaded(CHECKOUT), &target(None), None)
                .unwrap_err()
                .to_string();
            assert!(
                err.contains("1 function rule (rule 2 function orders.submit) cannot be installed"),
                "{err}"
            );
            assert!(err.contains(reason), "{err}");
            assert!(err.contains("Nothing was installed"), "{err}");
            // Only the dry run happened.
            assert_eq!(session.log(), ["host use (dry run)"]);
        }
    }

    #[test]
    fn a_capable_companion_gets_the_function_rules_first_and_loses_them_on_failure() {
        let session = Fake::new(Some(vec!["requests", SCENARIO_FUNCTION]));
        install(&session, &loaded(CHECKOUT), &target(None), None).unwrap();
        assert_eq!(
            session.log(),
            [
                "host use (dry run)",
                "companion session.companion.scenario.use \"dev\"",
                "host use"
            ]
        );

        let mut failing = Fake::new(Some(vec![SCENARIO_FUNCTION]));
        failing.host_fails = true;
        let err = install(&failing, &loaded(CHECKOUT), &target(None), None).unwrap_err();
        assert!(
            format!("{err:#}").contains("nothing of it stays installed"),
            "{err:#}"
        );
        assert_eq!(
            failing.log(),
            [
                "host use (dry run)",
                "companion session.companion.scenario.use \"dev\"",
                "host use",
                "companion session.companion.scenario.clear \"dev\""
            ]
        );

        // Companion validation errors name the rule in the file.
        let mut refusing = Fake::new(Some(vec![SCENARIO_FUNCTION]));
        refusing.companion_error = Some(CallError {
            code: companion::INVALID_RULES.into(),
            message: "invalid".into(),
            data: Some(json!({ "errors": [{ "rule": 0, "message": "unknown Function" }] })),
        });
        let err = install(&refusing, &loaded(CHECKOUT), &target(None), None)
            .unwrap_err()
            .to_string();
        assert_eq!(
            err,
            "checkout: rule 2 (rules[1]) function orders.submit: unknown Function"
        );
        assert!(!refusing.log().contains(&"host use".to_string()));
    }

    #[test]
    fn an_http_only_scenario_clears_the_previous_function_rules() {
        let session = Fake::new(Some(vec![SCENARIO_FUNCTION]));
        install(&session, &loaded(WIFI), &target(Some("b")), None).unwrap();
        assert_eq!(
            session.log(),
            [
                "host use (dry run)",
                "companion session.companion.scenario.clear \"dev\"",
                "host use"
            ]
        );
        // Without a companion there is nothing to clear.
        let plain = Fake::new(None);
        install(&plain, &loaded(WIFI), &target(Some("b")), None).unwrap();
        assert_eq!(plain.log(), ["host use (dry run)", "host use"]);
        // An unknown variant fails before anything is sent.
        let err = install(&plain, &loaded(WIFI), &target(Some("c")), None)
            .unwrap_err()
            .to_string();
        assert!(err.contains("no variant 'c' (variants: a, b)"), "{err}");
    }

    #[test]
    fn status_puts_companion_hits_on_function_rules_and_says_when_nothing_arrived() {
        let mut session = Fake::new(Some(vec![SCENARIO_FUNCTION]));
        session.status = json!({
            "active": true,
            "suspended": false,
            "scenario": {
                "label": "Checkout", "requests": 0,
                "rules": [
                    { "index": 1, "target": "GET **/cart", "kind": "http", "hits": 0, "installed": true },
                    { "index": 2, "target": "function orders.submit", "kind": "function" }
                ]
            }
        });
        let status = status(&session).unwrap();
        assert_eq!(status["scenario"]["rules"][1]["hits"], 3);
        assert_eq!(reached(&status), 3);
        let lines = rule_lines(&status["scenario"]);
        assert_eq!(lines[0], "rule 1 GET **/cart             answered 0×");
        assert_eq!(lines[1], "rule 2 function orders.submit  answered 3×");

        let idle =
            json!({ "scenario": { "requests": 0, "rules": [{ "kind": "http", "hits": 0 }] } });
        assert_eq!(reached(&idle), 0);
    }

    #[test]
    fn clear_asks_both_sides() {
        let session = Fake::new(Some(vec![SCENARIO_FUNCTION]));
        let result = clear(&session).unwrap();
        assert_eq!(result["cleared"], true);
        assert_eq!(
            session.log(),
            [
                "host session.network.scenario.clear",
                "companion session.companion.scenario.clear \"dev\""
            ]
        );
        let plain = Fake::new(None);
        clear(&plain).unwrap();
        assert_eq!(plain.log(), ["host session.network.scenario.clear"]);
    }

    #[test]
    fn watch_reinstalls_valid_saves_and_keeps_the_last_valid_one() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("wifi.json");
        std::fs::write(&path, WIFI).unwrap();
        let session = Fake::new(None);
        let target = Target {
            path: path.clone(),
            source: "wifi".into(),
            variant: Some("a".into()),
        };
        let saves = [
            // An edit that installs.
            WIFI.replace("\"A\"", "\"A2\""),
            // An invalid edit: reported, nothing installed.
            WIFI.replace(
                "\"GET **/wifi/main\", \"json\": { \"ssid\": \"A\" }",
                "\"**/wifi/main\", \"json\": {}",
            ),
            // Broken JSON.
            "{".to_string(),
            // Fixed again.
            WIFI.to_string(),
        ];
        let step = RefCell::new(0usize);
        let mut events = Vec::new();
        watch(
            &session,
            &target,
            None,
            Some(WIFI.to_string()),
            Duration::from_millis(1),
            &|| {
                let mut step = step.borrow_mut();
                if *step >= saves.len() * 2 {
                    return true;
                }
                // Write a new version every other poll; an unchanged file
                // is not reinstalled.
                if step.is_multiple_of(2) {
                    std::fs::write(&path, &saves[*step / 2]).unwrap();
                }
                *step += 1;
                false
            },
            &mut |event| events.push(event),
        );
        assert_eq!(events.len(), 4, "{events:?}");
        assert!(matches!(events[0], WatchEvent::Installed(_)));
        let WatchEvent::Rejected(invalid) = &events[1] else {
            panic!("expected a rejection: {events:?}");
        };
        assert!(invalid.contains("variants.a.rules[0]"), "{invalid}");
        let WatchEvent::Rejected(broken) = &events[2] else {
            panic!("expected a rejection: {events:?}");
        };
        assert!(broken.contains("is not valid JSON"), "{broken}");
        assert!(matches!(events[3], WatchEvent::Installed(_)));
        let installs = session
            .log()
            .iter()
            .filter(|line| *line == "host use")
            .count();
        assert_eq!(installs, 2);
    }
}
