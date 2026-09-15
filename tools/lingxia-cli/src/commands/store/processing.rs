//! Store processing results and bounded polling, independent of review/release.

use anyhow::{Result, bail};
use clap::Args;
use serde::Serialize;
use std::time::{Duration, Instant};

#[derive(Args, Clone, Debug)]
pub struct ProcessingOptions {
    /// Wait for this build/package to finish processing (Apple and Harmony).
    #[arg(long)]
    pub wait: bool,
    /// Processing deadline in seconds (after upload for submit).
    #[arg(long, default_value_t = 1800, value_parser = clap::value_parser!(u64).range(1..), requires = "wait")]
    pub timeout: u64,
    /// Seconds between processing queries.
    #[arg(long, default_value_t = 15, value_parser = clap::value_parser!(u64).range(1..), requires = "wait")]
    pub poll_interval: u64,
    /// Emit one JSON result to stdout; progress goes to stderr (Apple and Harmony).
    #[arg(
        long,
        long_help = "Emit one JSON result to stdout; progress goes to stderr (Apple and Harmony).\n\nSchema version 1 includes: action, platform, ok, uploaded, artifact, results, and error. Each result includes app_id, submission_id, version, build_number, state, and raw_state.\n\n`ok` describes command success; without --wait it does not imply processing is complete. Results use uploaded/pending/processing/complete/failed/unknown. With --wait, only complete succeeds. Failures and timeouts exit nonzero and include error.code; the last known submission identity remains available for another status query."
    )]
    pub json: bool,
}

#[derive(Args, Clone, Debug, Default)]
pub struct BuildSelection {
    /// Apple marketing version; IPA uploads read this from Info.plist.
    #[arg(long, requires = "build_number")]
    pub version: Option<String>,
    /// Apple CFBundleVersion; required with --version for PKG upload waiting.
    #[arg(long, requires = "version")]
    pub build_number: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum State {
    Uploaded,
    Pending,
    Processing,
    Complete,
    Failed,
    Unknown,
}

#[derive(Clone, Debug, Serialize)]
pub struct Record {
    pub app_id: String,
    pub submission_id: Option<String>,
    pub version: Option<String>,
    pub build_number: Option<String>,
    pub state: State,
    pub raw_state: Option<String>,
}

impl Record {
    pub fn new(app_id: impl Into<String>, state: State) -> Self {
        Self {
            app_id: app_id.into(),
            submission_id: None,
            version: None,
            build_number: None,
            state,
            raw_state: None,
        }
    }
}

#[derive(Debug)]
pub struct ProcessingError {
    pub code: &'static str,
    pub message: String,
}

impl std::fmt::Display for ProcessingError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.code, self.message)
    }
}

impl std::error::Error for ProcessingError {}

pub fn check_http_status(status: u16) -> Result<()> {
    if (200..300).contains(&status) {
        return Ok(());
    }
    let code = if matches!(status, 408 | 429 | 500..=599) {
        "STORE_REQUEST_RETRYABLE"
    } else {
        "STORE_REQUEST_FAILED"
    };
    Err(failure(code, format!("Store returned HTTP {status}")))
}

pub fn failure(code: &'static str, message: impl Into<String>) -> anyhow::Error {
    ProcessingError {
        code,
        message: message.into(),
    }
    .into()
}

/// The query receives the remaining budget, including HTTP time. Keeping the
/// last observation lets CI resume after a timeout without uploading again.
pub fn wait(
    record: &mut Record,
    options: &ProcessingOptions,
    mut query: impl FnMut(Duration) -> Result<Record>,
) -> Result<()> {
    let start = Instant::now();
    poll(
        record,
        Duration::from_secs(options.timeout),
        Duration::from_secs(options.poll_interval),
        &mut query,
        || start.elapsed(),
        std::thread::sleep,
    )
}

fn poll(
    record: &mut Record,
    timeout: Duration,
    interval: Duration,
    query: &mut impl FnMut(Duration) -> Result<Record>,
    elapsed: impl Fn() -> Duration,
    mut sleep: impl FnMut(Duration),
) -> Result<()> {
    loop {
        let remaining = timeout.saturating_sub(elapsed());
        if remaining.is_zero() {
            return Err(failure(
                "STORE_PROCESSING_TIMEOUT",
                "Processing deadline exceeded; query this submission again without re-uploading",
            ));
        }
        let observation = query(remaining.min(Duration::from_secs(30)));
        if let Ok(observation) = observation.as_ref() {
            if record
                .submission_id
                .as_ref()
                .zip(observation.submission_id.as_ref())
                .is_some_and(|(expected, actual)| expected != actual)
            {
                return Err(failure(
                    "STORE_PROCESSING_IDENTITY_MISMATCH",
                    "Processing query returned a different submission",
                ));
            }
            let mut observation = observation.clone();
            observation.submission_id = observation
                .submission_id
                .or_else(|| record.submission_id.clone());
            observation.version = observation.version.or_else(|| record.version.clone());
            observation.build_number = observation
                .build_number
                .or_else(|| record.build_number.clone());
            *record = observation;
        }
        if elapsed() >= timeout {
            return Err(failure(
                "STORE_PROCESSING_TIMEOUT",
                "Processing deadline exceeded",
            ));
        }
        if let Err(err) = observation {
            let retryable = err.downcast_ref::<ureq::Error>().is_some()
                || err
                    .downcast_ref::<ProcessingError>()
                    .is_some_and(|e| e.code == "STORE_REQUEST_RETRYABLE");
            if !retryable {
                return Err(err);
            }
            eprintln!("  transient processing query failure; retrying: {err}");
            sleep(interval.min(timeout.saturating_sub(elapsed())));
            continue;
        }
        eprintln!(
            "  processing {:?}: {:?}",
            record.submission_id, record.state
        );
        match record.state {
            State::Complete => return Ok(()),
            State::Failed => {
                return Err(failure(
                    "STORE_PROCESSING_FAILED",
                    format!(
                        "Store rejected processing ({})",
                        record.raw_state.as_deref().unwrap_or("unknown")
                    ),
                ));
            }
            State::Unknown => {
                return Err(failure(
                    "STORE_PROCESSING_UNKNOWN",
                    format!(
                        "Unrecognized store state: {}",
                        record.raw_state.as_deref().unwrap_or("missing")
                    ),
                ));
            }
            State::Uploaded | State::Pending | State::Processing => {}
        }
        sleep(interval.min(timeout.saturating_sub(elapsed())));
    }
}

pub fn validate_selection(selection: &BuildSelection) -> Result<()> {
    if selection.version.is_some() != selection.build_number.is_some()
        || selection
            .version
            .as_deref()
            .is_some_and(|s| s.trim().is_empty())
        || selection
            .build_number
            .as_deref()
            .is_some_and(|s| s.trim().is_empty())
    {
        bail!("Provide both a nonempty --version and --build-number");
    }
    Ok(())
}

#[cfg(test)]
pub fn test_server(status: u16, body: &str) -> (String, std::thread::JoinHandle<String>) {
    use std::io::{Read, Write};
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let body = body.to_owned();
    let handle = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        let mut request = Vec::new();
        let mut byte = [0];
        while !request.ends_with(b"\r\n\r\n") {
            stream.read_exact(&mut byte).unwrap();
            request.push(byte[0]);
        }
        write!(stream, "HTTP/1.1 {status} Test\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).unwrap();
        String::from_utf8(request).unwrap()
    });
    (url, handle)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

    #[test]
    fn waits_through_visibility_delay_and_processing() {
        let clock = Cell::new(Duration::ZERO);
        let mut states = [State::Pending, State::Processing, State::Complete].into_iter();
        let mut record = Record::new("app", State::Uploaded);
        poll(
            &mut record,
            Duration::from_secs(30),
            Duration::from_secs(2),
            &mut |_| Ok(Record::new("app", states.next().unwrap())),
            || clock.get(),
            |d| clock.set(clock.get() + d),
        )
        .unwrap();
        assert_eq!(record.state, State::Complete);
        assert_eq!(clock.get(), Duration::from_secs(4));
    }

    #[test]
    fn failure_unknown_and_timeout_never_report_success() {
        for (state, code) in [
            (State::Failed, "STORE_PROCESSING_FAILED"),
            (State::Unknown, "STORE_PROCESSING_UNKNOWN"),
            (State::Processing, "STORE_PROCESSING_TIMEOUT"),
        ] {
            let clock = Cell::new(Duration::ZERO);
            let mut record = Record::new("app", State::Uploaded);
            let err = poll(
                &mut record,
                Duration::from_secs(3),
                Duration::from_secs(15),
                &mut |_| Ok(Record::new("app", state)),
                || clock.get(),
                |d| clock.set(clock.get() + d),
            )
            .unwrap_err();
            assert_eq!(err.downcast_ref::<ProcessingError>().unwrap().code, code);
            assert_eq!(record.state, state);
            assert!(clock.get() <= Duration::from_secs(3));
        }
    }

    #[test]
    fn late_http_success_does_not_defeat_deadline() {
        let clock = Cell::new(Duration::ZERO);
        let mut record = Record::new("app", State::Uploaded);
        let err = poll(
            &mut record,
            Duration::from_secs(3),
            Duration::from_secs(1),
            &mut |remaining| {
                assert_eq!(remaining, Duration::from_secs(3));
                clock.set(Duration::from_secs(4));
                Ok(Record::new("app", State::Complete))
            },
            || clock.get(),
            |_| panic!("must not sleep"),
        )
        .unwrap_err();
        assert_eq!(
            err.downcast_ref::<ProcessingError>().unwrap().code,
            "STORE_PROCESSING_TIMEOUT"
        );
    }

    #[test]
    fn transient_http_failure_retries_but_auth_failure_stops() {
        for (status, should_retry) in [(429, true), (503, true), (401, false), (403, false)] {
            let clock = Cell::new(Duration::ZERO);
            let calls = Cell::new(0);
            let mut record = Record::new("app", State::Uploaded);
            let result = poll(
                &mut record,
                Duration::from_secs(10),
                Duration::from_secs(1),
                &mut |_| {
                    calls.set(calls.get() + 1);
                    if calls.get() == 1 {
                        check_http_status(status)?;
                    }
                    Ok(Record::new("app", State::Complete))
                },
                || clock.get(),
                |d| clock.set(clock.get() + d),
            );
            assert_eq!(result.is_ok(), should_retry);
            assert_eq!(calls.get(), if should_retry { 2 } else { 1 });
        }
    }

    #[test]
    fn timeout_keeps_submission_identity_even_if_it_temporarily_disappears() {
        let clock = Cell::new(Duration::ZERO);
        let mut record = Record::new("app", State::Processing);
        record.submission_id = Some("new".into());
        record.version = Some("2.0".into());
        record.build_number = Some("42".into());
        let result = poll(
            &mut record,
            Duration::from_secs(1),
            Duration::from_secs(1),
            &mut |_| Ok(Record::new("app", State::Pending)),
            || clock.get(),
            |d| clock.set(clock.get() + d),
        );
        assert!(result.is_err());
        assert_eq!(record.submission_id.as_deref(), Some("new"));
        assert_eq!(record.version.as_deref(), Some("2.0"));
        assert_eq!(record.build_number.as_deref(), Some("42"));
    }

    #[test]
    fn a_different_submission_cannot_complete_the_wait() {
        let mut record = Record::new("app", State::Processing);
        record.submission_id = Some("new".into());
        let err = poll(
            &mut record,
            Duration::from_secs(1),
            Duration::from_secs(1),
            &mut |_| {
                let mut old = Record::new("app", State::Complete);
                old.submission_id = Some("old".into());
                Ok(old)
            },
            || Duration::ZERO,
            |_| panic!("must not sleep"),
        )
        .unwrap_err();
        assert_eq!(
            err.downcast_ref::<ProcessingError>().unwrap().code,
            "STORE_PROCESSING_IDENTITY_MISMATCH"
        );
        assert_eq!(record.state, State::Processing);
    }
}
