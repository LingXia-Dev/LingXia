//! `session.companion.*`: the dev server's relay to the session companion
//! for scenario `function` rules. A client (`lxdev scenario`) and the
//! runtime (a host run's `t.app.scenario()`) both send these; the server
//! forwards `scenario.*` to a companion that declared
//! [`capabilities::SCENARIO_FUNCTION`], and clears an owner's rules when
//! its test run ends or the runtime connection drops.

use super::DevServerState;
use lingxia_control_protocol::dev_session::{DevSessionMessage, capabilities};
use lingxia_control_protocol::methods::session::companion as method;
use lingxia_control_protocol::scenario::companion as protocol;
use lingxia_control_protocol::ControlResponse;
use std::time::Duration;

/// How long a companion may take to install or report rules.
const COMPANION_TIMEOUT: Duration = Duration::from_secs(15);

pub(super) const NO_COMPANION: &str = "this dev session has no companion \
     (.lingxia/dev-companion.json), so nothing answers function rules";
pub(super) const NOT_DECLARED: &str = "the dev session's companion does not answer function rules \
     yet (it did not declare the `scenario.function` capability)";

impl DevServerState {
    /// Answer one `session.companion.*` request.
    pub(super) fn companion_reply(
        &self,
        id: String,
        method_name: &str,
        params: Option<serde_json::Value>,
    ) -> DevSessionMessage {
        let link = self.companion.get();
        if method_name == method::CAPABILITIES {
            let capabilities = link
                .map(|link| link.capabilities().to_vec())
                .unwrap_or_default();
            return DevSessionMessage::success(
                id,
                Some(serde_json::json!({
                    "companion": link.is_some(),
                    "capabilities": capabilities,
                })),
            );
        }
        let forward = method_name.strip_prefix(method::PREFIX).filter(|name| {
            [
                protocol::USE,
                protocol::CLEAR,
                protocol::STATUS,
                protocol::CALLS,
            ]
            .contains(name)
        });
        let Some(forward) = forward else {
            return DevSessionMessage::error(
                id,
                "unknown_method",
                format!("unknown companion method {method_name}"),
            );
        };
        let Some(link) = link else {
            return DevSessionMessage::error(id, method::UNSUPPORTED, NO_COMPANION);
        };
        if !link.supports(capabilities::SCENARIO_FUNCTION) {
            return DevSessionMessage::error(id, method::UNSUPPORTED, NOT_DECLARED);
        }
        let owner = params
            .as_ref()
            .and_then(|params| params.get("owner"))
            .and_then(serde_json::Value::as_str)
            .map(str::to_string);
        match link.request(forward, params, COMPANION_TIMEOUT) {
            Ok(result) => {
                if let Some(owner) = owner {
                    let mut owners = self.lock_companion_owners();
                    if forward == protocol::USE {
                        owners.insert(owner);
                    } else if forward == protocol::CLEAR {
                        owners.remove(&owner);
                    }
                }
                DevSessionMessage::success(id, result)
            }
            Err(error) => DevSessionMessage::Response(ControlResponse {
                id,
                result: None,
                error: Some(error),
            }),
        }
    }

    fn lock_companion_owners(
        &self,
    ) -> std::sync::MutexGuard<'_, std::collections::HashSet<String>> {
        self.companion_owners
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// Clear `owner`'s rules in the companion if it has any: its test run
    /// ended, or (for `dev`) the runtime connection dropped. Off the calling
    /// thread, which may be the connection's reader.
    pub(super) fn clear_companion_owner(&self, owner: String) {
        if !self.lock_companion_owners().remove(&owner) {
            return;
        }
        let Some(link) = self.companion.get().cloned() else {
            return;
        };
        std::thread::spawn(move || {
            let params = serde_json::json!({ "owner": owner });
            if let Err(error) = link.request(protocol::CLEAR, Some(params), COMPANION_TIMEOUT) {
                eprintln!(
                    "[lingxia dev] could not clear the companion's scenario for {owner}: {}",
                    error.message
                );
            }
        });
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use crate::commands::dev::companion::DevCompanion;
    use lingxia_control_protocol::ControlError;
    use crate::commands::dev::server::SessionLogWriter;
    use lingxia_control_protocol::dev_session::DevSessionMessage;
    use std::sync::Arc;
    use std::sync::atomic::AtomicBool;

    fn state() -> DevServerState {
        DevServerState::new(
            std::env::temp_dir(),
            Arc::new(AtomicBool::new(false)),
            None,
            false,
        )
    }

    fn error_of(reply: DevSessionMessage) -> ControlError {
        match reply {
            DevSessionMessage::Response(ControlResponse {
                error: Some(error), ..
            }) => error,
            other => panic!("expected an error, got {other:?}"),
        }
    }

    fn result_of(reply: DevSessionMessage) -> serde_json::Value {
        match reply {
            DevSessionMessage::Response(ControlResponse {
                result,
                error: None,
                ..
            }) => result.unwrap_or_default(),
            other => panic!("expected a result, got {other:?}"),
        }
    }

    #[test]
    fn without_a_companion_function_rules_fail_clearly() {
        let state = state();
        let caps = result_of(state.companion_reply("1".into(), method::CAPABILITIES, None));
        assert_eq!(caps["companion"], false);
        let error = error_of(state.companion_reply(
            "2".into(),
            method::SCENARIO_USE,
            Some(serde_json::json!({ "owner": "dev", "rules": [] })),
        ));
        assert_eq!(error.code, method::UNSUPPORTED);
        assert_eq!(error.message, NO_COMPANION);
        let error = error_of(state.companion_reply("3".into(), "session.companion.shell", None));
        assert_eq!(error.code, "unknown_method");
    }

    /// A companion script: `hello` with `capabilities`, then it answers
    /// `session.prepare` and every later request with `answer` (a JSON
    /// response body without the id), echoing the request id.
    fn companion(root: &std::path::Path, capabilities: &str, answer: &str) -> DevCompanion {
        let config_dir = root.join(".lingxia");
        std::fs::create_dir_all(&config_dir).unwrap();
        let script = format!(
            "printf '%s\\n' '{{\"type\":\"hello\",\"version\":2,\"role\":\"companion\",\"capabilities\":{capabilities}}}'; \
             IFS= read -r request; \
             printf '%s\\n' '{{\"type\":\"response\",\"id\":\"session-prepare\",\"result\":{{\"active\":true}}}}'; \
             while IFS= read -r line; do \
               id=$(printf '%s' \"$line\" | sed 's/.*\"id\":\"\\([^\"]*\\)\".*/\\1/'); \
               printf '%s\\n' \"$line\" >> requests.log; \
               printf '{{\"type\":\"response\",\"id\":\"%s\",%s}}\\n' \"$id\" '{answer}'; \
             done"
        );
        std::fs::write(
            config_dir.join("dev-companion.json"),
            serde_json::to_vec(&serde_json::json!({ "run": ["sh", "-c", script] })).unwrap(),
        )
        .unwrap();
        let session = crate::commands::dev::log_store::create_session(root).unwrap();
        let writer = Arc::new(SessionLogWriter::new(&session).unwrap());
        DevCompanion::start(root, Arc::new(AtomicBool::new(false)), writer)
            .unwrap()
            .unwrap()
    }

    #[test]
    fn a_companion_without_the_capability_gets_nothing_forwarded() {
        let root = tempfile::tempdir().unwrap();
        let companion = companion(
            root.path(),
            r#"["requests"]"#,
            r#""result":{"installed":1}"#,
        );
        let state = state();
        let _ = state.companion.set(companion.link());
        let caps = result_of(state.companion_reply("1".into(), method::CAPABILITIES, None));
        assert_eq!(
            caps,
            serde_json::json!({ "companion": true, "capabilities": ["requests"] })
        );
        let error = error_of(state.companion_reply(
            "2".into(),
            method::SCENARIO_USE,
            Some(serde_json::json!({ "owner": "dev", "rules": [] })),
        ));
        assert_eq!(error.code, method::UNSUPPORTED);
        assert_eq!(error.message, NOT_DECLARED);
        drop(companion);
        assert!(!root.path().join("requests.log").exists());
    }

    #[test]
    fn scenario_requests_reach_a_capable_companion_and_owners_are_cleared() {
        let root = tempfile::tempdir().unwrap();
        let companion = companion(
            root.path(),
            r#"["requests","scenario.function"]"#,
            r#""result":{"installed":1}"#,
        );
        let state = state();
        let _ = state.companion.set(companion.link());
        let params = serde_json::json!({
            "owner": "test:run-1",
            "scenario": { "name": "Checkout" },
            "rules": [{ "function": "orders.submit", "fault": "unknown" }]
        });
        let result =
            result_of(state.companion_reply("7".into(), method::SCENARIO_USE, Some(params)));
        assert_eq!(result["installed"], 1);
        assert!(state.lock_companion_owners().contains("test:run-1"));

        // The run ends: its owner is cleared in the companion.
        state.clear_companion_owner("test:run-1".into());
        assert!(!state.lock_companion_owners().contains("test:run-1"));
        let log = root.path().join("requests.log");
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        let lines = loop {
            let text = std::fs::read_to_string(&log).unwrap_or_default();
            if text.lines().count() >= 2 {
                break text;
            }
            assert!(
                std::time::Instant::now() < deadline,
                "the clear never arrived: {text}"
            );
            std::thread::sleep(Duration::from_millis(20));
        };
        let requests: Vec<serde_json::Value> = lines
            .lines()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect();
        assert_eq!(requests[0]["method"], protocol::USE);
        assert_eq!(
            requests[0]["params"]["rules"][0]["function"],
            "orders.submit"
        );
        assert_eq!(requests[1]["method"], protocol::CLEAR);
        assert_eq!(
            requests[1]["params"],
            serde_json::json!({ "owner": "test:run-1" })
        );
        // Clearing an owner with nothing installed sends nothing.
        state.clear_companion_owner("dev".into());
        drop(companion);
    }

    #[test]
    fn companion_errors_pass_through_with_their_data() {
        let root = tempfile::tempdir().unwrap();
        let companion = companion(
            root.path(),
            r#"["requests","scenario.function"]"#,
            r#""error":{"code":"invalid_rules","message":"bad","data":{"errors":[{"rule":0,"message":"unknown Function"}]}}"#,
        );
        let state = state();
        let _ = state.companion.set(companion.link());
        let error = error_of(state.companion_reply(
            "9".into(),
            method::SCENARIO_USE,
            Some(serde_json::json!({ "owner": "dev", "rules": [{}] })),
        ));
        assert_eq!(error.code, protocol::INVALID_RULES);
        assert_eq!(
            error.data.unwrap()["errors"][0]["message"],
            "unknown Function"
        );
        assert!(!state.lock_companion_owners().contains("dev"));
        drop(companion);
    }
}
