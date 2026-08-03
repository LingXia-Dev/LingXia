//! Cross-platform menu contract for switchable main surfaces.
//!
//! Content providers contribute resolved actions. The shell appends common
//! management/lifecycle actions in a deterministic order. Platform SDKs render
//! the snapshot and return an intent; they never execute provider behavior.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SurfaceMenuBuiltinAction {
    Rename,
    ResetTitle,
    Close,
    CloseOthers,
    CloseAfter,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum LxappSurfaceMenuAction {
    Restart,
    CleanCacheRestart,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "owner",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum SurfaceMenuAction {
    Information {},
    Switcher {
        action: SurfaceMenuBuiltinAction,
    },
    Lxapp {
        action: LxappSurfaceMenuAction,
    },
    External {
        namespace: String,
        generation: u64,
        action_id: String,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SurfaceMenuItemRole {
    Normal,
    Destructive,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SurfaceMenuItem {
    pub action: SurfaceMenuAction,
    /// Built-ins are localized by semantic action. External actions carry
    /// their provider-resolved label here.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    /// Resolved local icon reference or a shared built-in icon name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
    pub enabled: bool,
    pub role: SurfaceMenuItemRole,
}

impl SurfaceMenuItem {
    pub fn information(label: impl Into<String>) -> Self {
        Self {
            action: SurfaceMenuAction::Information {},
            label: Some(label.into()),
            icon: None,
            enabled: false,
            role: SurfaceMenuItemRole::Normal,
        }
    }

    pub fn lxapp(action: LxappSurfaceMenuAction) -> Self {
        Self {
            action: SurfaceMenuAction::Lxapp { action },
            label: None,
            icon: None,
            enabled: true,
            role: SurfaceMenuItemRole::Normal,
        }
    }

    pub fn external(
        namespace: impl Into<String>,
        generation: u64,
        action_id: impl Into<String>,
        label: impl Into<String>,
        icon: Option<String>,
    ) -> Self {
        Self {
            action: SurfaceMenuAction::External {
                namespace: namespace.into(),
                generation,
                action_id: action_id.into(),
            },
            label: Some(label.into()),
            icon,
            enabled: true,
            role: SurfaceMenuItemRole::Normal,
        }
    }

    fn built_in(action: SurfaceMenuBuiltinAction, role: SurfaceMenuItemRole) -> Self {
        Self {
            action: SurfaceMenuAction::Switcher { action },
            label: None,
            icon: None,
            enabled: true,
            role,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SurfaceMenuSectionKind {
    Content,
    Management,
    Lifecycle,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SurfaceMenuSection {
    pub kind: SurfaceMenuSectionKind,
    pub items: Vec<SurfaceMenuItem>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SurfaceMenuSnapshot {
    pub revision: u64,
    pub surface_id: String,
    pub sections: Vec<SurfaceMenuSection>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SurfaceMenuContext {
    pub revision: u64,
    pub surface_id: String,
    pub closable: bool,
    pub renameable: bool,
    pub title_overridden: bool,
    pub has_other_closable: bool,
    pub has_closable_before: bool,
    pub has_closable_after: bool,
}

pub fn compose_surface_menu(
    context: SurfaceMenuContext,
    content_groups: Vec<Vec<SurfaceMenuItem>>,
) -> SurfaceMenuSnapshot {
    let mut sections = Vec::with_capacity(3);
    for items in content_groups.into_iter().filter(|items| !items.is_empty()) {
        sections.push(SurfaceMenuSection {
            kind: SurfaceMenuSectionKind::Content,
            items,
        });
    }

    let mut management = Vec::new();
    if context.renameable {
        management.push(SurfaceMenuItem::built_in(
            SurfaceMenuBuiltinAction::Rename,
            SurfaceMenuItemRole::Normal,
        ));
        if context.title_overridden {
            management.push(SurfaceMenuItem::built_in(
                SurfaceMenuBuiltinAction::ResetTitle,
                SurfaceMenuItemRole::Normal,
            ));
        }
    }
    if !management.is_empty() {
        sections.push(SurfaceMenuSection {
            kind: SurfaceMenuSectionKind::Management,
            items: management,
        });
    }

    let mut lifecycle = Vec::new();
    if context.closable {
        lifecycle.push(SurfaceMenuItem::built_in(
            SurfaceMenuBuiltinAction::Close,
            SurfaceMenuItemRole::Destructive,
        ));
    }
    if context.has_other_closable {
        lifecycle.push(SurfaceMenuItem::built_in(
            SurfaceMenuBuiltinAction::CloseOthers,
            SurfaceMenuItemRole::Destructive,
        ));
    }
    if context.has_closable_before && context.has_closable_after {
        lifecycle.push(SurfaceMenuItem::built_in(
            SurfaceMenuBuiltinAction::CloseAfter,
            SurfaceMenuItemRole::Destructive,
        ));
    }
    if !lifecycle.is_empty() {
        sections.push(SurfaceMenuSection {
            kind: SurfaceMenuSectionKind::Lifecycle,
            items: lifecycle,
        });
    }

    SurfaceMenuSnapshot {
        revision: context.revision,
        surface_id: context.surface_id,
        sections,
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SurfaceMenuIntent {
    pub revision: u64,
    pub surface_id: String,
    pub action: SurfaceMenuAction,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub value: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn actions(snapshot: &SurfaceMenuSnapshot) -> Vec<SurfaceMenuAction> {
        snapshot
            .sections
            .iter()
            .flat_map(|section| section.items.iter())
            .map(|item| item.action.clone())
            .collect()
    }

    #[test]
    fn root_deduplicates_equivalent_batch_actions() {
        let snapshot = compose_surface_menu(
            SurfaceMenuContext {
                revision: 7,
                surface_id: "home".into(),
                closable: false,
                renameable: false,
                title_overridden: false,
                has_other_closable: true,
                has_closable_before: false,
                has_closable_after: true,
            },
            Vec::new(),
        );

        let actions = actions(&snapshot);
        assert!(!actions.contains(&SurfaceMenuAction::Switcher {
            action: SurfaceMenuBuiltinAction::Close,
        }));
        assert!(actions.contains(&SurfaceMenuAction::Switcher {
            action: SurfaceMenuBuiltinAction::CloseOthers,
        }));
        assert!(!actions.contains(&SurfaceMenuAction::Switcher {
            action: SurfaceMenuBuiltinAction::CloseAfter,
        }));
    }

    #[test]
    fn provider_actions_precede_shell_management_and_lifecycle() {
        let more_action = SurfaceMenuItem::external(
            "lxapp:home",
            12,
            "feedback",
            "Feedback",
            Some("lx://bundle/feedback.svg".into()),
        );
        let snapshot = compose_surface_menu(
            SurfaceMenuContext {
                revision: 9,
                surface_id: "terminal".into(),
                closable: true,
                renameable: true,
                title_overridden: true,
                has_other_closable: true,
                has_closable_before: true,
                has_closable_after: false,
            },
            vec![vec![more_action]],
        );

        assert_eq!(
            snapshot
                .sections
                .iter()
                .map(|section| section.kind)
                .collect::<Vec<_>>(),
            vec![
                SurfaceMenuSectionKind::Content,
                SurfaceMenuSectionKind::Management,
                SurfaceMenuSectionKind::Lifecycle,
            ]
        );
        assert_eq!(snapshot.revision, 9);
        assert_eq!(snapshot.surface_id, "terminal");
    }

    #[test]
    fn middle_surface_keeps_distinct_batch_actions() {
        let snapshot = compose_surface_menu(
            SurfaceMenuContext {
                revision: 10,
                surface_id: "middle".into(),
                closable: true,
                renameable: false,
                title_overridden: false,
                has_other_closable: true,
                has_closable_before: true,
                has_closable_after: true,
            },
            Vec::new(),
        );

        let actions = actions(&snapshot);
        assert!(actions.contains(&SurfaceMenuAction::Switcher {
            action: SurfaceMenuBuiltinAction::CloseOthers,
        }));
        assert!(actions.contains(&SurfaceMenuAction::Switcher {
            action: SurfaceMenuBuiltinAction::CloseAfter,
        }));
    }

    #[test]
    fn external_action_keeps_generation_for_stale_intent_rejection() {
        let item = SurfaceMenuItem::external("browser", 41, "copy-link", "Copy Link", None);
        let json = serde_json::to_value(&item).unwrap();

        assert_eq!(json["action"]["owner"], "external");
        assert_eq!(json["action"]["namespace"], "browser");
        assert_eq!(json["action"]["generation"], 41);
        assert_eq!(json["action"]["actionId"], "copy-link");
    }

    #[test]
    fn lxapp_groups_keep_metadata_maintenance_and_more_actions_separate() {
        let snapshot = compose_surface_menu(
            SurfaceMenuContext {
                revision: 3,
                surface_id: "home".into(),
                closable: false,
                renameable: false,
                title_overridden: false,
                has_other_closable: false,
                has_closable_before: false,
                has_closable_after: false,
            },
            vec![
                vec![SurfaceMenuItem::information("Showcase · 1.0.0 [DEV]")],
                vec![
                    SurfaceMenuItem::lxapp(LxappSurfaceMenuAction::Restart),
                    SurfaceMenuItem::lxapp(LxappSurfaceMenuAction::CleanCacheRestart),
                ],
                vec![SurfaceMenuItem::external(
                    "showcase", 2, "0", "Feedback", None,
                )],
            ],
        );

        assert_eq!(snapshot.sections.len(), 3);
        assert!(snapshot.sections[0].items.iter().all(|item| !item.enabled));
        assert!(matches!(
            snapshot.sections[1].items[0].action,
            SurfaceMenuAction::Lxapp {
                action: LxappSurfaceMenuAction::Restart
            }
        ));
        assert!(matches!(
            snapshot.sections[2].items[0].action,
            SurfaceMenuAction::External { .. }
        ));
    }
}
