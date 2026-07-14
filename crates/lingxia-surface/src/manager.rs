//! `SurfaceManager` — the stateful per-window driver platforms bind to.
//!
//! Wraps a [`SurfaceGraph`] with the current size band and arbitration policy:
//! open/close requests go through the pure arbiter, width changes resolve the
//! `SizeClass` with hysteresis, and `derive()` produces the `DerivedLayout` the
//! skin renders. All layout decisions stay in the shared core; the platform
//! only maps legacy primitives in and binds the output.

use crate::arbitrate::{Decision, Policy, arbitrate};
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
    /// Floor for the resolved size class. Desktop shells set this to `Medium`
    /// so a narrow desktop window squeezes (sidebar → rail) instead of
    /// projecting to the mobile compact layout; mobile leaves it `Compact`.
    min_size_class: SizeClass,
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
            size_class: SizeClass::from_width(width).max(SizeClass::Compact),
            min_size_class: SizeClass::Compact,
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

    /// Update the container width. Returns `true` if the `SizeClass` changed
    /// (after hysteresis and the min-class floor) — i.e. when the skin must
    /// re-derive its layout.
    pub fn set_width(&mut self, width: f64) -> bool {
        self.width = width;
        let next = SizeClass::resolve(Some(self.size_class), width, self.hysteresis)
            .max(self.min_size_class);
        let changed = next != self.size_class;
        self.size_class = next;
        changed
    }

    /// Floor the resolved size class (desktop shells set `Medium` so a narrow
    /// window never projects to the mobile compact layout). Re-resolves the
    /// current width against the new floor; returns `true` if the class moved.
    pub fn set_min_size_class(&mut self, min: SizeClass) -> bool {
        self.min_size_class = min;
        let next = SizeClass::resolve(Some(self.size_class), self.width, self.hysteresis).max(min);
        let changed = next != self.size_class;
        self.size_class = next;
        changed
    }

    /// Open (or replace by id) a surface through the arbiter at the current size.
    /// Always leaves the graph valid; returns the structured decision.
    pub fn open(&mut self, request: Surface) -> Decision {
        let (next, decision) = arbitrate(&self.graph, request, &self.policy, self.size_class);
        self.graph = next;
        decision
    }

    /// Close a surface; returns the ids actually removed (target + cascades).
    pub fn close(&mut self, id: &str) -> Vec<SurfaceId> {
        self.graph.remove(id)
    }

    pub fn set_active_main(&mut self, id: &str) -> bool {
        self.graph.set_active_main(id)
    }
    pub fn set_focus(&mut self, id: &str) -> bool {
        self.graph.set_focus(id)
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
        self.graph
            .presentation_plan(self.size_class, self.width, &self.policy)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
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
    fn aside_on_compact_promotes_without_host_switcher() {
        let mut m = SurfaceManager::new(390.0); // phone width
        assert_eq!(m.size_class(), SizeClass::Compact);
        m.open(main_s("home"));
        // arbitration promotes the aside to a main on compact.
        assert_eq!(
            m.open(aside_s("assistant", Edge::Right)),
            Decision::FullScreenFallback
        );
        let d = m.derive();
        assert_eq!(d.switcher_form, SwitcherForm::None);
        assert_eq!(d.bottom_owner, crate::BottomOwner::App);
        assert!(m.graph().is_valid());
    }

    #[test]
    fn desktop_min_class_floor_never_projects_to_compact() {
        // A desktop shell floors at Medium: a narrow window that would
        // otherwise resolve Compact stays Medium (squeeze, not mobile-project).
        let mut m = SurfaceManager::new(1200.0);
        m.set_min_size_class(SizeClass::Medium);
        // min-window workspace (720 - 180 sidebar = 540) → Compact by width,
        // floored to Medium.
        m.set_width(540.0);
        assert_eq!(m.size_class(), SizeClass::Medium);
        // Widening past the medium/expanded boundary still reaches Expanded.
        m.set_width(1000.0);
        assert_eq!(m.size_class(), SizeClass::Expanded);
        // Mobile (no floor) still reaches Compact.
        let mut phone = SurfaceManager::new(390.0);
        assert_eq!(phone.size_class(), SizeClass::Compact);
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
        // role unchanged → widening back restores the split (reversible).
        assert_eq!(m.graph().role_of("assistant"), Some(Role::Aside));
        m.set_width(1200.0);
        assert_eq!(m.derive().split_form, SplitForm::Split);
    }
}
