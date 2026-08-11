//! Explicit acknowledgement to act, and what a failure costs the caller.
//!
//! The flags prevent an accidental mutating invocation; they are not proof of
//! user consent because the caller can add them. Every mount applies the same
//! acknowledgement rule, and every failure carries a code the caller can branch
//! on instead of collapsing to prose and exit 1.

#[cfg(feature = "desktop")]
use lingxia_device_io as device_io;

const ENCODED_EXIT_CODES: [(&str, i32); 9] = [
    ("usage", 2),
    ("not_found", 3),
    ("ambiguous", 4),
    ("timeout", 5),
    ("permission", 6),
    ("unsupported", 7),
    ("unavailable", 8),
    ("stale", 9),
    ("failed", 10),
];

/// A local command was refused because its explicit safety acknowledgement
/// was missing. This error belongs to the command surface, not device I/O.
#[derive(Debug, thiserror::Error)]
#[error("{message}")]
pub struct GuardError {
    message: String,
}

impl GuardError {
    fn permission(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

/// Whether this invocation may change anything.
///
/// `destructive` marks the commands that lose work rather than merely causing
/// it — closing a window, quitting an app, killing a process, clearing
/// cookies. Those need the second flag as well.
pub fn gate(
    allow_control: bool,
    destructive: bool,
    allow_destructive: bool,
) -> Result<(), GuardError> {
    if !(allow_control || env_flag("LXDEV_DESKTOP_ALLOW_CONTROL")) {
        return Err(GuardError::permission(
            "mutating command needs --allow-control (or LXDEV_DESKTOP_ALLOW_CONTROL=1)",
        ));
    }
    if destructive && !(allow_destructive || env_flag("LXDEV_DESKTOP_ALLOW_DESTRUCTIVE")) {
        return Err(GuardError::permission(
            "destructive command needs --allow-destructive (or LXDEV_DESKTOP_ALLOW_DESTRUCTIVE=1)",
        ));
    }
    Ok(())
}

/// Apply the command guard at a desktop call site while preserving the
/// device-I/O error contract used by JSON output and process exit codes.
#[cfg(feature = "desktop")]
pub(crate) fn desktop_gate(
    allow_control: bool,
    destructive: bool,
    allow_destructive: bool,
) -> device_io::Result<()> {
    gate(allow_control, destructive, allow_destructive)
        .map_err(|error| device_io::Error::Permission(error.to_string()))
}

pub fn env_flag(name: &str) -> bool {
    std::env::var(name).is_ok_and(|value| {
        let value = value.trim();
        !(value.is_empty() || value == "0" || value.eq_ignore_ascii_case("false"))
    })
}

/// Rebuild the host's error from what crossed the transport.
///
/// The transport hands back a rendered string, but the code in front of it is
/// the contract — it decides the exit status, so a failure that arrives as
/// plain prose has already lost.
#[cfg(feature = "desktop")]
pub(crate) fn decode_failure(error: &anyhow::Error) -> device_io::Error {
    // A failure raised on this side never crossed a transport and has its code
    // intact. Reading it out of the rendered string first would find nothing
    // and file every local refusal as a generic failure.
    if let Some(local) = error.downcast_ref::<device_io::Error>() {
        return device_io::Error::from_code(local.code(), local.to_string());
    }
    let text = error.to_string();
    if let Some((slug, message, _)) = encoded_failure(&text) {
        let code = device_io::ErrorCode::parse(slug).expect("known encoded device-I/O error code");
        return device_io::Error::from_code(code, message);
    }
    // The control socket's own refusals do not come from the desktop backend
    // and so carry codes of their own. A namespace the product never declared
    // is a permission answer, and saying so lets a caller stop rather than
    // retry.
    if text.contains("(not_declared): ") {
        return device_io::Error::Permission(text);
    }
    device_io::Error::Failed(text)
}

/// The documented exit status for a failure that crossed the transport.
pub fn exit_code(error: &anyhow::Error) -> i32 {
    if error.downcast_ref::<GuardError>().is_some() {
        return 6;
    }
    #[cfg(feature = "desktop")]
    if let Some(local) = error.downcast_ref::<device_io::Error>() {
        return local.exit_code();
    }
    let text = error.to_string();
    encoded_failure(&text)
        .map(|(_, _, exit_code)| exit_code)
        .or_else(|| text.contains("(not_declared): ").then_some(6))
        .unwrap_or(10)
}

fn encoded_failure(text: &str) -> Option<(&'static str, &str, i32)> {
    ENCODED_EXIT_CODES.iter().find_map(|&(slug, exit_code)| {
        let marker = format!("({slug}): ");
        text.find(&marker)
            .map(|at| (slug, &text[at + marker.len()..], exit_code))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// An agent branches on these. Collapsing them to `1` means "something went
    /// wrong, read the English" — which is how a refusal gets retried forever.
    #[test]
    fn a_failure_keeps_the_code_it_crossed_with() {
        let refused = anyhow::anyhow!(
            "browser.open failed (not_declared): browserUse is not declared by this product"
        );
        assert_eq!(
            exit_code(&refused),
            6,
            "an undeclared namespace is a permission answer"
        );

        let missing = anyhow::anyhow!("desktop.window.focus failed (not_found): no such window");
        assert_eq!(exit_code(&missing), 3);

        // Anything with no code at all is still a failure, not a success.
        let bare = anyhow::anyhow!("the socket went away");
        assert_eq!(exit_code(&bare), 10);

        // A refusal raised locally never crossed a transport, so there is no
        // marker in its text — but it is still a permission answer, and an
        // agent branching on the code must see one.
        let refused: anyhow::Error = GuardError::permission("needs --allow-control").into();
        assert_eq!(exit_code(&refused), 6);
    }

    #[cfg(feature = "desktop")]
    #[test]
    fn a_local_device_error_keeps_its_exit_code() {
        let missing: anyhow::Error = device_io::Error::NotFound("no such window".into()).into();
        assert_eq!(exit_code(&missing), 3);
    }
}
