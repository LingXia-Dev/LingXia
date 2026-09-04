//! Bounded predecode and diagnostics for hostile browser-document ingress.

use lingxia_webview::LogLevel;
use serde::Deserialize;
use std::sync::Mutex;
use std::time::{Duration, Instant};

pub(crate) const MAX_BROWSER_INBOUND_BYTES: usize = 64 * 1024;
const MAX_BINDING_FIELD_BYTES: usize = 512;
const CONSOLE_WINDOW: Duration = Duration::from_secs(1);
const CONSOLE_MESSAGES_PER_WINDOW: u32 = 32;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(usize)]
pub(crate) enum BrowserInboundRejectReason {
    Oversized = 0,
    MalformedEnvelope = 1,
    UnsupportedVersion = 2,
    UnsupportedKind = 3,
    WrongBinding = 4,
    WrongNativeView = 5,
    StaleGeneration = 6,
    ChildFrame = 7,
    UnprovenFrame = 8,
    UnsupportedTransport = 9,
    AndroidLegacyDegraded = 10,
    SessionNotReady = 11,
    ConsoleRateLimited = 12,
    DispatchFailed = 13,
}

impl BrowserInboundRejectReason {
    const COUNT: usize = 14;

    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::Oversized => "oversized",
            Self::MalformedEnvelope => "malformed_envelope",
            Self::UnsupportedVersion => "unsupported_version",
            Self::UnsupportedKind => "unsupported_kind",
            Self::WrongBinding => "wrong_binding",
            Self::WrongNativeView => "wrong_native_view",
            Self::StaleGeneration => "stale_generation",
            Self::ChildFrame => "child_frame",
            Self::UnprovenFrame => "unproven_frame",
            Self::UnsupportedTransport => "unsupported_transport",
            Self::AndroidLegacyDegraded => "android_21_22_unproven_transport",
            Self::SessionNotReady => "session_not_ready",
            Self::ConsoleRateLimited => "console_rate_limited",
            Self::DispatchFailed => "dispatch_failed",
        }
    }
}

#[derive(Default)]
pub(crate) struct BrowserInboundDiagnostics {
    counters: Mutex<[u64; BrowserInboundRejectReason::COUNT]>,
}

impl BrowserInboundDiagnostics {
    pub(crate) fn reject(&self, reason: BrowserInboundRejectReason) {
        let count = {
            let mut counters = self
                .counters
                .lock()
                .unwrap_or_else(|error| error.into_inner());
            let counter = &mut counters[reason as usize];
            *counter = counter.saturating_add(1);
            *counter
        };
        if should_sample(count) {
            // Never include the frame, reported URL, public session id, or
            // secret in rejection diagnostics.
            lxapp::warn!(
                "[BrowserInbound] rejected reason={} count={}",
                reason.as_str(),
                count
            );
        }
    }

    #[cfg(test)]
    pub(crate) fn count(&self, reason: BrowserInboundRejectReason) -> u64 {
        self.counters
            .lock()
            .unwrap_or_else(|error| error.into_inner())[reason as usize]
    }
}

fn should_sample(count: u64) -> bool {
    count == 1 || count.is_power_of_two()
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum BrowserV3InboundKind {
    Hello,
    Frame,
}

pub(crate) struct BorrowedBrowserBinding<'a> {
    pub(crate) session_id: &'a str,
    pub(crate) secret: &'a str,
}

pub(crate) enum BorrowedBrowserEnvelope<'a> {
    Bridge {
        kind: BrowserV3InboundKind,
        binding: BorrowedBrowserBinding<'a>,
    },
    Console {
        binding: BorrowedBrowserBinding<'a>,
        level: LogLevel,
        message: &'a str,
    },
}

#[derive(Deserialize)]
struct BorrowedEnvelopeProbe<'a> {
    v: Option<u8>,
    kind: Option<&'a str>,
    #[serde(rename = "sessionId")]
    session_id: Option<&'a str>,
    secret: Option<&'a str>,
    #[serde(rename = "__lingxia_console__")]
    console: Option<bool>,
    level: Option<&'a str>,
    message: Option<&'a str>,
}

/// Read only fixed routing and binding fields before the full typed codec.
/// Payload fields are skipped without building a hostile `serde_json::Value`.
pub(crate) fn parse_browser_envelope(
    frame: &str,
) -> Result<BorrowedBrowserEnvelope<'_>, BrowserInboundRejectReason> {
    if frame.len() > MAX_BROWSER_INBOUND_BYTES {
        return Err(BrowserInboundRejectReason::Oversized);
    }
    let probe = serde_json::from_str::<BorrowedEnvelopeProbe<'_>>(frame)
        .map_err(|_| BrowserInboundRejectReason::MalformedEnvelope)?;
    if probe.v != Some(3) {
        return Err(BrowserInboundRejectReason::UnsupportedVersion);
    }
    let session_id =
        bounded_nonempty(probe.session_id).ok_or(BrowserInboundRejectReason::MalformedEnvelope)?;
    let secret =
        bounded_nonempty(probe.secret).ok_or(BrowserInboundRejectReason::MalformedEnvelope)?;
    let binding = BorrowedBrowserBinding { session_id, secret };

    if probe.console == Some(true) {
        if probe.kind != Some("console") {
            return Err(BrowserInboundRejectReason::UnsupportedKind);
        }
        let message = probe
            .message
            .ok_or(BrowserInboundRejectReason::MalformedEnvelope)?;
        return Ok(BorrowedBrowserEnvelope::Console {
            binding,
            level: parse_console_level(probe.level),
            message,
        });
    }

    let kind = match probe.kind {
        Some("hello") => BrowserV3InboundKind::Hello,
        Some(
            "req" | "res" | "notify" | "cancel" | "ch.open" | "ch.data" | "ch.close" | "state.ack",
        ) => BrowserV3InboundKind::Frame,
        _ => return Err(BrowserInboundRejectReason::UnsupportedKind),
    };
    Ok(BorrowedBrowserEnvelope::Bridge { kind, binding })
}

fn bounded_nonempty(value: Option<&str>) -> Option<&str> {
    value.filter(|value| !value.is_empty() && value.len() <= MAX_BINDING_FIELD_BYTES)
}

fn parse_console_level(level: Option<&str>) -> LogLevel {
    match level {
        Some("error") => LogLevel::Error,
        Some("warn") => LogLevel::Warn,
        Some("debug") => LogLevel::Debug,
        Some("verbose") => LogLevel::Verbose,
        _ => LogLevel::Info,
    }
}

pub(crate) struct ConsoleRateLimiter {
    window_started: Instant,
    accepted: u32,
}

impl Default for ConsoleRateLimiter {
    fn default() -> Self {
        Self {
            window_started: Instant::now(),
            accepted: 0,
        }
    }
}

impl ConsoleRateLimiter {
    pub(crate) fn allow(&mut self) -> bool {
        self.allow_at(Instant::now())
    }

    fn allow_at(&mut self, now: Instant) -> bool {
        if now.saturating_duration_since(self.window_started) >= CONSOLE_WINDOW {
            self.window_started = now;
            self.accepted = 0;
        }
        if self.accepted >= CONSOLE_MESSAGES_PER_WINDOW {
            return false;
        }
        self.accepted += 1;
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn borrowed_probe_reads_only_fixed_bridge_fields() {
        let frame = r#"{"v":3,"kind":"req","sessionId":"session","secret":"secret","params":{"large":[1,2,3]}}"#;
        let parsed = parse_browser_envelope(frame).expect("valid envelope");
        let BorrowedBrowserEnvelope::Bridge { kind, binding } = parsed else {
            panic!("expected bridge envelope");
        };
        assert_eq!(kind, BrowserV3InboundKind::Frame);
        assert_eq!(binding.session_id, "session");
        assert_eq!(binding.secret, "secret");
    }

    #[test]
    fn oversized_and_malformed_envelopes_have_distinct_reasons() {
        let oversized = "x".repeat(MAX_BROWSER_INBOUND_BYTES + 1);
        assert!(matches!(
            parse_browser_envelope(&oversized),
            Err(BrowserInboundRejectReason::Oversized)
        ));
        assert!(matches!(
            parse_browser_envelope("{"),
            Err(BrowserInboundRejectReason::MalformedEnvelope)
        ));
    }

    #[test]
    fn console_requires_a_v3_binding_and_fixed_console_kind() {
        assert!(matches!(
            parse_browser_envelope(
                r#"{"__lingxia_console__":true,"level":"info","message":"forged"}"#
            ),
            Err(BrowserInboundRejectReason::UnsupportedVersion)
        ));
        let parsed = parse_browser_envelope(
            r#"{"v":3,"kind":"console","sessionId":"session","secret":"secret","__lingxia_console__":true,"level":"warn","message":"safe"}"#,
        )
        .expect("bound console envelope");
        assert!(matches!(
            parsed,
            BorrowedBrowserEnvelope::Console {
                level: LogLevel::Warn,
                message: "safe",
                ..
            }
        ));
    }

    #[test]
    fn console_spam_is_limited_per_window() {
        let started = Instant::now();
        let mut limiter = ConsoleRateLimiter {
            window_started: started,
            accepted: 0,
        };
        for _ in 0..CONSOLE_MESSAGES_PER_WINDOW {
            assert!(limiter.allow_at(started));
        }
        assert!(!limiter.allow_at(started));
        assert!(limiter.allow_at(started + CONSOLE_WINDOW));
    }

    #[test]
    fn rejection_counters_are_reason_coded_and_sampling_is_bounded() {
        let diagnostics = BrowserInboundDiagnostics::default();
        diagnostics.reject(BrowserInboundRejectReason::WrongBinding);
        diagnostics.reject(BrowserInboundRejectReason::WrongBinding);
        diagnostics.reject(BrowserInboundRejectReason::MalformedEnvelope);
        diagnostics.reject(BrowserInboundRejectReason::StaleGeneration);
        diagnostics.reject(BrowserInboundRejectReason::AndroidLegacyDegraded);
        assert_eq!(
            diagnostics.count(BrowserInboundRejectReason::WrongBinding),
            2
        );
        assert_eq!(
            diagnostics.count(BrowserInboundRejectReason::MalformedEnvelope),
            1
        );
        assert_eq!(
            diagnostics.count(BrowserInboundRejectReason::StaleGeneration),
            1
        );
        assert_eq!(
            diagnostics.count(BrowserInboundRejectReason::AndroidLegacyDegraded),
            1
        );
        assert!(should_sample(1));
        assert!(should_sample(2));
        assert!(!should_sample(3));
        assert!(should_sample(4));
    }
}
