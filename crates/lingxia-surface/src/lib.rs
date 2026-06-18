//! `lingxia-surface` — the platform-agnostic core of the Adaptive Surface
//! Layout model (see `docs/draft/adaptive-surface-layout.md`).
//!
//! Pure Rust, no UI: the Surface Graph, its invariants and state transitions,
//! the two-axis derivation into `DerivedLayout`, and the Host arbitration
//! pure function. Each platform skin binds the `DerivedLayout` output.

mod arbitrate;
mod graph;
mod layout;
mod manager;
mod model;

pub use arbitrate::{Decision, Policy, arbitrate};
pub use graph::SurfaceGraph;
pub use manager::SurfaceManager;
pub use layout::{
    Axis, BottomOwner, DEFAULT_HYSTERESIS, DerivedLayout, LayoutPresentationPlan, LayoutTree,
    PlanAside, PlanFloat, SizeClass, SplitForm, SwitcherForm,
};
pub use model::{
    Edge, FloatAnchor, FloatDismiss, FloatSpec, Placement, Role, Surface, SurfaceContent, SurfaceId,
    SurfaceOwner, SurfaceState,
};

#[cfg(test)]
mod tests {
    use super::*;

    fn main_s(id: &str) -> Surface {
        Surface::entry(id, Role::Main, id)
    }
    fn aside_s(id: &str, edge: Edge) -> Surface {
        let mut s = Surface::entry(id, Role::Aside, id);
        s.placement.edge = Some(edge);
        s
    }
    fn web_aside_s(id: &str, url: &str, edge: Edge) -> Surface {
        let mut s = aside_s(id, edge);
        s.content = SurfaceContent::Web {
            url: url.to_string(),
        };
        s
    }

    // ---- invariants & state transitions (§1.3 / §1.5) ----

    #[test]
    fn empty_graph_is_valid() {
        let g = SurfaceGraph::new();
        assert!(g.is_valid());
        assert_eq!(g.active_main_id, None);
        assert_eq!(g.focused_surface_id, None);
    }

    #[test]
    fn first_main_becomes_active_and_focused() {
        let mut g = SurfaceGraph::new();
        g.insert(main_s("home"));
        assert_eq!(g.active_main_id.as_deref(), Some("home"));
        assert_eq!(g.focused_surface_id.as_deref(), Some("home"));
        assert!(g.is_valid());
    }

    #[test]
    fn aside_requires_a_main_invariant() {
        // Construct an illegal graph directly and assert the checker catches it.
        let mut g = SurfaceGraph::new();
        g.insert(main_s("home"));
        g.insert(aside_s("assistant", Edge::Right));
        assert!(g.is_valid());
        // Removing the only main cascades the aside closed (§1.5).
        let removed = g.remove("home");
        assert!(removed.contains(&"assistant".to_string()));
        assert!(g.asides().is_empty());
        assert_eq!(g.active_main_id, None);
        assert!(g.is_valid());
    }

    #[test]
    fn closing_active_main_picks_adjacent_successor() {
        let mut g = SurfaceGraph::new();
        g.insert(main_s("a"));
        g.insert(main_s("b"));
        g.insert(main_s("c"));
        g.set_active_main("b");
        g.remove("b");
        // prefer the next main after the removed position.
        assert_eq!(g.active_main_id.as_deref(), Some("c"));
        assert!(g.is_valid());
    }

    #[test]
    fn modal_float_restores_focus_on_close() {
        let mut g = SurfaceGraph::new();
        g.insert(main_s("home"));
        assert_eq!(g.focused_surface_id.as_deref(), Some("home"));
        let mut modal = Surface::entry("dialog", Role::Float, "confirm");
        modal.float = Some(FloatSpec {
            modal: true,
            ..Default::default()
        });
        g.insert(modal);
        g.set_focus("dialog");
        g.remove("dialog");
        assert_eq!(g.focused_surface_id.as_deref(), Some("home"));
        assert!(g.is_valid());
    }

    // ---- two-axis derivation (§2 / §6) ----

    #[test]
    fn single_main_no_switcher_no_split() {
        let mut g = SurfaceGraph::new();
        g.insert(main_s("home"));
        let d = g.derive_layout(SizeClass::Expanded);
        assert_eq!(d.switcher_form, SwitcherForm::None);
        assert_eq!(d.split_form, SplitForm::None);
        assert!(matches!(d.layout_tree, Some(LayoutTree::Leaf { .. })));
    }

    #[test]
    fn switcher_only_with_multiple_mains() {
        let mut g = SurfaceGraph::new();
        g.insert(main_s("a"));
        assert_eq!(
            g.derive_layout(SizeClass::Expanded).switcher_form,
            SwitcherForm::None
        );
        g.insert(main_s("b"));
        assert_eq!(
            g.derive_layout(SizeClass::Expanded).switcher_form,
            SwitcherForm::Sidebar
        );
    }

    #[test]
    fn aside_splits_on_expanded_peer_fallback_on_compact() {
        let mut g = SurfaceGraph::new();
        g.insert(main_s("home"));
        g.insert(aside_s("assistant", Edge::Right));
        assert_eq!(
            g.derive_layout(SizeClass::Expanded).split_form,
            SplitForm::Split
        );
        assert_eq!(
            g.derive_layout(SizeClass::Compact).split_form,
            SplitForm::PeerFallback
        );
    }

    #[test]
    fn compact_bottom_owner() {
        let mut g = SurfaceGraph::new();
        g.insert(main_s("a"));
        // single main → app owns bottom
        assert_eq!(
            g.derive_layout(SizeClass::Compact).bottom_owner,
            BottomOwner::App
        );
        g.insert(main_s("b"));
        // multiple mains → host switcher owns bottom
        assert_eq!(
            g.derive_layout(SizeClass::Compact).bottom_owner,
            BottomOwner::Host
        );
    }

    #[test]
    fn canonical_layout_validates() {
        let mut g = SurfaceGraph::new();
        g.insert(main_s("a"));
        g.insert(main_s("b"));
        g.insert(aside_s("assistant", Edge::Right));
        let tree = g.canonical_layout(SizeClass::Expanded).unwrap();
        tree.validate().expect("canonical tree must be valid");
        // floats never appear in the tree.
        g.insert(Surface::entry("toast", Role::Float, "toast"));
        let ids = g
            .canonical_layout(SizeClass::Expanded)
            .unwrap()
            .surface_ids();
        assert!(!ids.contains(&"toast".to_string()));
    }

    // ---- sizeClass breakpoints + hysteresis (§6.1) ----

    #[test]
    fn breakpoints_align_to_material() {
        assert_eq!(SizeClass::from_width(599.0), SizeClass::Compact);
        assert_eq!(SizeClass::from_width(600.0), SizeClass::Medium);
        assert_eq!(SizeClass::from_width(840.0), SizeClass::Medium);
        assert_eq!(SizeClass::from_width(841.0), SizeClass::Expanded);
    }

    #[test]
    fn hysteresis_holds_class_near_boundary() {
        // sitting just under the 600 boundary, within margin, keeps Medium.
        let held = SizeClass::resolve(Some(SizeClass::Medium), 590.0, DEFAULT_HYSTERESIS);
        assert_eq!(held, SizeClass::Medium);
        // clearly past the boundary switches.
        let switched = SizeClass::resolve(Some(SizeClass::Medium), 500.0, DEFAULT_HYSTERESIS);
        assert_eq!(switched, SizeClass::Compact);
    }

    // ---- arbitration (§3.4) ----

    #[test]
    fn aside_over_limit_replaces_oldest() {
        let mut g = SurfaceGraph::new();
        g.insert(main_s("home"));
        g.insert(aside_s("a1", Edge::Right));
        g.insert(aside_s("a2", Edge::Right));
        // expanded max=2 → a third aside replaces the oldest same-edge one.
        let (next, decision) = arbitrate(
            &g,
            aside_s("a3", Edge::Right),
            &Policy::default(),
            SizeClass::Expanded,
        );
        assert_eq!(decision, Decision::ReplacedExisting);
        assert!(next.get("a1").is_none());
        assert!(next.get("a3").is_some());
        assert_eq!(next.asides().len(), 2);
        assert!(next.is_valid());
    }

    #[test]
    fn web_aside_is_singleton_replacing_existing() {
        let mut g = SurfaceGraph::new();
        g.insert(main_s("home"));
        g.insert(web_aside_s("browser-1", "https://a.example", Edge::Right));
        // A second browser aside replaces the first regardless of the generic
        // cap (expanded allows 2 asides), and only on a different edge too.
        let (next, decision) = arbitrate(
            &g,
            web_aside_s("browser-2", "https://b.example", Edge::Left),
            &Policy::default(),
            SizeClass::Expanded,
        );
        assert_eq!(decision, Decision::ReplacedExisting);
        assert!(next.get("browser-1").is_none());
        assert!(next.get("browser-2").is_some());
        // exactly one web aside survives.
        assert_eq!(
            next.asides()
                .iter()
                .filter(|s| matches!(s.content, SurfaceContent::Web { .. }))
                .count(),
            1
        );
        assert!(next.is_valid());
    }

    #[test]
    fn web_aside_coexists_with_a_page_aside() {
        let mut g = SurfaceGraph::new();
        g.insert(main_s("home"));
        g.insert(aside_s("assistant", Edge::Right));
        // A browser aside does NOT evict a non-web (declared/page) aside; both
        // coexist under the expanded cap of 2.
        let (next, decision) = arbitrate(
            &g,
            web_aside_s("browser-1", "https://a.example", Edge::Left),
            &Policy::default(),
            SizeClass::Expanded,
        );
        assert_eq!(decision, Decision::Accepted);
        assert!(next.get("assistant").is_some());
        assert!(next.get("browser-1").is_some());
        assert_eq!(next.asides().len(), 2);
        assert!(next.is_valid());
    }

    #[test]
    fn aside_peer_fallback_on_compact() {
        let mut g = SurfaceGraph::new();
        g.insert(main_s("home"));
        let (next, decision) = arbitrate(
            &g,
            aside_s("assistant", Edge::Right),
            &Policy::default(),
            SizeClass::Compact,
        );
        assert_eq!(decision, Decision::PeerFallback);
        // promoted to a main, no longer an aside.
        assert_eq!(next.role_of("assistant"), Some(Role::Main));
        assert!(next.is_valid());
    }

    #[test]
    fn aside_without_primary_promotes_to_main() {
        // No main yet, expanded (room for asides) — an aside has nothing to
        // dock to, so it must become the main, keeping the graph valid.
        let g = SurfaceGraph::new();
        let (next, decision) = arbitrate(
            &g,
            aside_s("assistant", Edge::Right),
            &Policy::default(),
            SizeClass::Expanded,
        );
        assert_eq!(decision, Decision::DowngradedRole);
        assert_eq!(next.role_of("assistant"), Some(Role::Main));
        assert!(next.is_valid());
    }

    #[test]
    fn arbitrate_is_pure_and_keeps_graph_valid() {
        let mut g = SurfaceGraph::new();
        g.insert(main_s("home"));
        let before = g.surfaces().len();
        let (next, _) = arbitrate(
            &g,
            aside_s("x", Edge::Left),
            &Policy::default(),
            SizeClass::Expanded,
        );
        // original graph untouched (pure); result is valid.
        assert_eq!(g.surfaces().len(), before);
        assert!(next.is_valid());
    }

    // ---- serde round-trip (shared core <-> JSON for ui.json / FFI) ----

    #[test]
    fn surface_json_round_trip() {
        let s = aside_s("assistant", Edge::Right);
        let json = serde_json::to_string(&s).unwrap();
        let back: Surface = serde_json::from_str(&json).unwrap();
        assert_eq!(s, back);
    }
}
