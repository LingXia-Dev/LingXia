//! What the user last did to the sidebar, remembered across launches.
//!
//! Only the user's own choice lives here. The adaptive projection — the icon
//! rail a narrow window forces — is derived from the window every launch and
//! must never be written down, or a window that was briefly narrow would teach
//! the app to open as a rail forever.

use serde::{Deserialize, Serialize};

/// How wide the user left the sidebar.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum SidebarMode {
    #[default]
    Expanded,
    /// Icon-only column.
    Rail,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct SidebarChrome {
    pub mode: SidebarMode,
}

impl SidebarChrome {
    pub fn rail(&self) -> bool {
        self.mode == SidebarMode::Rail
    }

    pub fn with_rail(rail: bool) -> Self {
        Self {
            mode: if rail {
                SidebarMode::Rail
            } else {
                SidebarMode::Expanded
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Never having chosen is the expanded case — a first launch shows the
    /// whole sidebar rather than a rail nobody asked for.
    #[test]
    fn the_default_is_expanded() {
        assert_eq!(SidebarChrome::default().mode, SidebarMode::Expanded);
        assert!(!SidebarChrome::default().rail());
    }

    #[test]
    fn a_mode_round_trips_through_json() {
        for rail in [false, true] {
            let saved = SidebarChrome::with_rail(rail);
            let raw = serde_json::to_string(&saved).expect("serialize");
            let loaded: SidebarChrome = serde_json::from_str(&raw).expect("deserialize");
            assert_eq!(loaded, saved);
            assert_eq!(loaded.rail(), rail);
        }
    }

    /// A file written by a build that knew other modes — macOS once persisted
    /// `hidden` — must not brick the sidebar. Unknown values read as the
    /// default, which is the state a user can always get out of.
    #[test]
    fn an_unknown_mode_reads_as_expanded() {
        let loaded: SidebarChrome =
            serde_json::from_str(r#"{"mode":"hidden"}"#).unwrap_or_default();
        assert_eq!(loaded.mode, SidebarMode::Expanded);
    }
}
