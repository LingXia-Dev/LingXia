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
    /// Pin the lxapp to the sidebar, or remove its Pin. The platform runs
    /// these: a full Pin grid is reported in platform UI.
    Pin,
    Unpin,
    /// Move the live lxapp main into the side panel (the reverse of an aside
    /// tab's "Open in Main Area"); never offered for the root main.
    OpenInSide,
    Restart,
    CleanCacheRestart,
    Uninstall,
}

/// Actions on a main-area browser tab. A browser tab is not a graph surface:
/// its menu is keyed by the browser tab id, and the platform runs every
/// action, the close family included, over its browser tabs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum BrowserSurfaceMenuAction {
    Pin,
    Unpin,
    CopyLink,
    /// Move the live tab into the side panel, keeping its WebView.
    OpenInSide,
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
    Browser {
        action: BrowserSurfaceMenuAction,
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

    pub fn browser(action: BrowserSurfaceMenuAction) -> Self {
        Self {
            action: SurfaceMenuAction::Browser { action },
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
    /// Where the content shows: "Open in Side Panel".
    Placement,
    Lifecycle,
    /// Leaves the app for good (Uninstall), after every close.
    Removal,
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

/// Provider items around the shell's own sections. Every main-area row reads
/// the same way: what the content offers, where it shows, how it closes, and
/// last what removes it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SurfaceMenuParts {
    pub content: Vec<Vec<SurfaceMenuItem>>,
    pub placement: Vec<SurfaceMenuItem>,
    pub removal: Vec<SurfaceMenuItem>,
}

pub fn compose_surface_menu(
    context: SurfaceMenuContext,
    content_groups: Vec<Vec<SurfaceMenuItem>>,
) -> SurfaceMenuSnapshot {
    compose_surface_menu_parts(
        context,
        SurfaceMenuParts {
            content: content_groups,
            ..SurfaceMenuParts::default()
        },
    )
}

pub fn compose_surface_menu_parts(
    context: SurfaceMenuContext,
    parts: SurfaceMenuParts,
) -> SurfaceMenuSnapshot {
    let mut sections = Vec::with_capacity(5);
    for items in parts.content.into_iter().filter(|items| !items.is_empty()) {
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

    if !parts.placement.is_empty() {
        sections.push(SurfaceMenuSection {
            kind: SurfaceMenuSectionKind::Placement,
            items: parts.placement,
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
    if !parts.removal.is_empty() {
        sections.push(SurfaceMenuSection {
            kind: SurfaceMenuSectionKind::Removal,
            items: parts.removal,
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

    #[test]
    fn placement_sits_between_content_and_closes_and_removal_comes_last() {
        let mut uninstall = SurfaceMenuItem::lxapp(LxappSurfaceMenuAction::Uninstall);
        uninstall.role = SurfaceMenuItemRole::Destructive;
        let snapshot = compose_surface_menu_parts(
            SurfaceMenuContext {
                revision: 4,
                surface_id: "guest".into(),
                closable: true,
                renameable: false,
                title_overridden: false,
                has_other_closable: true,
                has_closable_before: true,
                has_closable_after: true,
            },
            SurfaceMenuParts {
                content: vec![vec![SurfaceMenuItem::lxapp(
                    LxappSurfaceMenuAction::Restart,
                )]],
                placement: vec![SurfaceMenuItem::lxapp(LxappSurfaceMenuAction::OpenInSide)],
                removal: vec![uninstall],
            },
        );

        assert_eq!(
            snapshot
                .sections
                .iter()
                .map(|section| section.kind)
                .collect::<Vec<_>>(),
            vec![
                SurfaceMenuSectionKind::Content,
                SurfaceMenuSectionKind::Placement,
                SurfaceMenuSectionKind::Lifecycle,
                SurfaceMenuSectionKind::Removal,
            ]
        );
    }

    #[test]
    fn browser_action_serializes_under_its_own_owner() {
        let item = SurfaceMenuItem::browser(BrowserSurfaceMenuAction::CopyLink);
        let json = serde_json::to_value(&item).unwrap();

        assert_eq!(json["action"]["owner"], "browser");
        assert_eq!(json["action"]["action"], "copyLink");
    }
}
