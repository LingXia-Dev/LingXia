//! `lxdev scenario`: put the running app into a named product state
//! ("gateway offline", "stale data") in a dev session, outside any test run.
//!
//! A scenario file has one section per provider. `http` (or top-level
//! `routes`) answers the app's Logic `fetch` and `Rong.SSE` through the
//! host's `session.network.scenario.*` methods; `worker` is reserved and
//! refused until a provider exists. lxdev splits the file and installs each
//! section through its provider, rolling back on failure, so the host
//! protocol only ever sees HTTP routes.

use crate::network;
use crate::project::SessionInfo;
use anyhow::{Context, Result, anyhow, bail};
use clap::{Args, Subcommand};
use lingxia_control_protocol::dev_session::broker::SessionContent;
use lingxia_control_protocol::methods::session::network as method;
use owo_colors::OwoColorize;
use serde_json::{Map, Value, json};
use std::collections::BTreeMap;
use std::path::{Component, Path, PathBuf};

/// Where named scenarios live, relative to a project directory.
pub const SCENARIO_DIR: &str = "tests/scenarios";

/// Sections a scenario file may have, in install order: a provider that
/// cannot be rolled back cheaply goes first.
const SECTIONS: [&str; 2] = ["worker", "http"];
const FILE_KEYS: [&str; 6] = ["$schema", "name", "description", "routes", "http", "worker"];

#[derive(Args, Clone)]
pub struct ScenarioOptions {
    #[command(subcommand)]
    command: ScenarioCommand,
}

#[derive(Subcommand, Clone)]
enum ScenarioCommand {
    /// List the scenarios under tests/scenarios/ of the session's project
    List {
        /// Print JSON output
        #[arg(long)]
        json: bool,
    },
    /// Install a scenario by name (`qoe/offline`) or path until `clear`,
    /// another `use`, or the end of the dev session
    Use {
        /// Scenario name under tests/scenarios/ (without `.json`), or a file
        scenario: String,
        /// Target lxapp (default: the home lxapp, else the current one)
        #[arg(long)]
        appid: Option<String>,
        /// Print JSON output
        #[arg(long)]
        json: bool,
    },
    /// Show the active scenario per section, and the last one cleared
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

/// A scenario file split into sections.
#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct ScenarioFile {
    pub name: Option<String>,
    pub description: Option<String>,
    /// Section name → its content. Top-level `routes` is `http: { routes }`.
    pub sections: BTreeMap<String, Value>,
}

pub(crate) fn parse_file(value: &Value) -> Result<ScenarioFile, String> {
    let Value::Object(fields) = value else {
        return Err("a scenario must be a JSON object".into());
    };
    if let Some(unknown) = fields.keys().find(|key| !FILE_KEYS.contains(&key.as_str())) {
        return Err(format!(
            "unknown scenario field '{unknown}' (allowed: {})",
            FILE_KEYS.join(", ")
        ));
    }
    let text = |key: &str| match fields.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(text)) => Ok(Some(text.clone())),
        Some(_) => Err(format!("scenario {key} must be a string")),
    };
    let mut file = ScenarioFile {
        name: text("name")?,
        description: text("description")?,
        sections: BTreeMap::new(),
    };
    match (fields.get("routes"), fields.get("http")) {
        (Some(_), Some(_)) => {
            return Err(
                "a scenario lists its routes at the top level or under http, not both".into(),
            );
        }
        (Some(routes), None) => {
            file.sections
                .insert("http".into(), json!({ "routes": routes }));
        }
        (None, Some(Value::Object(http))) => {
            if let Some(unknown) = http.keys().find(|key| key.as_str() != "routes") {
                return Err(format!(
                    "unknown http section field '{unknown}' (allowed: routes)"
                ));
            }
            let routes = http
                .get("routes")
                .ok_or("the http section needs a routes array")?;
            file.sections
                .insert("http".into(), json!({ "routes": routes }));
        }
        (None, Some(_)) => return Err("the http section must be an object".into()),
        (None, None) => {}
    }
    if let Some(worker) = fields.get("worker") {
        file.sections.insert("worker".into(), worker.clone());
    }
    if file.sections.is_empty() {
        return Err("a scenario needs an http section (or top-level routes)".into());
    }
    Ok(file)
}

pub(crate) fn read_file(path: &Path) -> Result<ScenarioFile> {
    let text = std::fs::read_to_string(path)
        .with_context(|| format!("cannot read scenario {}", path.display()))?;
    let value: Value = serde_json::from_str(&text).map_err(|err| {
        anyhow!(
            "{} is not valid JSON (line {}, column {}): {err}",
            path.display(),
            err.line(),
            err.column()
        )
    })?;
    parse_file(&value).map_err(|err| anyhow!("{}: {err}", path.display()))
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
                file: read_file(&path).map_err(|err| format!("{err:#}")),
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

/// A scenario argument: a file that exists, else a name under `roots`.
/// Returns the file and the label the host logs it by.
pub(crate) fn resolve(arg: &str, roots: &[PathBuf], cwd: &Path) -> Result<(PathBuf, String)> {
    let as_path = cwd.join(arg);
    if as_path.is_file() {
        return Ok((as_path, arg.to_string()));
    }
    let name = arg.replace('\\', "/");
    let name = name.strip_suffix(".json").unwrap_or(&name);
    let valid = !name.is_empty()
        && Path::new(name)
            .components()
            .all(|part| matches!(part, Component::Normal(_)));
    if valid {
        for root in roots {
            let candidate = root.join(format!("{name}.json"));
            if candidate.is_file() {
                return Ok((candidate, name.to_string()));
            }
        }
    }
    let names: Vec<String> = discover(roots)
        .into_iter()
        .map(|found| found.name)
        .collect();
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

// ------------------------------- providers -------------------------------

/// Installs one section of a scenario file into the running app.
pub(crate) trait SectionProvider {
    /// The file section it owns (`http`, `worker`).
    fn section(&self) -> &'static str;
    /// Whether its state stands aside by itself while an `lxdev test` run
    /// is active. A provider that cannot keeps test runs from starting.
    fn suspendable(&self) -> bool;
    fn install(
        &self,
        file: &ScenarioFile,
        section: &Value,
        source: &str,
        appid: Option<&str>,
    ) -> Result<Value>;
    /// Remove its state; `{ cleared: bool }`.
    fn clear(&self) -> Result<Value>;
    /// `{ active: bool, … }`.
    fn status(&self) -> Result<Value>;
    fn print_status(&self, status: &Value) {
        println!("{}: {status}", self.section());
    }
}

/// The `http` section: the host's dev-session route table.
struct HttpProvider {
    ws: String,
}

impl SectionProvider for HttpProvider {
    fn section(&self) -> &'static str {
        "http"
    }

    fn suspendable(&self) -> bool {
        // Dev routes stand aside while a run is active (the host checks).
        true
    }

    fn install(
        &self,
        file: &ScenarioFile,
        section: &Value,
        source: &str,
        appid: Option<&str>,
    ) -> Result<Value> {
        let mut scenario = Map::new();
        if let Some(name) = &file.name {
            scenario.insert("name".into(), json!(name));
        }
        if let Some(description) = &file.description {
            scenario.insert("description".into(), json!(description));
        }
        // The host takes the flat form; the section holds only `routes`.
        scenario.insert("routes".into(), section["routes"].clone());
        network::call(
            &self.ws,
            method::SCENARIO_USE,
            Some(json!({ "scenario": scenario, "source": source, "appid": appid })),
        )
    }

    fn clear(&self) -> Result<Value> {
        network::call(&self.ws, method::SCENARIO_CLEAR, None)
    }

    fn status(&self) -> Result<Value> {
        network::call(&self.ws, method::STATUS, None)
    }

    fn print_status(&self, status: &Value) {
        network::print_status(status);
    }
}

pub(crate) type Providers = Vec<Box<dyn SectionProvider>>;

/// The providers this lxdev has, in install order.
pub(crate) fn providers(ws_url: &str) -> Providers {
    vec![Box::new(HttpProvider {
        ws: ws_url.to_string(),
    })]
}

fn ordered(providers: &[Box<dyn SectionProvider>]) -> Vec<&dyn SectionProvider> {
    let mut ordered: Vec<&dyn SectionProvider> =
        providers.iter().map(|provider| provider.as_ref()).collect();
    ordered.sort_by_key(|provider| {
        SECTIONS
            .iter()
            .position(|section| *section == provider.section())
            .unwrap_or(SECTIONS.len())
    });
    ordered
}

/// Install every section of `file`, replacing the active scenario. When a
/// section fails, the sections installed before it are cleared again.
pub(crate) fn install(
    providers: &[Box<dyn SectionProvider>],
    file: &ScenarioFile,
    source: &str,
    appid: Option<&str>,
) -> Result<Map<String, Value>> {
    if let Some(section) = file
        .sections
        .keys()
        .find(|section| !providers.iter().any(|p| p.section() == section.as_str()))
    {
        bail!(
            "{source}: the {section} section is not supported yet; this lxdev installs only \
             http routes"
        );
    }
    let mut installed: Vec<(&dyn SectionProvider, Value)> = Vec::new();
    for provider in ordered(providers) {
        let Some(section) = file.sections.get(provider.section()) else {
            continue;
        };
        match provider.install(file, section, source, appid) {
            Ok(status) => installed.push((provider, status)),
            Err(err) => {
                let mut rollback = Vec::new();
                for (done, _) in installed.iter().rev() {
                    if let Err(undo) = done.clear() {
                        rollback.push(format!("{}: {undo:#}", done.section()));
                    }
                }
                let err = err.context(format!(
                    "the {} section of {source} failed; nothing of it stays installed",
                    provider.section()
                ));
                if rollback.is_empty() {
                    return Err(err);
                }
                return Err(err.context(format!(
                    "rolling back also failed ({}); run `lxdev scenario clear`",
                    rollback.join("; ")
                )));
            }
        }
    }
    // The file replaces the active scenario as a whole: a section it lacks
    // must not keep answering from the previous one.
    for provider in ordered(providers) {
        if !file.sections.contains_key(provider.section()) {
            provider.clear()?;
        }
    }
    Ok(installed
        .into_iter()
        .map(|(provider, status)| (provider.section().to_string(), status))
        .collect())
}

/// Clear every section; every provider is asked even when one fails.
pub(crate) fn clear(providers: &[Box<dyn SectionProvider>]) -> Result<Map<String, Value>> {
    let mut results = Map::new();
    let mut first_err = None;
    for provider in ordered(providers) {
        match provider.clear() {
            Ok(result) => {
                results.insert(provider.section().into(), result);
            }
            Err(err) => {
                first_err.get_or_insert(
                    err.context(format!("cannot clear the {} section", provider.section())),
                );
            }
        }
    }
    match first_err {
        Some(err) => Err(err),
        None => Ok(results),
    }
}

pub(crate) fn status(providers: &[Box<dyn SectionProvider>]) -> Result<Map<String, Value>> {
    ordered(providers)
        .into_iter()
        .map(|provider| Ok((provider.section().to_string(), provider.status()?)))
        .collect()
}

/// `lxdev test` refuses to start while a section that cannot stand aside
/// during a run is active: it would steer the tests.
pub(crate) fn refuse_test_while_blocking(providers: &[Box<dyn SectionProvider>]) -> Result<()> {
    for provider in providers.iter().filter(|provider| !provider.suspendable()) {
        let status = provider.status()?;
        if status["active"] == true {
            bail!(
                "scenario_active: the dev scenario's {} section is active and cannot stand aside \
                 during a test run; clear it with `lxdev scenario clear`, then run the tests",
                provider.section()
            );
        }
    }
    Ok(())
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
    let providers = providers(&info.ws_url);
    match options.command {
        ScenarioCommand::List { .. } => unreachable!("handled above"),
        ScenarioCommand::Use {
            scenario,
            appid,
            json,
        } => {
            let (path, source) = resolve(&scenario, &roots, &cwd)?;
            let file = read_file(&path)?;
            let sections = install(&providers, &file, &source, appid.as_deref())?;
            let value = json!({ "source": source, "path": path, "sections": sections });
            network::print(&value, json, |_| {
                for provider in ordered(&providers) {
                    if let Some(status) = sections.get(provider.section()) {
                        provider.print_status(status);
                        network::print_warning(status);
                    }
                }
                eprintln!(
                    "{} the app answers from this scenario until `lxdev scenario clear`, another \
                     `lxdev scenario use`, or the end of the dev session",
                    "warning".yellow().bold()
                );
            })
        }
        ScenarioCommand::Status { json } => {
            let sections = status(&providers)?;
            let active = sections.values().any(|status| status["active"] == true);
            let value = json!({ "active": active, "sections": sections });
            network::print(&value, json, |_| {
                for provider in ordered(&providers) {
                    if let Some(status) = sections.get(provider.section()) {
                        provider.print_status(status);
                    }
                }
            })
        }
        ScenarioCommand::Clear { json } => {
            let sections = clear(&providers)?;
            let cleared = sections.values().any(|result| result["cleared"] == true);
            let value = json!({ "cleared": cleared, "sections": sections });
            network::print(&value, json, |_| {
                if cleared {
                    println!("scenario cleared; the app reaches its real backends again");
                } else {
                    println!("no scenario was active");
                }
            })
        }
    }
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
                "sections": file.sections.keys().collect::<Vec<_>>(),
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
        let width = found
            .iter()
            .map(|found| found.name.len())
            .max()
            .unwrap_or(0);
        for found in &found {
            match &found.file {
                Ok(file) => {
                    let sections = file.sections.keys().cloned().collect::<Vec<_>>().join(",");
                    let about = match (&file.name, &file.description) {
                        (Some(name), Some(description)) => format!("{name} — {description}"),
                        (Some(text), None) | (None, Some(text)) => text.clone(),
                        (None, None) => String::new(),
                    };
                    println!(
                        "{:<width$}  [{sections}]  {about}",
                        found.name,
                        width = width
                    );
                }
                Err(err) => println!(
                    "{:<width$}  {} {err}",
                    found.name,
                    "invalid".red(),
                    width = width
                ),
            }
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    use std::rc::Rc;

    #[test]
    fn a_file_splits_into_sections() {
        let flat = parse_file(&json!({
            "$schema": "x", "name": "offline", "routes": [{ "url": "**", "status": 503 }]
        }))
        .unwrap();
        assert_eq!(flat.name.as_deref(), Some("offline"));
        assert_eq!(
            flat.sections["http"],
            json!({ "routes": [{ "url": "**", "status": 503 }] })
        );

        let sectioned = parse_file(&json!({
            "description": "d",
            "http": { "routes": [{ "url": "**", "status": 503 }] },
            "worker": { "state": {} }
        }))
        .unwrap();
        assert_eq!(
            sectioned.sections.keys().collect::<Vec<_>>(),
            vec!["http", "worker"]
        );
        assert_eq!(sectioned.description.as_deref(), Some("d"));

        for (file, expected) in [
            (json!([]), "JSON object"),
            (json!({ "route": [] }), "unknown scenario field 'route'"),
            (json!({ "routes": [], "http": {} }), "not both"),
            (json!({ "http": [] }), "must be an object"),
            (json!({ "http": {} }), "needs a routes array"),
            (
                json!({ "http": { "routes": [], "x": 1 } }),
                "unknown http section field 'x'",
            ),
            (json!({ "name": 1, "routes": [] }), "name must be a string"),
            (json!({ "name": "empty" }), "needs an http section"),
        ] {
            let err = parse_file(&file).unwrap_err();
            assert!(err.contains(expected), "{file}: {err}");
        }
    }

    #[test]
    fn a_bad_scenario_file_names_where_it_broke() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("bad.json");
        std::fs::write(&file, "{\n  \"routes\": [,]\n}").unwrap();
        let err = read_file(&file).unwrap_err().to_string();
        assert!(err.contains("line 2"), "{err}");
    }

    fn write(path: &Path, text: &str) {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, text).unwrap();
    }

    #[test]
    fn names_resolve_against_the_roots_in_order() {
        let dir = tempfile::tempdir().unwrap();
        let content = dir.path().join("app/tests/scenarios");
        let project = dir.path().join("tests/scenarios");
        write(
            &content.join("qoe/offline.json"),
            r#"{ "name": "content offline", "routes": [{ "url": "**", "status": 503 }] }"#,
        );
        write(
            &project.join("qoe/offline.json"),
            r#"{ "name": "project offline", "routes": [{ "url": "**", "status": 503 }] }"#,
        );
        write(
            &project.join("stale.json"),
            r#"{ "http": { "routes": [{ "url": "**", "json": {} }] } }"#,
        );
        write(&project.join("broken.json"), "{");
        write(&project.join(".hidden.json"), "{}");
        write(&project.join("notes.txt"), "x");
        let roots = vec![content.clone(), project.clone()];

        let found = discover(&roots);
        let names: Vec<&str> = found.iter().map(|found| found.name.as_str()).collect();
        assert_eq!(names, vec!["broken", "qoe/offline", "stale"]);
        // The content directory shadows the project root.
        assert_eq!(
            found[1].file.as_ref().unwrap().name.as_deref(),
            Some("content offline")
        );
        assert!(found[0].file.is_err());

        let cwd = dir.path();
        let (path, source) = resolve("qoe/offline", &roots, cwd).unwrap();
        assert_eq!(path, content.join("qoe/offline.json"));
        assert_eq!(source, "qoe/offline");
        assert_eq!(
            resolve("stale.json", &roots, cwd).unwrap().0,
            project.join("stale.json")
        );
        // A path wins, and is logged as given.
        let (path, source) = resolve("tests/scenarios/stale.json", &roots, cwd).unwrap();
        assert_eq!(path, cwd.join("tests/scenarios/stale.json"));
        assert_eq!(source, "tests/scenarios/stale.json");

        let err = resolve("qoe/online", &roots, cwd).unwrap_err().to_string();
        assert!(
            err.contains("available: broken, qoe/offline, stale"),
            "{err}"
        );
        assert!(resolve("../scenarios/stale", &roots, &content).is_err());
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
        };
        assert_eq!(
            search_roots(&info, &lxapp),
            vec![project.join(SCENARIO_DIR), lxapp.join(SCENARIO_DIR)]
        );
        // A directory outside the session adds nothing.
        let elsewhere = tempfile::tempdir().unwrap();
        assert_eq!(
            search_roots(&info, elsewhere.path()),
            vec![project.join(SCENARIO_DIR)]
        );
    }

    /// Records what it was asked, and fails `install` when told to.
    struct Fake {
        section: &'static str,
        suspendable: bool,
        fail: bool,
        active: bool,
        log: Rc<RefCell<Vec<String>>>,
    }

    impl SectionProvider for Fake {
        fn section(&self) -> &'static str {
            self.section
        }
        fn suspendable(&self) -> bool {
            self.suspendable
        }
        fn install(&self, _: &ScenarioFile, _: &Value, _: &str, _: Option<&str>) -> Result<Value> {
            self.log
                .borrow_mut()
                .push(format!("install {}", self.section));
            if self.fail {
                bail!("{} refused", self.section);
            }
            Ok(json!({ "active": true }))
        }
        fn clear(&self) -> Result<Value> {
            self.log
                .borrow_mut()
                .push(format!("clear {}", self.section));
            Ok(json!({ "cleared": true }))
        }
        fn status(&self) -> Result<Value> {
            self.log
                .borrow_mut()
                .push(format!("status {}", self.section));
            Ok(json!({ "active": self.active }))
        }
    }

    fn fake(
        section: &'static str,
        log: &Rc<RefCell<Vec<String>>>,
        configure: impl FnOnce(&mut Fake),
    ) -> Box<dyn SectionProvider> {
        let mut fake = Fake {
            section,
            suspendable: true,
            fail: false,
            active: false,
            log: log.clone(),
        };
        configure(&mut fake);
        Box::new(fake)
    }

    fn both() -> ScenarioFile {
        parse_file(&json!({
            "http": { "routes": [{ "url": "**", "status": 503 }] },
            "worker": { "state": {} }
        }))
        .unwrap()
    }

    #[test]
    fn sections_install_worker_first_and_roll_back_on_failure() {
        let log = Rc::new(RefCell::new(Vec::new()));
        let providers = vec![
            fake("http", &log, |fake| fake.fail = true),
            fake("worker", &log, |_| {}),
        ];
        let err = install(&providers, &both(), "qoe/offline", None).unwrap_err();
        assert!(
            format!("{err:#}").contains("http section of qoe/offline failed"),
            "{err:#}"
        );
        assert_eq!(
            *log.borrow(),
            vec!["install worker", "install http", "clear worker"]
        );

        let log = Rc::new(RefCell::new(Vec::new()));
        let providers = vec![fake("http", &log, |_| {}), fake("worker", &log, |_| {})];
        let flat = parse_file(&json!({ "routes": [{ "url": "**", "status": 503 }] })).unwrap();
        let sections = install(&providers, &flat, "offline", None).unwrap();
        assert_eq!(sections.keys().collect::<Vec<_>>(), vec!["http"]);
        // A section the new file lacks stops answering from the old one.
        assert_eq!(*log.borrow(), vec!["install http", "clear worker"]);
    }

    #[test]
    fn a_section_without_a_provider_is_not_supported_yet() {
        let log = Rc::new(RefCell::new(Vec::new()));
        let providers = vec![fake("http", &log, |_| {})];
        let err = install(&providers, &both(), "qoe/offline", None).unwrap_err();
        assert!(
            err.to_string()
                .contains("the worker section is not supported yet"),
            "{err}"
        );
        assert!(log.borrow().is_empty(), "nothing is installed");
        // The real provider set has no worker provider either.
        assert!(
            providers_for_test()
                .iter()
                .all(|provider| provider.section() != "worker")
        );
    }

    fn providers_for_test() -> Providers {
        providers("ws://127.0.0.1:1")
    }

    #[test]
    fn a_test_run_refuses_to_start_under_an_active_non_suspendable_section() {
        let log = Rc::new(RefCell::new(Vec::new()));
        let providers = vec![
            fake("http", &log, |fake| fake.active = true),
            fake("worker", &log, |fake| {
                fake.suspendable = false;
                fake.active = true;
            }),
        ];
        let err = refuse_test_while_blocking(&providers)
            .unwrap_err()
            .to_string();
        assert!(err.starts_with("scenario_active:"), "{err}");
        assert!(err.contains("lxdev scenario clear"), "{err}");
        // A suspendable section is never asked.
        assert_eq!(*log.borrow(), vec!["status worker"]);

        let idle = vec![fake("worker", &log, |fake| fake.suspendable = false)];
        refuse_test_while_blocking(&idle).unwrap();
        // HTTP stands aside by itself, so today's providers never block and
        // never cost a round trip.
        refuse_test_while_blocking(&providers_for_test()).unwrap();
    }

    #[test]
    fn clearing_asks_every_provider() {
        let log = Rc::new(RefCell::new(Vec::new()));
        let providers = vec![fake("http", &log, |_| {}), fake("worker", &log, |_| {})];
        let cleared = clear(&providers).unwrap();
        assert_eq!(cleared.len(), 2);
        assert_eq!(*log.borrow(), vec!["clear worker", "clear http"]);
    }
}
