//! Resolved content identities carried by the surface graph.
//!
//! Authoring lookup keys are resolved before entering this crate. Keeping the
//! variants explicit prevents platform renderers from guessing whether an
//! opaque entry names an lxapp, a browser document, or a native capability.

use serde::{Deserialize, Serialize};

/// Content hosted by a surface after declaration/runtime request resolution.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "lowercase",
    rename_all_fields = "camelCase"
)]
pub enum SurfaceContent {
    Lxapp {
        app_id: String,
        /// Initial route only; later navigation belongs to the lxapp.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        path: Option<String>,
    },
    Page {
        app_id: String,
        path: String,
    },
    Browser {
        initial_url: String,
        /// Ordinary browser asides reuse an existing matching initial URL.
        /// Callback surfaces opt out to keep navigation and data isolated.
        #[serde(
            default = "default_reuse_by_url",
            skip_serializing_if = "is_reuse_by_url"
        )]
        reuse_by_url: bool,
    },
    Native {
        capability: String,
    },
}

/// Rendering-engine grouping used by adaptive aside slots.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SlotKind {
    Lxapp,
    Browser,
    Native,
}

impl SurfaceContent {
    pub fn slot_kind(&self) -> SlotKind {
        match self {
            Self::Lxapp { .. } | Self::Page { .. } => SlotKind::Lxapp,
            Self::Browser { .. } => SlotKind::Browser,
            Self::Native { .. } => SlotKind::Native,
        }
    }
}

const fn default_reuse_by_url() -> bool {
    true
}

fn is_reuse_by_url(value: &bool) -> bool {
    *value
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serialization_keeps_content_kinds_explicit() {
        let cases = [
            (
                SurfaceContent::Lxapp {
                    app_id: "home".into(),
                    path: None,
                },
                "lxapp",
            ),
            (
                SurfaceContent::Page {
                    app_id: "home".into(),
                    path: "/settings".into(),
                },
                "page",
            ),
            (
                SurfaceContent::Browser {
                    initial_url: "https://example.com".into(),
                    reuse_by_url: true,
                },
                "browser",
            ),
            (
                SurfaceContent::Native {
                    capability: "terminal".into(),
                },
                "native",
            ),
        ];

        for (content, expected_kind) in cases {
            let value = serde_json::to_value(content).unwrap();
            assert_eq!(value["kind"], expected_kind);
        }
    }

    #[test]
    fn content_kind_selects_engine_slot_without_identity_special_cases() {
        let terminal = SurfaceContent::Native {
            capability: "terminal".into(),
        };
        let editor = SurfaceContent::Native {
            capability: "editor".into(),
        };

        assert_eq!(terminal.slot_kind(), SlotKind::Native);
        assert_eq!(editor.slot_kind(), SlotKind::Native);
    }
}
