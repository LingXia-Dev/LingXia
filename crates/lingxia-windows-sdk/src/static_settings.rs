use lingxia::SettingsDestinationResolution;
use lingxia_app_context::SettingsDestination;

pub(crate) const STATIC_SETTINGS_ACTION_ID: &str = "lingxia:static-settings";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StaticSettingsDestinationKind {
    ControlAppPage,
    BrowserControlPage,
    NativeAction,
}

/// Bootstrap-owned projection of the sealed Settings declaration. It retains
/// no live app, tab, session, or callback; activation always re-enters core's
/// resolver so current runtime identities are used.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct WindowsStaticSettingsSource {
    pub(crate) destination_kind: StaticSettingsDestinationKind,
}

impl WindowsStaticSettingsSource {
    pub(crate) fn from_destination(destination: Option<&SettingsDestination>) -> Option<Self> {
        let destination_kind = match destination? {
            SettingsDestination::ControlAppPage { .. } => {
                StaticSettingsDestinationKind::ControlAppPage
            }
            SettingsDestination::BrowserControlPage { .. } => {
                StaticSettingsDestinationKind::BrowserControlPage
            }
            SettingsDestination::NativeAction { .. } => StaticSettingsDestinationKind::NativeAction,
        };
        Some(Self { destination_kind })
    }

    pub(crate) fn activate<F>(&self, item_id: &str, resolver: F) -> bool
    where
        F: FnOnce() -> Result<
            SettingsDestinationResolution,
            lingxia::SettingsDestinationResolveError,
        >,
    {
        item_id == STATIC_SETTINGS_ACTION_ID && settings_resolution_succeeded(resolver())
    }

    /// Source type, not presentation strings, grants the static resolver.
    /// The reserved id is the sole merge collision a runtime item cannot own.
    pub(crate) fn accepts_runtime_action(id: &str) -> bool {
        id != STATIC_SETTINGS_ACTION_ID
    }
}

fn settings_resolution_succeeded(
    result: Result<SettingsDestinationResolution, lingxia::SettingsDestinationResolveError>,
) -> bool {
    result.is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

    fn destinations() -> [SettingsDestination; 3] {
        [
            SettingsDestination::ControlAppPage {
                app_id: "control".to_string(),
                page: "settings".to_string(),
                query: None,
            },
            SettingsDestination::BrowserControlPage {
                route: "settings".to_string(),
                query: None,
            },
            SettingsDestination::NativeAction {
                action_id: "preferences".to_string(),
            },
        ]
    }

    #[test]
    fn none_does_not_create_a_static_settings_source() {
        assert_eq!(WindowsStaticSettingsSource::from_destination(None), None);
    }

    #[test]
    fn all_three_static_descriptors_create_the_expected_source() {
        let kinds = [
            StaticSettingsDestinationKind::ControlAppPage,
            StaticSettingsDestinationKind::BrowserControlPage,
            StaticSettingsDestinationKind::NativeAction,
        ];
        for (destination, expected) in destinations().iter().zip(kinds) {
            assert_eq!(
                WindowsStaticSettingsSource::from_destination(Some(destination))
                    .map(|source| source.destination_kind),
                Some(expected)
            );
        }
    }

    #[test]
    fn static_click_accepts_every_resolver_variant() {
        let source = WindowsStaticSettingsSource {
            destination_kind: StaticSettingsDestinationKind::ControlAppPage,
        };
        let resolutions = [
            SettingsDestinationResolution::ControlAppPage {
                app_id: "control".to_string(),
                session_id: 1,
            },
            SettingsDestinationResolution::BrowserControlPage {
                tab_id: "settings".to_string(),
                browser_session_id: 2,
            },
            SettingsDestinationResolution::NativeAction {
                action_id: "preferences".to_string(),
            },
        ];
        for resolution in resolutions {
            let called = Cell::new(false);
            assert!(source.activate(STATIC_SETTINGS_ACTION_ID, || {
                called.set(true);
                Ok(resolution)
            }));
            assert!(called.get());
        }
    }

    #[test]
    fn wrong_static_id_does_not_invoke_resolver() {
        let source = WindowsStaticSettingsSource {
            destination_kind: StaticSettingsDestinationKind::NativeAction,
        };
        let called = Cell::new(false);
        assert!(!source.activate("settings", || {
            called.set(true);
            Ok(SettingsDestinationResolution::NativeAction {
                action_id: "preferences".to_string(),
            })
        }));
        assert!(!called.get());
    }

    #[test]
    fn runtime_presentation_strings_never_grant_static_authority() {
        assert!(!WindowsStaticSettingsSource::accepts_runtime_action(
            STATIC_SETTINGS_ACTION_ID
        ));
        assert!(WindowsStaticSettingsSource::accepts_runtime_action(
            "settings"
        ));
        assert!(WindowsStaticSettingsSource::accepts_runtime_action(
            "anything"
        ));

        let source = WindowsStaticSettingsSource {
            destination_kind: StaticSettingsDestinationKind::BrowserControlPage,
        };
        let called = Cell::new(false);
        assert!(!source.activate("settings", || {
            called.set(true);
            Ok(SettingsDestinationResolution::BrowserControlPage {
                tab_id: "settings".to_string(),
                browser_session_id: 1,
            })
        }));
        assert!(!called.get());
    }
}
