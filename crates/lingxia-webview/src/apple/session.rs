//! The login session an automation run shares with the person at the Mac.
//! Public API only.

use objc2_core_foundation::{CFBoolean, CFDictionary, CFString, CFType};

/// The key `CGSessionCopyCurrentDictionary` sets while the session's screen
/// is locked; absent otherwise.
const SCREEN_IS_LOCKED_KEY: &str = "CGSSessionScreenIsLocked";

/// Whether this process's login session has its screen locked, or `None`
/// when the process has no window-server session to ask about.
///
/// A locked session keeps running, but WebKit treats every page as hidden:
/// animation frames stop and sheets never slide in.
pub fn screen_locked() -> Option<bool> {
    let session = objc2_core_graphics::CGSessionCopyCurrentDictionary()?;
    Some(screen_locked_in(&session))
}

fn screen_locked_in(session: &CFDictionary) -> bool {
    // SAFETY: the session dictionary's keys are strings and its values are
    // CoreFoundation objects; the value is type-checked before use.
    let session: &CFDictionary<CFString, CFType> = unsafe { session.cast_unchecked() };
    session
        .get(&CFString::from_static_str(SCREEN_IS_LOCKED_KEY))
        .and_then(|value| value.downcast::<CFBoolean>().ok())
        .is_some_and(|locked| locked.as_bool())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn session(
        entries: &[(&'static str, &CFType)],
    ) -> objc2_core_foundation::CFRetained<CFDictionary> {
        let keys: Vec<_> = entries
            .iter()
            .map(|(key, _)| CFString::from_static_str(key))
            .collect();
        let key_refs: Vec<&CFString> = keys.iter().map(|key| &**key).collect();
        let values: Vec<&CFType> = entries.iter().map(|(_, value)| *value).collect();
        let typed = CFDictionary::<CFString, CFType>::from_slices(&key_refs, &values);
        // SAFETY: an untyped view of the same dictionary.
        unsafe { objc2_core_foundation::CFRetained::cast_unchecked(typed) }
    }

    #[test]
    fn a_session_says_locked_only_through_its_lock_key() {
        let locked = session(&[(SCREEN_IS_LOCKED_KEY, &**CFBoolean::new(true))]);
        assert!(screen_locked_in(&locked));

        let unlocked = session(&[(SCREEN_IS_LOCKED_KEY, &**CFBoolean::new(false))]);
        assert!(!screen_locked_in(&unlocked));

        // The key is absent while the screen is unlocked.
        let absent = session(&[("kCGSSessionOnConsoleKey", &**CFBoolean::new(true))]);
        assert!(!screen_locked_in(&absent));
    }

    #[test]
    fn a_lock_key_of_another_type_is_not_a_lock() {
        let odd = CFString::from_static_str("yes");
        let odd = session(&[(SCREEN_IS_LOCKED_KEY, &**odd)]);
        assert!(!screen_locked_in(&odd));
    }
}
