//! Mocks: the lxapp's `mocks/index.ts` handlers and the selection that
//! decides, per call, whether a handler or the real backend answers.
//!
//! ```text
//! per call:   test route  >  scenario rule  >  selection → mock handler | real
//! ```
//!
//! Rust decides, JS executes: [`Mocks::decide`] names the handler key (or
//! says no handler matches) after routes and rules passed, and the Logic
//! wrapper runs the handler. The loaded source lives here per appid; each
//! Logic context evaluates it lazily and again whenever its `generation`
//! moves (a reload, a reset, each spec of a run), which is the whole "fresh
//! state" mechanism.

use super::capture::{REDACTED, redact_url};
use super::registry::{UrlMatcher, now_ms};
use lingxia_control_protocol::mock::{
    MockConfig, MockEntry, MockMode, MockOwner, MockTarget, Selection, describe_baseline,
    describe_selection, parse_handler_key, parse_target,
};
use serde_json::{Value, json};
use std::cell::RefCell;
use std::sync::Arc;

/// Owner of the requests a mock answer holds open (`hang`, an SSE stream
/// without `drop`); reloads and resets release them.
pub(crate) const MOCK_HOLDER: &str = "@mocks";
/// Unhandled calls and handler errors a set keeps for status.
const MAX_DIAGNOSTICS: usize = 50;

/// One handler key, compiled.
#[derive(Debug, Clone)]
pub(crate) struct MockKey {
    pub key: String,
    method: Option<String>,
    matcher: UrlMatcher,
    pub hits: u64,
}

impl MockKey {
    fn matches(&self, method: &str, url: &str) -> bool {
        self.method
            .as_deref()
            .is_none_or(|expected| expected.eq_ignore_ascii_case(method))
            && self.matcher.is_match(url)
    }
}

/// Compile an HTTP target (`METHOD url-glob`).
fn compile(target: &str) -> Result<(Option<String>, UrlMatcher), String> {
    match parse_target(target)? {
        MockTarget::Http { method, url } => {
            let matcher = super::scenario::parse_url_string(&url)
                .map_err(|err| format!("'{target}': {err}"))?;
            Ok((method, matcher))
        }
        MockTarget::Function { name } => Err(format!(
            "'{name}' is a Function name; the runtime selects HTTP targets only"
        )),
    }
}

/// A call no handler key matched, for status.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Unhandled {
    pub method: String,
    /// Credentials redacted.
    pub url: String,
    pub count: u64,
    pub last_ms: u64,
}

/// A handler that threw or returned something that is not an answer.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct HandlerError {
    pub key: String,
    pub message: String,
    pub count: u64,
    pub last_ms: u64,
}

/// Why handler state started over.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Fresh {
    /// The first load of the session.
    Load,
    /// A save under `mocks/`.
    Reload,
    /// `lxdev mock reset`.
    Reset,
    /// A spec of a test run started.
    Spec,
}

impl Fresh {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Load => "load",
            Self::Reload => "reload",
            Self::Reset => "reset",
            Self::Spec => "spec",
        }
    }
}

/// The mocks of one lxapp.
#[derive(Debug, Clone)]
pub(crate) struct MockSet {
    pub appid: String,
    /// A script whose value is the default export of `mocks/index.ts`.
    pub source: Arc<str>,
    pub keys: Vec<MockKey>,
    /// `mocks/config.json`; `None` when the file is absent.
    pub config: Option<MockConfig>,
    /// The config's entries as the lowest owner.
    config_selection: Selection,
    /// Moves on every load and reset; Logic contexts re-evaluate `source`
    /// when it does.
    pub generation: u64,
    pub fresh_ms: u64,
    pub fresh: Fresh,
    pub unhandled: Vec<Unhandled>,
    pub errors: Vec<HandlerError>,
}

impl MockSet {
    fn key_index(&self, method: &str, url: &str) -> Option<usize> {
        self.keys.iter().position(|key| key.matches(method, url))
    }
}

/// How the selection answers one call.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum MockDecision {
    /// The real backend answers.
    Real,
    /// The handler under this key answers.
    Mock { key: String },
    /// The selection says mock, but no key matches: the call fails.
    /// `first` when this method and URL were not seen unhandled before.
    Unhandled { detail: String, first: bool },
}

/// Every lxapp's mocks and the session-wide selection owners.
#[derive(Debug, Default)]
pub(crate) struct Mocks {
    pub sets: Vec<MockSet>,
    /// `baseline`, `dev` and run owners; each set carries its `config`.
    pub selection: Selection,
    next_generation: u64,
    /// Compiled selection targets.
    compiled: RefCell<Vec<(String, Option<Compiled>)>>,
}

type Compiled = (Option<String>, UrlMatcher);

impl Mocks {
    pub(crate) const fn new() -> Self {
        Self {
            sets: Vec::new(),
            selection: Selection::new(),
            next_generation: 0,
            compiled: RefCell::new(Vec::new()),
        }
    }

    pub(crate) fn set(&self, appid: &str) -> Option<&MockSet> {
        self.sets.iter().find(|set| set.appid == appid)
    }

    fn set_mut(&mut self, appid: &str) -> Option<&mut MockSet> {
        self.sets.iter_mut().find(|set| set.appid == appid)
    }

    fn bump(&mut self) -> u64 {
        self.next_generation += 1;
        self.next_generation
    }

    /// Load (or replace) an app's handlers and config. All or nothing: an
    /// invalid key or config keeps the previous set. Returns the new
    /// generation.
    pub(crate) fn load(
        &mut self,
        appid: &str,
        source: &str,
        keys: &[String],
        config: Option<&Value>,
    ) -> Result<u64, String> {
        let mut compiled = Vec::with_capacity(keys.len());
        for key in keys {
            parse_handler_key(key).map_err(|err| format!("mocks/index.ts: {err}"))?;
            let (method, matcher) = compile(key).map_err(|err| format!("mocks/index.ts: {err}"))?;
            if compiled.iter().any(|seen: &MockKey| seen.key == *key) {
                return Err(format!("mocks/index.ts: '{key}' is listed twice"));
            }
            compiled.push(MockKey {
                key: key.clone(),
                method,
                matcher,
                hits: 0,
            });
        }
        let config = config
            .filter(|value| !value.is_null())
            .map(|value| MockConfig::parse(value, Some(keys)))
            .transpose()?;
        let mut config_selection = Selection::default();
        if let Some(config) = &config {
            config_selection.replace(MockOwner::Config, Selection::config_entries(config));
        }
        let generation = self.bump();
        let now = now_ms();
        let previous = self.set_mut(appid).map(|set| {
            (
                std::mem::take(&mut set.unhandled),
                std::mem::take(&mut set.errors),
                set.keys
                    .iter()
                    .map(|key| (key.key.clone(), key.hits))
                    .collect::<Vec<_>>(),
            )
        });
        let fresh = if previous.is_some() {
            Fresh::Reload
        } else {
            Fresh::Load
        };
        let (unhandled, errors, hits) = previous.unwrap_or_default();
        for key in &mut compiled {
            key.hits = hits
                .iter()
                .find(|(seen, _)| *seen == key.key)
                .map_or(0, |(_, hits)| *hits);
        }
        let set = MockSet {
            appid: appid.to_string(),
            source: Arc::from(source),
            keys: compiled,
            config,
            config_selection,
            generation,
            fresh_ms: now,
            fresh,
            unhandled,
            errors,
        };
        self.sets.retain(|set| set.appid != appid);
        self.sets.push(set);
        Ok(generation)
    }

    /// Drop an app's handlers and config (its `mocks/` is gone): the app
    /// then has no mocks, as if none had been loaded. Whether it had any.
    pub(crate) fn unload(&mut self, appid: &str) -> bool {
        let before = self.sets.len();
        self.sets.retain(|set| set.appid != appid);
        let removed = self.sets.len() != before;
        if removed {
            self.bump();
        }
        removed
    }

    /// Start handler state over for `appid`, or for every app. Returns the
    /// new generation of each.
    pub(crate) fn reset(&mut self, appid: Option<&str>, why: Fresh) -> Vec<(String, u64)> {
        let now = now_ms();
        let apps: Vec<String> = self
            .sets
            .iter()
            .filter(|set| appid.is_none_or(|appid| set.appid == appid))
            .map(|set| set.appid.clone())
            .collect();
        apps.into_iter()
            .map(|appid| {
                let generation = self.bump();
                if let Some(set) = self.set_mut(&appid) {
                    set.generation = generation;
                    set.fresh_ms = now;
                    set.fresh = why;
                }
                (appid, generation)
            })
            .collect()
    }

    /// Drop every set and every owner's selection: the dev session ended.
    pub(crate) fn clear(&mut self) {
        self.sets.clear();
        self.selection = Selection::default();
    }

    /// Whether `target` matches the call. An uncompilable target (never
    /// installed; defensive) matches nothing.
    fn target_matches(&self, target: &str, method: &str, url: &str) -> bool {
        let mut compiled = self.compiled.borrow_mut();
        let position = match compiled.iter().position(|(seen, _)| seen == target) {
            Some(position) => position,
            None => {
                compiled.push((target.to_string(), compile(target).ok()));
                compiled.len() - 1
            }
        };
        compiled[position]
            .1
            .as_ref()
            .is_some_and(|(expected, matcher)| {
                expected
                    .as_deref()
                    .is_none_or(|expected| expected.eq_ignore_ascii_case(method))
                    && matcher.is_match(url)
            })
    }

    /// The mode the selection gives this call of `appid` — the session
    /// owners first, then the app's config — and the layer that decided
    /// (`default` when none did).
    pub(crate) fn mode(
        &self,
        appid: &str,
        method: &str,
        url: &str,
        run_active: bool,
    ) -> (MockMode, &'static str) {
        let matches = |target: &str| self.target_matches(target, method, url);
        let decided = self.selection.decide(run_active, &matches).or_else(|| {
            self.set(appid)
                .and_then(|set| set.config_selection.decide(run_active, &matches))
        });
        match decided {
            Some((owner, entry)) => (entry.mode(), owner.layer(entry)),
            None => (MockMode::None, "default"),
        }
    }

    /// Who answers a call routes and rules passed on, and the selection
    /// layer that decided it (`None` when `appid` has no mocks: they cannot
    /// answer, whatever the selection says).
    pub(crate) fn decide(
        &mut self,
        appid: &str,
        method: &str,
        url: &str,
        run_active: bool,
    ) -> (MockDecision, Option<&'static str>) {
        if self.set(appid).is_none() {
            return (MockDecision::Real, None);
        }
        let (mode, layer) = self.mode(appid, method, url, run_active);
        if mode == MockMode::None {
            return (MockDecision::Real, Some(layer));
        }
        let Some(set) = self.set_mut(appid) else {
            return (MockDecision::Real, None);
        };
        let decision = match set.key_index(method, url) {
            Some(index) => {
                let key = &mut set.keys[index];
                key.hits += 1;
                MockDecision::Mock {
                    key: key.key.clone(),
                }
            }
            None => {
                let method = method.to_ascii_uppercase();
                let shown = redact_url(url, REDACTED);
                let now = now_ms();
                let mut first = false;
                match set
                    .unhandled
                    .iter_mut()
                    .find(|seen| seen.method == method && seen.url == shown)
                {
                    Some(seen) => {
                        seen.count += 1;
                        seen.last_ms = now;
                    }
                    None => {
                        first = true;
                        if set.unhandled.len() >= MAX_DIAGNOSTICS {
                            set.unhandled.remove(0);
                        }
                        set.unhandled.push(Unhandled {
                            method: method.clone(),
                            url: shown.clone(),
                            count: 1,
                            last_ms: now,
                        });
                    }
                }
                let suggested = format!("{method} {}", suggested_glob(url));
                MockDecision::Unhandled {
                    detail: format!(
                        "no mock handler for {method} {shown}; add '{suggested}' to \
                         mocks/index.ts, or run: lxdev mock none '{suggested}'"
                    ),
                    first,
                }
            }
        };
        (decision, Some(layer))
    }

    /// Note a handler failure for status.
    pub(crate) fn failed(&mut self, appid: &str, key: &str, message: &str) {
        let Some(set) = self.set_mut(appid) else {
            return;
        };
        let now = now_ms();
        let message: String = message.chars().take(300).collect();
        match set.errors.iter_mut().find(|seen| seen.key == key) {
            Some(seen) => {
                seen.count += 1;
                seen.message = message;
                seen.last_ms = now;
            }
            None => {
                if set.errors.len() >= MAX_DIAGNOSTICS {
                    set.errors.remove(0);
                }
                set.errors.push(HandlerError {
                    key: key.to_string(),
                    message,
                    count: 1,
                    last_ms: now,
                });
            }
        }
    }

    /// The session baseline (`lingxia dev --mock`).
    pub(crate) fn baseline(&self) -> Option<MockMode> {
        self.selection
            .entries(&MockOwner::Baseline)
            .iter()
            .rev()
            .find_map(|entry| match entry {
                MockEntry::Whole(mode) => Some(*mode),
                MockEntry::Targets(..) => None,
            })
    }

    /// `session.network.mock.status`: per app the effective selection in
    /// one line (`selection`) and what a test run sees (`runSelection`),
    /// its handlers and their hits, unhandled calls and handler errors;
    /// the session's baseline and live entries.
    pub(crate) fn status(&self, run_active: bool) -> Value {
        let baseline = self.baseline();
        let live = self.selection.entries(&MockOwner::Dev);
        let apps: Vec<Value> = self
            .sets
            .iter()
            .map(|set| {
                json!({
                    "appid": set.appid,
                    "selection": describe_selection(set.config.as_ref(), baseline, live),
                    "runSelection": describe_baseline(set.config.as_ref(), baseline),
                    "config": set.config,
                    "handlers": set.keys.len(),
                    "keys": set.keys.iter().map(|key| json!({ "key": key.key, "hits": key.hits })).collect::<Vec<_>>(),
                    "unhandled": set.unhandled.iter().map(|entry| json!({
                        "method": entry.method,
                        "url": entry.url,
                        "count": entry.count,
                        "last": super::scenario::iso_utc(entry.last_ms as i64),
                    })).collect::<Vec<_>>(),
                    "errors": set.errors.iter().map(|entry| json!({
                        "key": entry.key,
                        "message": entry.message,
                        "count": entry.count,
                        "last": super::scenario::iso_utc(entry.last_ms as i64),
                    })).collect::<Vec<_>>(),
                    "generation": set.generation,
                    "fresh": {
                        "reason": set.fresh.as_str(),
                        "at": super::scenario::iso_utc(set.fresh_ms as i64),
                    },
                })
            })
            .collect();
        json!({
            "apps": apps,
            "baseline": baseline,
            "live": live.iter().map(MockEntry::to_json).collect::<Vec<_>>(),
            // Live entries stand aside while a test run is active.
            "suspended": run_active && !live.is_empty(),
        })
    }

    /// Apply `all` / `none` for an owner. Targets must be HTTP targets.
    pub(crate) fn select(
        &mut self,
        owner: MockOwner,
        mode: MockMode,
        targets: Vec<String>,
    ) -> Result<Vec<MockEntry>, String> {
        for target in &targets {
            match parse_target(target)? {
                MockTarget::Http { .. } => {
                    compile(target)?;
                }
                MockTarget::Function { name } => {
                    return Err(format!(
                        "'{name}' is a Function name; the companion selects Functions"
                    ));
                }
            }
        }
        self.selection.set(owner.clone(), mode, targets);
        Ok(self.selection.entries(&owner).to_vec())
    }
}

/// A glob a handler key could use for exactly this URL: `**` and its path,
/// `?*` for any query.
pub(crate) fn suggested_glob(url: &str) -> String {
    let url = url.split('#').next().unwrap_or(url);
    let (base, query) = match url.split_once('?') {
        Some((base, _)) => (base, true),
        None => (url, false),
    };
    let path = base
        .split_once("://")
        .and_then(|(_, rest)| rest.find('/').map(|slash| &rest[slash..]))
        .unwrap_or("/");
    format!("**{path}{}", if query { "?*" } else { "" })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn keys(list: &[&str]) -> Vec<String> {
        list.iter().map(|key| key.to_string()).collect()
    }

    fn loaded(config: Option<Value>) -> Mocks {
        let mut mocks = Mocks::new();
        mocks
            .load(
                "app",
                "({})",
                &keys(&["POST **/sessions", "GET **/devices/*", "* **/devices/**"]),
                config.as_ref(),
            )
            .unwrap();
        mocks
    }

    #[test]
    fn keys_are_tried_in_order_and_counted() {
        let mut mocks = loaded(Some(json!({ "mock": "all" })));
        assert_eq!(
            mocks.decide("app", "get", "https://h/devices/1", false).0,
            MockDecision::Mock {
                key: "GET **/devices/*".into()
            }
        );
        assert_eq!(
            mocks.decide("app", "PATCH", "https://h/devices/1", false).0,
            MockDecision::Mock {
                key: "* **/devices/**".into()
            }
        );
        let set = mocks.set("app").unwrap();
        assert_eq!(set.keys[1].hits, 1);
        assert_eq!(set.keys[2].hits, 1);
        // Another app has no mocks: the real backend answers it.
        assert_eq!(
            mocks.decide("other", "GET", "https://h/devices/1", false).0,
            MockDecision::Real
        );
    }

    #[test]
    fn an_unhandled_call_names_the_key_to_add() {
        let mut mocks = loaded(Some(json!({ "mock": "all" })));
        let MockDecision::Unhandled { detail, first } = mocks
            .decide(
                "app",
                "GET",
                "https://api.example.com/sub/qoe/summary?token=secret",
                false,
            )
            .0
        else {
            panic!("expected unhandled");
        };
        assert_eq!(
            detail,
            "no mock handler for GET https://api.example.com/sub/qoe/summary?token=***; \
             add 'GET **/sub/qoe/summary?*' to mocks/index.ts, or run: lxdev mock none \
             'GET **/sub/qoe/summary?*'"
        );
        assert!(first);
        let _ = mocks
            .decide(
                "app",
                "GET",
                "https://api.example.com/sub/qoe/summary?token=secret",
                false,
            )
            .0;
        let set = mocks.set("app").unwrap();
        assert_eq!(set.unhandled.len(), 1);
        assert_eq!(set.unhandled[0].count, 2);
        assert_eq!(suggested_glob("https://h/a/b"), "**/a/b");
        assert_eq!(suggested_glob("https://h"), "**/");
    }

    #[test]
    fn the_selection_layers_config_baseline_dev_and_run() {
        // No config: none.
        let mut mocks = loaded(None);
        assert_eq!(
            mocks.decide("app", "GET", "https://h/devices/1", false).0,
            MockDecision::Real
        );
        // Config `none` with an override that routes one key to mocks.
        let mut mocks = loaded(Some(
            json!({ "mock": "none", "overrides": ["GET **/devices/*"] }),
        ));
        assert!(matches!(
            mocks.decide("app", "GET", "https://h/devices/1", false).0,
            MockDecision::Mock { .. }
        ));
        assert_eq!(
            mocks.decide("app", "POST", "https://h/sessions", false).0,
            MockDecision::Real
        );
        // A baseline replaces the config as a whole, overrides included.
        mocks
            .selection
            .replace(MockOwner::Baseline, vec![MockEntry::Whole(MockMode::None)]);
        assert_eq!(
            mocks.decide("app", "GET", "https://h/devices/1", false).0,
            MockDecision::Real
        );
        // Dev targets win for what they match.
        mocks
            .select(MockOwner::Dev, MockMode::All, keys(&["POST **/sessions"]))
            .unwrap();
        assert!(matches!(
            mocks.decide("app", "POST", "https://h/sessions", false).0,
            MockDecision::Mock { .. }
        ));
        // ... but stand aside during a run.
        assert_eq!(
            mocks.decide("app", "POST", "https://h/sessions", true).0,
            MockDecision::Real
        );
        // A run's own selection applies to it, and is dropped with it.
        mocks
            .select(MockOwner::Run("r".into()), MockMode::All, Vec::new())
            .unwrap();
        assert!(matches!(
            mocks.decide("app", "GET", "https://h/devices/1", true).0,
            MockDecision::Mock { .. }
        ));
        mocks.selection.drop_runs();
        assert_eq!(
            mocks.decide("app", "GET", "https://h/devices/1", true).0,
            MockDecision::Real
        );
        // Function names belong to the companion.
        let err = mocks
            .select(MockOwner::Dev, MockMode::All, keys(&["orders.submit"]))
            .unwrap_err();
        assert!(err.contains("Function name"), "{err}");
        let err = mocks
            .select(MockOwner::Dev, MockMode::All, keys(&["**/x"]))
            .unwrap_err();
        assert!(err.contains("is not a target"), "{err}");
    }

    #[test]
    fn loads_are_all_or_nothing_and_generations_move() {
        let mut mocks = loaded(Some(json!({ "mock": "all" })));
        let first = mocks.set("app").unwrap().generation;
        let _ = mocks.decide("app", "GET", "https://h/devices/1", false).0;
        let err = mocks
            .load("app", "({})", &keys(&["GET/devices"]), None)
            .unwrap_err();
        assert!(
            err.starts_with("mocks/index.ts: 'GET/devices' is not a target"),
            "{err}"
        );
        let err = mocks
            .load(
                "app",
                "({})",
                &keys(&["GET **/a"]),
                Some(&json!({ "mock": "on" })),
            )
            .unwrap_err();
        assert_eq!(
            err,
            "mocks/config.json: mock must be \"all\" or \"none\", got \"on\""
        );
        let err = mocks
            .load("app", "({})", &keys(&["GET **/a", "GET **/a"]), None)
            .unwrap_err();
        assert!(err.contains("listed twice"), "{err}");
        // The previous set still answers.
        let set = mocks.set("app").unwrap();
        assert_eq!((set.generation, set.keys.len()), (first, 3));

        // A reload keeps the hits of keys that stay, and moves the generation.
        let generation = mocks
            .load(
                "app",
                "({})",
                &keys(&["GET **/devices/*"]),
                Some(&json!({ "mock": "all" })),
            )
            .unwrap();
        assert!(generation > first);
        let set = mocks.set("app").unwrap();
        assert_eq!((set.keys[0].hits, set.fresh), (1, Fresh::Reload));
        let reset = mocks.reset(None, Fresh::Spec);
        assert_eq!(reset.len(), 1);
        assert!(reset[0].1 > generation);
        assert_eq!(mocks.set("app").unwrap().fresh, Fresh::Spec);
    }

    #[test]
    fn an_unloaded_app_has_no_mocks_left() {
        let mut mocks = loaded(Some(json!({ "mock": "all" })));
        assert!(mocks.unload("app"));
        assert!(mocks.set("app").is_none());
        assert_eq!(mocks.status(false)["apps"], json!([]));
        // Nothing can answer with mocks for it now.
        assert!(!matches!(
            mocks.decide("app", "GET", "https://h/devices/1", false).0,
            MockDecision::Mock { .. }
        ));
        assert!(!mocks.unload("app"), "a second unload has nothing to drop");
    }

    #[test]
    fn status_reports_the_selection_handlers_and_diagnostics() {
        let mut mocks = loaded(Some(
            json!({ "mock": "all", "overrides": ["GET **/qoe/*"] }),
        ));
        mocks.decide("app", "GET", "https://h/devices/1", false);
        mocks.decide("app", "GET", "https://h/insights", false);
        mocks.failed("app", "GET **/devices/*", "TypeError: x is undefined");
        let status = mocks.status(false);
        let app = &status["apps"][0];
        assert_eq!(
            app["selection"],
            "all — from mocks/config.json · real: GET **/qoe/*"
        );
        assert_eq!(app["runSelection"], "all (mocks/config.json, overrides)");
        assert_eq!(app["handlers"], 3);
        assert_eq!(
            app["keys"][1],
            json!({ "key": "GET **/devices/*", "hits": 1 })
        );
        assert_eq!(app["unhandled"][0]["url"], "https://h/insights");
        assert_eq!(app["errors"][0]["count"], 1);
        assert_eq!(app["fresh"]["reason"], "load");

        mocks
            .select(MockOwner::Dev, MockMode::None, Vec::new())
            .unwrap();
        let status = mocks.status(false);
        assert_eq!(
            status["apps"][0]["selection"],
            "none — live (lxdev mock none) over mocks/config.json: all, 1 real override; lxdev \
             mock reset to return"
        );
        assert_eq!(status["live"], json!([{ "mode": "none" }]));
        assert_eq!(mocks.status(true)["suspended"], true);

        let plain = loaded(None);
        assert_eq!(
            plain.status(false)["apps"][0]["selection"],
            "none — default (no mocks/config.json)"
        );
    }
}
