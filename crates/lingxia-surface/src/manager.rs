//! `SurfaceManager` — the stateful per-window driver platforms bind to.
//!
//! Wraps a [`SurfaceGraph`] with the current size band and arbitration policy:
//! open/close requests go through the pure arbiter, width changes resolve the
//! `SizeClass` with hysteresis, and `derive()` produces the `DerivedLayout` the
//! skin renders. All layout decisions stay in the shared core; the platform
//! only maps legacy primitives in and binds the output.

use crate::arbitrate::{OpenOutcome, Policy, arbitrate};
use crate::graph::SurfaceGraph;
use crate::layout::{DEFAULT_HYSTERESIS, DerivedLayout, LayoutPresentationPlan, SizeClass};
use crate::model::{Surface, SurfaceId};

/// One window's stateful surface driver.
#[derive(Debug, Clone)]
pub struct SurfaceManager {
    graph: SurfaceGraph,
    policy: Policy,
    width: f64,
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
            policy,
            width,
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
        let sidebar_width = match self.size_class {
            SizeClass::Compact => 0.0,
            SizeClass::Medium => self.policy.sidebar_medium_width,
            SizeClass::Expanded => self.policy.sidebar_expanded_width,
        };
        (self.width - sidebar_width).max(0.0)
    }

    /// Update the container width. Returns `true` if the `SizeClass` changed
    /// after hysteresis, i.e. when the skin must re-derive its layout.
    pub fn set_width(&mut self, width: f64) -> bool {
        self.width = width;
        let next = SizeClass::resolve(Some(self.size_class), width, self.hysteresis);
        let changed = next != self.size_class;
        self.size_class = next;
        changed
    }

    /// Open (or replace by id) a surface through the arbiter at the current size.
    /// Always leaves the graph valid; returns the structured decision.
    pub fn open(&mut self, request: Surface) -> OpenOutcome {
        let (next, mut outcome) = arbitrate(&self.graph, request, &self.policy, self.size_class);
        self.graph = next;
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
            }
        }
        outcome
    }

    /// Close a surface; returns the ids actually removed (target + cascades).
    pub fn close(&mut self, id: &str) -> Vec<SurfaceId> {
        let removed = self.graph.remove(id);
        if self
            .overlay_fallback_surface_id
            .as_ref()
            .is_some_and(|fallback| removed.contains(fallback))
        {
            self.overlay_fallback_surface_id = self
                .graph
                .focused_surface_id
                .as_deref()
                .filter(|focused| self.graph.role_of(focused) == Some(crate::model::Role::Aside))
                .map(str::to_string);
        }
        removed
    }

    pub fn set_active_main(&mut self, id: &str) -> bool {
        self.graph.set_active_main(id)
    }
    pub fn set_focus(&mut self, id: &str) -> bool {
        self.graph.set_focus(id)
    }

    pub fn show(&mut self, id: &str) -> bool {
        let shown = self.graph.show(id);
        if shown && self.graph.role_of(id) == Some(crate::model::Role::Aside) {
            let admitted = self
                .graph
                .aside_slots_admitted(self.size_class, self.workspace_width(), &self.policy)
                .into_iter()
                .find(|slot| slot.children.iter().any(|child| child == id))
                .is_some_and(|slot| slot.visible);
            if self.size_class == SizeClass::Compact || !admitted {
                self.overlay_fallback_surface_id = Some(id.to_string());
            }
        }
        shown
    }

    pub fn hide(&mut self, id: &str) -> bool {
        let hidden = self.graph.hide(id);
        if hidden && self.overlay_fallback_surface_id.as_deref() == Some(id) {
            self.overlay_fallback_surface_id = self
                .graph
                .focused_surface_id
                .as_deref()
                .filter(|focused| self.graph.role_of(focused) == Some(crate::model::Role::Aside))
                .map(str::to_string);
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
    use crate::model::{Edge, Role, Surface};

    fn main_s(id: &str) -> Surface {
        Surface::entry(id, Role::Main, id)
    }
    fn aside_s(id: &str, edge: Edge) -> Surface {
        let mut s = Surface::entry(id, Role::Aside, id);
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
        manager.open(main_s("home"));
        manager.open(aside_s("lxapp", Edge::Right));
        let mut browser = Surface::entry("browser", Role::Aside, "browser");
        browser.content = crate::model::SurfaceContent::Web {
            url: "https://example.com".to_string(),
        };
        browser.placement.edge = Some(Edge::Right);
        manager.open(browser);
        let mut native = Surface::entry("terminal", Role::Aside, "terminal");
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
