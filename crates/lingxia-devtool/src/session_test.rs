//! `session.test.*` adapter over the generic host automation runtime.

use lingxia_automation::runtime::{
    AutomationCancelArgs, AutomationEventPayload, AutomationPollArgs, AutomationPollResponse,
    AutomationRunError, AutomationRunState, AutomationRuntime, AutomationStartArgs,
};
use lingxia_devtool_protocol::{handlers, session_test::*};
use serde::Deserialize;
use serde::de::DeserializeOwned;
use serde_json::Value;
use std::sync::OnceLock;

pub(crate) fn handle_session_test_command(
    handler: &str,
    args: Option<Value>,
) -> Option<Result<Option<Value>, String>> {
    if !handler.starts_with("session.test.") {
        return None;
    }
    Some(handle_session_test_command_impl(handler, args))
}

fn runtime() -> Result<&'static AutomationRuntime, String> {
    static RUNTIME: OnceLock<Result<AutomationRuntime, String>> = OnceLock::new();
    RUNTIME
        .get_or_init(AutomationRuntime::new)
        .as_ref()
        .map_err(Clone::clone)
}

fn handle_session_test_command_impl(
    handler: &str,
    args: Option<Value>,
) -> Result<Option<Value>, String> {
    match handler {
        handlers::session::test::START => {
            let args: TestStartArgs = parse(handler, args)?;
            let response = runtime()?.start(AutomationStartArgs {
                source: args.source,
                source_name: args.source_name,
                timeout_ms: args.timeout_ms,
                args: args.args,
            })?;
            respond(TestStartResponse {
                run_id: response.run_id,
                state: TestRunState::Running,
            })
        }
        handlers::session::test::POLL => {
            let args: TestPollArgs = parse(handler, args)?;
            let response = runtime()?.poll(AutomationPollArgs {
                run_id: args.run_id,
                after_seq: args.after_seq,
            })?;
            respond(test_poll_response(response)?)
        }
        handlers::session::test::CANCEL => {
            let args: TestCancelArgs = parse(handler, args)?;
            let response = runtime()?.cancel(AutomationCancelArgs {
                run_id: args.run_id,
                reason: args.reason,
            })?;
            respond(TestCancelResponse {
                run_id: response.run_id,
                state: map_state(response.state),
            })
        }
        other => Err(format!("unknown session.test handler: {other}")),
    }
}

fn test_poll_response(response: AutomationPollResponse) -> Result<TestPollResponse, String> {
    let events = response
        .events
        .into_iter()
        .map(|event| {
            let payload = match event.payload {
                AutomationEventPayload::Console { level, message } => {
                    TestEventPayload::Console { level, message }
                }
                AutomationEventPayload::Artifact {
                    name,
                    mime_type,
                    base64,
                } => TestEventPayload::Artifact {
                    name,
                    mime_type,
                    base64,
                },
                AutomationEventPayload::Event { value } => {
                    framework_event(value).unwrap_or_else(|message| TestEventPayload::Console {
                        level: "warn".to_string(),
                        message: format!("ignored invalid @rongjs/test event: {message}"),
                    })
                }
            };
            TestEvent {
                seq: event.seq,
                payload,
            }
        })
        .collect();

    let (state, result) = match response.result {
        None => (map_state(response.state), None),
        Some(result) => test_result(response.state, result)?,
    };
    Ok(TestPollResponse {
        run_id: response.run_id,
        state,
        next_seq: response.next_seq,
        events,
        result,
    })
}

fn test_result(
    state: AutomationRunState,
    result: lingxia_automation::runtime::AutomationRunResult,
) -> Result<(TestRunState, Option<TestRunResult>), String> {
    let duration_ms = result.duration_ms;
    match state {
        AutomationRunState::Succeeded => {
            let report = result
                .output
                .ok_or_else(|| "@rongjs/test returned no report".to_string())
                .and_then(|value| {
                    serde_json::from_value::<TestReport>(value)
                        .map_err(|err| format!("@rongjs/test returned an invalid report: {err}"))
                })
                .and_then(|report| {
                    validate_report(&report)?;
                    Ok(report)
                });
            let report = match report {
                Ok(report) => report,
                Err(message) => {
                    return Ok((
                        TestRunState::InternalError,
                        Some(TestRunResult {
                            duration_ms,
                            error: Some(TestRunError {
                                name: "TestProtocolError".to_string(),
                                message,
                                stack: None,
                                causes: Vec::new(),
                            }),
                            report: None,
                        }),
                    ));
                }
            };
            let state = if report.failed == 0 {
                TestRunState::Passed
            } else {
                TestRunState::Failed
            };
            Ok((
                state,
                Some(TestRunResult {
                    duration_ms,
                    error: None,
                    report: Some(report),
                }),
            ))
        }
        other => Ok((
            map_state(other),
            Some(TestRunResult {
                duration_ms,
                error: result.error.map(map_error),
                report: None,
            }),
        )),
    }
}

fn map_state(state: AutomationRunState) -> TestRunState {
    match state {
        AutomationRunState::Running => TestRunState::Running,
        AutomationRunState::Succeeded => TestRunState::Passed,
        AutomationRunState::Failed => TestRunState::Failed,
        AutomationRunState::TimedOut => TestRunState::TimedOut,
        AutomationRunState::Cancelled => TestRunState::Cancelled,
        AutomationRunState::InternalError => TestRunState::InternalError,
    }
}

fn map_error(error: AutomationRunError) -> TestRunError {
    TestRunError {
        name: error.name,
        message: error.message,
        stack: error.stack,
        causes: error.causes.into_iter().map(map_error).collect(),
    }
}

#[derive(Deserialize)]
struct FrameworkEvent {
    #[serde(rename = "type")]
    event_type: String,
    name: Option<String>,
    full_name: Option<String>,
    status: Option<TestCaseStatus>,
    duration_ms: Option<u64>,
    error: Option<TestRunError>,
}

fn framework_event(value: Value) -> Result<TestEventPayload, String> {
    let event: FrameworkEvent = serde_json::from_value(value)
        .map_err(|err| format!("invalid @rongjs/test event: {err}"))?;
    let required = |value: Option<String>, field: &str| {
        value.ok_or_else(|| format!("@rongjs/test event is missing {field}"))
    };
    match event.event_type.as_str() {
        "case_started" => Ok(TestEventPayload::CaseStarted {
            name: required(event.name, "name")?,
            full_name: required(event.full_name, "full_name")?,
        }),
        "case_finished" => Ok(TestEventPayload::CaseFinished {
            name: required(event.name, "name")?,
            full_name: required(event.full_name, "full_name")?,
            status: event
                .status
                .ok_or_else(|| "@rongjs/test case_finished is missing status".to_string())?,
            duration_ms: event.duration_ms.unwrap_or_default(),
            error: event.error,
        }),
        other => Err(format!("unknown @rongjs/test event type: {other}")),
    }
}

fn validate_report(report: &TestReport) -> Result<(), String> {
    if report.total != report.cases.len() {
        return Err(format!(
            "@rongjs/test report total {} does not match {} cases",
            report.total,
            report.cases.len()
        ));
    }
    let counted_total = report
        .passed
        .checked_add(report.failed)
        .and_then(|count| count.checked_add(report.skipped));
    if counted_total != Some(report.total) {
        return Err("@rongjs/test report counts do not add up to total".to_string());
    }
    let (mut passed, mut failed, mut skipped) = (0, 0, 0);
    for case in &report.cases {
        match case.status {
            TestCaseStatus::Passed => {
                passed += 1;
                if case.error.is_some() {
                    return Err("@rongjs/test passed case contains an error".to_string());
                }
            }
            TestCaseStatus::Failed => {
                failed += 1;
                if case.error.is_none() {
                    return Err("@rongjs/test failed case is missing its error".to_string());
                }
            }
            TestCaseStatus::Skipped => {
                skipped += 1;
                if case.error.is_some() {
                    return Err("@rongjs/test skipped case contains an error".to_string());
                }
            }
        }
    }
    if (passed, failed, skipped) != (report.passed, report.failed, report.skipped) {
        return Err("@rongjs/test report counts do not match case statuses".to_string());
    }
    Ok(())
}

fn parse<T: DeserializeOwned>(handler: &str, args: Option<Value>) -> Result<T, String> {
    let value = args.ok_or_else(|| format!("missing args for {handler}"))?;
    serde_json::from_value(value).map_err(|err| format!("invalid args for {handler}: {err}"))
}

fn respond<T: serde::Serialize>(response: T) -> Result<Option<Value>, String> {
    serde_json::to_value(response)
        .map(Some)
        .map_err(|err| err.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn case(status: TestCaseStatus) -> TestCaseResult {
        TestCaseResult {
            name: "case".to_string(),
            full_name: "suite > case".to_string(),
            status,
            duration_ms: 1,
            error: None,
        }
    }

    #[test]
    fn report_counts_must_match_cases() {
        let report = TestReport {
            total: 1,
            passed: 1,
            failed: 0,
            skipped: 0,
            duration_ms: 1,
            cases: vec![case(TestCaseStatus::Failed)],
        };
        assert!(validate_report(&report).is_err());
    }

    #[test]
    fn report_count_overflow_is_rejected() {
        let report = TestReport {
            total: 0,
            passed: usize::MAX,
            failed: 1,
            skipped: 0,
            duration_ms: 0,
            cases: Vec::new(),
        };
        assert!(validate_report(&report).is_err());
    }

    #[test]
    fn unknown_framework_event_is_forward_compatible() {
        let payload = framework_event(serde_json::json!({ "type": "suite_started" }));

        assert!(payload.is_err());
    }

    #[test]
    fn invalid_terminal_report_becomes_internal_error() {
        let (state, result) = test_result(
            AutomationRunState::Succeeded,
            lingxia_automation::runtime::AutomationRunResult {
                duration_ms: 1,
                error: None,
                output: Some(serde_json::json!({ "total": 1 })),
            },
        )
        .unwrap();

        assert_eq!(state, TestRunState::InternalError);
        assert_eq!(result.unwrap().error.unwrap().name, "TestProtocolError");
    }
}
