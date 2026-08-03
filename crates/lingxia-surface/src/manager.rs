//! `SurfaceManager` — the stateful per-window driver platforms bind to.
//!
//! Wraps a [`SurfaceGraph`] with the current size band and arbitration policy:
//! open/close requests go through the pure arbiter, width changes resolve the
//! `SizeClass` with hysteresis, and `derive()` produces the `DerivedLayout` the
//! skin renders. All layout decisions stay in the shared core; the platform
//! only maps legacy primitives in and binds the output.

use std::collections::HashMap;

use crate::arbitrate::{OpenOutcome, Policy, arbitrate};
use crate::graph::SurfaceGraph;
use crate::layout::{DEFAULT_HYSTERESIS, DerivedLayout, LayoutPresentationPlan, SizeClass};
use crate::model::{Surface, SurfaceId};
use crate::{CloseOutcome, ReplaceMainsError, Role, SurfacePresentation, SurfaceSwitcherSnapshot};

/// One window's stateful surface driver.
#[derive(Debug, Clone)]
pub struct SurfaceManager {
    graph: SurfaceGraph,
    presentations: HashMap<SurfaceId, SurfacePresentation>,
    revision: u64,
    policy: Policy,
    width: f64,
    sidebar_width: f64,
    hysteresis: f64,
    size_class: SizeClass,
    /// Most recently explicitly shown aside that could not be admitted as a
    /// dock. It remains live and is projected over the main until hidden.
    overlay_fallback_surface_id: Option<SurfaceId>,
}

impl SurfaceManager {
    /// New manager for a container of `width` logical px, default policy.
    pub fn new(width: f64) -> Self {
        Self::with_policy(width, Policy::default())
    }

    pub fn with_policy(width: f64, policy: Policy) -> Self {
        Self {
            graph: SurfaceGraph::new(),
            presentations: HashMap::new(),
            revision: 0,
            policy,
            width,
            sidebar_width: 0.0,
            hysteresis: DEFAULT_HYSTERESIS,
            size_class: SizeClass::from_width(width),
            overlay_fallback_surface_id: None,
        }
    }

    pub fn graph(&self) -> &SurfaceGraph {
        &self.graph
    }
    pub fn size_class(&self) -> SizeClass {
        self.size_class
    }
    pub fn width(&self) -> f64 {
        self.width
    }

    fn workspace_width(&self) -> f64 {
        (self.width - self.sidebar_width).max(0.0)
    }

    fn needs_overlay(&self, id: &str) -> bool {
        self.graph.role_of(id) == Some(crate::model::Role::Aside)
            && (self.size_class == SizeClass::Compact
                || !self
                    .graph
                    .aside_slots_admitted(self.size_class, self.workspace_width(), &self.policy)
                    .into_iter()
                    .find(|slot| slot.children.iter().any(|child| child == id))
                    .is_some_and(|slot| slot.visible))
    }

    fn reconcile_overlay_fallback(&mut self) {
        if self
            .overlay_fallback_surface_id
            .as_deref()
            .is_some_and(|id| !self.needs_overlay(id))
        {
            self.overlay_fallback_surface_id = None;
        }
    }

    /// Update the container width. Returns `true` if the `SizeClass` changed
    /// after hysteresis, i.e. when the skin must re-derive its layout.
    pub fn set_width(&mut self, width: f64) -> bool {
        self.width = width;
        let next = SizeClass::resolve(Some(self.size_class), width, self.hysteresis);
        let changed = next != self.size_class;
        self.size_class = next;
        self.reconcile_overlay_fallback();
        changed
    }

    /// Report the platform's live sidebar allocation. Mobile/custom hosts use
    /// zero; desktop shells update this during resize and collapse.
    pub fn set_sidebar_width(&mut self, width: f64) -> bool {
        let width = if width.is_finite() {
            width.max(0.0).min(self.width)
        } else {
            0.0
        };
        let changed = (self.sidebar_width - width).abs() > f64::EPSILON;
        self.sidebar_width = width;
        self.reconcile_overlay_fallback();
        changed
    }

    /// Open (or replace by id) a surface through the arbiter at the current size.
    /// Always leaves the graph valid; returns the structured decision.
    pub fn open(&mut self, request: Surface) -> OpenOutcome {
        let requested_id = request.id.clone();
        let default_presentation = SurfacePresentation::for_content(&request.content);
        let content_changed = self
            .graph
            .get(&requested_id)
            .is_some_and(|surface| surface.content != request.content);
        let (next, mut outcome) = arbitrate(&self.graph, request, &self.policy, self.size_class);
        self.graph = next;
        if outcome.resolved_surface_id == requested_id {
            if content_changed {
                self.presentations
                    .insert(requested_id, default_presentation);
            } else {
                self.presentations
                    .entry(requested_id)
                    .or_insert(default_presentation);
            }
        }
        self.presentations
            .retain(|id, _| self.graph.get(id).is_some());
        self.bump_revision();
        if outcome.resolved_role == crate::model::Role::Aside {
            let admitted = self
                .graph
                .aside_slots_admitted(self.size_class, self.workspace_width(), &self.policy)
                .into_iter()
                .find(|slot| slot.children.contains(&outcome.resolved_surface_id))
                .is_some_and(|slot| slot.visible);
            outcome.overlay |= self.size_class == SizeClass::Compact || !admitted;
            if outcome.overlay {
                self.overlay_fallback_surface_id = Some(outcome.resolved_surface_id.clone());
            } else {
                self.overlay_fallback_surface_id = None;
            }
        }
        outcome
    }

    pub fn close(&mut self, id: &str) -> CloseOutcome {
        let outcome = self.graph.close(id);
        let removed = outcome.removed();
        if self
            .overlay_fallback_surface_id
            .as_ref()
            .is_some_and(|fallback| removed.contains(fallback))
        {
            let focused = self
                .graph
                .focused_surface_id
                .as_deref()
                .filter(|focused| self.graph.role_of(focused) == Some(crate::model::Role::Aside))
                .map(str::to_string);
            self.overlay_fallback_surface_id =
                focused.filter(|focused| self.needs_overlay(focused));
        }
        for id in removed {
            self.presentations.remove(id);
        }
        if matches!(outcome, CloseOutcome::Closed { .. }) {
            self.bump_revision();
        }
        outcome
    }

    pub fn replace_mains(
        &mut self,
        mains: Vec<(Surface, SurfacePresentation)>,
    ) -> Result<SurfaceSwitcherSnapshot, ReplaceMainsError> {
        let mut ids = std::collections::HashSet::with_capacity(mains.len());
        for (surface, _) in &mains {
            if surface.role != Role::Main {
                return Err(ReplaceMainsError::InvalidRole {
                    surface_id: surface.id.clone(),
                });
            }
            if !ids.insert(surface.id.clone()) {
                return Err(ReplaceMainsError::DuplicateId {
                    surface_id: surface.id.clone(),
                });
            }
        }
        let old_main_ids: Vec<_> = self
            .graph
            .mains()
            .into_iter()
            .map(|surface| surface.id.clone())
            .collect();
        self.remove_presentations(&old_main_ids);

        let (surfaces, presentations): (Vec<_>, Vec<_>) = mains.into_iter().unzip();
        self.graph.replace_mains(surfaces);
        for (surface, presentation) in self.graph.mains().into_iter().zip(presentations) {
            self.presentations.insert(surface.id.clone(), presentation);
        }
        self.presentations
            .retain(|id, _| self.graph.get(id).is_some());
        self.bump_revision();
        Ok(self.switcher_snapshot())
    }

    pub fn open_main(
        &mut self,
        surface: Surface,
        presentation: SurfacePresentation,
    ) -> Result<SurfaceSwitcherSnapshot, ReplaceMainsError> {
        if surface.role != Role::Main {
            return Err(ReplaceMainsError::InvalidRole {
                surface_id: surface.id,
            });
        }
        let surface_id = surface.id.clone();
        // Main registration has a stricter contract than a general open: the
        // requested identity must become that exact main. Bypass aside reuse
        // and role arbitration so a future policy change cannot redirect the
        // request while we publish presentation metadata under the old id.
        self.graph.insert(surface);
        self.presentations.insert(surface_id.clone(), presentation);
        self.presentations
            .retain(|id, _| self.graph.get(id).is_some());
        self.bump_revision();
        self.set_active_main(&surface_id);
        Ok(self.switcher_snapshot())
    }

    pub fn close_other_mains(&mut self, keeping: &str) -> Vec<SurfaceId> {
        if self.graph.role_of(keeping) != Some(Role::Main) {
            return Vec::new();
        }
        let targets: Vec<_> = self
            .switcher_snapshot()
            .items
            .into_iter()
            .filter(|item| item.surface_id != keeping && item.closable)
            .map(|item| item.surface_id)
            .collect();
        let removed = targets
            .into_iter()
            .flat_map(|id| self.graph.close(&id).into_removed())
            .collect::<Vec<_>>();
        self.remove_presentations(&removed);
        if !removed.is_empty() {
            self.bump_revision();
        }
        removed
    }

    pub fn close_mains_after(&mut self, id: &str) -> Vec<SurfaceId> {
        let snapshot = self.switcher_snapshot();
        let Some(index) = snapshot.items.iter().position(|item| item.surface_id == id) else {
            return Vec::new();
        };
        let removed = snapshot
            .items
            .into_iter()
            .skip(index + 1)
            .filter(|item| item.closable)
            .flat_map(|item| self.graph.close(&item.surface_id).into_removed())
            .collect::<Vec<_>>();
        self.remove_presentations(&removed);
        if !removed.is_empty() {
            self.bump_revision();
        }
        removed
    }

    fn remove_presentations(&mut self, ids: &[SurfaceId]) {
        for id in ids {
            self.presentations.remove(id);
        }
    }

    pub fn set_presentation(&mut self, id: &str, presentation: SurfacePresentation) -> bool {
        if self.graph.get(id).is_none() {
            return false;
        }
        self.presentations.insert(id.to_string(), presentation);
        self.bump_revision();
        true
    }

    pub fn update_automatic_title(&mut self, id: &str, title: Option<&str>) -> bool {
        let Some(presentation) = self.presentations.get_mut(id) else {
            return false;
        };
        let title = title
            .map(str::trim)
            .filter(|title| !title.is_empty())
            .map(str::to_string);
        if presentation.automatic_title == title {
            return false;
        }
        presentation.automatic_title = title;
        self.bump_revision();
        true
    }

    pub fn rename(&mut self, id: &str, title: Option<&str>) -> bool {
        let Some(presentation) = self.presentations.get_mut(id) else {
            return false;
        };
        if !presentation.capabilities.rename {
            return false;
        }
        presentation.set_custom_title(title);
        self.bump_revision();
        true
    }

    pub fn switcher_snapshot(&self) -> SurfaceSwitcherSnapshot {
        let mut snapshot = SurfaceSwitcherSnapshot::derive(&self.graph, &self.presentations);
        snapshot.revision = self.revision;
        snapshot
    }

    fn bump_revision(&mut self) {
        self.revision = self.revision.wrapping_add(1);
    }

    pub fn set_active_main(&mut self, id: &str) -> bool {
        let changed = self.graph.active_main_id.as_deref() != Some(id);
        let active = self.graph.set_active_main(id);
        if active {
            self.overlay_fallback_surface_id = None;
            if changed {
                self.bump_revision();
            }
        }
        active
    }
    pub fn set_focus(&mut self, id: &str) -> bool {
        let focused = self.graph.set_focus(id);
        if focused {
            self.overlay_fallback_surface_id = self.needs_overlay(id).then(|| id.to_string());
        }
        focused
    }

    pub fn show(&mut self, id: &str) -> bool {
        let role = self.graph.role_of(id);
        let activates_main =
            role == Some(Role::Main) && self.graph.active_main_id.as_deref() != Some(id);
        let shown = self.graph.show(id);
        if shown && role == Some(Role::Main) {
            self.overlay_fallback_surface_id = None;
            if activates_main {
                // `SurfaceGraph::show` performs the selection. Mirror
                // `set_active_main`'s observable switcher revision here.
                self.bump_revision();
            }
        }
        if shown && role == Some(Role::Aside) {
            let admitted = self
                .graph
                .aside_slots_admitted(self.size_class, self.workspace_width(), &self.policy)
                .into_iter()
                .find(|slot| slot.children.iter().any(|child| child == id))
                .is_some_and(|slot| slot.visible);
            if self.size_class == SizeClass::Compact || !admitted {
                self.overlay_fallback_surface_id = Some(id.to_string());
            } else {
                self.overlay_fallback_surface_id = None;
            }
        }
        shown
    }

    pub fn hide(&mut self, id: &str) -> bool {
        let hidden = self.graph.hide(id);
        if hidden && self.overlay_fallback_surface_id.as_deref() == Some(id) {
            let focused = self
                .graph
                .focused_surface_id
                .as_deref()
                .filter(|focused| self.graph.role_of(focused) == Some(crate::model::Role::Aside))
                .map(str::to_string);
            self.overlay_fallback_surface_id =
                focused.filter(|focused| self.needs_overlay(focused));
        }
        hidden
    }

    /// Derive the platform-agnostic layout output at the current size.
    pub fn derive(&self) -> DerivedLayout {
        self.graph.derive_layout(self.size_class)
    }

    /// Build the stable, skin-bindable [`LayoutPresentationPlan`] at the current
    /// size — the renderable contract platforms reconcile against. Slot
    /// admission respects both the size-class ceiling and the physical fit at
    /// the current width (§3.3).
    pub fn presentation_plan(&self) -> LayoutPresentationPlan {
        let mut plan =
            self.graph
                .presentation_plan(self.size_class, self.workspace_width(), &self.policy);
        plan.main_switcher = self.switcher_snapshot();
        if self.size_class == SizeClass::Compact {
            for slot in plan.aside_slots.iter_mut().filter(|slot| slot.visible) {
                slot.overlay = true;
            }
        }
        if let Some(id) = self.overlay_fallback_surface_id.as_deref()
            && let Some(slot) = plan
                .aside_slots
                .iter_mut()
                .find(|slot| slot.children.iter().any(|child| child == id))
            && (self.size_class == SizeClass::Compact || !slot.visible)
        {
            slot.visible = true;
            slot.active_child = Some(id.to_string());
            slot.overlay = true;
        }
        plan
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Decision;
    use crate::layout::{SplitForm, SwitcherForm};
    use crate::{Edge, Role, Surface, SurfaceContent};

    fn main_s(id: &str) -> Surface {
        Surface::lxapp(id, Role::Main, id)
    }
    fn aside_s(id: &str, edge: Edge) -> Surface {
        let mut s = Surface::lxapp(id, Role::Aside, id);
        s.placement.edge = Some(edge);
        s
    }

    #[test]
    fn open_then_derive_on_expanded() {
        let mut m = SurfaceManager::new(1200.0);
        assert_eq!(m.size_class(), SizeClass::Expanded);
        assert_eq!(m.open(main_s("home")), Decision::Accepted);
        assert_eq!(
            m.open(aside_s("assistant", Edge::Right)),
            Decision::Accepted
        );
        let d = m.derive();
        assert_eq!(d.split_form, SplitForm::Split);
        assert!(m.graph().is_valid());
    }

    #[test]
    fn aside_on_compact_overlays_without_host_switcher() {
        let mut m = SurfaceManager::new(390.0); // phone width
        assert_eq!(m.size_class(), SizeClass::Compact);
        m.open(main_s("home"));
        // Arbitration preserves the aside role and marks it as a full-screen
        // overlay; compact still has no sidebar switcher.
        assert_eq!(
            m.open(aside_s("assistant", Edge::Right)),
            Decision::FullScreenFallback
        );
        let d = m.derive();
        assert_eq!(d.switcher_form, SwitcherForm::None);
        assert_eq!(d.bottom_owner, crate::BottomOwner::App);
        let slot = &m.presentation_plan().aside_slots[0];
        assert!(slot.visible);
        assert!(slot.overlay);
        assert!(m.graph().is_valid());
    }

    #[test]
    fn width_changes_recompute_physical_admission_within_a_size_class() {
        let mut manager = SurfaceManager::new(1400.0);
        manager.set_sidebar_width(184.0);
        manager.open(main_s("home"));
        manager.open(aside_s("lxapp", Edge::Right));
        let mut browser = Surface::lxapp("browser", Role::Aside, "browser");
        browser.content = SurfaceContent::Browser {
            initial_url: "https://example.com".to_string(),
            reuse_by_url: true,
        };
        browser.placement.edge = Some(Edge::Right);
        manager.open(browser);
        let mut native = Surface::native("terminal", Role::Aside, "terminal");
        native.placement.edge = Some(Edge::Right);
        manager.open(native);
        assert_eq!(
            manager
                .presentation_plan()
                .aside_slots
                .iter()
                .filter(|slot| slot.visible)
                .count(),
            3
        );

        // Both widths are Expanded. After the full sidebar is allocated, only
        // one horizontal slot fits at a 900-wide client area.
        assert!(!manager.set_width(900.0));
        assert_eq!(manager.size_class(), SizeClass::Expanded);
        assert_eq!(
            manager
                .presentation_plan()
                .aside_slots
                .iter()
                .filter(|slot| slot.visible)
                .count(),
            1
        );
    }

    #[test]
    fn explicitly_opened_non_fitting_aside_overlays_until_it_can_dock() {
        let mut manager = SurfaceManager::new(500.0);
        manager.open(main_s("home"));
        let outcome = manager.open(aside_s("assistant", Edge::Right));
        assert!(outcome.overlay);
        let slot = &manager.presentation_plan().aside_slots[0];
        assert!(slot.visible);
        assert!(slot.overlay);

        manager.set_width(700.0);
        let slot = &manager.presentation_plan().aside_slots[0];
        assert!(slot.visible);
        assert!(!slot.overlay);
    }

    #[test]
    fn compact_focus_updates_the_overlay_tab() {
        let mut manager = SurfaceManager::new(500.0);
        manager.open(main_s("home"));
        manager.open(aside_s("first", Edge::Right));
        manager.open(aside_s("second", Edge::Right));

        assert!(manager.set_focus("first"));
        let slot = &manager.presentation_plan().aside_slots[0];
        assert_eq!(slot.active_child.as_deref(), Some("first"));
    }

    #[test]
    fn showing_an_existing_main_advances_the_switcher_revision() {
        let mut manager = SurfaceManager::new(1200.0);
        manager.open(main_s("home"));
        manager.open(main_s("workspace"));
        manager.set_active_main("workspace");
        let before = manager.switcher_snapshot();

        assert!(manager.show("home"));
        let after = manager.switcher_snapshot();

        assert_eq!(after.active_surface_id.as_deref(), Some("home"));
        assert!(after.revision > before.revision);
    }

    #[test]
    fn docked_fallback_does_not_reappear_after_later_resize() {
        let policy = Policy {
            main_min_width: 400.0,
            aside_min_width: 240.0,
            ..Policy::default()
        };
        let mut manager = SurfaceManager::with_policy(620.0, policy);
        manager.open(main_s("home"));
        let mut browser = Surface::lxapp("browser", Role::Aside, "browser");
        browser.content = SurfaceContent::Browser {
            initial_url: "https://example.com".to_string(),
            reuse_by_url: true,
        };
        browser.placement.edge = Some(Edge::Right);
        assert!(manager.open(browser).overlay);

        manager.set_width(1000.0);
        manager.open(aside_s("chat", Edge::Right));
        assert!(manager.set_focus("chat"));
        manager.set_width(700.0);

        let visible: Vec<_> = manager
            .presentation_plan()
            .aside_slots
            .into_iter()
            .filter(|slot| slot.visible)
            .map(|slot| slot.kind)
            .collect();
        assert_eq!(visible, vec![crate::SlotKind::Lxapp]);
    }

    #[test]
    fn live_sidebar_width_controls_physical_admission() {
        let mut manager = SurfaceManager::new(900.0);
        manager.open(main_s("home"));
        manager.open(aside_s("chat", Edge::Right));
        let mut browser = Surface::lxapp("browser", Role::Aside, "browser");
        browser.content = SurfaceContent::Browser {
            initial_url: "https://example.com".to_string(),
            reuse_by_url: true,
        };
        browser.placement.edge = Some(Edge::Right);
        manager.open(browser);

        assert_eq!(
            manager
                .presentation_plan()
                .aside_slots
                .iter()
                .filter(|slot| slot.visible)
                .count(),
            2
        );
        manager.set_sidebar_width(300.0);
        assert_eq!(
            manager
                .presentation_plan()
                .aside_slots
                .iter()
                .filter(|slot| slot.visible)
                .count(),
            1
        );
        manager.set_sidebar_width(0.0);
        assert_eq!(
            manager
                .presentation_plan()
                .aside_slots
                .iter()
                .filter(|slot| slot.visible)
                .count(),
            2
        );
    }

    #[test]
    fn width_change_reports_sizeclass_flip_with_hysteresis() {
        let mut m = SurfaceManager::new(1200.0);
        // small nudge that stays expanded → no change reported.
        assert!(!m.set_width(900.0));
        assert_eq!(m.size_class(), SizeClass::Expanded);
        // drop to phone width → flips to compact.
        assert!(m.set_width(390.0));
        assert_eq!(m.size_class(), SizeClass::Compact);
        // hovering just under the 600 boundary keeps compact (hysteresis).
        assert!(!m.set_width(590.0));
        assert_eq!(m.size_class(), SizeClass::Compact);
    }

    #[test]
    fn resize_reflows_existing_aside_without_mutating_roles() {
        let mut m = SurfaceManager::new(1200.0);
        m.open(main_s("home"));
        m.open(aside_s("assistant", Edge::Right));
        // expanded: real split, aside stays an aside.
        assert_eq!(m.derive().split_form, SplitForm::Split);
        assert_eq!(m.graph().role_of("assistant"), Some(Role::Aside));
        // shrink to compact: same graph, layout re-flows to full-screen.
        m.set_width(390.0);
        let d = m.derive();
        assert_eq!(d.split_form, SplitForm::FullScreen);
        assert_eq!(d.switcher_form, SwitcherForm::None);
        let slot = &m.presentation_plan().aside_slots[0];
        assert!(slot.visible);
        assert!(slot.overlay);
        // role unchanged → widening back restores the split (reversible).
        assert_eq!(m.graph().role_of("assistant"), Some(Role::Aside));
        m.set_width(1200.0);
        assert_eq!(m.derive().split_form, SplitForm::Split);
    }
}
