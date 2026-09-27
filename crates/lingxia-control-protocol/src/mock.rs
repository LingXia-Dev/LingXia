//! Mocks: who answers a call when no test route or scenario rule does.
//!
//! An lxapp keeps the complete set of its mock handlers in `mocks/index.ts`
//! (keys are HTTP targets, `"METHOD url-glob"`) and its default selection in
//! `mocks/config.json`. A *selection* decides, per call, whether the mock
//! handler or the real backend answers:
//!
//! ```text
//! per call:   test route  >  scenario rule  >  selection → mock handler | real
//! ```
//!
//! The selection is a stack of owners, lowest first: `config`
//! (`mocks/config.json`), `baseline` (`lingxia dev --mock`), `dev`
//! (`lxdev mock all|none`) and a test run. Each owner holds entries, a
//! *whole* (`all` / `none` for everything) or *targets* (`all` / `none` for
//! the calls they match). See [`Selection`] for the resolution.
//!
//! Function calls of a Worker project are selected by the session's
//! companion ([`companion`]); an HTTP target contains whitespace, a Function
//! target is a dotted name.

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Where an lxapp keeps its mocks, relative to its root.
pub const MOCKS_DIR: &str = "mocks";
/// The handlers file stems (`mocks/index.ts` or `mocks/index.js`).
pub const HANDLERS_FILES: [&str; 2] = ["index.ts", "index.js"];
/// The selection file, relative to the lxapp root.
pub const CONFIG_FILE: &str = "mocks/config.json";
/// Keys `mocks/config.json` may have.
pub const CONFIG_KEYS: [&str; 3] = ["$schema", "mock", "overrides"];
/// Fields a scenario rule has that a mock answer does not: a handler is
/// code, so it answers one call at a time.
pub const SCENARIO_ONLY_FIELDS: [&str; 4] = ["sequence", "times", "match", "bodyBase64"];

/// Owner of the selection a dev session set (`lxdev mock all|none`).
pub const DEV_OWNER: &str = "dev";

/// `all`: mocks answer; `none`: the real backend answers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MockMode {
    All,
    None,
}

impl MockMode {
    pub fn parse(text: &str) -> Option<Self> {
        match text {
            "all" => Some(Self::All),
            "none" => Some(Self::None),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::All => "all",
            Self::None => "none",
        }
    }

    pub fn opposite(self) -> Self {
        match self {
            Self::All => Self::None,
            Self::None => Self::All,
        }
    }
}

impl std::fmt::Display for MockMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// What a selection target names.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MockTarget {
    /// `GET **/qoe/*`: upper-case method (`None` for `*`) and URL glob or
    /// `/regex/flags`, as a scenario `http` rule writes it.
    Http { method: Option<String>, url: String },
    /// `orders.submit`: a Worker Function, selected by the companion.
    Function { name: String },
}

impl MockTarget {
    /// `GET **/x`, `* **/x`, or the Function name.
    pub fn label(&self) -> String {
        match self {
            Self::Http { method, url } => format!("{} {url}", method.as_deref().unwrap_or("*")),
            Self::Function { name } => name.clone(),
        }
    }

    pub fn is_http(&self) -> bool {
        matches!(self, Self::Http { .. })
    }
}

/// Parse a selection target: text with whitespace is an HTTP target
/// (`METHOD url-glob`), a dotted name is a Function.
pub fn parse_target(text: &str) -> Result<MockTarget, String> {
    let text = text.trim();
    if text.contains(char::is_whitespace) {
        return match crate::scenario::parse_http_target(text) {
            Ok(crate::scenario::Target::Http { method, url }) => {
                Ok(MockTarget::Http { method, url })
            }
            Ok(crate::scenario::Target::Function { .. }) => unreachable!("an http target"),
            Err(_) => Err(not_a_target(text)),
        };
    }
    if is_function_name(text) {
        return Ok(MockTarget::Function {
            name: text.to_string(),
        });
    }
    Err(not_a_target(text))
}

fn not_a_target(text: &str) -> String {
    format!(
        "'{text}' is not a target: an HTTP target is 'METHOD url-glob' (use '*' for any method), \
         a Function target is a name like orders.submit"
    )
}

/// `[A-Za-z_][A-Za-z0-9_]*` segments joined by `.`.
pub fn is_function_name(text: &str) -> bool {
    !text.is_empty()
        && text.split('.').all(|segment| {
            let mut chars = segment.chars();
            chars
                .next()
                .is_some_and(|first| first.is_ascii_alphabetic() || first == '_')
                && chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
        })
}

/// Parse a handler key of `mocks/index.ts`: an HTTP target only.
pub fn parse_handler_key(key: &str) -> Result<MockTarget, String> {
    match parse_target(key) {
        Ok(target @ MockTarget::Http { .. }) => Ok(target),
        Ok(MockTarget::Function { .. }) | Err(_) => Err(format!(
            "'{key}' is not a target: a handler key is 'METHOD url-glob' (use '*' for any method)"
        )),
    }
}

/// `mocks/config.json`, validated.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MockConfig {
    /// The project default; absent means `none`.
    pub mock: MockMode,
    /// HTTP targets answered the other way: real under `all`, mocked under
    /// `none`.
    #[serde(default)]
    pub overrides: Vec<String>,
}

impl Default for MockConfig {
    fn default() -> Self {
        Self {
            mock: MockMode::None,
            overrides: Vec::new(),
        }
    }
}

impl MockConfig {
    /// Parse and validate `mocks/config.json`. `keys` are the handler keys
    /// of `mocks/index.ts`, `None` when there is none. Every error names the
    /// file: `mocks/config.json: …`.
    pub fn parse(value: &Value, keys: Option<&[String]>) -> Result<Self, String> {
        let Some(keys) = keys else {
            return Err(format!("{CONFIG_FILE} needs mocks/index.ts"));
        };
        Self::parse_inner(value, keys).map_err(|err| format!("{CONFIG_FILE}: {err}"))
    }

    fn parse_inner(value: &Value, keys: &[String]) -> Result<Self, String> {
        let Value::Object(fields) = value else {
            return Err(
                "must be an object { \"mock\": \"all\" | \"none\", \"overrides\": [...] }".into(),
            );
        };
        if let Some(unknown) = fields
            .keys()
            .find(|key| !CONFIG_KEYS.contains(&key.as_str()))
        {
            return Err(format!(
                "unknown key \"{unknown}\"; the keys are mock and overrides"
            ));
        }
        let mock = match fields.get("mock") {
            None | Some(Value::Null) => MockMode::None,
            Some(Value::String(text)) => MockMode::parse(text)
                .ok_or_else(|| format!("mock must be \"all\" or \"none\", got \"{text}\""))?,
            Some(other) => return Err(format!("mock must be \"all\" or \"none\", got {other}")),
        };
        let overrides = match fields.get("overrides") {
            None | Some(Value::Null) => Vec::new(),
            Some(Value::Array(items)) => items
                .iter()
                .enumerate()
                .map(|(index, item)| match item {
                    Value::String(text) => Ok(text.trim().to_string()),
                    other => Err(format!(
                        "overrides[{index}] must be a target string, got {other}"
                    )),
                })
                .collect::<Result<Vec<_>, _>>()?,
            Some(other) => {
                return Err(format!(
                    "overrides must be an array of targets, got {other}"
                ));
            }
        };
        for (index, text) in overrides.iter().enumerate() {
            match parse_target(text) {
                Ok(MockTarget::Http { .. }) => {}
                Ok(MockTarget::Function { .. }) => {
                    return Err(format!(
                        "overrides[{index}] \"{text}\" is a Function name; the Worker project \
                         selects its own mocks"
                    ));
                }
                Err(_) => {
                    return Err(format!(
                        "overrides[{index}] \"{text}\" is not a target: an HTTP target is \
                         'METHOD url-glob' (use '*' for any method)"
                    ));
                }
            }
            if let Some(first) = overrides[..index].iter().position(|seen| seen == text) {
                return Err(format!("overrides[{index}] repeats overrides[{first}]"));
            }
            // Under `none` an override routes calls to a handler, so one
            // must exist; under `all` it sends them to the real backend.
            if mock == MockMode::None && !keys.iter().any(|key| key.trim() == text) {
                return Err(format!(
                    "overrides[{index}] \"{text}\" matches no handler in mocks/index.ts"
                ));
            }
        }
        Ok(Self { mock, overrides })
    }
}

/// Who set a selection entry, lowest first.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum MockOwner {
    /// `mocks/config.json`.
    Config,
    /// `lingxia dev --mock`.
    Baseline,
    /// `lxdev mock all|none`.
    Dev,
    /// A test run (`test:<run id>` on the wire).
    Run(String),
}

impl MockOwner {
    /// The layer an entry of this owner is shown as, next to the call it
    /// decided: `config`, `--mock`, `live`, `live target`, `test`.
    pub fn layer(&self, entry: &MockEntry) -> &'static str {
        match (self, entry) {
            (Self::Config, _) => "config",
            (Self::Baseline, _) => "--mock",
            (Self::Dev, MockEntry::Whole(_)) => "live",
            (Self::Dev, MockEntry::Targets(..)) => "live target",
            (Self::Run(_), _) => "test",
        }
    }

    fn rank(&self) -> u8 {
        match self {
            Self::Config => 0,
            Self::Baseline => 1,
            Self::Dev => 2,
            Self::Run(_) => 3,
        }
    }

    /// `config`, `baseline`, `dev`, `test:<run>`.
    pub fn label(&self) -> String {
        match self {
            Self::Config => "config".into(),
            Self::Baseline => "baseline".into(),
            Self::Dev => DEV_OWNER.into(),
            Self::Run(run) => crate::scenario::test_owner(run),
        }
    }
}

/// One selection entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MockEntry {
    /// Everything answers this way.
    Whole(MockMode),
    /// The calls these targets match answer this way.
    Targets(MockMode, Vec<String>),
}

impl MockEntry {
    pub fn mode(&self) -> MockMode {
        match self {
            Self::Whole(mode) | Self::Targets(mode, _) => *mode,
        }
    }

    /// `{ mode, targets? }`.
    pub fn to_json(&self) -> Value {
        match self {
            Self::Whole(mode) => serde_json::json!({ "mode": mode }),
            Self::Targets(mode, targets) => serde_json::json!({ "mode": mode, "targets": targets }),
        }
    }

    /// `all`, or `none 'GET **/qoe/*' 'GET **/x'`.
    pub fn describe(&self) -> String {
        match self {
            Self::Whole(mode) => mode.to_string(),
            Self::Targets(mode, targets) => format!(
                "{mode} {}",
                targets
                    .iter()
                    .map(|target| format!("'{target}'"))
                    .collect::<Vec<_>>()
                    .join(" ")
            ),
        }
    }
}

/// The owner-layered selection.
///
/// - `all` / `none` without targets installs a whole entry for its owner
///   and drops that owner's earlier entries; with targets it appends a
///   targets entry.
/// - For one call: take the highest owner that has entries (`dev` is
///   skipped while a test run is active); walk its entries newest first;
///   the first targets entry that matches the call, or the first whole
///   entry, decides. When none of that owner's entries decides, fall to the
///   next owner down. With no entries anywhere: `none`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Selection {
    /// `(owner, entries oldest first)`.
    layers: Vec<(MockOwner, Vec<MockEntry>)>,
}

impl Selection {
    pub const fn new() -> Self {
        Self { layers: Vec::new() }
    }

    /// Apply `all` / `none` for `owner`: a whole entry without targets,
    /// else a targets entry.
    pub fn set(&mut self, owner: MockOwner, mode: MockMode, targets: Vec<String>) {
        let entries = self.entries_mut(owner);
        if targets.is_empty() {
            entries.clear();
            entries.push(MockEntry::Whole(mode));
        } else {
            entries.push(MockEntry::Targets(mode, targets));
        }
    }

    /// Replace `owner`'s entries.
    pub fn replace(&mut self, owner: MockOwner, entries: Vec<MockEntry>) {
        self.layers.retain(|(layer, _)| *layer != owner);
        if !entries.is_empty() {
            self.layers.push((owner, entries));
        }
    }

    /// Drop `owner`'s entries; whether it had any.
    pub fn drop_owner(&mut self, owner: &MockOwner) -> bool {
        let before = self.layers.len();
        self.layers.retain(|(layer, _)| layer != owner);
        self.layers.len() != before
    }

    /// Drop every run owner's entries.
    pub fn drop_runs(&mut self) {
        self.layers
            .retain(|(owner, _)| !matches!(owner, MockOwner::Run(_)));
    }

    pub fn entries(&self, owner: &MockOwner) -> &[MockEntry] {
        self.layers
            .iter()
            .find(|(layer, _)| layer == owner)
            .map_or(&[], |(_, entries)| entries.as_slice())
    }

    fn entries_mut(&mut self, owner: MockOwner) -> &mut Vec<MockEntry> {
        let position = match self.layers.iter().position(|(layer, _)| *layer == owner) {
            Some(position) => position,
            None => {
                self.layers.push((owner, Vec::new()));
                self.layers.len() - 1
            }
        };
        &mut self.layers[position].1
    }

    /// Owners that have entries, highest first.
    pub fn owners(&self) -> Vec<&MockOwner> {
        let mut owners: Vec<&MockOwner> = self
            .layers
            .iter()
            .filter(|(_, entries)| !entries.is_empty())
            .map(|(owner, _)| owner)
            .collect();
        owners.sort_by_key(|owner| std::cmp::Reverse(owner.rank()));
        owners
    }

    /// Decide one call. `matches(target)` says whether a target matches
    /// it; `dev_aside` skips the `dev` owner (a test run is active). Returns
    /// the mode and the owner that decided, `None` when nothing did.
    pub fn resolve(
        &self,
        dev_aside: bool,
        matches: &dyn Fn(&str) -> bool,
    ) -> (MockMode, Option<MockOwner>) {
        match self.decide(dev_aside, matches) {
            Some((owner, entry)) => (entry.mode(), Some(owner.clone())),
            None => (MockMode::None, None),
        }
    }

    /// [`Self::resolve`], naming the owner and the entry that decided.
    pub fn decide(
        &self,
        dev_aside: bool,
        matches: &dyn Fn(&str) -> bool,
    ) -> Option<(&MockOwner, &MockEntry)> {
        for owner in self.owners() {
            if dev_aside && *owner == MockOwner::Dev {
                continue;
            }
            for entry in self.entries(owner).iter().rev() {
                let decides = match entry {
                    MockEntry::Whole(_) => true,
                    MockEntry::Targets(_, targets) => targets.iter().any(|target| matches(target)),
                };
                if decides {
                    return Some((owner, entry));
                }
            }
        }
        None
    }

    /// The entries of `config`: `whole(mock)`, then the overrides the
    /// opposite way.
    pub fn config_entries(config: &MockConfig) -> Vec<MockEntry> {
        let mut entries = vec![MockEntry::Whole(config.mock)];
        if !config.overrides.is_empty() {
            entries.push(MockEntry::Targets(
                config.mock.opposite(),
                config.overrides.clone(),
            ));
        }
        entries
    }
}

/// The effective selection of an app in one line, and where it comes
/// from, for the `lingxia dev` banner and `lxdev mock`:
///
/// - `all — from mocks/config.json · real: GET **/qoe/*`
/// - `none — from lingxia dev --mock (mocks/config.json: all, 2 real overrides — not in effect)`
/// - `all — live (lxdev mock all) over lingxia dev --mock none; lxdev mock reset to return`
/// - `none — default (no mocks/config.json)`
///
/// `live` are the dev session's entries, oldest first; live targets are
/// listed after the whole selection they sit on.
pub fn describe_selection(
    config: Option<&MockConfig>,
    baseline: Option<MockMode>,
    live: &[MockEntry],
) -> String {
    let overrides = |config: &MockConfig| {
        let n = config.overrides.len();
        format!(
            "{n} {} override{}",
            config.mock.opposite().noun(),
            if n == 1 { "" } else { "s" }
        )
    };
    let config_text = |config: &MockConfig| {
        if config.overrides.is_empty() {
            format!("{CONFIG_FILE}: {}", config.mock)
        } else {
            format!("{CONFIG_FILE}: {}, {}", config.mock, overrides(config))
        }
    };
    let below = || match (baseline, config) {
        (Some(mode), _) => format!("lingxia dev --mock {mode}"),
        (None, Some(config)) => config_text(config),
        (None, None) => "the default none (no mocks/config.json)".to_string(),
    };
    let live_whole = live.iter().rev().find_map(|entry| match entry {
        MockEntry::Whole(mode) => Some(*mode),
        MockEntry::Targets(..) => None,
    });
    let live_targets: Vec<&MockEntry> = live
        .iter()
        .filter(|entry| matches!(entry, MockEntry::Targets(..)))
        .collect();
    let mut text = match (live_whole, baseline, config) {
        (Some(mode), _, _) => format!("{mode} — live (lxdev mock {mode}) over {}", below()),
        (None, Some(mode), Some(config)) => format!(
            "{mode} — from lingxia dev --mock ({} — not in effect)",
            config_text(config)
        ),
        (None, Some(mode), None) => format!("{mode} — from lingxia dev --mock"),
        (None, None, Some(config)) if config.overrides.is_empty() => {
            format!("{} — from {CONFIG_FILE}", config.mock)
        }
        (None, None, Some(config)) => format!(
            "{} — from {CONFIG_FILE} · {}: {}",
            config.mock,
            config.mock.opposite().noun(),
            config.overrides.join(", ")
        ),
        (None, None, None) => "none — default (no mocks/config.json)".to_string(),
    };
    if !live_targets.is_empty() {
        text.push_str(" · live: ");
        text.push_str(
            &live_targets
                .iter()
                .map(|entry| entry.describe())
                .collect::<Vec<_>>()
                .join(", "),
        );
    }
    if !live.is_empty() {
        text.push_str("; lxdev mock reset to return");
    }
    text
}

/// The selection a test run sees (live changes stand aside): the layer
/// name only, for the run header — `all (lingxia dev --mock)`,
/// `none (mocks/config.json)`, `none (default)`.
pub fn describe_baseline(config: Option<&MockConfig>, baseline: Option<MockMode>) -> String {
    match (baseline, config) {
        (Some(mode), _) => format!("{mode} (lingxia dev --mock)"),
        (None, Some(config)) if config.overrides.is_empty() => {
            format!("{} ({CONFIG_FILE})", config.mock)
        }
        (None, Some(config)) => format!("{} ({CONFIG_FILE}, overrides)", config.mock),
        (None, None) => "none (default)".to_string(),
    }
}

impl MockMode {
    /// `mock` or `real`: what this mode answers with.
    pub fn noun(self) -> &'static str {
        match self {
            Self::All => "mock",
            Self::None => "real",
        }
    }
}

// ------------------------------- companion -------------------------------

/// The companion side of mocks: Function calls of a Worker project. The
/// dev server forwards these to a companion that declared
/// [`crate::dev_session::capabilities::MOCK`]. See
/// `docs/internal/companion-protocol.md`.
pub mod companion {
    use serde::{Deserialize, Serialize};

    /// Select mock or real Function handlers for an owner.
    pub const SET: &str = "mock.set";
    /// The companion's selection and handler hits.
    pub const STATUS: &str = "mock.status";
    /// Start handler memory over for an owner.
    pub const RESET: &str = "mock.reset";

    /// Error code of a `mock.set` that named unknown Functions; `data` is
    /// [`InvalidTargets`].
    pub const INVALID_TARGETS: &str = "invalid_targets";

    /// `mode` of `mock.set`: `all`, `none`, or `default` (drop the owner's
    /// entries: the owners below decide, down to the companion's own
    /// configured selection).
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
    #[serde(rename_all = "lowercase")]
    pub enum SetMode {
        All,
        None,
        Default,
    }

    /// `mock.set` params.
    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
    pub struct SetParams {
        /// `dev`, or `test:<run id>`.
        pub owner: String,
        pub mode: SetMode,
        /// Function names; absent for a whole entry. Not allowed with
        /// `default`.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub targets: Option<Vec<String>>,
    }

    /// `mock.set` result, and the head of `mock.status`.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
    pub struct SetResult {
        /// Functions whose mock handler answers now.
        pub mocked: u64,
        /// Functions the Worker project defines.
        pub total: u64,
    }

    /// `data` of an [`INVALID_TARGETS`] error.
    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
    pub struct InvalidTargets {
        pub unknown: Vec<String>,
    }

    /// One selection entry of an owner.
    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
    pub struct Entry {
        /// `all` or `none`.
        pub mode: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub targets: Option<Vec<String>>,
    }

    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
    pub struct OwnerStatus {
        pub owner: String,
        /// Whether its entries decide now (`dev` stands aside under a test
        /// owner).
        pub active: bool,
        pub entries: Vec<Entry>,
    }

    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
    pub struct HandlerStatus {
        pub function: String,
        /// Whether its mock handler answers now.
        pub mock: bool,
        /// Calls it answered since the session started.
        pub hits: u64,
    }

    /// `mock.status` result.
    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
    pub struct StatusResult {
        pub mocked: u64,
        pub total: u64,
        #[serde(default)]
        pub owners: Vec<OwnerStatus>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub handlers: Option<Vec<HandlerStatus>>,
        /// `fresh`: `mock.reset` starts handler memory over; `shared`: it
        /// cannot, handler memory lives as long as the companion.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub reset: Option<String>,
    }

    /// `mock.reset` params.
    #[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
    pub struct ResetParams {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub owner: Option<String>,
    }

    /// `mock.reset` result.
    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
    pub struct ResetResult {
        pub reset: bool,
        /// Why handler memory was not started over.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub reason: Option<String>,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn targets_are_http_with_whitespace_or_function_names() {
        assert_eq!(
            parse_target("get **/qoe/*").unwrap(),
            MockTarget::Http {
                method: Some("GET".into()),
                url: "**/qoe/*".into()
            }
        );
        assert_eq!(parse_target("* **/x").unwrap().label(), "* **/x");
        assert_eq!(
            parse_target("orders.submit").unwrap(),
            MockTarget::Function {
                name: "orders.submit".into()
            }
        );
        assert!(parse_target("_private.fn_2").is_ok());
        for bad in ["**/qoe/*", "orders..submit", "1orders", "GET1 x", ""] {
            let err = parse_target(bad).unwrap_err();
            assert!(
                err.ends_with(
                    "is not a target: an HTTP target is 'METHOD url-glob' (use '*' for any \
                     method), a Function target is a name like orders.submit"
                ),
                "{bad}: {err}"
            );
        }
        assert_eq!(
            parse_target("**/qoe/*").unwrap_err(),
            "'**/qoe/*' is not a target: an HTTP target is 'METHOD url-glob' (use '*' for any \
             method), a Function target is a name like orders.submit"
        );
        assert!(parse_handler_key("GET **/x").is_ok());
        assert_eq!(
            parse_handler_key("orders.submit").unwrap_err(),
            "'orders.submit' is not a target: a handler key is 'METHOD url-glob' (use '*' for \
             any method)"
        );
    }

    fn keys() -> Vec<String> {
        vec!["GET **/qoe/*".into(), "POST **/sessions".into()]
    }

    #[test]
    fn config_validation_names_the_file_and_the_entry() {
        let ok = MockConfig::parse(
            &json!({ "$schema": "x", "mock": "all", "overrides": ["GET **/real/*"] }),
            Some(&keys()),
        )
        .unwrap();
        assert_eq!(ok.mock, MockMode::All);
        assert_eq!(ok.overrides, ["GET **/real/*"]);
        assert_eq!(
            MockConfig::parse(&json!({}), Some(&keys())).unwrap(),
            MockConfig::default()
        );
        let cases = [
            (
                json!({ "targets": [] }),
                "mocks/config.json: unknown key \"targets\"; the keys are mock and overrides",
            ),
            (
                json!({ "mock": "on" }),
                "mocks/config.json: mock must be \"all\" or \"none\", got \"on\"",
            ),
            (
                json!({ "mock": "all", "overrides": ["GET **/a", "orders.submit"] }),
                "mocks/config.json: overrides[1] \"orders.submit\" is a Function name; the Worker \
                 project selects its own mocks",
            ),
            (
                json!({ "mock": "all", "overrides": ["**/a"] }),
                "mocks/config.json: overrides[0] \"**/a\" is not a target: an HTTP target is \
                 'METHOD url-glob' (use '*' for any method)",
            ),
            (
                json!({ "mock": "all", "overrides": ["GET **/a", "GET **/b", "GET **/a"] }),
                "mocks/config.json: overrides[2] repeats overrides[0]",
            ),
            (
                json!({ "mock": "none", "overrides": ["GET **/qoe/*", "GET **/other"] }),
                "mocks/config.json: overrides[1] \"GET **/other\" matches no handler in \
                 mocks/index.ts",
            ),
        ];
        for (value, message) in cases {
            assert_eq!(
                MockConfig::parse(&value, Some(&keys())).unwrap_err(),
                message
            );
        }
        assert_eq!(
            MockConfig::parse(&json!({ "mock": "all" }), None).unwrap_err(),
            "mocks/config.json needs mocks/index.ts"
        );
    }

    #[test]
    fn selection_resolves_the_highest_owner_newest_first() {
        let matches_qoe = |target: &str| target.contains("qoe");
        let mut selection = Selection::default();
        assert_eq!(
            selection.resolve(false, &matches_qoe),
            (MockMode::None, None)
        );

        let config = MockConfig {
            mock: MockMode::All,
            overrides: vec!["GET **/qoe/*".into()],
        };
        selection.replace(MockOwner::Config, Selection::config_entries(&config));
        assert_eq!(selection.resolve(false, &matches_qoe).0, MockMode::None);
        assert_eq!(selection.resolve(false, &|_| false).0, MockMode::All);

        // Live targets win for what they match; the config decides the rest.
        selection.set(MockOwner::Dev, MockMode::All, vec!["GET **/qoe/*".into()]);
        assert_eq!(
            selection.resolve(false, &matches_qoe),
            (MockMode::All, Some(MockOwner::Dev))
        );
        assert_eq!(
            selection.resolve(false, &|_| false),
            (MockMode::All, Some(MockOwner::Config))
        );
        // Dev stands aside during a run.
        assert_eq!(
            selection.resolve(true, &matches_qoe),
            (MockMode::None, Some(MockOwner::Config))
        );

        // A whole entry drops the owner's earlier entries and wins.
        selection.set(MockOwner::Dev, MockMode::None, Vec::new());
        assert_eq!(
            selection.entries(&MockOwner::Dev),
            [MockEntry::Whole(MockMode::None)]
        );
        assert_eq!(selection.resolve(false, &|_| false).0, MockMode::None);
        // Newest first within an owner.
        selection.set(MockOwner::Dev, MockMode::All, vec!["GET **/qoe/*".into()]);
        assert_eq!(selection.resolve(false, &matches_qoe).0, MockMode::All);
        assert_eq!(selection.resolve(false, &|_| false).0, MockMode::None);

        // A baseline replaces the config as a whole, overrides included.
        selection.drop_owner(&MockOwner::Dev);
        selection.replace(MockOwner::Baseline, vec![MockEntry::Whole(MockMode::All)]);
        assert_eq!(selection.resolve(false, &matches_qoe).0, MockMode::All);

        // A run sits above everything and is dropped with its run.
        selection.set(MockOwner::Run("r1".into()), MockMode::None, Vec::new());
        assert_eq!(
            selection.resolve(true, &matches_qoe),
            (MockMode::None, Some(MockOwner::Run("r1".into())))
        );
        selection.drop_runs();
        assert_eq!(selection.resolve(true, &matches_qoe).0, MockMode::All);
        assert_eq!(MockOwner::Run("r1".into()).label(), "test:r1");
    }

    #[test]
    fn the_selection_line_names_its_layer_and_what_it_overrides() {
        let config = MockConfig {
            mock: MockMode::All,
            overrides: vec!["GET **/qoe/*".into(), "POST **/qoe/actions".into()],
        };
        assert_eq!(
            describe_selection(Some(&config), None, &[]),
            "all — from mocks/config.json · real: GET **/qoe/*, POST **/qoe/actions"
        );
        assert_eq!(
            describe_selection(Some(&config), Some(MockMode::None), &[]),
            "none — from lingxia dev --mock (mocks/config.json: all, 2 real overrides — not in \
             effect)"
        );
        assert_eq!(
            describe_selection(
                None,
                Some(MockMode::None),
                &[MockEntry::Whole(MockMode::All)]
            ),
            "all — live (lxdev mock all) over lingxia dev --mock none; lxdev mock reset to return"
        );
        assert_eq!(
            describe_selection(Some(&config), None, &[MockEntry::Whole(MockMode::None)]),
            "none — live (lxdev mock none) over mocks/config.json: all, 2 real overrides; lxdev \
             mock reset to return"
        );
        assert_eq!(
            describe_selection(
                Some(&MockConfig::default()),
                None,
                &[MockEntry::Targets(MockMode::All, vec!["GET **/a".into()])]
            ),
            "none — from mocks/config.json · live: all 'GET **/a'; lxdev mock reset to return"
        );
        assert_eq!(
            describe_selection(None, None, &[]),
            "none — default (no mocks/config.json)"
        );
        assert_eq!(
            describe_baseline(Some(&config), Some(MockMode::All)),
            "all (lingxia dev --mock)"
        );
        assert_eq!(
            describe_baseline(Some(&MockConfig::default()), None),
            "none (mocks/config.json)"
        );
        assert_eq!(describe_baseline(None, None), "none (default)");
    }

    #[test]
    fn companion_messages_round_trip() {
        use companion::*;
        let set: SetParams = serde_json::from_value(
            json!({ "owner": "dev", "mode": "all", "targets": ["orders.submit"] }),
        )
        .unwrap();
        assert_eq!(set.mode, SetMode::All);
        assert_eq!(
            serde_json::to_value(SetParams {
                owner: "test:r".into(),
                mode: SetMode::Default,
                targets: None
            })
            .unwrap(),
            json!({ "owner": "test:r", "mode": "default" })
        );
        let status: StatusResult = serde_json::from_value(json!({
            "mocked": 7, "total": 12,
            "owners": [{ "owner": "dev", "active": true, "entries": [{ "mode": "all" }] }],
            "handlers": [{ "function": "orders.submit", "mock": true, "hits": 3 }],
            "reset": "fresh"
        }))
        .unwrap();
        assert_eq!((status.mocked, status.total), (7, 12));
        assert_eq!(status.handlers.unwrap()[0].hits, 3);
        let reset: ResetResult =
            serde_json::from_value(json!({ "reset": false, "reason": "rebuild" })).unwrap();
        assert_eq!(reset.reason.as_deref(), Some("rebuild"));
        assert_eq!(
            serde_json::to_value(ResetParams::default()).unwrap(),
            json!({})
        );
    }
}
