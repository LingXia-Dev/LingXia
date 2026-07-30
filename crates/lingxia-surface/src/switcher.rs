//! Platform-neutral projection of ordered `main` surfaces.
//!
//! Desktop skins may render this as sidebar tabs while compact skins may not
//! render a switcher at all. The identity and lifecycle semantics stay the
//! same in both cases.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::{SurfaceContent, SurfaceGraph, SurfaceIcon, SurfaceId, SurfacePresentation};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum SwitcherContentKind {
    Lxapp,
    Page,
    Browser,
    Native { capability: String },
}

impl From<&SurfaceContent> for SwitcherContentKind {
    fn from(content: &SurfaceContent) -> Self {
        match content {
            SurfaceContent::Lxapp { .. } => Self::Lxapp,
            SurfaceContent::Page { .. } => Self::Page,
            SurfaceContent::Browser { .. } => Self::Browser,
            SurfaceContent::Native { capability } => Self::Native {
                capability: capability.clone(),
            },
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SurfaceSwitcherItem {
    pub surface_id: SurfaceId,
    pub content: SwitcherContentKind,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub icon: Option<SurfaceIcon>,
    pub active: bool,
    pub root: bool,
    pub closable: bool,
    pub renameable: bool,
    pub title_overridden: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SurfaceSwitcherSnapshot {
    pub revision: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub root_surface_id: Option<SurfaceId>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub active_surface_id: Option<SurfaceId>,
    pub items: Vec<SurfaceSwitcherItem>,
}

impl SurfaceSwitcherSnapshot {
    pub(crate) fn derive(
        graph: &SurfaceGraph,
        presentations: &HashMap<SurfaceId, SurfacePresentation>,
    ) -> Self {
        let root_surface_id = graph.root_main_id().map(str::to_string);
        let active_surface_id = graph.active_main_id.clone();
        let items = graph
            .mains()
            .into_iter()
            .map(|surface| {
                let presentation = presentations
                    .get(&surface.id)
                    .cloned()
                    .unwrap_or_else(|| SurfacePresentation::for_content(&surface.content));
                let root = root_surface_id.as_deref() == Some(surface.id.as_str());
                SurfaceSwitcherItem {
                    surface_id: surface.id.clone(),
                    content: (&surface.content).into(),
                    title: presentation.title().map(str::to_string),
                    icon: presentation.icon,
                    active: active_surface_id.as_deref() == Some(surface.id.as_str()),
                    root,
                    closable: !root && presentation.capabilities.close,
                    renameable: presentation.capabilities.rename,
                    title_overridden: presentation.custom_title.is_some(),
                }
            })
            .collect();
        Self {
            revision: 0,
            root_surface_id,
            active_surface_id,
            items,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CloseOutcome {
    Closed { removed: Vec<SurfaceId> },
    RejectedRoot { surface_id: SurfaceId },
    NotFound,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ReplaceMainsError {
    #[error("surface '{surface_id}' is not a main")]
    InvalidRole { surface_id: SurfaceId },
    #[error("duplicate main surface id '{surface_id}'")]
    DuplicateId { surface_id: SurfaceId },
}

impl CloseOutcome {
    pub fn removed(&self) -> &[SurfaceId] {
        match self {
            Self::Closed { removed } => removed,
            Self::RejectedRoot { .. } | Self::NotFound => &[],
        }
    }

    pub fn into_removed(self) -> Vec<SurfaceId> {
        match self {
            Self::Closed { removed } => removed,
            Self::RejectedRoot { .. } | Self::NotFound => Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Role, Surface, SurfaceCapabilities, SurfaceManager, SurfacePresentation};

    fn enable_rename(manager: &mut SurfaceManager, id: &str) {
        let content = manager.graph().get(id).unwrap().content.clone();
        let mut presentation = SurfacePresentation::for_content(&content);
        presentation.capabilities = SurfaceCapabilities {
            close: true,
            rename: true,
        };
        assert!(manager.set_presentation(id, presentation));
    }

    #[test]
    fn first_main_is_the_stable_non_closable_root() {
        let mut manager = SurfaceManager::new(1200.0);
        manager.open(Surface::browser(
            "browser-root",
            Role::Main,
            "https://example.com",
        ));
        manager.open(Surface::native("terminal", Role::Main, "terminal"));

        let snapshot = manager.switcher_snapshot();
        assert_eq!(snapshot.root_surface_id.as_deref(), Some("browser-root"));
        assert!(snapshot.items[0].root);
        assert!(!snapshot.items[0].closable);
        assert!(snapshot.items[1].closable);
        assert_eq!(
            manager.close("browser-root"),
            CloseOutcome::RejectedRoot {
                surface_id: "browser-root".into()
            }
        );
    }

    #[test]
    fn close_other_mains_crosses_content_kinds_but_preserves_root() {
        let mut manager = SurfaceManager::new(1200.0);
        manager.open(Surface::lxapp("home", Role::Main, "home"));
        manager.open(Surface::browser(
            "docs",
            Role::Main,
            "https://docs.example.com",
        ));
        manager.open(Surface::native("terminal", Role::Main, "terminal"));

        assert_eq!(manager.close_other_mains("terminal"), vec!["docs"]);
        let snapshot = manager.switcher_snapshot();
        assert_eq!(
            snapshot
                .items
                .iter()
                .map(|item| item.surface_id.as_str())
                .collect::<Vec<_>>(),
            vec!["home", "terminal"]
        );
    }

    #[test]
    fn custom_title_overrides_provider_updates_until_reset() {
        let mut manager = SurfaceManager::new(1200.0);
        manager.open(Surface::native("terminal", Role::Main, "terminal"));
        enable_rename(&mut manager, "terminal");

        assert!(manager.update_automatic_title("terminal", Some("~/github")));
        assert!(manager.rename("terminal", Some("workspace")));
        assert!(manager.update_automatic_title("terminal", Some("~/github/LingXia")));
        assert_eq!(
            manager.switcher_snapshot().items[0].title.as_deref(),
            Some("workspace")
        );

        assert!(manager.rename("terminal", None));
        assert_eq!(
            manager.switcher_snapshot().items[0].title.as_deref(),
            Some("~/github/LingXia")
        );
    }

    #[test]
    fn lxapp_titles_are_not_user_renameable() {
        let mut manager = SurfaceManager::new(1200.0);
        manager.open(Surface::lxapp("home", Role::Main, "home"));

        assert!(!manager.rename("home", Some("renamed")));
        assert_eq!(
            manager.switcher_snapshot().items[0].title.as_deref(),
            Some("home")
        );
    }

    #[test]
    fn close_after_uses_global_switcher_order() {
        let mut manager = SurfaceManager::new(1200.0);
        manager.open(Surface::lxapp("home", Role::Main, "home"));
        manager.open(Surface::browser(
            "docs",
            Role::Main,
            "https://docs.example.com",
        ));
        manager.open(Surface::native("terminal", Role::Main, "terminal"));

        assert_eq!(manager.close_mains_after("home"), vec!["docs", "terminal"]);
        assert_eq!(manager.switcher_snapshot().items.len(), 1);
    }

    #[test]
    fn declaration_replace_overrides_an_early_seeded_root_atomically() {
        let mut manager = SurfaceManager::new(1200.0);
        manager.open(Surface::lxapp("home", Role::Main, "home"));
        let terminal = Surface::native("terminal", Role::Main, "terminal");
        let browser = Surface::browser("browser", Role::Main, "https://example.com");

        let snapshot = manager
            .replace_mains(vec![
                (
                    terminal.clone(),
                    SurfacePresentation::for_content(&terminal.content),
                ),
                (
                    browser.clone(),
                    SurfacePresentation::for_content(&browser.content),
                ),
            ])
            .unwrap();

        assert_eq!(snapshot.root_surface_id.as_deref(), Some("terminal"));
        assert_eq!(snapshot.active_surface_id.as_deref(), Some("terminal"));
        assert_eq!(
            snapshot
                .items
                .iter()
                .map(|item| item.surface_id.as_str())
                .collect::<Vec<_>>(),
            vec!["terminal", "browser"]
        );
        assert!(manager.graph().get("home").is_none());
    }

    #[test]
    fn invalid_declaration_replace_is_atomic() {
        let mut manager = SurfaceManager::new(1200.0);
        manager.open(Surface::lxapp("home", Role::Main, "home"));
        let before = manager.switcher_snapshot();
        let duplicate = Surface::native("terminal", Role::Main, "terminal");

        assert_eq!(
            manager.replace_mains(vec![
                (
                    duplicate.clone(),
                    SurfacePresentation::for_content(&duplicate.content),
                ),
                (
                    duplicate.clone(),
                    SurfacePresentation::for_content(&duplicate.content),
                ),
            ]),
            Err(ReplaceMainsError::DuplicateId {
                surface_id: "terminal".into()
            })
        );
        assert_eq!(manager.switcher_snapshot(), before);
    }

    #[test]
    fn closed_registered_main_can_be_opened_again() {
        let mut manager = SurfaceManager::new(1200.0);
        manager.open(Surface::lxapp("home", Role::Main, "home"));
        let terminal = Surface::native("terminal", Role::Main, "terminal");
        let presentation = SurfacePresentation::for_content(&terminal.content);
        manager
            .open_main(terminal.clone(), presentation.clone())
            .unwrap();
        assert!(matches!(
            manager.close("terminal"),
            CloseOutcome::Closed { .. }
        ));

        let snapshot = manager.open_main(terminal, presentation).unwrap();
        assert_eq!(snapshot.active_surface_id.as_deref(), Some("terminal"));
        assert_eq!(
            snapshot
                .items
                .iter()
                .map(|item| item.surface_id.as_str())
                .collect::<Vec<_>>(),
            vec!["home", "terminal"]
        );
    }
}
