//! `session.companion.*`: the dev server's relay to the session companion
//! for scenario `function` rules and the Function half of mocks. A client
//! (`lxdev mock`) and the runtime (a host run's `t.app.mock.*`) both send
//! these; the server forwards `scenario.*` to a companion that declared
//! [`capabilities::SCENARIO_FUNCTION`] and `mock.*` to one that declared
//! [`capabilities::MOCK`], and clears an owner's rules and mock selection
//! when its test run ends or the runtime connection drops.

use super::DevServerState;
use lingxia_control_protocol::ControlResponse;
use lingxia_control_protocol::dev_session::{DevSessionMessage, capabilities};
use lingxia_control_protocol::methods::session::companion as method;
use lingxia_control_protocol::mock::companion as mock;
use lingxia_control_protocol::scenario::companion as protocol;
use std::time::Duration;

/// How long a companion may take to install or report rules.
const COMPANION_TIMEOUT: Duration = Duration::from_secs(15);

pub(super) const NO_COMPANION: &str = "this dev session has no companion \
     (.lingxia/dev-companion.json), so nothing answers function rules";
pub(super) const NOT_DECLARED: &str = "the dev session's companion does not answer function rules \
     yet (it did not declare the `scenario.function` capability)";
pub(super) const NO_MOCK_COMPANION: &str = "this dev session has no companion";
pub(super) const MOCK_NOT_DECLARED: &str =
    "the companion does not switch mocks (no 'mock' capability)";

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
        let forward = method_name.strip_prefix(method::PREFIX);
        let capability = match forward {
            Some(name @ (protocol::USE | protocol::CLEAR | protocol::STATUS | protocol::CALLS)) => {
                Some((name, capabilities::SCENARIO_FUNCTION))
            }
            Some(name @ (mock::SET | mock::STATUS | mock::RESET)) => {
                Some((name, capabilities::MOCK))
            }
            _ => None,
        };
        let Some((forward, capability)) = capability else {
            return DevSessionMessage::error(
                id,
                "unknown_method",
                format!("unknown companion method {method_name}"),
            );
        };
        let mocks = capability == capabilities::MOCK;
        let Some(link) = link else {
            let message = if mocks {
                NO_MOCK_COMPANION
            } else {
                NO_COMPANION
            };
            return DevSessionMessage::error(id, method::UNSUPPORTED, message);
        };
        if !link.supports(capability) {
            let message = if mocks {
                MOCK_NOT_DECLARED
            } else {
                NOT_DECLARED
            };
            return DevSessionMessage::error(id, method::UNSUPPORTED, message);
        }
        let owner = params
            .as_ref()
            .and_then(|params| params.get("owner"))
            .and_then(serde_json::Value::as_str)
            .map(str::to_string);
        let dropping = params
            .as_ref()
            .and_then(|params| params.get("mode"))
            .and_then(serde_json::Value::as_str)
            == Some("default");
        match link.request(forward, params, COMPANION_TIMEOUT) {
            Ok(result) => {
                if let Some(owner) = owner {
                    if forward == protocol::USE {
                        self.lock_companion_owners().insert(owner);
                    } else if forward == protocol::CLEAR {
                        self.lock_companion_owners().remove(&owner);
                    } else if forward == mock::SET && dropping {
                        self.lock_companion_mock_owners().remove(&owner);
                    } else if forward == mock::SET || forward == mock::RESET {
                        // A run's owner exists from its first spec's reset.
                        self.lock_companion_mock_owners().insert(owner);
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

    fn lock_companion_mock_owners(
        &self,
    ) -> std::sync::MutexGuard<'_, std::collections::HashSet<String>> {
        self.companion_mock_owners
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// Drop `owner`'s mock selection in the companion if it holds one: its
    /// test run ended, or (for `dev`) the runtime connection dropped. Off
    /// the calling thread.
    pub(super) fn drop_companion_mock_owner(&self, owner: String) {
        if !self.lock_companion_mock_owners().remove(&owner) {
            return;
        }
        let Some(link) = self.companion.get().cloned() else {
            return;
        };
        std::thread::spawn(move || {
            let params = serde_json::json!({ "owner": owner, "mode": "default" });
            if let Err(error) = link.request(mock::SET, Some(params), COMPANION_TIMEOUT) {
                eprintln!(
                    "[lingxia dev] could not drop the companion's mock selection for {owner}: {}",
                    error.message
                );
            }
        });
    }

    /// A test run started while a dev scenario has `function` rules: give
    /// the run an empty owner, which sits above `dev`, so the dev rules
    /// stand aside until the run ends — as its `http` rules do.
    pub(super) fn hide_companion_dev_scenario(&self, owner: String) {
        if !self
            .lock_companion_owners()
            .contains(lingxia_control_protocol::scenario::DEV_OWNER)
        {
            return;
        }
        let Some(link) = self.companion.get().cloned() else {
            return;
        };
        self.lock_companion_owners().insert(owner.clone());
        std::thread::spawn(move || {
            let params = serde_json::json!({ "owner": owner, "scenario": {}, "rules": [] });
            if let Err(error) = link.request(protocol::USE, Some(params), COMPANION_TIMEOUT) {
                eprintln!(
                    "[lingxia dev] could not set the dev scenario's function rules aside for {owner}: {}",
                    error.message
                );
            }
        });
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
    use crate::commands::dev::server::SessionLogWriter;
    use lingxia_control_protocol::ControlError;
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

        // A dev scenario with function rules stands aside during a run.
        state.hide_companion_dev_scenario("test:run-2".into());
        assert!(
            !state.lock_companion_owners().contains("test:run-2"),
            "no dev scenario, nothing to hide"
        );
        let dev = serde_json::json!({ "owner": "dev", "scenario": {}, "rules": [{ "function": "f", "result": 1 }] });
        result_of(state.companion_reply("8".into(), method::SCENARIO_USE, Some(dev)));
        state.hide_companion_dev_scenario("test:run-2".into());
        assert!(state.lock_companion_owners().contains("test:run-2"));
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        let requests = loop {
            let text = std::fs::read_to_string(&log).unwrap_or_default();
            if text.lines().count() >= 4 {
                break text;
            }
            assert!(
                std::time::Instant::now() < deadline,
                "the hiding owner never arrived: {text}"
            );
            std::thread::sleep(Duration::from_millis(20));
        };
        let hidden: serde_json::Value =
            serde_json::from_str(requests.lines().nth(3).unwrap()).unwrap();
        assert_eq!(hidden["method"], protocol::USE);
        assert_eq!(
            hidden["params"],
            serde_json::json!({ "owner": "test:run-2", "scenario": {}, "rules": [] })
        );
        drop(companion);
    }

    #[test]
    fn mock_requests_need_the_mock_capability_and_run_owners_are_dropped() {
        let relay = state();
        let error = error_of(relay.companion_reply(
            "1".into(),
            method::MOCK_SET,
            Some(serde_json::json!({ "owner": "dev", "mode": "all" })),
        ));
        assert_eq!(
            (error.code.as_str(), error.message.as_str()),
            (method::UNSUPPORTED, NO_MOCK_COMPANION)
        );

        // A companion that answers scenarios but does not switch mocks.
        let root = tempfile::tempdir().unwrap();
        let scenario_only = companion(
            root.path(),
            r#"["requests","scenario.function"]"#,
            r#""result":{"mocked":1,"total":2}"#,
        );
        let relay = state();
        let _ = relay.companion.set(scenario_only.link());
        let error = error_of(relay.companion_reply(
            "2".into(),
            method::MOCK_STATUS,
            Some(serde_json::json!({})),
        ));
        assert_eq!(error.message, MOCK_NOT_DECLARED);
        drop(scenario_only);

        let root = tempfile::tempdir().unwrap();
        let capable = companion(
            root.path(),
            r#"["requests","mock"]"#,
            r#""result":{"mocked":1,"total":2}"#,
        );
        let relay = state();
        let _ = relay.companion.set(capable.link());
        let result = result_of(relay.companion_reply(
            "3".into(),
            method::MOCK_RESET,
            Some(serde_json::json!({ "owner": "test:run-9" })),
        ));
        assert_eq!(result["mocked"], 1);
        assert!(relay.lock_companion_mock_owners().contains("test:run-9"));
        // Scenario methods still need their own capability.
        let error = error_of(relay.companion_reply(
            "4".into(),
            method::SCENARIO_STATUS,
            Some(serde_json::json!({})),
        ));
        assert_eq!(error.message, NOT_DECLARED);

        // The run ends: its mock selection is dropped in the companion.
        relay.drop_companion_mock_owner("test:run-9".into());
        assert!(!relay.lock_companion_mock_owners().contains("test:run-9"));
        let log = root.path().join("requests.log");
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        let lines = loop {
            let text = std::fs::read_to_string(&log).unwrap_or_default();
            if text.lines().count() >= 2 {
                break text;
            }
            assert!(
                std::time::Instant::now() < deadline,
                "no drop arrived: {text}"
            );
            std::thread::sleep(Duration::from_millis(20));
        };
        let requests: Vec<serde_json::Value> = lines
            .lines()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect();
        assert_eq!(requests[0]["method"], mock::RESET);
        assert_eq!(requests[1]["method"], mock::SET);
        assert_eq!(
            requests[1]["params"],
            serde_json::json!({ "owner": "test:run-9", "mode": "default" })
        );
        drop(capable);
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
