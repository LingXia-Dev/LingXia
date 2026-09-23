//! Route table for test network interception: matching, fulfillment specs,
//! and the matched-request log. Pure Rust so it is unit-testable without a
//! JS engine; `super` adapts it to the automation driver and Logic `fetch`.

use regex::Regex;
use std::collections::VecDeque;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Mutex, MutexGuard};
use std::time::{SystemTime, UNIX_EPOCH};

/// Matched-request entries kept per process; the oldest are dropped first.
const MAX_LOG_ENTRIES: usize = 1_000;
/// Fulfillment bodies are test fixtures, not payload transfer.
pub(crate) const MAX_BODY_BYTES: usize = 8 * 1024 * 1024;

#[derive(Debug, Clone)]
pub(crate) enum UrlMatcher {
    Glob {
        pattern: String,
        regex: Regex,
    },
    Regex {
        source: String,
        flags: String,
        regex: Regex,
    },
}

impl UrlMatcher {
    /// Playwright-style URL glob over the whole URL: `**` matches any run of
    /// characters, `*` any run without `/`, `{a,b}` alternatives. Everything
    /// else, `?` included, is literal so query strings can be written as-is.
    pub(crate) fn glob(pattern: &str) -> Result<Self, String> {
        if pattern.is_empty() {
            return Err("route URL pattern must not be empty".into());
        }
        let mut out = String::from("^");
        let mut chars = pattern.chars().peekable();
        let mut in_group = false;
        while let Some(c) = chars.next() {
            match c {
                '*' if chars.peek() == Some(&'*') => {
                    chars.next();
                    out.push_str(".*");
                }
                '*' => out.push_str("[^/]*"),
                '{' if !in_group => {
                    in_group = true;
                    out.push_str("(?:");
                }
                '}' if in_group => {
                    in_group = false;
                    out.push(')');
                }
                ',' if in_group => out.push('|'),
                other => out.push_str(&regex::escape(other.encode_utf8(&mut [0; 4]))),
            }
        }
        if in_group {
            return Err(format!(
                "route URL pattern '{pattern}' has an unclosed '{{'"
            ));
        }
        out.push('$');
        let regex = Regex::new(&out).map_err(|err| format!("invalid URL glob: {err}"))?;
        Ok(Self::Glob {
            pattern: pattern.to_string(),
            regex,
        })
    }

    /// A JS `RegExp` (`source` + `flags`), searched like `RegExp.test`.
    /// Rust `regex` syntax applies: no lookaround or backreferences.
    pub(crate) fn regex(source: &str, flags: &str) -> Result<Self, String> {
        let mut inline = String::new();
        for flag in flags.chars() {
            match flag {
                'i' | 's' | 'm' => inline.push(flag),
                // Stateful or encoding flags do not change a single test.
                'g' | 'y' | 'u' | 'd' | 'v' => {}
                other => return Err(format!("unsupported RegExp flag '{other}'")),
            }
        }
        let pattern = if inline.is_empty() {
            source.to_string()
        } else {
            format!("(?{inline}){source}")
        };
        let regex = Regex::new(&pattern)
            .map_err(|err| format!("route RegExp /{source}/{flags} is not supported: {err}"))?;
        Ok(Self::Regex {
            source: source.to_string(),
            flags: flags.to_string(),
            regex,
        })
    }

    pub(crate) fn is_match(&self, url: &str) -> bool {
        match self {
            Self::Glob { regex, .. } | Self::Regex { regex, .. } => regex.is_match(url),
        }
    }

    pub(crate) fn label(&self) -> String {
        match self {
            Self::Glob { pattern, .. } => pattern.clone(),
            Self::Regex { source, flags, .. } => format!("/{source}/{flags}"),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Fulfill {
    pub status: u16,
    pub status_text: Option<String>,
    pub headers: Vec<(String, String)>,
    pub body: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum RouteAction {
    Fulfill(Fulfill),
    /// Reject like a transport failure; the string is the reported reason.
    Abort(String),
    Continue,
}

impl RouteAction {
    fn kind(&self) -> &'static str {
        match self {
            Self::Fulfill(_) => "fulfill",
            Self::Abort(_) => "abort",
            Self::Continue => "continue",
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct RouteSpec {
    pub matcher: UrlMatcher,
    /// Upper-case method; `None` matches every method.
    pub method: Option<String>,
    /// Remaining matches before the route removes itself.
    pub times: Option<u32>,
    pub action: RouteAction,
}

#[derive(Debug)]
struct Route {
    id: u64,
    run_id: String,
    appid: String,
    spec: RouteSpec,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct RequestEntry {
    pub route_id: u64,
    pub pattern: String,
    pub method: String,
    pub url: String,
    /// `fulfill`, `abort`, or `continue`.
    pub action: &'static str,
    pub status: Option<u16>,
    pub timestamp_ms: u64,
    run_id: String,
    appid: String,
}

#[derive(Debug, Default)]
pub(crate) struct Registry {
    next_id: u64,
    routes: Vec<Route>,
    log: VecDeque<RequestEntry>,
}

impl Registry {
    /// Install a route for `appid` owned by `run_id`. `run_active` is checked
    /// under the table lock, so a run that finalizes concurrently either sees
    /// this route in its `clear_run` or rejects it here.
    pub(crate) fn install(
        &mut self,
        run_id: &str,
        appid: &str,
        spec: RouteSpec,
        run_active: impl FnOnce() -> bool,
    ) -> Result<u64, String> {
        if !run_active() {
            return Err("the automation run that owns this driver has ended".into());
        }
        self.next_id += 1;
        let id = self.next_id;
        self.routes.push(Route {
            id,
            run_id: run_id.to_string(),
            appid: appid.to_string(),
            spec,
        });
        Ok(id)
    }

    pub(crate) fn remove(&mut self, run_id: &str, id: u64) -> bool {
        let before = self.routes.len();
        self.routes
            .retain(|route| !(route.id == id && route.run_id == run_id));
        self.routes.len() != before
    }

    pub(crate) fn remove_app(&mut self, run_id: &str, appid: &str) -> usize {
        let before = self.routes.len();
        self.routes
            .retain(|route| !(route.run_id == run_id && route.appid == appid));
        before - self.routes.len()
    }

    /// Drop every route and log entry a run owns.
    pub(crate) fn clear_run(&mut self, run_id: &str) {
        self.routes.retain(|route| route.run_id != run_id);
        self.log.retain(|entry| entry.run_id != run_id);
    }

    /// The most recently installed matching route wins. `allowed` reports
    /// whether the app's network policy admits the URL: a fulfillment never
    /// answers a request the real `fetch` would have refused.
    pub(crate) fn decide(
        &mut self,
        appid: &str,
        method: &str,
        url: &str,
        allowed: impl FnOnce() -> bool,
    ) -> Option<RouteAction> {
        let index = self.routes.iter().rposition(|route| {
            route.appid == appid
                && route
                    .spec
                    .method
                    .as_deref()
                    .is_none_or(|expected| expected.eq_ignore_ascii_case(method))
                && route.spec.matcher.is_match(url)
        })?;
        let route = &mut self.routes[index];
        let mut action = route.spec.action.clone();
        if matches!(action, RouteAction::Fulfill(_)) && !allowed() {
            action = RouteAction::Continue;
        }
        let entry = RequestEntry {
            route_id: route.id,
            pattern: route.spec.matcher.label(),
            method: method.to_ascii_uppercase(),
            url: url.to_string(),
            action: action.kind(),
            status: match &action {
                RouteAction::Fulfill(fulfill) => Some(fulfill.status),
                _ => None,
            },
            timestamp_ms: now_ms(),
            run_id: route.run_id.clone(),
            appid: route.appid.clone(),
        };
        if let Some(times) = route.spec.times.as_mut() {
            *times = times.saturating_sub(1);
            if *times == 0 {
                self.routes.remove(index);
            }
        }
        if self.log.len() == MAX_LOG_ENTRIES {
            self.log.pop_front();
        }
        self.log.push_back(entry);
        Some(action)
    }

    pub(crate) fn requests(&self, run_id: &str, appid: &str) -> Vec<RequestEntry> {
        self.log
            .iter()
            .filter(|entry| entry.run_id == run_id && entry.appid == appid)
            .cloned()
            .collect()
    }

    fn len(&self) -> usize {
        self.routes.len()
    }
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_millis() as u64)
}

static ROUTES: Mutex<Registry> = Mutex::new(Registry {
    next_id: 0,
    routes: Vec::new(),
    log: VecDeque::new(),
});
/// Installed route count, mirrored outside the lock so Logic `fetch` pays one
/// atomic load when no test route exists.
static ACTIVE: AtomicUsize = AtomicUsize::new(0);

/// Run `f` against the process route table, keeping [`ACTIVE`] in sync.
pub(crate) fn with_registry<R>(f: impl FnOnce(&mut Registry) -> R) -> R {
    let mut guard: MutexGuard<'_, Registry> = ROUTES
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let result = f(&mut guard);
    ACTIVE.store(guard.len(), Ordering::Release);
    result
}

pub(crate) fn any_active() -> bool {
    ACTIVE.load(Ordering::Acquire) != 0
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fulfill(status: u16) -> RouteAction {
        RouteAction::Fulfill(Fulfill {
            status,
            status_text: None,
            headers: Vec::new(),
            body: Some("{}".into()),
        })
    }

    fn spec(matcher: UrlMatcher, method: Option<&str>, action: RouteAction) -> RouteSpec {
        RouteSpec {
            matcher,
            method: method.map(str::to_string),
            times: None,
            action,
        }
    }

    #[test]
    fn glob_semantics() {
        let glob = UrlMatcher::glob("**/v1/devices/*").unwrap();
        assert!(glob.is_match("https://api.example.com/v1/devices/abc"));
        assert!(!glob.is_match("https://api.example.com/v1/devices/abc/name"));
        assert!(glob.is_match("https://api.example.com/v1/devices/abc?x=1"));

        let deep = UrlMatcher::glob("https://api.example.com/**").unwrap();
        assert!(deep.is_match("https://api.example.com/a/b?c=d"));
        assert!(!deep.is_match("https://evil.example.com/a"));

        let literal = UrlMatcher::glob("**/search?q=a.b").unwrap();
        assert!(literal.is_match("http://x/search?q=a.b"));
        assert!(!literal.is_match("http://x/searchXq=aXb"));

        let alt = UrlMatcher::glob("**/*.{png,jpg}").unwrap();
        assert!(alt.is_match("http://x/a/b.png"));
        assert!(alt.is_match("http://x/b.jpg"));
        assert!(!alt.is_match("http://x/b.gif"));

        assert!(UrlMatcher::glob("").is_err());
        assert!(UrlMatcher::glob("**/{a,b").is_err());
    }

    #[test]
    fn regex_semantics_follow_regexp_test() {
        let re = UrlMatcher::regex(r"/v1/devices/\w+$", "").unwrap();
        assert!(re.is_match("https://h/v1/devices/abc"));
        assert!(!re.is_match("https://h/v1/devices/abc/x"));
        let ci = UrlMatcher::regex("DEVICES", "gi").unwrap();
        assert!(ci.is_match("https://h/v1/devices"));
        assert_eq!(ci.label(), "/DEVICES/gi");
        assert!(UrlMatcher::regex("(?=x)", "").is_err());
        assert!(UrlMatcher::regex("x", "z").is_err());
    }

    #[test]
    fn latest_matching_route_wins_and_method_filters() {
        let mut registry = Registry::default();
        let any = UrlMatcher::glob("**/devices/*").unwrap();
        registry
            .install("run", "app", spec(any.clone(), None, fulfill(200)), || true)
            .unwrap();
        registry
            .install("run", "app", spec(any, Some("PATCH"), fulfill(501)), || {
                true
            })
            .unwrap();

        assert_eq!(
            registry.decide("app", "patch", "https://h/devices/1", || true),
            Some(fulfill(501))
        );
        assert_eq!(
            registry.decide("app", "GET", "https://h/devices/1", || true),
            Some(fulfill(200))
        );
        assert_eq!(
            registry.decide("other", "GET", "https://h/devices/1", || true),
            None
        );
        assert_eq!(
            registry.decide("app", "GET", "https://h/users/1", || true),
            None
        );
    }

    #[test]
    fn times_expire_and_log_records_actions() {
        let mut registry = Registry::default();
        let matcher = UrlMatcher::glob("**/x").unwrap();
        let mut once = spec(matcher.clone(), None, RouteAction::Abort("failed".into()));
        once.times = Some(1);
        registry
            .install(
                "run",
                "app",
                spec(matcher, None, RouteAction::Continue),
                || true,
            )
            .unwrap();
        let aborting = registry.install("run", "app", once, || true).unwrap();

        assert_eq!(
            registry.decide("app", "GET", "http://h/x", || true),
            Some(RouteAction::Abort("failed".into()))
        );
        // The one-shot route is gone; the older continue route answers now.
        assert_eq!(
            registry.decide("app", "POST", "http://h/x", || true),
            Some(RouteAction::Continue)
        );
        assert!(!registry.remove("run", aborting));

        let log = registry.requests("run", "app");
        assert_eq!(log.len(), 2);
        assert_eq!((log[0].action, log[0].method.as_str()), ("abort", "GET"));
        assert_eq!(
            (log[1].action, log[1].method.as_str()),
            ("continue", "POST")
        );
        assert!(registry.requests("other-run", "app").is_empty());
    }

    #[test]
    fn disallowed_domains_are_not_fulfilled() {
        let mut registry = Registry::default();
        registry
            .install(
                "run",
                "app",
                spec(UrlMatcher::glob("**").unwrap(), None, fulfill(200)),
                || true,
            )
            .unwrap();
        assert_eq!(
            registry.decide("app", "GET", "https://blocked/x", || false),
            Some(RouteAction::Continue)
        );
        assert_eq!(registry.requests("run", "app")[0].status, None);
    }

    #[test]
    fn run_scoping_and_cleanup() {
        let mut registry = Registry::default();
        let matcher = UrlMatcher::glob("**").unwrap();
        assert!(
            registry
                .install(
                    "done",
                    "app",
                    spec(matcher.clone(), None, fulfill(200)),
                    || false
                )
                .is_err()
        );
        let a = registry
            .install(
                "a",
                "app",
                spec(matcher.clone(), None, fulfill(200)),
                || true,
            )
            .unwrap();
        registry
            .install(
                "b",
                "app",
                spec(matcher.clone(), None, fulfill(404)),
                || true,
            )
            .unwrap();
        registry
            .install("a", "other", spec(matcher, None, fulfill(500)), || true)
            .unwrap();
        // Another run cannot remove a route it does not own.
        assert!(!registry.remove("b", a));
        registry.decide("app", "GET", "http://h/", || true);

        registry.clear_run("b");
        assert_eq!(registry.len(), 2);
        assert!(registry.requests("b", "app").is_empty());
        assert_eq!(registry.remove_app("a", "app"), 1);
        registry.clear_run("a");
        assert_eq!(registry.len(), 0);
        assert_eq!(registry.decide("app", "GET", "http://h/", || true), None);
    }

    #[test]
    fn log_is_bounded() {
        let mut registry = Registry::default();
        registry
            .install(
                "run",
                "app",
                spec(UrlMatcher::glob("**").unwrap(), None, RouteAction::Continue),
                || true,
            )
            .unwrap();
        for i in 0..(MAX_LOG_ENTRIES + 5) {
            registry.decide("app", "GET", &format!("http://h/{i}"), || true);
        }
        let log = registry.requests("run", "app");
        assert_eq!(log.len(), MAX_LOG_ENTRIES);
        assert_eq!(log[0].url, "http://h/5");
    }
}
