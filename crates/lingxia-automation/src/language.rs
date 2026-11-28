//! The run's display language. `control.locale` opens the host's test-run
//! language layer for the run, and every terminal transition closes it, so a
//! run (finished, cancelled or cut off) never changes the user's saved
//! preference; specs that set the preference only move the run's layer.

use std::sync::Mutex;

static ACTIVE: Mutex<Option<(String, lxapp::DisplayLanguageSessionOwner)>> = Mutex::new(None);

/// `'auto'` or a BCP-47 tag, as `lx.host.languagePreference.set` takes.
pub(crate) fn parse(locale: &str) -> Result<lxapp::DisplayLanguagePreference, String> {
    locale.parse().map_err(|error| {
        format!("E_INVALID_ARG: locale must be 'auto' or a language tag such as en-US: {error}")
    })
}

pub(crate) fn begin_run(run_id: &str, preference: lxapp::DisplayLanguagePreference) {
    let owner = lxapp::begin_display_language_test_run(preference);
    let previous = ACTIVE
        .lock()
        .unwrap_or_else(|err| err.into_inner())
        .replace((run_id.to_string(), owner));
    // One run at a time; a layer left by an earlier run is closed, not stacked.
    if let Some((_, stale)) = previous {
        lxapp::end_display_language_test_run(stale);
    }
}

pub(crate) fn clear_run(run_id: &str) {
    let owner = {
        let mut active = ACTIVE.lock().unwrap_or_else(|err| err.into_inner());
        match active.as_ref() {
            Some((owner_run, _)) if owner_run == run_id => active.take().map(|(_, owner)| owner),
            _ => None,
        }
    };
    if let Some(owner) = owner {
        lxapp::end_display_language_test_run(owner);
    }
}
