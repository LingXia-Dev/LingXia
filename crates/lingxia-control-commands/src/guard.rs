//! Explicit acknowledgement to act, and what a failure costs the caller.
//!
//! The flags prevent an accidental mutating invocation; they are not proof of
//! user consent because the caller can add them. Every mount applies the same
//! acknowledgement rule, and every failure carries a code the caller can branch
//! on instead of collapsing to prose and exit 1.

use lingxia_device_io as cu;

/// Whether this invocation may change anything.
///
/// `destructive` marks the commands that lose work rather than merely causing
/// it — closing a window, quitting an app, killing a process, clearing
/// cookies. Those need the second flag as well.
pub fn gate(allow_control: bool, destructive: bool, allow_destructive: bool) -> cu::Result<()> {
    if !(allow_control || env_flag("LXDEV_DESKTOP_ALLOW_CONTROL")) {
        return Err(cu::Error::Permission(
            "mutating command needs --allow-control (or LXDEV_DESKTOP_ALLOW_CONTROL=1)".into(),
        ));
    }
    if destructive && !(allow_destructive || env_flag("LXDEV_DESKTOP_ALLOW_DESTRUCTIVE")) {
        return Err(cu::Error::Permission(
            "destructive command needs --allow-destructive (or LXDEV_DESKTOP_ALLOW_DESTRUCTIVE=1)"
                .into(),
        ));
    }
    Ok(())
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
pub fn decode_failure(error: &anyhow::Error) -> cu::Error {
    // A failure raised on this side never crossed a transport and has its code
    // intact. Reading it out of the rendered string first would find nothing
    // and file every local refusal as a generic failure.
    if let Some(local) = error.downcast_ref::<cu::Error>() {
        return cu::Error::from_code(local.code(), local.to_string());
    }
    let text = error.to_string();
    for code in [
        cu::ErrorCode::Usage,
        cu::ErrorCode::NotFound,
        cu::ErrorCode::Ambiguous,
        cu::ErrorCode::Timeout,
        cu::ErrorCode::Permission,
        cu::ErrorCode::Unsupported,
        cu::ErrorCode::Unavailable,
        cu::ErrorCode::Stale,
        cu::ErrorCode::Failed,
    ] {
        let marker = format!("({}): ", code.as_str());
        if let Some(at) = text.find(&marker) {
            return cu::Error::from_code(code, &text[at + marker.len()..]);
        }
    }
    // The control socket's own refusals do not come from the desktop backend
    // and so carry codes of their own. A namespace the product never declared
    // is a permission answer, and saying so lets a caller stop rather than
    // retry.
    if text.contains("(not_declared): ") {
        return cu::Error::Permission(text);
    }
    cu::Error::Failed(text)
}

/// The documented exit status for a failure that crossed the transport.
pub fn exit_code(error: &anyhow::Error) -> i32 {
    decode_failure(error).exit_code()
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
        let refused: anyhow::Error = cu::Error::Permission("needs --allow-control".into()).into();
        assert_eq!(exit_code(&refused), 6);
    }
}
