//! What a caught panic said and where, for a dev session's failure text.
use std::cell::RefCell;

thread_local! {
    static LOCATION: RefCell<Option<String>> = const { RefCell::new(None) };
}

/// Records where this thread's panics happen, under a dev session only. Chained in front of
/// the process's hook on the first worker job: the embedder installs its own before any page.
pub(super) fn watch() {
    static HOOK: std::sync::Once = std::sync::Once::new();
    if !crate::engine::dev_globals() {
        return;
    }
    HOOK.call_once(|| {
        let previous = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            let location = info
                .location()
                .map(|at| format!("{}:{}", at.file(), at.line()));
            LOCATION.with(|slot| *slot.borrow_mut() = location);
            previous(info);
        }));
    });
}

/// The failure a caught panic becomes: the short form, or with its message and location
/// under a dev session.
pub(super) fn describe(panic: Box<dyn std::any::Any + Send>) -> String {
    let location = LOCATION.with(|slot| slot.borrow_mut().take());
    if !crate::engine::dev_globals() {
        return crate::session::PANIC.to_owned();
    }
    let message = panic
        .downcast_ref::<&str>()
        .map(|message| (*message).to_owned())
        .or_else(|| panic.downcast_ref::<String>().cloned());
    text(message.as_deref(), location.as_deref())
}

fn text(message: Option<&str>, location: Option<&str>) -> String {
    let mut text = crate::session::PANIC.to_owned();
    if let Some(message) = message {
        text.push_str(": ");
        text.push_str(message);
    }
    if let Some(location) = location {
        text.push_str(" at ");
        text.push_str(location);
    }
    text
}

#[cfg(test)]
mod tests {
    #[test]
    fn a_dev_failure_names_the_panic_and_where_it_was() {
        assert_eq!(
            super::text(Some("min > max"), Some("core/f32.rs:1565")),
            "UI_RENDER: native panic: min > max at core/f32.rs:1565"
        );
        assert_eq!(super::text(None, None), crate::session::PANIC);
        // Outside a dev session the payload is not read.
        assert_eq!(super::describe(Box::new("secret")), crate::session::PANIC);
    }
}
