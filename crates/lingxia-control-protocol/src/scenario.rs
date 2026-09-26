//! Scenario files: the format `lxdev scenario use` and `t.app.scenario()`
//! share, and the companion protocol that carries `function` rules.
//!
//! ```json
//! {
//!   "name": "Wi-Fi settings",
//!   "rules": [ { "http": "GET **/wifi/main", "json": { "ssid": "Home" } } ],
//!   "variants": {
//!     "offline": { "rules": [ { "http": "GET **/wifi/main", "status": 503 } ] }
//!   }
//! }
//! ```
//!
//! A rule targets an HTTP request (`"http": "METHOD url"`) or a Worker
//! Function call (`"function": "orders.submit"`), may narrow it with
//! `match`, and answers. This module checks the shape every consumer
//! agrees on and resolves a variant into one ordered rule list; the host
//! validates HTTP answers, the companion validates `function` rules against
//! its Function Definitions.

use serde_json::{Map, Value};

/// Keys a scenario file may have.
pub const FILE_KEYS: [&str; 5] = ["$schema", "name", "description", "rules", "variants"];
/// Keys a variant may have.
pub const VARIANT_KEYS: [&str; 2] = ["description", "rules"];
/// Keys every rule may have besides its answer.
pub const RULE_KEYS: [&str; 5] = ["http", "function", "match", "times", "note"];
/// Answer keys of a `function` rule.
pub const FUNCTION_ANSWER_KEYS: [&str; 4] = ["result", "error", "fault", "delay"];
/// `fault` values of a `function` rule.
pub const FUNCTION_FAULTS: [&str; 2] = ["notRun", "unknown"];
/// Rules one resolved scenario may hold.
pub const MAX_RULES: usize = 200;
/// Answers one `sequence` may list.
pub const MAX_SEQUENCE: usize = 100;
/// Longest `delay` an answer may ask for, in milliseconds.
pub const MAX_DELAY_MS: u64 = 30_000;

/// Owner of the scenario a dev session installed (`lxdev scenario use`).
pub const DEV_OWNER: &str = "dev";

/// Owner of the scenario a test run installed (`t.app.scenario()`).
pub fn test_owner(run_id: &str) -> String {
    format!("test:{run_id}")
}

/// A parsed scenario file: shared rules and variants, in file order.
#[derive(Debug, Clone, PartialEq)]
pub struct ScenarioFile {
    pub name: Option<String>,
    pub description: Option<String>,
    pub rules: Vec<Value>,
    pub variants: Vec<(String, Variant)>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Variant {
    pub description: Option<String>,
    pub rules: Vec<Value>,
}

/// What a rule answers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Target {
    /// `method` is upper case, `None` for `*`; `url` is a glob or `/regex/`.
    Http {
        method: Option<String>,
        url: String,
    },
    Function {
        name: String,
    },
}

impl Target {
    /// `GET **/wifi/main`, `* **/x`, `function orders.submit`.
    pub fn label(&self) -> String {
        match self {
            Self::Http { method, url } => format!("{} {url}", method.as_deref().unwrap_or("*")),
            Self::Function { name } => format!("function {name}"),
        }
    }

    pub fn kind(&self) -> &'static str {
        match self {
            Self::Http { .. } => "http",
            Self::Function { .. } => "function",
        }
    }
}

/// One rule of a resolved scenario.
#[derive(Debug, Clone, PartialEq)]
pub struct Rule {
    /// 1-based position in the resolved list: `rule 3`.
    pub index: usize,
    /// Where it is written: `rules[2]`, `variants.offline.rules[0]`.
    pub path: String,
    pub target: Target,
    /// The rule object as written.
    pub value: Value,
}

/// A scenario file with one variant applied: the variant's rules, then the
/// shared ones. File order is precedence.
#[derive(Debug, Clone, PartialEq)]
pub struct Resolved {
    pub name: Option<String>,
    pub description: Option<String>,
    pub variant: Option<String>,
    pub rules: Vec<Rule>,
}

impl Resolved {
    pub fn http_rules(&self) -> impl Iterator<Item = &Rule> {
        self.rules
            .iter()
            .filter(|rule| matches!(rule.target, Target::Http { .. }))
    }

    pub fn function_rules(&self) -> impl Iterator<Item = &Rule> {
        self.rules
            .iter()
            .filter(|rule| matches!(rule.target, Target::Function { .. }))
    }

    /// `name:variant` for messages: the file name when it has none.
    pub fn label(&self, fallback: &str) -> String {
        let name = self.name.as_deref().unwrap_or(fallback);
        match &self.variant {
            Some(variant) => format!("{name}:{variant}"),
            None => name.to_string(),
        }
    }
}

impl Resolved {
    /// `rule 2 function orders.submit, rule 4 function coupons.apply`.
    pub fn function_summary(&self) -> String {
        self.function_rules()
            .map(|rule| format!("rule {} {}", rule.index, rule.target.label()))
            .collect::<Vec<_>>()
            .join(", ")
    }

    /// The `function` rule at `position` of the list sent to the companion.
    pub fn function_rule(&self, position: usize) -> Option<&Rule> {
        self.function_rules().nth(position)
    }

    /// Why the `function` rules of this scenario cannot be installed.
    pub fn functions_unsupported(&self, reason: &str) -> String {
        let count = self.function_rules().count();
        format!(
            "{count} function rule{} ({}) cannot be installed: {reason}. Nothing was installed \
             (http rules are answered by the app host, function rules by the dev session's \
             companion)",
            if count == 1 { "" } else { "s" },
            self.function_summary()
        )
    }

    /// A companion's `scenario.use` error, its rule positions turned into
    /// this scenario's rule numbers: `rule 3 (variants.b.rules[0]) function
    /// orders.sbmit: unknown Function`.
    pub fn companion_error(&self, code: &str, message: &str, data: Option<&Value>) -> String {
        let errors = (code == companion::INVALID_RULES)
            .then(|| data.cloned())
            .flatten()
            .and_then(|data| serde_json::from_value::<companion::RuleErrors>(data).ok());
        let Some(errors) = errors.filter(|errors| !errors.errors.is_empty()) else {
            return format!("the companion refused the function rules: {message}");
        };
        errors
            .errors
            .iter()
            .map(|error| match self.function_rule(error.rule) {
                Some(rule) => format!(
                    "rule {} ({}) {}: {}",
                    rule.index,
                    rule.path,
                    rule.target.label(),
                    error.message
                ),
                None => format!("function rule {}: {}", error.rule, error.message),
            })
            .collect::<Vec<_>>()
            .join("; ")
    }

    /// The `scenario.use` params for `owner`.
    pub fn companion_use(&self, owner: &str, source: Option<&str>) -> companion::UseParams {
        companion::UseParams {
            owner: owner.to_string(),
            scenario: companion::ScenarioRef {
                name: self.name.clone(),
                variant: self.variant.clone(),
                source: source.map(str::to_string),
            },
            rules: self
                .function_rules()
                .map(|rule| rule.value.clone())
                .collect(),
        }
    }
}

impl ScenarioFile {
    pub fn variant_names(&self) -> impl Iterator<Item = &str> {
        self.variants.iter().map(|(name, _)| name.as_str())
    }

    /// Whether `use <name>` without a variant installs anything.
    pub fn usable_without_variant(&self) -> bool {
        !self.rules.is_empty()
    }

    /// The rule list `variant` (or none) resolves to.
    pub fn resolve(&self, variant: Option<&str>) -> Result<Resolved, String> {
        let (variant_rules, variant_name) = match variant {
            None => (Vec::new(), None),
            Some(name) => {
                let Some((_, found)) = self.variants.iter().find(|(key, _)| key == name) else {
                    return Err(match self.variants.is_empty() {
                        true => format!("this scenario has no variants, so there is no ':{name}'"),
                        false => format!(
                            "no variant '{name}' (variants: {})",
                            self.variant_names().collect::<Vec<_>>().join(", ")
                        ),
                    });
                };
                (
                    found
                        .rules
                        .iter()
                        .enumerate()
                        .map(|(i, rule)| (format!("variants.{name}.rules[{i}]"), rule))
                        .collect(),
                    Some(name.to_string()),
                )
            }
        };
        let shared = self
            .rules
            .iter()
            .enumerate()
            .map(|(i, rule)| (format!("rules[{i}]"), rule));
        let rules: Vec<(String, &Value)> = variant_rules.into_iter().chain(shared).collect();
        if rules.is_empty() {
            return Err(if self.variants.is_empty() {
                "a scenario needs at least one rule".to_string()
            } else {
                format!(
                    "this scenario has no shared rules; pick a variant: {}",
                    self.variant_names()
                        .map(|name| format!(":{name}"))
                        .collect::<Vec<_>>()
                        .join(", ")
                )
            });
        }
        if rules.len() > MAX_RULES {
            return Err(format!(
                "a scenario may hold at most {MAX_RULES} rules, got {}",
                rules.len()
            ));
        }
        let rules = rules
            .into_iter()
            .enumerate()
            .map(|(i, (path, value))| {
                // Checked by `parse_file`; only the target is read again.
                let target = rule_target(value).map_err(|err| format!("{path}: {err}"))?;
                Ok(Rule {
                    index: i + 1,
                    path,
                    target,
                    value: value.clone(),
                })
            })
            .collect::<Result<Vec<_>, String>>()?;
        Ok(Resolved {
            name: self.name.clone(),
            description: self.description.clone(),
            variant: variant_name,
            rules,
        })
    }
}

/// Split `wifi:b` into the scenario and its variant. A `:` inside a path
/// (`C:\x.json`) or followed by something that is not a variant name stays
/// part of the scenario.
pub fn split_variant(arg: &str) -> (&str, Option<&str>) {
    match arg.rsplit_once(':') {
        Some((name, variant))
            if !name.is_empty() && valid_variant_name(variant) && !name.ends_with(['/', '\\']) =>
        {
            (name, Some(variant))
        }
        _ => (arg, None),
    }
}

/// Letters, digits, `-`, `_`, `.`.
pub fn valid_variant_name(name: &str) -> bool {
    !name.is_empty()
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
}

/// Parse and check a scenario file. Errors name where they are:
/// `rules[2]: …`, `variants.offline.rules[0]: …`.
pub fn parse_file(value: &Value) -> Result<ScenarioFile, String> {
    let Value::Object(fields) = value else {
        return Err("a scenario must be a JSON object with a rules array".into());
    };
    for key in fields.keys() {
        if FILE_KEYS.contains(&key.as_str()) {
            continue;
        }
        return Err(match key.as_str() {
            "routes" | "http" | "worker" => format!(
                "'{key}' is the old scenario format; list every answer under \"rules\", \
                 e.g. {{ \"http\": \"GET **/path\", \"json\": {{…}} }}"
            ),
            _ => format!(
                "unknown scenario field '{key}' (allowed: {})",
                FILE_KEYS.join(", ")
            ),
        });
    }
    let name = optional_text(fields, "name", "scenario name")?;
    let description = optional_text(fields, "description", "scenario description")?;
    let rules = rule_list(fields.get("rules"), "rules")?;
    let variants = match fields.get("variants") {
        None | Some(Value::Null) => Vec::new(),
        Some(Value::Object(variants)) => variants
            .iter()
            .map(|(name, variant)| parse_variant(name, variant).map(|v| (name.clone(), v)))
            .collect::<Result<Vec<_>, String>>()?,
        Some(_) => return Err("variants must be an object of { rules } by variant name".into()),
    };
    if rules.is_empty() && variants.is_empty() {
        return Err("a scenario needs a non-empty rules array (or variants)".into());
    }
    Ok(ScenarioFile {
        name,
        description,
        rules,
        variants,
    })
}

fn parse_variant(name: &str, value: &Value) -> Result<Variant, String> {
    if !valid_variant_name(name) {
        return Err(format!(
            "variant name '{name}' may use only letters, digits, '-', '_' and '.'"
        ));
    }
    let Value::Object(fields) = value else {
        return Err(format!(
            "variants.{name} must be an object with a rules array"
        ));
    };
    if let Some(key) = fields
        .keys()
        .find(|key| !VARIANT_KEYS.contains(&key.as_str()))
    {
        return Err(format!(
            "unknown field '{key}' in variants.{name} (allowed: {})",
            VARIANT_KEYS.join(", ")
        ));
    }
    let description = optional_text(
        fields,
        "description",
        &format!("variants.{name}.description"),
    )?;
    let path = format!("variants.{name}.rules");
    let rules = rule_list(fields.get("rules"), &path)?;
    if rules.is_empty() {
        return Err(format!("{path} must not be empty"));
    }
    Ok(Variant { description, rules })
}

fn optional_text(
    fields: &Map<String, Value>,
    key: &str,
    what: &str,
) -> Result<Option<String>, String> {
    match fields.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(text)) => Ok(Some(text.clone())),
        Some(_) => Err(format!("{what} must be a string")),
    }
}

fn rule_list(value: Option<&Value>, path: &str) -> Result<Vec<Value>, String> {
    let rules = match value {
        None | Some(Value::Null) => return Ok(Vec::new()),
        Some(Value::Array(rules)) => rules,
        Some(_) => return Err(format!("{path} must be an array of rules")),
    };
    if rules.len() > MAX_RULES {
        return Err(format!(
            "{path} may list at most {MAX_RULES} rules, got {}",
            rules.len()
        ));
    }
    for (i, rule) in rules.iter().enumerate() {
        check_rule(rule).map_err(|err| format!("{path}[{i}]: {err}"))?;
    }
    Ok(rules.clone())
}

/// The target of a rule object.
pub fn rule_target(rule: &Value) -> Result<Target, String> {
    let Value::Object(fields) = rule else {
        return Err("a rule must be an object".into());
    };
    match (fields.get("http"), fields.get("function")) {
        (Some(_), Some(_)) => Err("a rule targets http or a function, not both".into()),
        (Some(Value::String(target)), None) => parse_http_target(target),
        (Some(_), None) => Err(
            "http must be a string \"METHOD url\", e.g. \"GET **/wifi/main\" or \"* **/wifi/*\""
                .into(),
        ),
        (None, Some(Value::String(name))) => parse_function_name(name),
        (None, Some(_)) => Err("function must be a Function name string".into()),
        (None, None) => Err(
            "a rule needs a target: \"http\": \"GET **/path\" or \"function\": \"orders.submit\""
                .into(),
        ),
    }
}

/// `GET **/wifi/main`: a method (`*` for any) and a URL glob or `/regex/`.
pub fn parse_http_target(target: &str) -> Result<Target, String> {
    let target = target.trim();
    let Some((method, url)) = target.split_once(char::is_whitespace) else {
        return Err(format!(
            "http '{target}' needs a method and a URL: \"GET {target}\", or \"* {target}\" for any method"
        ));
    };
    let url = url.trim();
    let method = method.to_ascii_uppercase();
    let method = if method == "*" {
        None
    } else if method.bytes().all(|b| b.is_ascii_alphabetic() || b == b'-') {
        Some(method)
    } else {
        return Err(format!(
            "http '{target}': '{method}' is not an HTTP method (use * for any)"
        ));
    };
    if url.is_empty() {
        return Err(format!("http '{target}' needs a URL glob after the method"));
    }
    Ok(Target::Http {
        method,
        url: url.to_string(),
    })
}

fn parse_function_name(name: &str) -> Result<Target, String> {
    if name.is_empty() || name.chars().any(char::is_whitespace) {
        return Err(format!(
            "function '{name}' must be a Function name without spaces"
        ));
    }
    if name.contains(['*', '?', '{', '}', '[', ']']) {
        return Err(format!(
            "function '{name}': name globs are not supported; name one Function per rule"
        ));
    }
    Ok(Target::Function {
        name: name.to_string(),
    })
}

/// Check the parts of a rule every consumer agrees on: its target, `match`,
/// `times`, `note`, and the shape of a `function` answer. HTTP answers are
/// checked by the host that serves them.
pub fn check_rule(rule: &Value) -> Result<(), String> {
    let target = rule_target(rule)?;
    let Value::Object(fields) = rule else {
        unreachable!("rule_target checked the object");
    };
    if let Some(matcher) = fields.get("match") {
        check_match(&target, matcher)?;
    }
    match fields.get("times") {
        None | Some(Value::Null) => {}
        Some(Value::Number(n))
            if n.as_u64()
                .is_some_and(|n| (1..=u64::from(u32::MAX)).contains(&n)) => {}
        Some(other) => return Err(format!("times must be a positive integer, got {other}")),
    }
    if !matches!(
        fields.get("note"),
        None | Some(Value::Null) | Some(Value::String(_))
    ) {
        return Err("note must be a string".into());
    }
    let answer: Map<String, Value> = fields
        .iter()
        .filter(|(key, _)| !RULE_KEYS.contains(&key.as_str()))
        .map(|(key, value)| (key.clone(), value.clone()))
        .collect();
    match target {
        Target::Http { .. } => {
            if answer.is_empty() {
                return Err(
                    "an http rule needs an answer (status/json/body, abort, continue, hang, sse) \
                     or a sequence"
                        .into(),
                );
            }
            if let Some(key) = FUNCTION_ANSWER_KEYS
                .iter()
                .filter(|key| **key != "delay")
                .find(|key| answer.contains_key(**key))
            {
                return Err(format!(
                    "'{key}' answers a function rule; an http rule answers with status/json/body"
                ));
            }
            Ok(())
        }
        Target::Function { .. } => check_function_answers(&answer),
    }
}

fn check_match(target: &Target, matcher: &Value) -> Result<(), String> {
    let Value::Object(fields) = matcher else {
        return Err(match target {
            Target::Http { .. } => "match must be an object: { \"json\": { … } }".into(),
            Target::Function { .. } => "match must be an object: { \"args\": { … } }".into(),
        });
    };
    let allowed = match target {
        Target::Http { .. } => "json",
        Target::Function { .. } => "args",
    };
    for key in fields.keys() {
        if key == allowed {
            continue;
        }
        return Err(match (key.as_str(), target) {
            ("args", Target::Http { .. }) => {
                "match.args is for function rules; an http rule matches its JSON body with match.json"
                    .into()
            }
            ("json", Target::Function { .. }) => {
                "match.json is for http rules; a function rule matches its arguments with match.args"
                    .into()
            }
            ("query", _) => "match.query is not supported; put the query in the URL glob".into(),
            ("headers", _) => "match.headers is not supported".into(),
            _ => format!("unknown match field '{key}' (allowed: {allowed})"),
        });
    }
    if fields.is_empty() {
        return Err(format!("match needs {allowed}"));
    }
    Ok(())
}

fn check_function_answers(answer: &Map<String, Value>) -> Result<(), String> {
    let Some(sequence) = answer.get("sequence") else {
        return check_function_answer(answer);
    };
    if let Some(other) = answer.keys().find(|key| *key != "sequence") {
        return Err(format!(
            "a sequence lists whole answers; move '{other}' into its items"
        ));
    }
    let Value::Array(items) = sequence else {
        return Err("sequence must be an array of answers".into());
    };
    if items.is_empty() || items.len() > MAX_SEQUENCE {
        return Err(format!(
            "sequence must list 1..={MAX_SEQUENCE} answers, got {}",
            items.len()
        ));
    }
    for (i, item) in items.iter().enumerate() {
        let Value::Object(fields) = item else {
            return Err(format!("sequence[{i}]: an answer must be an object"));
        };
        if fields.contains_key("sequence") {
            return Err(format!("sequence[{i}]: sequences do not nest"));
        }
        check_function_answer(fields).map_err(|err| format!("sequence[{i}]: {err}"))?;
    }
    Ok(())
}

fn check_function_answer(answer: &Map<String, Value>) -> Result<(), String> {
    if let Some(key) = answer
        .keys()
        .find(|key| !FUNCTION_ANSWER_KEYS.contains(&key.as_str()))
    {
        return Err(match key.as_str() {
            "status" | "json" | "body" | "headers" | "abort" | "hang" | "sse" => format!(
                "'{key}' answers an http rule; a function rule answers with result, error or fault"
            ),
            _ => format!(
                "unknown function answer field '{key}' (allowed: {})",
                FUNCTION_ANSWER_KEYS.join(", ")
            ),
        });
    }
    let answers: Vec<&str> = ["result", "error", "fault"]
        .into_iter()
        .filter(|key| answer.contains_key(*key))
        .collect();
    match answers.as_slice() {
        [] => return Err("a function rule answers with one of result, error or fault".into()),
        [_] => {}
        many => {
            return Err(format!(
                "a function answer takes one of {}",
                many.join(", ")
            ));
        }
    }
    if let Some(error) = answer.get("error") {
        match error {
            Value::Object(fields) if matches!(fields.get("code"), Some(Value::String(code)) if !code.is_empty()) =>
                {}
            _ => {
                return Err(
                    "error must be an object with a string code: { \"code\": \"COUPON_EXPIRED\" }"
                        .into(),
                );
            }
        }
    }
    if let Some(fault) = answer.get("fault")
        && !fault
            .as_str()
            .is_some_and(|fault| FUNCTION_FAULTS.contains(&fault))
    {
        return Err(format!(
            "fault must be one of {}, got {fault}",
            FUNCTION_FAULTS
                .map(|fault| format!("\"{fault}\""))
                .join(", ")
        ));
    }
    match answer.get("delay") {
        None | Some(Value::Null) => Ok(()),
        Some(Value::Number(n)) if n.as_u64().is_some_and(|ms| ms <= MAX_DELAY_MS) => Ok(()),
        Some(other) => Err(format!(
            "delay must be milliseconds between 0 and {MAX_DELAY_MS}, got {other}"
        )),
    }
}

// ------------------------------- matching -------------------------------

/// `"/source/flags"` with flags among `imsu`: a regex. Anything else is a
/// literal string (`"/devices/1"` included).
pub fn regex_literal(text: &str) -> Option<(&str, &str)> {
    let rest = text.strip_prefix('/')?;
    let (source, flags) = rest.rsplit_once('/')?;
    (!source.is_empty() && flags.chars().all(|c| matches!(c, 'i' | 'm' | 's' | 'u')))
        .then_some((source, flags))
}

#[cfg(feature = "scenario-match")]
pub use matcher::{Matcher, Mismatch};

#[cfg(feature = "scenario-match")]
mod matcher {
    use super::regex_literal;
    use serde_json::Value;

    /// A compiled `match` value. Objects match when every listed key is
    /// present and matches (other keys are ignored); arrays match element by
    /// element with the same length; a `"/regex/"` string matches a scalar
    /// whose text it finds; any other scalar matches an equal value.
    #[derive(Debug, Clone)]
    pub struct Matcher(Node);

    #[derive(Debug, Clone)]
    enum Node {
        Object(Vec<(String, Node)>),
        Array(Vec<Node>),
        Regex(regex::Regex, String),
        Literal(Value),
    }

    /// Why a value did not match: `path` is `match.json.items[0].id`.
    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct Mismatch {
        pub path: String,
        pub reason: String,
    }

    impl std::fmt::Display for Mismatch {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(f, "{}: {}", self.path, self.reason)
        }
    }

    impl Matcher {
        /// Errors name the value's path under `root` (`match.json.a`).
        pub fn compile(value: &Value, root: &str) -> Result<Self, String> {
            compile(value, root).map(Self)
        }

        /// `Ok` when `actual` matches; otherwise the first difference, with
        /// its path under `root` (`match.json`).
        pub fn check(&self, actual: &Value, root: &str) -> Result<(), Mismatch> {
            check(&self.0, actual, root)
        }
    }

    fn compile(value: &Value, path: &str) -> Result<Node, String> {
        Ok(match value {
            Value::Object(fields) => {
                // Sorted, so the difference reported first does not depend on
                // how the JSON map orders keys.
                let mut keys: Vec<&String> = fields.keys().collect();
                keys.sort();
                Node::Object(
                    keys.into_iter()
                        .map(|key| {
                            Ok((
                                key.clone(),
                                compile(&fields[key], &format!("{path}.{key}"))?,
                            ))
                        })
                        .collect::<Result<_, String>>()?,
                )
            }
            Value::Array(items) => Node::Array(
                items
                    .iter()
                    .enumerate()
                    .map(|(i, item)| compile(item, &format!("{path}[{i}]")))
                    .collect::<Result<_, _>>()?,
            ),
            Value::String(text) => match regex_literal(text) {
                Some((source, flags)) => {
                    let inline: String = flags.chars().filter(|c| *c != 'u').collect();
                    let pattern = if inline.is_empty() {
                        source.to_string()
                    } else {
                        format!("(?{inline}){source}")
                    };
                    let regex = regex::Regex::new(&pattern)
                        .map_err(|err| format!("{path}: {text} is not a supported regex: {err}"))?;
                    Node::Regex(regex, text.clone())
                }
                None => Node::Literal(value.clone()),
            },
            other => Node::Literal(other.clone()),
        })
    }

    fn short(value: &Value) -> String {
        let text = value.to_string();
        if text.chars().count() <= 60 {
            return text;
        }
        let cut: String = text.chars().take(57).collect();
        format!("{cut}...")
    }

    fn kind(value: &Value) -> &'static str {
        match value {
            Value::Null => "null",
            Value::Bool(_) => "a boolean",
            Value::Number(_) => "a number",
            Value::String(_) => "a string",
            Value::Array(_) => "an array",
            Value::Object(_) => "an object",
        }
    }

    fn check(node: &Node, actual: &Value, path: &str) -> Result<(), Mismatch> {
        let fail = |reason: String| {
            Err(Mismatch {
                path: path.to_string(),
                reason,
            })
        };
        match node {
            Node::Object(fields) => {
                let Value::Object(actual) = actual else {
                    return fail(format!("expected an object, got {}", kind(actual)));
                };
                for (key, node) in fields {
                    let at = format!("{path}.{key}");
                    match actual.get(key) {
                        Some(value) => check(node, value, &at)?,
                        None => {
                            return Err(Mismatch {
                                path: at,
                                reason: "missing".into(),
                            });
                        }
                    }
                }
                Ok(())
            }
            Node::Array(items) => {
                let Value::Array(actual) = actual else {
                    return fail(format!("expected an array, got {}", kind(actual)));
                };
                if actual.len() != items.len() {
                    return fail(format!(
                        "expected an array of {}, got {}",
                        items.len(),
                        actual.len()
                    ));
                }
                for (i, (node, value)) in items.iter().zip(actual).enumerate() {
                    check(node, value, &format!("{path}[{i}]"))?;
                }
                Ok(())
            }
            Node::Regex(regex, label) => {
                let text = match actual {
                    Value::String(text) => text.clone(),
                    Value::Number(_) | Value::Bool(_) => actual.to_string(),
                    other => {
                        return fail(format!(
                            "expected text matching {label}, got {}",
                            kind(other)
                        ));
                    }
                };
                if regex.is_match(&text) {
                    Ok(())
                } else {
                    fail(format!("{} does not match {label}", short(actual)))
                }
            }
            Node::Literal(expected) => {
                let equal = match (expected, actual) {
                    (Value::Number(a), Value::Number(b)) => a.as_f64() == b.as_f64(),
                    _ => expected == actual,
                };
                if equal {
                    Ok(())
                } else {
                    fail(format!(
                        "expected {}, got {}",
                        short(expected),
                        short(actual)
                    ))
                }
            }
        }
    }
}

// ------------------------------- companion -------------------------------

/// The companion protocol for `function` rules: the dev server forwards
/// these to a companion that declared [`crate::dev_session::capabilities::SCENARIO_FUNCTION`].
/// See `docs/internal/scenario-companion-protocol.md`.
pub mod companion {
    use serde::{Deserialize, Serialize};
    use serde_json::Value;

    /// Install `rules` for `owner`, replacing what that owner had. All or
    /// nothing: on an error no rule of the call stays installed and the
    /// owner's previous rules keep answering.
    pub const USE: &str = "scenario.use";
    /// Remove `owner`'s rules; `{ cleared }`.
    pub const CLEAR: &str = "scenario.clear";
    /// Every owner's rules and hit counts.
    pub const STATUS: &str = "scenario.status";
    /// Function calls since a time.
    pub const CALLS: &str = "scenario.calls";

    /// Error code of a `scenario.use` that rejected rules; `data` is
    /// [`RuleErrors`].
    pub const INVALID_RULES: &str = "invalid_rules";

    #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
    pub struct ScenarioRef {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub name: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub variant: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub source: Option<String>,
    }

    /// `scenario.use` params.
    #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
    pub struct UseParams {
        /// `dev`, or `test:<run id>`; a test owner sits above `dev` until
        /// it is cleared.
        pub owner: String,
        pub scenario: ScenarioRef,
        /// The `function` rules as written, in precedence order.
        pub rules: Vec<Value>,
    }

    /// `scenario.use` result.
    #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
    pub struct UseResult {
        pub installed: usize,
    }

    /// `scenario.clear` params.
    #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
    pub struct ClearParams {
        pub owner: String,
    }

    /// `scenario.clear` result.
    #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
    pub struct ClearResult {
        pub cleared: bool,
    }

    /// `scenario.status` result.
    #[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
    pub struct StatusResult {
        #[serde(default)]
        pub owners: Vec<OwnerStatus>,
    }

    #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
    pub struct OwnerStatus {
        pub owner: String,
        /// Whether its rules answer now (`dev` stands aside under a test
        /// owner).
        pub active: bool,
        /// Per rule, in `scenario.use` order.
        pub rules: Vec<RuleStatus>,
    }

    #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
    pub struct RuleStatus {
        pub hits: u64,
    }

    /// `scenario.calls` params.
    #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
    pub struct CallsParams {
        /// Epoch milliseconds; calls that started at or after it.
        pub since: u64,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub owner: Option<String>,
    }

    /// `scenario.calls` result.
    #[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
    pub struct CallsResult {
        #[serde(default)]
        pub calls: Vec<FunctionCall>,
    }

    /// One Function call the companion saw.
    #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
    pub struct FunctionCall {
        /// Epoch milliseconds.
        pub time: u64,
        pub function: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub args: Option<Value>,
        /// The owner whose rule answered; `None` when the default handler
        /// did.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub owner: Option<String>,
        /// 0-based position of the answering rule in its owner's
        /// `scenario.use` list.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub rule: Option<usize>,
        /// `result`, `error`, `fault` or `default`.
        pub outcome: String,
        /// Why rules for this Function did not match, when none did.
        #[serde(default, skip_serializing_if = "Option::is_none", rename = "noMatch")]
        pub no_match: Option<String>,
    }

    /// `data` of an [`INVALID_RULES`] error.
    #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
    pub struct RuleErrors {
        pub errors: Vec<RuleError>,
    }

    #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
    pub struct RuleError {
        /// 0-based position in the `scenario.use` list.
        pub rule: usize,
        pub message: String,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn file(value: Value) -> ScenarioFile {
        parse_file(&value).unwrap()
    }

    fn err(value: Value) -> String {
        parse_file(&value).unwrap_err()
    }

    #[test]
    fn rules_parse_with_targets_and_answers() {
        let parsed = file(json!({
            "$schema": "x",
            "name": "Checkout",
            "description": "d",
            "rules": [
                { "http": "GET **/wifi/main", "json": { "ssid": "Home" } },
                { "http": "* /^https:\\/\\/h\\/x$/", "status": 503 },
                { "http": "patch **/devices/*", "match": { "json": { "name": "Office" } }, "status": 409, "times": 2, "note": "n" },
                { "function": "coupons.apply", "match": { "args": { "code": "SPRING" } }, "error": { "code": "COUPON_EXPIRED" } },
                { "function": "orders.submit", "fault": "unknown", "delay": 100 },
                { "function": "orders.status", "sequence": [ { "result": { "state": "pending" } }, { "result": null } ] }
            ]
        }));
        let resolved = parsed.resolve(None).unwrap();
        let labels: Vec<String> = resolved
            .rules
            .iter()
            .map(|rule| rule.target.label())
            .collect();
        assert_eq!(
            labels,
            [
                "GET **/wifi/main",
                "* /^https:\\/\\/h\\/x$/",
                "PATCH **/devices/*",
                "function coupons.apply",
                "function orders.submit",
                "function orders.status"
            ]
        );
        assert_eq!(resolved.http_rules().count(), 3);
        assert_eq!(
            resolved
                .function_rules()
                .map(|rule| rule.index)
                .collect::<Vec<_>>(),
            [4, 5, 6]
        );
        assert_eq!(resolved.rules[2].path, "rules[2]");
        assert_eq!(resolved.label("file"), "Checkout");
    }

    #[test]
    fn unknown_fields_and_old_forms_are_errors() {
        for (value, expected) in [
            (json!([]), "JSON object"),
            (
                json!({ "routes": [] }),
                "'routes' is the old scenario format",
            ),
            (
                json!({ "http": { "routes": [] } }),
                "'http' is the old scenario format",
            ),
            (
                json!({ "worker": {} }),
                "'worker' is the old scenario format",
            ),
            (json!({ "rule": [] }), "unknown scenario field 'rule'"),
            (json!({ "name": 1, "rules": [] }), "name must be a string"),
            (json!({ "rules": [] }), "non-empty rules array"),
            (json!({ "rules": {} }), "rules must be an array"),
            (
                json!({ "rules": [{ "http": "GET **", "stauts": 200, "status": 200 }], "variants": [] }),
                "variants must be an object",
            ),
            (
                json!({ "variants": { "a": { "rules": [], } } }),
                "variants.a.rules must not be empty",
            ),
            (
                json!({ "variants": { "a": { "rules": [{ "http": "GET x", "status": 1 }], "x": 1 } } }),
                "unknown field 'x' in variants.a",
            ),
            (
                json!({ "variants": { "a b": { "rules": [] } } }),
                "variant name 'a b'",
            ),
            (
                json!({ "variants": { "a": { "rules": [{ "status": 200 }] } } }),
                "variants.a.rules[0]: a rule needs a target",
            ),
        ] {
            let message = err(value.clone());
            assert!(message.contains(expected), "{value}: {message}");
        }
    }

    #[test]
    fn rule_shapes_are_checked() {
        for (rule, expected) in [
            (json!("x"), "a rule must be an object"),
            (
                json!({ "http": "GET x", "function": "f", "status": 1 }),
                "not both",
            ),
            (
                json!({ "http": "**/wifi", "status": 1 }),
                "needs a method and a URL: \"GET **/wifi\"",
            ),
            (
                json!({ "http": "GE7 x", "status": 1 }),
                "not an HTTP method",
            ),
            (json!({ "http": ["GET", "x"] }), "http must be a string"),
            (json!({ "http": "GET x" }), "an http rule needs an answer"),
            (
                json!({ "http": "GET x", "result": 1 }),
                "'result' answers a function rule",
            ),
            (
                json!({ "http": "GET x", "status": 1, "match": { "args": {} } }),
                "match.args is for function rules",
            ),
            (
                json!({ "http": "GET x", "status": 1, "match": { "query": {} } }),
                "match.query is not supported",
            ),
            (
                json!({ "http": "GET x", "status": 1, "match": { "headers": {} } }),
                "match.headers is not supported",
            ),
            (
                json!({ "http": "GET x", "status": 1, "match": {} }),
                "match needs json",
            ),
            (
                json!({ "http": "GET x", "status": 1, "match": [] }),
                "match must be an object",
            ),
            (
                json!({ "http": "GET x", "status": 1, "times": 0 }),
                "times must be a positive integer",
            ),
            (
                json!({ "http": "GET x", "status": 1, "note": 1 }),
                "note must be a string",
            ),
            (
                json!({ "function": "orders.*", "result": 1 }),
                "name globs are not supported",
            ),
            (json!({ "function": "a b", "result": 1 }), "without spaces"),
            (json!({ "function": "f" }), "one of result, error or fault"),
            (
                json!({ "function": "f", "result": 1, "error": { "code": "X" } }),
                "takes one of result, error",
            ),
            (
                json!({ "function": "f", "status": 200 }),
                "'status' answers an http rule",
            ),
            (
                json!({ "function": "f", "reslt": 1 }),
                "unknown function answer field 'reslt'",
            ),
            (
                json!({ "function": "f", "error": "X" }),
                "error must be an object with a string code",
            ),
            (
                json!({ "function": "f", "fault": "not_run" }),
                "fault must be one of \"notRun\", \"unknown\"",
            ),
            (
                json!({ "function": "f", "result": 1, "delay": 40000 }),
                "delay must be milliseconds",
            ),
            (
                json!({ "function": "f", "match": { "json": {} }, "result": 1 }),
                "match.json is for http rules",
            ),
            (
                json!({ "function": "f", "sequence": [] }),
                "sequence must list",
            ),
            (
                json!({ "function": "f", "sequence": [{ "result": 1 }], "delay": 1 }),
                "move 'delay' into its items",
            ),
            (
                json!({ "function": "f", "sequence": [{ "sequence": [] }] }),
                "sequence[0]: sequences do not nest",
            ),
            (
                json!({ "function": "f", "sequence": [{ "result": 1 }, { "fault": "x" }] }),
                "sequence[1]: fault must be",
            ),
        ] {
            let message = check_rule(&rule).unwrap_err();
            assert!(message.contains(expected), "{rule}: {message}");
        }
        check_rule(&json!({ "function": "f", "fault": "notRun" })).unwrap();
        check_rule(&json!({ "http": "* **", "continue": true, "delay": 5 })).unwrap();
    }

    #[test]
    fn variants_resolve_before_shared_rules() {
        let parsed = file(json!({
            "name": "Wi-Fi",
            "rules": [ { "http": "GET **/wifi/other", "json": {} } ],
            "variants": {
                "a": { "rules": [ { "http": "GET **/wifi/main", "json": { "ssid": "A" } } ] },
                "b": { "description": "B side", "rules": [
                    { "http": "GET **/wifi/main", "json": { "ssid": "B" } },
                    { "function": "f", "result": 1 }
                ] }
            }
        }));
        assert_eq!(parsed.variant_names().collect::<Vec<_>>(), ["a", "b"]);
        let b = parsed.resolve(Some("b")).unwrap();
        assert_eq!(b.variant.as_deref(), Some("b"));
        assert_eq!(b.label("wifi"), "Wi-Fi:b");
        let paths: Vec<&str> = b.rules.iter().map(|rule| rule.path.as_str()).collect();
        assert_eq!(
            paths,
            ["variants.b.rules[0]", "variants.b.rules[1]", "rules[0]"]
        );
        assert_eq!(
            b.rules.iter().map(|rule| rule.index).collect::<Vec<_>>(),
            [1, 2, 3]
        );
        assert_eq!(b.rules[0].value["json"]["ssid"], "B");

        let shared = parsed.resolve(None).unwrap();
        assert_eq!(shared.rules.len(), 1);
        assert_eq!(shared.variant, None);
        assert!(
            parsed
                .resolve(Some("c"))
                .unwrap_err()
                .contains("no variant 'c' (variants: a, b)")
        );

        let only_variants =
            file(json!({ "variants": { "x": { "rules": [{ "http": "GET y", "status": 1 }] } } }));
        assert!(!only_variants.usable_without_variant());
        assert!(
            only_variants
                .resolve(None)
                .unwrap_err()
                .contains("pick a variant: :x")
        );
        let plain = file(json!({ "rules": [{ "http": "GET y", "status": 1 }] }));
        assert!(
            plain
                .resolve(Some("x"))
                .unwrap_err()
                .contains("has no variants")
        );
    }

    #[test]
    fn a_variant_splits_off_the_name() {
        assert_eq!(split_variant("wifi:b"), ("wifi", Some("b")));
        assert_eq!(
            split_variant("net/wifi:off-line.2"),
            ("net/wifi", Some("off-line.2"))
        );
        assert_eq!(split_variant("wifi"), ("wifi", None));
        assert_eq!(
            split_variant("C:\\s\\wifi.json"),
            ("C:\\s\\wifi.json", None)
        );
        assert_eq!(split_variant(":b"), (":b", None));
        assert_eq!(split_variant("wifi:"), ("wifi:", None));
    }

    #[test]
    fn regex_strings_need_known_flags() {
        assert_eq!(regex_literal("/^a$/i"), Some(("^a$", "i")));
        assert_eq!(regex_literal("/devices/1"), None);
        assert_eq!(regex_literal("/devices/abc"), None);
        assert_eq!(regex_literal("/x/"), Some(("x", "")));
        assert_eq!(regex_literal("plain"), None);
        assert_eq!(regex_literal("//"), None);
    }

    #[cfg(feature = "scenario-match")]
    #[test]
    fn match_is_a_deep_subset_with_exact_arrays_and_regex_scalars() {
        let matcher = Matcher::compile(
            &json!({
                "name": "Office",
                "tags": ["a", "/^b/"],
                "owner": { "id": 7 },
                "code": "/^SP/i",
                "count": 2
            }),
            "match.json",
        )
        .unwrap();
        let ok = json!({
            "name": "Office", "extra": true, "tags": ["a", "bee"], "owner": { "id": 7.0, "x": 1 },
            "code": "spring", "count": 2
        });
        matcher.check(&ok, "match.json").unwrap();

        // One difference at a time, on top of a matching body.
        let cases = [
            (
                "name",
                json!("Den"),
                "match.json.name: expected \"Office\", got \"Den\"",
            ),
            ("tags", Value::Null, "match.json.tags: missing"),
            (
                "tags",
                json!(["a"]),
                "match.json.tags: expected an array of 2, got 1",
            ),
            (
                "tags",
                json!(["a", "c"]),
                "match.json.tags[1]: \"c\" does not match /^b/",
            ),
            (
                "owner",
                json!(1),
                "match.json.owner: expected an object, got a number",
            ),
            (
                "code",
                json!(null),
                "match.json.code: expected text matching /^SP/i, got null",
            ),
            ("count", json!(3), "match.json.count: expected 2, got 3"),
        ];
        for (key, value, expected) in cases {
            let mut actual = ok.clone();
            if value.is_null() && key == "tags" {
                actual.as_object_mut().unwrap().remove(key);
            } else {
                actual[key] = value;
            }
            let mismatch = matcher.check(&actual, "match.json").unwrap_err();
            assert_eq!(mismatch.to_string(), expected, "{actual}");
        }
        // A number is matched by its text.
        Matcher::compile(&json!("/^4\\d$/"), "m")
            .unwrap()
            .check(&json!(42), "m")
            .unwrap();
        // Arrays are exact, including order.
        let array = Matcher::compile(&json!([1, 2]), "m").unwrap();
        assert!(array.check(&json!([2, 1]), "m").is_err());
        assert!(array.check(&json!([1, 2, 3]), "m").is_err());
        let err = Matcher::compile(&json!({ "a": "/(/" }), "match.args").unwrap_err();
        assert!(
            err.contains("match.args.a: /(/ is not a supported regex"),
            "{err}"
        );
    }

    #[test]
    fn companion_errors_name_the_rule_in_the_file() {
        let resolved = file(json!({
            "rules": [{ "http": "GET x", "status": 1 }, { "function": "orders.status", "result": 1 }],
            "variants": { "b": { "rules": [{ "function": "orders.sbmit", "fault": "unknown" }] } }
        }))
        .resolve(Some("b"))
        .unwrap();
        assert_eq!(
            resolved.function_summary(),
            "rule 1 function orders.sbmit, rule 3 function orders.status"
        );
        let params = resolved.companion_use(DEV_OWNER, Some("checkout"));
        assert_eq!(params.rules.len(), 2);
        assert_eq!(params.scenario.variant.as_deref(), Some("b"));
        let data = json!({ "errors": [
            { "rule": 0, "message": "unknown Function 'orders.sbmit'" },
            { "rule": 1, "message": "result does not match OrderStatus" }
        ] });
        assert_eq!(
            resolved.companion_error(companion::INVALID_RULES, "invalid", Some(&data)),
            "rule 1 (variants.b.rules[0]) function orders.sbmit: unknown Function 'orders.sbmit'; \
             rule 3 (rules[1]) function orders.status: result does not match OrderStatus"
        );
        assert_eq!(
            resolved.companion_error("internal", "boom", None),
            "the companion refused the function rules: boom"
        );
        let unsupported = resolved.functions_unsupported("this dev session has no companion");
        assert!(
            unsupported.starts_with("2 function rules (rule 1 function orders.sbmit, rule 3"),
            "{unsupported}"
        );
        assert!(
            unsupported.contains("Nothing was installed"),
            "{unsupported}"
        );
    }

    #[test]
    fn companion_messages_have_stable_shapes() {
        let params = companion::UseParams {
            owner: test_owner("run-1"),
            scenario: companion::ScenarioRef {
                name: Some("Checkout".into()),
                variant: Some("expired".into()),
                source: None,
            },
            rules: vec![json!({ "function": "orders.submit", "fault": "unknown" })],
        };
        assert_eq!(
            serde_json::to_value(&params).unwrap(),
            json!({
                "owner": "test:run-1",
                "scenario": { "name": "Checkout", "variant": "expired" },
                "rules": [{ "function": "orders.submit", "fault": "unknown" }]
            })
        );
        let status: companion::StatusResult = serde_json::from_value(json!({
            "owners": [{ "owner": "dev", "active": false, "rules": [{ "hits": 2 }] }]
        }))
        .unwrap();
        assert_eq!(status.owners[0].rules[0].hits, 2);
        let calls: companion::CallsResult = serde_json::from_value(json!({
            "calls": [
                { "time": 1, "function": "orders.submit", "owner": "dev", "rule": 0, "outcome": "fault" },
                { "time": 2, "function": "orders.status", "outcome": "default", "noMatch": "rule 0 match.args.id: missing" }
            ]
        }))
        .unwrap();
        assert_eq!(
            calls.calls[1].no_match.as_deref(),
            Some("rule 0 match.args.id: missing")
        );
        assert_eq!(
            serde_json::to_value(&calls.calls[0]).unwrap(),
            json!({ "time": 1, "function": "orders.submit", "owner": "dev", "rule": 0, "outcome": "fault" })
        );
        let errors = companion::RuleErrors {
            errors: vec![companion::RuleError {
                rule: 1,
                message: "unknown function".into(),
            }],
        };
        assert_eq!(
            serde_json::to_value(errors).unwrap(),
            json!({ "errors": [{ "rule": 1, "message": "unknown function" }] })
        );
    }
}
