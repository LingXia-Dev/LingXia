//! What `lxdev test` keeps out of the files and terminal output a run leaves
//! behind.
//!
//! `@lingxia/test` masks at the source: declared `--secret-arg` values in
//! events, reports and attachments, and credential-named `--arg` keys in
//! `meta.args`. That name heuristic lives only there; lxdev reads its verdict
//! from the masked `args` of the `run_started` event instead of guessing again.
//!
//! lxdev itself only scrubs the declared secret values, the one thing it knows
//! for certain, from everything it receives before it is written or printed.
//! That still covers what the framework never sees (console output) and an
//! older framework that masks nothing. The scrub works on data, never on
//! rendered markup: JSON is parsed and re-serialized, plain text is replaced,
//! and an HTML or XML artifact that carries a secret is withheld — lxdev then
//! renders its own report pages from the scrubbed JSON.

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD as BASE64;
use lingxia_control_protocol::dev_session::session_test::{TestEvent, TestEventPayload};
use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::Value;
use std::cell::RefCell;
use std::collections::HashMap;

pub const REDACTED: &str = "***";
/// Reserved control key listing the `--secret-arg` keys for the framework.
pub const SECRET_ARGS_KEY: &str = "secretArgs";
/// Shorter values are too likely to collide with ordinary report text.
const MIN_SCRUB_LENGTH: usize = 4;

pub struct RunSecrets {
    /// `--arg`, in command-line order.
    args: Vec<(String, String)>,
    /// `--secret-arg`, in command-line order.
    secret_args: Vec<(String, String)>,
    /// Declared secret values, longest first so a secret containing another
    /// is masked whole.
    values: Vec<String>,
    /// The framework's masked view of the args, from `run_started`.
    masked: RefCell<Option<HashMap<String, String>>>,
}

pub enum ArtifactScrub {
    Keep,
    Replace(String),
    /// A markup artifact carries a secret and cannot be rewritten safely.
    Withhold,
}

impl RunSecrets {
    pub fn new(args: &[(String, String)], secret_args: &[(String, String)]) -> Self {
        let mut values = secret_args
            .iter()
            .map(|(_, value)| value.clone())
            .filter(|value| value.len() >= MIN_SCRUB_LENGTH)
            .collect::<Vec<_>>();
        values.sort_by_key(|value| std::cmp::Reverse(value.len()));
        values.dedup();
        Self {
            args: args.to_vec(),
            secret_args: secret_args.to_vec(),
            values,
            masked: RefCell::new(None),
        }
    }

    /// `t.args`: `--arg` values, then `--secret-arg` values over them.
    pub fn spec_args(&self) -> HashMap<String, String> {
        self.args.iter().chain(&self.secret_args).cloned().collect()
    }

    /// The JSON list of declared secret keys, for the framework.
    pub fn secret_keys_json(&self) -> Option<String> {
        if self.secret_args.is_empty() {
            return None;
        }
        let mut keys = self
            .secret_args
            .iter()
            .map(|(key, _)| key.as_str())
            .collect::<Vec<_>>();
        keys.sort();
        keys.dedup();
        serde_json::to_string(&keys).ok()
    }

    pub fn note_run_started(&self, args: Option<&HashMap<String, String>>) {
        if let Some(args) = args {
            *self.masked.borrow_mut() = Some(args.clone());
        }
    }

    fn is_declared(&self, key: &str) -> bool {
        self.secret_args.iter().any(|(secret, _)| secret == key)
    }

    /// Whether reports may show `key`'s value.
    fn shows(&self, key: &str) -> bool {
        if self.is_declared(key) {
            return false;
        }
        // Without the framework's verdict there is no telling which names it
        // would have hidden, so hide them all.
        self.masked
            .borrow()
            .as_ref()
            .and_then(|masked| masked.get(key))
            .is_some_and(|value| value != REDACTED)
    }

    /// `meta.args` for a report lxdev writes itself.
    pub fn meta_args(&self) -> HashMap<String, String> {
        self.spec_args()
            .into_iter()
            .map(|(key, value)| {
                let value = if self.shows(&key) {
                    value
                } else {
                    REDACTED.to_string()
                };
                (key, value)
            })
            .collect()
    }

    /// `--arg`/`--secret-arg` flags for a rerun hint. A value reports would
    /// hide is printed as `<key>`: plainly not the value, and a prompt to
    /// supply it.
    pub fn rerun_flags(&self, quote: impl Fn(&str) -> String) -> String {
        let mut out = String::new();
        for (key, value) in &self.args {
            if self.is_declared(key) {
                continue;
            }
            let value = if self.shows(key) {
                value.clone()
            } else {
                format!("<{key}>")
            };
            out.push_str(&format!(" --arg {}", quote(&format!("{key}={value}"))));
        }
        for (key, _) in &self.secret_args {
            out.push_str(&format!(
                " --secret-arg {}",
                quote(&format!("{key}=<{key}>"))
            ));
        }
        out
    }

    fn scrub_str(&self, text: &str) -> Option<String> {
        let mut out: Option<String> = None;
        for value in &self.values {
            let current = out.as_deref().unwrap_or(text);
            if current.contains(value.as_str()) {
                out = Some(current.replace(value.as_str(), REDACTED));
            }
        }
        out
    }

    /// Mask declared secrets inside every string of `value`; true if any was.
    pub fn scrub_value(&self, value: &mut Value) -> bool {
        match value {
            Value::String(text) => match self.scrub_str(text) {
                Some(scrubbed) => {
                    *text = scrubbed;
                    true
                }
                None => false,
            },
            Value::Array(items) => items
                .iter_mut()
                .fold(false, |any, item| self.scrub_value(item) | any),
            Value::Object(map) => map
                .values_mut()
                .fold(false, |any, item| self.scrub_value(item) | any),
            _ => false,
        }
    }

    /// Round-trip a typed value through JSON to scrub it.
    pub fn scrub<T: Serialize + DeserializeOwned>(&self, typed: T) -> T {
        if self.values.is_empty() {
            return typed;
        }
        let Ok(mut value) = serde_json::to_value(&typed) else {
            return typed;
        };
        if !self.scrub_value(&mut value) {
            return typed;
        }
        serde_json::from_value(value).unwrap_or(typed)
    }

    /// Scrub one polled event. An artifact that cannot be scrubbed safely
    /// becomes a diagnostic saying so.
    pub fn scrub_event(&self, event: TestEvent) -> TestEvent {
        if self.values.is_empty() {
            return event;
        }
        let TestEvent { seq, payload } = event;
        let payload = match payload {
            TestEventPayload::Artifact {
                name,
                mime_type,
                base64,
            } => match self.scrub_artifact(&name, &mime_type, &base64) {
                ArtifactScrub::Keep => TestEventPayload::Artifact {
                    name,
                    mime_type,
                    base64,
                },
                ArtifactScrub::Replace(base64) => TestEventPayload::Artifact {
                    name,
                    mime_type,
                    base64,
                },
                ArtifactScrub::Withhold => TestEventPayload::Diagnostic {
                    phase: "redaction".to_string(),
                    message: format!(
                        "artifact {name} was not saved: it contains a --secret-arg value"
                    ),
                },
            },
            other => self.scrub(other),
        };
        TestEvent { seq, payload }
    }

    pub fn scrub_artifact(&self, name: &str, mime_type: &str, base64: &str) -> ArtifactScrub {
        if self.values.is_empty() {
            return ArtifactScrub::Keep;
        }
        let Ok(bytes) = BASE64.decode(base64.as_bytes()) else {
            return ArtifactScrub::Keep;
        };
        let Ok(text) = String::from_utf8(bytes) else {
            // Binary (screenshots): nothing to search.
            return ArtifactScrub::Keep;
        };
        if !self.values.iter().any(|value| {
            text.contains(value.as_str())
                || text.contains(json_inner(value).as_str())
                || text.contains(markup_escaped(value).as_str())
        }) {
            return ArtifactScrub::Keep;
        }
        let lower = name.to_ascii_lowercase();
        let mime = mime_type.to_ascii_lowercase();
        let scrubbed = if lower.ends_with(".jsonl") {
            text.lines()
                .map(|line| self.scrub_json_text(line, false))
                .collect::<Option<Vec<_>>>()
                .map(|lines| lines.join("\n") + if text.ends_with('\n') { "\n" } else { "" })
        } else if mime.starts_with("application/json") || lower.ends_with(".json") {
            self.scrub_json_text(&text, true)
        } else if mime.starts_with("text/plain")
            || lower.ends_with(".txt")
            || lower.ends_with(".log")
        {
            Some(self.scrub_str(&text).unwrap_or(text))
        } else {
            None
        };
        match scrubbed {
            Some(text) => ArtifactScrub::Replace(BASE64.encode(text)),
            None => ArtifactScrub::Withhold,
        }
    }

    fn scrub_json_text(&self, text: &str, pretty: bool) -> Option<String> {
        if text.trim().is_empty() {
            return Some(text.to_string());
        }
        let mut value: Value = serde_json::from_str(text).ok()?;
        self.scrub_value(&mut value);
        if pretty {
            serde_json::to_string_pretty(&value).ok()
        } else {
            serde_json::to_string(&value).ok()
        }
    }
}

fn json_inner(value: &str) -> String {
    let json = serde_json::to_string(value).unwrap_or_default();
    json.get(1..json.len().saturating_sub(1))
        .unwrap_or_default()
        .to_string()
}

fn markup_escaped(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn pairs(items: &[(&str, &str)]) -> Vec<(String, String)> {
        items
            .iter()
            .map(|(key, value)| (key.to_string(), value.to_string()))
            .collect()
    }

    fn quote(value: &str) -> String {
        format!("'{value}'")
    }

    fn secrets() -> RunSecrets {
        RunSecrets::new(
            &pairs(&[
                ("user", "alice"),
                ("PASSWORD", "hunter22"),
                ("maxTokens", "1000"),
            ]),
            &pairs(&[("pin", "4711-9")]),
        )
    }

    #[test]
    fn rerun_hint_asks_for_hidden_values_by_name() {
        let secrets = secrets();
        secrets.note_run_started(Some(&HashMap::from([
            ("user".to_string(), "alice".to_string()),
            ("PASSWORD".to_string(), REDACTED.to_string()),
            ("maxTokens".to_string(), "1000".to_string()),
            ("pin".to_string(), REDACTED.to_string()),
        ])));
        let hint = secrets.rerun_flags(quote);
        assert_eq!(
            hint,
            " --arg 'user=alice' --arg 'PASSWORD=<PASSWORD>' --arg 'maxTokens=1000' \
             --secret-arg 'pin=<pin>'"
        );
        let meta = secrets.meta_args();
        assert_eq!(meta["user"], "alice");
        assert_eq!(meta["maxTokens"], "1000");
        assert_eq!(meta["PASSWORD"], REDACTED);
        assert_eq!(meta["pin"], REDACTED);
    }

    #[test]
    fn without_the_framework_verdict_every_value_is_hidden() {
        let secrets = secrets();
        assert!(secrets.meta_args().values().all(|value| value == REDACTED));
        assert!(!secrets.rerun_flags(quote).contains("alice"));
    }

    #[test]
    fn only_declared_values_are_scrubbed_from_content() {
        let secrets = secrets();
        let mut value = json!({"note": "pw hunter22 pin 4711-9", "n": ["limit 1000"]});
        assert!(secrets.scrub_value(&mut value));
        // `PASSWORD` is only a guess from its name; its value is not searched.
        assert_eq!(
            value,
            json!({"note": "pw hunter22 pin ***", "n": ["limit 1000"]})
        );
    }

    #[test]
    fn artifacts_are_scrubbed_as_data_and_markup_is_withheld() {
        let secrets = RunSecrets::new(&[], &pairs(&[("token", "</script>")]));
        let encode = |text: &str| BASE64.encode(text);
        let json_text =
            serde_json::to_string_pretty(&json!({"meta": {"token": "</script>"}})).unwrap();
        let ArtifactScrub::Replace(base64) =
            secrets.scrub_artifact("report.json", "application/json", &encode(&json_text))
        else {
            panic!("json is rewritten");
        };
        let json: Value = serde_json::from_slice(&BASE64.decode(base64).unwrap()).unwrap();
        assert_eq!(json["meta"]["token"], REDACTED);

        let ArtifactScrub::Replace(base64) = secrets.scrub_artifact(
            "attachments/a/logs.txt",
            "text/plain; charset=utf-8",
            &encode("typed </script> ok"),
        ) else {
            panic!("text is rewritten");
        };
        assert_eq!(BASE64.decode(base64).unwrap(), b"typed *** ok");

        let html = "<script>const r = {\"t\":\"<\\/script>\"}</script><p>&lt;/script&gt;</p>";
        assert!(matches!(
            secrets.scrub_artifact("report.html", "text/html", &encode(html)),
            ArtifactScrub::Withhold
        ));
        assert!(matches!(
            secrets.scrub_artifact(
                "failure.png",
                "image/png",
                &BASE64.encode([0x89u8, 0x50, 0xff])
            ),
            ArtifactScrub::Keep
        ));
    }

    #[test]
    fn events_are_scrubbed_and_withheld_artifacts_become_diagnostics() {
        let secrets = RunSecrets::new(&[], &pairs(&[("pin", "4711-9")]));
        let console = secrets.scrub_event(TestEvent {
            seq: 1,
            payload: TestEventPayload::Console {
                level: "info".into(),
                message: "pin=4711-9".into(),
            },
        });
        assert!(matches!(
            console.payload,
            TestEventPayload::Console { ref message, .. } if message == "pin=***"
        ));
        let artifact = secrets.scrub_event(TestEvent {
            seq: 2,
            payload: TestEventPayload::Artifact {
                name: "junit.xml".into(),
                mime_type: "application/xml".into(),
                base64: BASE64.encode("<failure message=\"4711-9\"/>"),
            },
        });
        assert!(matches!(
            artifact.payload,
            TestEventPayload::Diagnostic { ref phase, .. } if phase == "redaction"
        ));
    }
}
