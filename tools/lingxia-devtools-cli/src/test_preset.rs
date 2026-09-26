//! `lxdev test --preset NAME`: named argument lists from `lxdev.json` in the
//! project root.
//!
//! ```json
//! { "$schema": "…/lxdev.schema.json",
//!   "test": {
//!     "entry": "tests/", "outputDir": "test-results", "openapi": ["api.yaml"],
//!     "presets": { "ci": ["--tag", "unit,routed", "--profile", "empty"] } } }
//! ```
//!
//! A preset is argv. The effective command line is the file's defaults, then
//! the preset's arguments, then the command line's, parsed once by clap:
//! repeatable flags add up, a scalar given later wins. A default is dropped
//! when the preset or the command line gives that flag (or an entry) itself.
//! The file is committed, so it may not carry secrets or anything that acts
//! on another run.

use anyhow::{Context, Result, anyhow, bail};
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::ffi::OsString;
use std::path::{Path, PathBuf};

pub const PRESET_FILE: &str = "lxdev.json";
const FILE_KEYS: [&str; 2] = ["$schema", "test"];
const TEST_KEYS: [&str; 5] = ["presets", "entry", "outputDir", "openapi", "tags"];

/// Flags a preset may not contain, and why.
const FORBIDDEN: [(&str, &str); 5] = [
    (
        "--secret-arg",
        "secrets stay on the command line or in the environment",
    ),
    ("--preset", "a preset cannot name another preset"),
    (
        "--cancel-active",
        "cancelling another client's run must be asked for explicitly",
    ),
    ("--list-presets", "it only lists presets"),
    ("--print-args", "it only prints arguments"),
];

#[derive(Debug)]
pub struct Presets {
    pub path: PathBuf,
    pub presets: BTreeMap<String, Vec<String>>,
    /// `test.entry|outputDir|openapi|tags`: `(flag, values)`, with `None`
    /// for the entry.
    pub defaults: Vec<(Option<&'static str>, Vec<String>)>,
}

impl Presets {
    /// The defaults a run starts from, as argv, leaving out any the preset
    /// or command line (`given`) sets itself.
    fn default_args(&self, given: &[String]) -> Vec<String> {
        let (has_entry, flags) = scan(given);
        let mut out = Vec::new();
        for (flag, values) in &self.defaults {
            match flag {
                None if !has_entry => out.extend(values.iter().cloned()),
                Some(flag) if !flags.contains(*flag) => {
                    for value in values {
                        out.push((*flag).to_string());
                        out.push(value.clone());
                    }
                }
                _ => {}
            }
        }
        out
    }
}

/// The results root when `--output-root` is not given.
pub const DEFAULT_OUTPUT_ROOT: &str = "test-results";

impl Presets {
    /// `test.outputDir`, else `test-results`, beside `lxdev.json`.
    fn results_root(&self) -> PathBuf {
        let base = self.path.parent().unwrap_or(Path::new("."));
        let configured = self.defaults.iter().find_map(|(flag, values)| {
            (*flag == Some("--output-root"))
                .then(|| values.first())
                .flatten()
        });
        normalize(&base.join(configured.map_or(DEFAULT_OUTPUT_ROOT, String::as_str)))
    }
}

/// Where runs go and `latest` is, for a command run in `cwd` without
/// `--output-root`: resolved against `lxdev.json` like its presets, so it
/// is the same from any subdirectory; `./test-results` without one.
pub fn results_root(cwd: &Path) -> PathBuf {
    match load(cwd) {
        Ok(Some(presets)) => presets.results_root(),
        _ => PathBuf::from(DEFAULT_OUTPUT_ROOT),
    }
}

/// `--last-failed`: put the rerun on the run it reruns — that run's
/// `--preset`, and its `--profile` (with `--profile-save`), recorded in its
/// report.json — unless the command line gives them. A different `--preset`
/// brings its own profile, so then only the preset's say counts. Inserted
/// right after `test`, before the command line's own arguments; `argv` is
/// unchanged when the previous run cannot be read (the run reports that).
pub fn carry_last_failed(argv: Vec<OsString>, cwd: &Path) -> Vec<OsString> {
    let Some(index) = test_subcommand_index(&argv) else {
        return argv;
    };
    let rest: Vec<String> = argv[index + 1..]
        .iter()
        .map(|token| token.to_string_lossy().into_owned())
        .take_while(|token| token != "--")
        .collect();
    if rest.first().is_some_and(|first| first == "report") {
        return argv;
    }
    let Some(last) = flag_value(&rest, "--last-failed", crate::test::LATEST) else {
        return argv;
    };
    let root = flag_value(&rest, "--output-root", "")
        .filter(|root| !root.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| results_root(cwd));
    let Ok(report) = crate::test::resolve_report_path(Path::new(&last), &root) else {
        return argv;
    };
    // Nothing failed: nothing runs, so nothing to carry.
    if crate::test::failed_ids(&report).is_ok_and(|ids| ids.is_empty()) {
        return argv;
    }
    let Some(settings) = std::fs::read(&report)
        .ok()
        .and_then(|bytes| serde_json::from_slice::<Value>(&bytes).ok())
        .map(|mut report| report["meta"][crate::test::RUN_SETTINGS_KEY].take())
    else {
        return argv;
    };
    let (_, given) = scan(&rest);
    let text = |key: &str| settings[key].as_str().filter(|value| !value.is_empty());
    let mut carried: Vec<String> = Vec::new();
    let mut preset_args: Vec<String> = Vec::new();
    if let Some(preset) = text("preset").filter(|_| !given.contains("--preset")) {
        let args = load(cwd)
            .ok()
            .flatten()
            .and_then(|presets| presets.presets.get(preset).cloned())
            .unwrap_or_default();
        carried.extend(["--preset".to_string(), preset.to_string()]);
        preset_args = args;
    }
    // The carried preset may set the same profile already.
    let preset_sets_it = flag_value(&preset_args, "--profile", "").as_deref() == text("profile")
        && flag_value(&preset_args, "--profile-save", "pass").as_deref() == text("profile_save");
    if let Some(profile) = text("profile")
        .filter(|_| !given.contains("--profile") && !given.contains("--preset") && !preset_sets_it)
    {
        carried.extend(["--profile".to_string(), profile.to_string()]);
        if let Some(when) = text("profile_save").filter(|_| !given.contains("--profile-save")) {
            carried.push(format!("--profile-save={when}"));
        }
    }
    if carried.is_empty() {
        return argv;
    }
    let run_dir = report.parent().unwrap_or(Path::new("."));
    eprintln!(
        "test --last-failed: reusing {} of {} (give them to override)",
        carried
            .iter()
            .map(|arg| quote(arg))
            .collect::<Vec<_>>()
            .join(" "),
        run_dir.display()
    );
    let mut out = argv[..=index].to_vec();
    out.extend(carried.into_iter().map(OsString::from));
    out.extend(argv[index + 1..].iter().cloned());
    out
}

/// The value of long flag `name` among `args` (the last one given):
/// `--name=V`, `--name V`, or `missing` when it takes an optional value and
/// none follows.
fn flag_value(args: &[String], name: &str, missing: &str) -> Option<String> {
    let mut found = None;
    let mut tokens = args.iter().peekable();
    while let Some(token) = tokens.next() {
        if let Some(value) = token.strip_prefix(&format!("{name}=")) {
            found = Some(value.to_string());
        } else if token == name {
            found = Some(match tokens.peek() {
                Some(next) if !next.starts_with('-') => tokens.next().cloned().unwrap_or_default(),
                _ => missing.to_string(),
            });
        }
    }
    found
}

/// `lxdev.json` of the project `dir` belongs to, if it has one.
pub fn load(dir: &Path) -> Result<Option<Presets>> {
    let root = crate::test_bundle::find_project_root(dir);
    let path = root.join(PRESET_FILE);
    if !path.is_file() {
        return Ok(None);
    }
    let text = std::fs::read_to_string(&path)
        .with_context(|| format!("cannot read {}", path.display()))?;
    parse(&text, &path).map(Some)
}

pub fn parse(text: &str, path: &Path) -> Result<Presets> {
    let value: Value = serde_json::from_str(text).map_err(|err| {
        anyhow!(
            "{} is not valid JSON (line {}, column {}): {err}",
            path.display(),
            err.line(),
            err.column()
        )
    })?;
    let at = |message: String| anyhow!("{}: {message}", path.display());
    let Value::Object(fields) = &value else {
        return Err(at("expected a JSON object".into()));
    };
    if let Some(unknown) = fields.keys().find(|key| !FILE_KEYS.contains(&key.as_str())) {
        return Err(at(format!(
            "unknown field '{unknown}' (allowed: {})",
            FILE_KEYS.join(", ")
        )));
    }
    let mut presets = BTreeMap::new();
    let mut defaults = Vec::new();
    match fields.get("test") {
        None => {}
        Some(Value::Object(test)) => {
            if let Some(unknown) = test.keys().find(|key| !TEST_KEYS.contains(&key.as_str())) {
                return Err(at(format!(
                    "unknown field 'test.{unknown}' (allowed: {})",
                    TEST_KEYS.join(", ")
                )));
            }
            for (key, flag, many) in [
                ("entry", None, false),
                // The results root (runs in <run-id>/ under it), like the
                // default test-results; `--output-dir` is one run's directory.
                ("outputDir", Some("--output-root"), false),
                ("openapi", Some("--openapi"), true),
                ("tags", Some("--tag"), true),
            ] {
                let values = match (test.get(key), many) {
                    (None, _) => continue,
                    (Some(Value::String(value)), _) if !value.is_empty() => vec![value.clone()],
                    (Some(Value::Array(items)), true) => items
                        .iter()
                        .map(|item| item.as_str().filter(|v| !v.is_empty()).map(str::to_string))
                        .collect::<Option<Vec<_>>>()
                        .ok_or_else(|| at(format!("test.{key} must be a list of strings")))?,
                    _ => {
                        return Err(at(format!(
                            "test.{key} must be {}",
                            if many {
                                "a string or a list of strings"
                            } else {
                                "a string"
                            }
                        )));
                    }
                };
                if let Some("--tag") = flag {
                    for tag in &values {
                        crate::test_contract::parse_tag_expr(tag)
                            .map_err(|err| at(format!("test.tags: {err}")))?;
                    }
                }
                defaults.push((flag, values));
            }
            match test.get("presets") {
                None => {}
                Some(Value::Object(entries)) => {
                    for (name, args) in entries {
                        if !valid_name(name) {
                            return Err(at(format!(
                                "preset name '{name}' may use letters, digits, '.', '_' and '-'"
                            )));
                        }
                        let args = match args {
                            Value::Array(items) => items
                                .iter()
                                .map(|item| item.as_str().map(str::to_string))
                                .collect::<Option<Vec<_>>>(),
                            _ => None,
                        }
                        .ok_or_else(|| {
                            at(format!(
                                "preset '{name}' must be a list of arguments (strings)"
                            ))
                        })?;
                        check_preset(&args).map_err(|err| at(format!("preset '{name}': {err}")))?;
                        presets.insert(name.clone(), args);
                    }
                }
                Some(_) => return Err(at("test.presets must be an object".into())),
            }
        }
        Some(_) => return Err(at("test must be an object".into())),
    }
    Ok(Presets {
        path: path.to_path_buf(),
        presets,
        defaults,
    })
}

fn valid_name(name: &str) -> bool {
    let mut chars = name.chars();
    chars.next().is_some_and(|c| c.is_ascii_alphanumeric())
        && chars.all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'))
}

/// A flag token's name: `--x=1` → `--x`.
fn flag_name(token: &str) -> &str {
    token.split_once('=').map_or(token, |(name, _)| name)
}

fn check_preset(args: &[String]) -> Result<(), String> {
    let mut tokens = args.iter().peekable();
    while let Some(token) = tokens.next() {
        let name = flag_name(token);
        if let Some((flag, why)) = FORBIDDEN.iter().find(|(flag, _)| *flag == name) {
            return Err(format!("{flag} is not allowed in a preset: {why}"));
        }
        if name == "--arg" {
            let value = match token.split_once('=') {
                Some((_, value)) => Some(value),
                None => tokens.peek().map(|value| value.as_str()),
            };
            if let Some(key) = value.and_then(|value| value.split_once('=')).map(|kv| kv.0)
                && looks_secret_key(key)
            {
                return Err(format!(
                    "--arg {key}=… is named like a credential; pass it with --secret-arg on \
                     the command line instead"
                ));
            }
        }
    }
    Ok(())
}

/// Whether an arg key names a credential: its last word is one of these,
/// as `@lingxia/test` decides for the report's arg list.
pub fn looks_secret_key(key: &str) -> bool {
    const WORDS: [&str; 9] = [
        "password",
        "passwd",
        "pwd",
        "passphrase",
        "secret",
        "token",
        "credential",
        "credentials",
        "apikey",
    ];
    const PAIRS: [&str; 2] = ["api key", "private key"];
    let words = key_words(key);
    let Some(last) = words.last() else {
        return false;
    };
    if WORDS.contains(&last.as_str()) {
        return true;
    }
    words.len() >= 2 && PAIRS.contains(&format!("{} {last}", words[words.len() - 2]).as_str())
}

/// Lower-case words split at non-alphanumerics and camelCase boundaries:
/// `DB_PASSWORD` → db password, `apiKey` → api key, `APIKey` → api key.
fn key_words(key: &str) -> Vec<String> {
    let chars: Vec<char> = key.chars().collect();
    let mut words = Vec::new();
    let mut word = String::new();
    for (i, &c) in chars.iter().enumerate() {
        if !c.is_ascii_alphanumeric() {
            if !word.is_empty() {
                words.push(std::mem::take(&mut word));
            }
            continue;
        }
        let prev = i.checked_sub(1).map(|p| chars[p]);
        let next = chars.get(i + 1).copied();
        let boundary = c.is_ascii_uppercase()
            && prev.is_some_and(|p| {
                p.is_ascii_lowercase()
                    || p.is_ascii_digit()
                    || (p.is_ascii_uppercase() && next.is_some_and(|n| n.is_ascii_lowercase()))
            });
        if boundary && !word.is_empty() {
            words.push(std::mem::take(&mut word));
        }
        word.push(c.to_ascii_lowercase());
    }
    if !word.is_empty() {
        words.push(word);
    }
    words
}

/// Index of the `test` subcommand in `argv`, skipping the global options
/// before it.
pub fn test_subcommand_index(argv: &[OsString]) -> Option<usize> {
    let mut index = 1;
    while let Some(token) = argv.get(index) {
        let token = token.to_str()?;
        if token == "--session" {
            index += 2;
            continue;
        }
        if token.starts_with('-') {
            index += 1;
            continue;
        }
        return (token == "test").then_some(index);
    }
    None
}

/// The `--preset` value among the test arguments, before any `--`.
fn preset_name(test_args: &[OsString]) -> Option<String> {
    let mut tokens = test_args.iter().map(|token| token.to_string_lossy());
    while let Some(token) = tokens.next() {
        if token == "--" {
            return None;
        }
        if token == "--preset" {
            return tokens.next().map(|name| name.into_owned());
        }
        if let Some(name) = token.strip_prefix("--preset=") {
            return Some(name.to_string());
        }
    }
    None
}

/// `argv` with the file's defaults and the named preset's arguments inserted
/// right after `test`, so the command line's own arguments come later and
/// win. Unchanged without `lxdev.json`, or for `lxdev test report`,
/// `--cancel-active` and `--list-presets`, which start no run.
pub fn expand(argv: Vec<OsString>, cwd: &Path) -> Result<Vec<OsString>> {
    let Some(index) = test_subcommand_index(&argv) else {
        return Ok(argv);
    };
    let rest: Vec<String> = argv[index + 1..]
        .iter()
        .map(|token| token.to_string_lossy().into_owned())
        .collect();
    if rest.first().is_some_and(|first| first == "report") {
        return Ok(argv);
    }
    let preset = preset_name(&argv[index + 1..]);
    let Some(presets) = (match &preset {
        Some(name) => Some(load(cwd)?.ok_or_else(|| {
            anyhow!(
                "--preset {name}: no {PRESET_FILE} in {}",
                crate::test_bundle::find_project_root(cwd).display()
            )
        })?),
        None => load(cwd)?,
    }) else {
        return Ok(argv);
    };
    let base = presets.path.parent().unwrap_or(Path::new("."));
    let preset_args = match &preset {
        None => Vec::new(),
        Some(name) => presets.presets.get(name).cloned().ok_or_else(|| {
            let known = presets.presets.keys().cloned().collect::<Vec<_>>();
            anyhow!(
                "--preset {name}: {} has no such preset (presets: {})",
                presets.path.display(),
                if known.is_empty() {
                    "none".to_string()
                } else {
                    known.join(", ")
                }
            )
        })?,
    };
    // Paths on the command line replace the preset's, like a flag does.
    let preset_args = if scan(&rest).0 {
        without_paths(&preset_args)
    } else {
        preset_args
    };
    let starts_no_run = rest
        .iter()
        .take_while(|token| *token != "--")
        .any(|token| token == "--cancel-active" || token == "--list-presets");
    let mut given = preset_args.clone();
    given.extend(rest.iter().take_while(|token| *token != "--").cloned());
    let defaults = if starts_no_run {
        Vec::new()
    } else {
        presets.default_args(&given)
    };
    if defaults.is_empty() && preset_args.is_empty() {
        return Ok(argv);
    }
    let mut expanded = argv[..=index].to_vec();
    expanded.extend(
        anchor_paths(&defaults, base)
            .into_iter()
            .map(OsString::from),
    );
    expanded.extend(
        anchor_paths(&preset_args, base)
            .into_iter()
            .map(OsString::from),
    );
    expanded.extend(argv[index + 1..].iter().cloned());
    Ok(expanded)
}

/// What a long flag of `lxdev test` consumes: `Some(true)` a value,
/// `Some(false)` a value only when the next token is not a flag, `None`
/// nothing (or not a flag of `lxdev test`).
fn takes_value(flag: &str) -> Option<bool> {
    let command =
        <crate::test::TestOptions as clap::Args>::augment_args(clap::Command::new("test"));
    let long = flag.strip_prefix("--")?;
    let arg = command.get_arguments().find(|arg| {
        arg.get_long() == Some(long)
            || arg
                .get_all_aliases()
                .is_some_and(|aliases| aliases.contains(&long))
    })?;
    if !arg.get_action().takes_values() {
        return None;
    }
    Some(
        !arg.get_num_args()
            .is_some_and(|range| range.min_values() == 0),
    )
}

/// Whether `args` hold a positional (the entry), and which long flags.
fn scan(args: &[String]) -> (bool, std::collections::HashSet<String>) {
    let mut positional = false;
    let mut flags = std::collections::HashSet::new();
    let mut tokens = args.iter().peekable();
    while let Some(token) = tokens.next() {
        if token == "--" {
            break;
        }
        if !token.starts_with('-') || token == "-" {
            positional = true;
            continue;
        }
        let name = flag_name(token);
        flags.insert(name.to_string());
        if token.contains('=') {
            continue;
        }
        let consumes = match takes_value(token) {
            Some(true) => true,
            Some(false) => tokens.peek().is_some_and(|next| !next.starts_with('-')),
            None => false,
        };
        if consumes {
            tokens.next();
        }
    }
    (positional, flags)
}

/// `args` without their positionals (the test paths).
fn without_paths(args: &[String]) -> Vec<String> {
    let mut out = Vec::with_capacity(args.len());
    let mut tokens = args.iter().peekable();
    while let Some(token) = tokens.next() {
        if token == "--" {
            out.push(token.clone());
            out.extend(tokens.cloned());
            break;
        }
        if !token.starts_with('-') || token == "-" {
            continue;
        }
        out.push(token.clone());
        if token.contains('=') {
            continue;
        }
        let consumes = match takes_value(token) {
            Some(true) => true,
            Some(false) => tokens.peek().is_some_and(|next| !next.starts_with('-')),
            None => false,
        };
        if consumes && let Some(value) = tokens.next() {
            out.push(value.clone());
        }
    }
    out
}

/// Flags whose value is a file or directory. `--profile` takes `empty`, a
/// NAME or a PATH; only a PATH is anchored.
const PATH_FLAGS: [&str; 7] = [
    "--openapi",
    "--covers-manifest",
    "--record-network",
    "--output-root",
    "--output-dir",
    "--last-failed",
    "--secrets-file",
];
const STATE_FLAGS: [&str; 1] = ["--profile"];

/// A preset's relative paths — the entry, and the values of path flags —
/// made relative to the directory of `lxdev.json` (`base`) instead of the
/// working directory, so a preset means the same files from any
/// subdirectory. A path on the command line keeps its working-directory
/// meaning.
fn anchor_paths(args: &[String], base: &Path) -> Vec<String> {
    let anchor = |value: &str| -> String {
        let path = Path::new(value);
        if value.is_empty() || path.is_absolute() {
            return value.to_string();
        }
        normalize(&base.join(path)).to_string_lossy().into_owned()
    };
    let anchor_flag_value = |flag: &str, value: &str| -> String {
        if PATH_FLAGS.contains(&flag)
            || (STATE_FLAGS.contains(&flag) && crate::test_state::names_a_path(value))
        {
            anchor(value)
        } else {
            value.to_string()
        }
    };

    let mut out = Vec::with_capacity(args.len());
    let mut tokens = args.iter().peekable();
    while let Some(token) = tokens.next() {
        if token == "--" {
            out.push(token.clone());
            out.extend(tokens.cloned());
            break;
        }
        if !token.starts_with('-') || token == "-" {
            // A positional: the test entry.
            out.push(anchor(token));
            continue;
        }
        if let Some((flag, value)) = token.split_once('=') {
            out.push(format!("{flag}={}", anchor_flag_value(flag, value)));
            continue;
        }
        out.push(token.clone());
        let consumes = match takes_value(token) {
            Some(true) => true,
            Some(false) => tokens.peek().is_some_and(|next| !next.starts_with('-')),
            None => false,
        };
        if consumes && let Some(value) = tokens.next() {
            out.push(anchor_flag_value(token, value));
        }
    }
    out
}

/// `a/./b/../c` → `a/c`, without touching the filesystem.
fn normalize(path: &Path) -> PathBuf {
    use std::path::Component;
    let mut out = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                if !out.pop() {
                    out.push("..");
                }
            }
            other => out.push(other.as_os_str()),
        }
    }
    out
}

/// The effective `lxdev test` arguments of an expanded `argv`, without
/// `--preset`/`--print-args`, and with secret values masked.
pub fn effective_args(argv: &[OsString]) -> Vec<String> {
    let Some(index) = test_subcommand_index(argv) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    let mut tokens = argv[index + 1..]
        .iter()
        .map(|token| token.to_string_lossy().into_owned());
    let mut passthrough = false;
    while let Some(token) = tokens.next() {
        if passthrough {
            out.push(token);
            continue;
        }
        let name = flag_name(&token).to_string();
        match name.as_str() {
            "--" => {
                passthrough = true;
                out.push(token);
            }
            "--print-args" => {}
            "--preset" => {
                if !token.contains('=') {
                    tokens.next();
                }
            }
            "--arg" | "--secret-arg" => {
                let inline = token.split_once('=').map(|(_, value)| value.to_string());
                let value = inline.or_else(|| tokens.next());
                out.push(name.clone());
                if let Some(value) = value {
                    out.push(mask_pair(&value, name == "--secret-arg"));
                }
            }
            _ => out.push(token),
        }
    }
    out
}

fn mask_pair(pair: &str, secret: bool) -> String {
    match pair.split_once('=') {
        Some((key, _)) if secret || looks_secret_key(key) => format!("{key}=***"),
        _ => pair.to_string(),
    }
}

/// POSIX-shell quoting, for a line one can paste.
fn quote(token: &str) -> String {
    let plain = !token.is_empty()
        && token
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || "_-./:=,@%+".contains(c));
    if plain {
        token.to_string()
    } else {
        format!("'{}'", token.replace('\'', r"'\''"))
    }
}

/// `lxdev test --print-args`: the command line, then — as `#` lines, so the
/// output still pastes — the args the environment and `--secrets-file` add,
/// each with where it comes from and secret values masked.
pub fn print_args(
    argv: &[OsString],
    json: bool,
    sources: &crate::test_secrets::ArgSources,
) -> Result<()> {
    let args = effective_args(argv);
    let external = external_args(sources);
    if json {
        let external = external
            .iter()
            .map(|arg| {
                json!({ "flag": arg.flag, "key": arg.key, "value": arg.value, "source": arg.source })
            })
            .collect::<Vec<_>>();
        println!("{}", json!({ "args": args, "external": external }));
    } else {
        let line = args.iter().map(|arg| quote(arg)).collect::<Vec<_>>();
        println!("lxdev test {}", line.join(" "));
        for arg in &external {
            println!(
                "# {} {}  (from {})",
                arg.flag,
                quote(&format!("{}={}", arg.key, arg.value)),
                arg.source
            );
        }
    }
    Ok(())
}

/// An arg from outside the command line, as `--print-args` shows it.
struct ExternalArg {
    flag: &'static str,
    key: String,
    value: String,
    /// `LXDEV_SECRET_PIN`, or `--secrets-file PATH`.
    source: String,
}

fn external_args(sources: &crate::test_secrets::ArgSources) -> Vec<ExternalArg> {
    use crate::test_secrets::{ENV_ARG_PREFIX, ENV_SECRET_PREFIX, REDACTED};
    let mut out = Vec::new();
    for (key, value) in &sources.env_args {
        out.push(ExternalArg {
            flag: "--arg",
            value: if looks_secret_key(key) {
                REDACTED.to_string()
            } else {
                value.clone()
            },
            source: format!("{ENV_ARG_PREFIX}{key}"),
            key: key.clone(),
        });
    }
    for (key, _) in &sources.env_secrets {
        out.push(ExternalArg {
            flag: "--secret-arg",
            key: key.clone(),
            value: REDACTED.to_string(),
            source: format!("{ENV_SECRET_PREFIX}{key}"),
        });
    }
    if let Some(path) = &sources.secrets_file {
        for (key, _) in &sources.file_secrets {
            out.push(ExternalArg {
                flag: "--secret-arg",
                key: key.clone(),
                value: REDACTED.to_string(),
                source: format!("--secrets-file {}", path.display()),
            });
        }
    }
    out
}

/// `lxdev test --list-presets`.
pub fn list(cwd: &Path, json: bool) -> Result<()> {
    let presets = load(cwd)?;
    if json {
        let value = match &presets {
            Some(presets) => json!({ "file": presets.path, "presets": presets.presets }),
            None => json!({ "file": null, "presets": {} }),
        };
        println!("{value}");
        return Ok(());
    }
    let Some(presets) = presets else {
        bail!(
            "no {PRESET_FILE} in {}; presets live in `test.presets` there",
            crate::test_bundle::find_project_root(cwd).display()
        );
    };
    if presets.presets.is_empty() {
        println!("{} defines no presets", presets.path.display());
        return Ok(());
    }
    let width = presets.presets.keys().map(String::len).max().unwrap_or(0);
    for (name, args) in &presets.presets {
        let line = args.iter().map(|arg| quote(arg)).collect::<Vec<_>>();
        println!("{name:<width$}  {}", line.join(" "));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn os(args: &[&str]) -> Vec<OsString> {
        args.iter().map(OsString::from).collect()
    }

    fn project(json: &str) -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("package.json"), "{}").unwrap();
        std::fs::write(dir.path().join(PRESET_FILE), json).unwrap();
        std::fs::create_dir_all(dir.path().join("tests/pages")).unwrap();
        dir
    }

    const FILE: &str = r#"{
        "$schema": "./node_modules/@lingxia/test/schemas/lxdev.schema.json",
        "test": { "presets": {
            "ci": ["--tag", "unit,routed", "--openapi", "api.yaml", "--profile", "empty", "--retries", "1"],
            "nightly": ["--profile", "demo", "--profile-save=always"]
        } }
    }"#;

    #[test]
    fn a_preset_goes_before_the_command_line() {
        let dir = project(FILE);
        let argv = os(&[
            "lxdev",
            "--session",
            "macos",
            "test",
            "tests/",
            "--preset",
            "ci",
            "--retries",
            "2",
        ]);
        // Found from a subdirectory, like the test project root.
        let expanded = expand(argv, &dir.path().join("tests/pages")).unwrap();
        let api = dir.path().join("api.yaml");
        assert_eq!(
            expanded,
            os(&[
                "lxdev",
                "--session",
                "macos",
                "test",
                "--tag",
                "unit,routed",
                "--openapi",
                api.to_str().unwrap(),
                "--profile",
                "empty",
                "--retries",
                "1",
                "tests/",
                "--preset",
                "ci",
                "--retries",
                "2",
            ])
        );
        // Unchanged without --preset, or outside `test`.
        let plain = os(&["lxdev", "test", "tests/"]);
        assert_eq!(expand(plain.clone(), dir.path()).unwrap(), plain);
        let other = os(&["lxdev", "logs", "--preset", "ci"]);
        assert_eq!(expand(other.clone(), dir.path()).unwrap(), other);
        let after_dashes = os(&["lxdev", "test", "t", "--", "--preset", "ci"]);
        assert_eq!(
            expand(after_dashes.clone(), dir.path()).unwrap(),
            after_dashes
        );
        let inline = expand(os(&["lxdev", "test", "--preset=nightly"]), dir.path()).unwrap();
        assert_eq!(inline[2], OsString::from("--profile"));
    }

    #[test]
    fn preset_paths_resolve_against_the_preset_file() {
        let dir = project(
            r#"{ "test": { "presets": { "p": [
                "tests/all.test.ts",
                "--openapi", "../contract.yaml",
                "--covers-manifest=tests/coverage.yaml",
                "--profile", "./snapshots/auth.lxstate",
                "--secrets-file", ".env.test",
                "--record-network", "recorded",
                "--output-dir", "/tmp/abs-results",
                "--tag", "unit/x",
                "--grep", "a/b",
                "--shuffle",
                "--arg", "dir=relative/path"
            ] } } }"#,
        );
        let root = dir.path();
        let at = |relative: &str| root.join(relative).to_string_lossy().into_owned();
        let parent = root.parent().unwrap().join("contract.yaml");
        // Run from a subdirectory: the preset still names the project's files.
        let expanded = expand(
            os(&["lxdev", "test", "--preset", "p", "--openapi", "local.yaml"]),
            &root.join("tests/pages"),
        )
        .unwrap();
        let expected = [
            "lxdev".to_string(),
            "test".to_string(),
            at("tests/all.test.ts"),
            "--openapi".to_string(),
            parent.to_string_lossy().into_owned(),
            format!("--covers-manifest={}", at("tests/coverage.yaml")),
            "--profile".to_string(),
            at("snapshots/auth.lxstate"),
            "--secrets-file".to_string(),
            at(".env.test"),
            "--record-network".to_string(),
            at("recorded"),
            "--output-dir".to_string(),
            "/tmp/abs-results".to_string(),
            "--tag".to_string(),
            "unit/x".to_string(),
            "--grep".to_string(),
            "a/b".to_string(),
            "--shuffle".to_string(),
            "--arg".to_string(),
            "dir=relative/path".to_string(),
            // The command line's own arguments are left as typed.
            "--preset".to_string(),
            "p".to_string(),
            "--openapi".to_string(),
            "local.yaml".to_string(),
        ];
        assert_eq!(
            expanded,
            expected.iter().map(OsString::from).collect::<Vec<_>>()
        );
    }

    #[test]
    fn file_defaults_come_first_and_give_way_to_what_is_given() {
        let dir = project(
            r#"{ "test": {
                "entry": "tests/",
                "outputDir": "results",
                "openapi": ["api.yaml"],
                "tags": "unit",
                "presets": { "smoke": ["tests/smoke.test.ts", "--tag", "smoke"] }
            } }"#,
        );
        let root = dir.path();
        let at = |relative: &str| root.join(relative).to_string_lossy().into_owned();
        let strings = |argv: Vec<OsString>| {
            argv.into_iter()
                .map(|token| token.to_string_lossy().into_owned())
                .collect::<Vec<_>>()
        };
        // No preset: every default, then the command line.
        let plain = strings(expand(os(&["lxdev", "test", "--grep", "x"]), root).unwrap());
        assert_eq!(
            plain,
            vec![
                "lxdev".to_string(),
                "test".to_string(),
                at("tests"),
                "--output-root".to_string(),
                at("results"),
                "--openapi".to_string(),
                at("api.yaml"),
                "--tag".to_string(),
                "unit".to_string(),
                "--grep".to_string(),
                "x".to_string(),
            ]
        );
        // A preset's entry and tags replace the defaults'; the rest stays.
        let smoke = strings(expand(os(&["lxdev", "test", "--preset", "smoke"]), root).unwrap());
        assert!(!smoke.contains(&at("tests")), "{smoke:?}");
        assert!(!smoke.contains(&"unit".to_string()), "{smoke:?}");
        assert!(smoke.contains(&at("results")), "{smoke:?}");
        // Paths on the command line replace the preset's.
        let one = strings(
            expand(
                os(&["lxdev", "test", "--preset", "smoke", "tests/a.test.ts:3"]),
                root,
            )
            .unwrap(),
        );
        assert!(!one.contains(&at("tests/smoke.test.ts")), "{one:?}");
        assert!(one.contains(&"smoke".to_string()), "{one:?}");
        // So does the command line's own entry or flag.
        let own =
            strings(expand(os(&["lxdev", "test", "a.test.ts", "--output-root=o"]), root).unwrap());
        assert!(
            !own.contains(&at("tests")) && !own.contains(&at("results")),
            "{own:?}"
        );
        // One run's own directory still has its `latest` in the root.
        let fixed =
            strings(expand(os(&["lxdev", "test", "a.test.ts", "--output-dir=o"]), root).unwrap());
        assert!(fixed.contains(&at("results")), "{fixed:?}");
        // Commands that start no run take no defaults.
        for argv in [
            &["lxdev", "test", "--cancel-active"][..],
            &["lxdev", "test", "report", "latest"],
            &["lxdev", "test", "--list-presets"],
        ] {
            assert_eq!(expand(os(argv), root).unwrap(), os(argv));
        }
        // Bad defaults are refused with the field named.
        for (text, expected) in [
            (
                r#"{ "test": { "entry": ["a"] } }"#,
                "test.entry must be a string",
            ),
            (r#"{ "test": { "tags": ["unit,"] } }"#, "test.tags"),
            (
                r#"{ "test": { "openapi": [1] } }"#,
                "test.openapi must be a list",
            ),
        ] {
            let err = format!("{:#}", parse(text, Path::new("lxdev.json")).unwrap_err());
            assert!(err.contains(expected), "{text}: {err}");
        }
    }

    #[test]
    fn an_unknown_preset_or_a_missing_file_says_what_exists() {
        let dir = project(FILE);
        let err = expand(os(&["lxdev", "test", "--preset", "cii"]), dir.path())
            .unwrap_err()
            .to_string();
        assert!(err.contains("presets: ci, nightly"), "{err}");
        let empty = tempfile::tempdir().unwrap();
        std::fs::write(empty.path().join("package.json"), "{}").unwrap();
        let err = expand(os(&["lxdev", "test", "--preset", "ci"]), empty.path())
            .unwrap_err()
            .to_string();
        assert!(err.contains("no lxdev.json"), "{err}");
    }

    #[test]
    fn presets_hold_no_secrets_and_act_on_no_other_run() {
        let path = Path::new("lxdev.json");
        for (args, expected) in [
            (
                r#"["--secret-arg", "token=x"]"#,
                "--secret-arg is not allowed",
            ),
            (r#"["--secret-arg=token=x"]"#, "--secret-arg is not allowed"),
            (r#"["--preset", "ci"]"#, "another preset"),
            (r#"["--cancel-active"]"#, "--cancel-active is not allowed"),
            (r#"["--print-args"]"#, "--print-args is not allowed"),
            (
                r#"["--arg", "DB_PASSWORD=x"]"#,
                "--arg DB_PASSWORD=… is named like a credential",
            ),
            (r#"["--arg=apiKey=x"]"#, "apiKey"),
            (r#"["--tag", 1]"#, "list of arguments"),
        ] {
            let text = format!(r#"{{ "test": {{ "presets": {{ "p": {args} }} }} }}"#);
            let err = format!("{:#}", parse(&text, path).unwrap_err());
            assert!(err.contains(expected), "{args}: {err}");
            assert!(
                err.contains("preset 'p'") || err.contains("list of"),
                "{err}"
            );
        }
        // Ordinary args are fine, including ones that only look close.
        let ok = r#"{ "test": { "presets": { "p": ["--arg", "maxTokens=3", "--arg", "passport=x"] } } }"#;
        assert_eq!(parse(ok, path).unwrap().presets["p"].len(), 4);

        for (text, expected) in [
            (r#"{ "presets": {} }"#, "unknown field 'presets'"),
            (
                r#"{ "test": { "profiles": {} } }"#,
                "unknown field 'test.profiles'",
            ),
            (
                r#"{ "test": { "presets": { "p": ["--secrets-file", ".env.test", "--secret-arg", "a=b"] } } }"#,
                "--secret-arg is not allowed",
            ),
            (
                r#"{ "test": { "presets": [] } }"#,
                "test.presets must be an object",
            ),
            (
                r#"{ "test": { "presets": { "a b": [] } } }"#,
                "preset name 'a b'",
            ),
            ("{ \"test\": ", "not valid JSON"),
        ] {
            let err = format!("{:#}", parse(text, path).unwrap_err());
            assert!(err.contains(expected), "{text}: {err}");
        }
    }

    #[test]
    fn credential_names_match_the_report_heuristic() {
        for key in [
            "password",
            "DB_PASSWORD",
            "apiKey",
            "APIKey",
            "authToken",
            "private_key",
        ] {
            assert!(looks_secret_key(key), "{key}");
        }
        for key in [
            "tokenCount",
            "maxTokens",
            "passport",
            "passWithNoTests",
            "bypassCache",
            "",
        ] {
            assert!(!looks_secret_key(key), "{key}");
        }
    }

    #[test]
    fn printed_args_mask_secrets_and_drop_the_preset_flags() {
        let argv = os(&[
            "lxdev",
            "test",
            "--tag",
            "unit",
            "tests/",
            "--preset",
            "ci",
            "--print-args",
            "--secret-arg",
            "token=abc123",
            "--secret-arg=pin=1234",
            "--arg",
            "userPassword=hunter2",
            "--arg=user=alice",
            "--grep",
            "a b",
        ]);
        let args = effective_args(&argv);
        assert_eq!(
            args,
            vec![
                "--tag",
                "unit",
                "tests/",
                "--secret-arg",
                "token=***",
                "--secret-arg",
                "pin=***",
                "--arg",
                "userPassword=***",
                "--arg",
                "user=alice",
                "--grep",
                "a b",
            ]
        );
        let line = args
            .iter()
            .map(|arg| quote(arg))
            .collect::<Vec<_>>()
            .join(" ");
        assert!(line.ends_with("--grep 'a b'"), "{line}");
        assert!(
            !line.contains("abc123") && !line.contains("hunter2"),
            "{line}"
        );
    }

    #[test]
    fn the_results_root_is_beside_lxdev_json_from_any_subdirectory() {
        let configured = project(r#"{ "test": { "outputDir": "out/runs" } }"#);
        let sub = configured.path().join("tests/pages");
        assert_eq!(results_root(&sub), configured.path().join("out/runs"));
        let default = project(r#"{ "test": {} }"#);
        assert_eq!(
            results_root(&default.path().join("tests")),
            default.path().join(DEFAULT_OUTPUT_ROOT)
        );
        let none = tempfile::tempdir().unwrap();
        assert_eq!(
            results_root(none.path()),
            PathBuf::from(DEFAULT_OUTPUT_ROOT)
        );
    }

    /// A project whose `latest` run is `report`, with `settings` recorded.
    fn last_run(dir: &Path, cases: &str, settings: Value) {
        let run = dir.join("test-results/r1");
        std::fs::create_dir_all(&run).unwrap();
        let report = json!({
            "cases": serde_json::from_str::<Value>(cases).unwrap(),
            "meta": { crate::test::RUN_SETTINGS_KEY: settings },
        });
        std::fs::write(run.join("report.json"), report.to_string()).unwrap();
        #[cfg(unix)]
        std::os::unix::fs::symlink(&run, dir.join("test-results/latest")).unwrap();
        #[cfg(not(unix))]
        std::fs::write(
            dir.join("test-results/latest"),
            run.to_string_lossy().as_bytes(),
        )
        .unwrap();
    }

    #[test]
    fn last_failed_carries_the_previous_runs_preset_and_profile() {
        let dir = project(FILE);
        let root = dir.path();
        let failed = r#"[{"id":"A","status":"failed"}]"#;
        // `--preset nightly --profile auth --profile-save`: the command line
        // had overridden the preset's profile.
        last_run(
            root,
            failed,
            json!({"preset": "nightly", "profile": "auth", "profile_save": "pass"}),
        );
        let strings = |argv: Vec<OsString>| {
            argv.into_iter()
                .map(|token| token.to_string_lossy().into_owned())
                .collect::<Vec<_>>()
        };
        let sub = root.join("tests/pages");
        // From a subdirectory: `latest` is the project's.
        let carried = strings(carry_last_failed(
            os(&["lxdev", "test", "tests/", "--last-failed"]),
            &sub,
        ));
        assert_eq!(
            carried,
            [
                "lxdev",
                "test",
                "--preset",
                "nightly",
                "--profile",
                "auth",
                "--profile-save=pass",
                "tests/",
                "--last-failed"
            ]
        );
        // What the command line gives wins; a new preset brings its own data.
        let own = strings(carry_last_failed(
            os(&["lxdev", "test", "t/", "--last-failed", "--profile", "empty"]),
            root,
        ));
        assert_eq!(
            own,
            [
                "lxdev",
                "test",
                "--preset",
                "nightly",
                "t/",
                "--last-failed",
                "--profile",
                "empty"
            ]
        );
        let other = os(&["lxdev", "test", "t/", "--last-failed", "--preset", "ci"]);
        assert_eq!(carry_last_failed(other.clone(), root), other);
        // Not a --last-failed run, or one naming another report: unchanged.
        let plain = os(&["lxdev", "test", "t/"]);
        assert_eq!(carry_last_failed(plain.clone(), root), plain);
        let missing = os(&[
            "lxdev",
            "test",
            "t/",
            "--last-failed",
            "nowhere/report.json",
        ]);
        assert_eq!(carry_last_failed(missing.clone(), root), missing);
    }

    #[test]
    fn last_failed_does_not_repeat_what_the_carried_preset_sets() {
        let dir = project(FILE);
        last_run(
            dir.path(),
            r#"[{"id":"A","status":"failed"}]"#,
            json!({"preset": "nightly", "profile": "demo", "profile_save": "always"}),
        );
        let carried = carry_last_failed(os(&["lxdev", "test", "--last-failed"]), dir.path());
        assert_eq!(
            carried,
            os(&["lxdev", "test", "--preset", "nightly", "--last-failed"])
        );
        // Without a preset, the profile alone.
        last_run_again(dir.path(), json!({"profile": "demo", "profile_save": null}));
        let carried = carry_last_failed(os(&["lxdev", "test", "--last-failed"]), dir.path());
        assert_eq!(
            carried,
            os(&["lxdev", "test", "--profile", "demo", "--last-failed"])
        );
    }

    /// Rewrite the settings of the run `last_run` made.
    fn last_run_again(dir: &Path, settings: Value) {
        let report = dir.join("test-results/r1/report.json");
        let mut value: Value = serde_json::from_slice(&std::fs::read(&report).unwrap()).unwrap();
        value["meta"][crate::test::RUN_SETTINGS_KEY] = settings;
        std::fs::write(report, value.to_string()).unwrap();
    }

    #[test]
    fn last_failed_after_a_clean_run_carries_nothing() {
        let dir = project(FILE);
        last_run(
            dir.path(),
            r#"[{"id":"A","status":"passed"}]"#,
            json!({"preset": "nightly", "profile": "demo"}),
        );
        let argv = os(&["lxdev", "test", "t/", "--last-failed"]);
        assert_eq!(carry_last_failed(argv.clone(), dir.path()), argv);
    }

    #[test]
    fn printed_args_include_the_environment_and_the_secrets_file_by_source() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join(".env.test");
        std::fs::write(&file, "PIN=4321\n").unwrap();
        let sources = crate::test_secrets::ArgSources::gather(
            [
                ("LXDEV_ARG_REGION".to_string(), "eu".to_string()),
                ("LXDEV_ARG_DB_PASSWORD".to_string(), "hunter2".to_string()),
                (
                    "LXDEV_SECRET_PASSWORD".to_string(),
                    "s3cret-value".to_string(),
                ),
            ],
            Some(&file),
        )
        .unwrap();
        let shown = external_args(&sources)
            .into_iter()
            .map(|arg| format!("{} {}={} ({})", arg.flag, arg.key, arg.value, arg.source))
            .collect::<Vec<_>>();
        let file_source = format!("--secrets-file {}", file.display());
        assert_eq!(
            shown,
            [
                "--arg DB_PASSWORD=*** (LXDEV_ARG_DB_PASSWORD)".to_string(),
                "--arg REGION=eu (LXDEV_ARG_REGION)".to_string(),
                // The key keeps the variable's case.
                "--secret-arg PASSWORD=*** (LXDEV_SECRET_PASSWORD)".to_string(),
                format!("--secret-arg PIN=*** ({file_source})"),
            ]
        );
        assert!(!shown.join(" ").contains("s3cret") && !shown.join(" ").contains("4321"));
    }

    #[test]
    fn the_test_subcommand_is_found_after_global_options() {
        assert_eq!(test_subcommand_index(&os(&["lxdev", "test"])), Some(1));
        assert_eq!(
            test_subcommand_index(&os(&["lxdev", "--session", "ios", "test", "x"])),
            Some(3)
        );
        assert_eq!(
            test_subcommand_index(&os(&["lxdev", "--session=ios", "test"])),
            Some(2)
        );
        assert_eq!(test_subcommand_index(&os(&["lxdev", "logs", "test"])), None);
    }
}
