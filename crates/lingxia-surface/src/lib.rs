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
pub use layout::PlanAsideSlot;
pub use layout::{
    Axis, BottomOwner, DEFAULT_HYSTERESIS, DerivedLayout, LayoutPresentationPlan, LayoutTree,
    PlanAside, PlanFloat, SizeClass, SplitForm, SwitcherForm,
};
pub use manager::SurfaceManager;
pub use model::{
    Edge, FloatAnchor, FloatDismiss, FloatSpec, Placement, Role, SlotKind, Surface, SurfaceContent,
    SurfaceId, SurfaceOwner, SurfaceState,
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
    fn terminal_aside_s(id: &str, edge: Edge) -> Surface {
        let mut s = Surface::entry(id, Role::Aside, "terminal");
        s.placement.edge = Some(edge);
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
    fn aside_splits_on_expanded_fullscreen_on_compact() {
        let mut g = SurfaceGraph::new();
        g.insert(main_s("home"));
        g.insert(aside_s("assistant", Edge::Right));
        assert_eq!(
            g.derive_layout(SizeClass::Expanded).split_form,
            SplitForm::Split
        );
        assert_eq!(
            g.derive_layout(SizeClass::Compact).split_form,
            SplitForm::FullScreen
        );
    }

    #[test]
    fn compact_plan_keeps_existing_aside_desired() {
        let mut g = SurfaceGraph::new();
        g.insert(main_s("home"));
        g.insert(aside_s("assistant", Edge::Right));

        let plan = g.presentation_plan(
            SizeClass::Compact,
            390.0,
            &crate::arbitrate::Policy::default(),
        );
        assert_eq!(plan.split_form, SplitForm::FullScreen);
        assert!(plan.asides.iter().any(|aside| aside.id == "assistant"));
        assert!(
            plan.tree
                .as_ref()
                .is_some_and(|tree| tree.surface_ids().iter().any(|id| id == "assistant"))
        );
    }

    #[test]
    fn compact_bottom_owner_stays_app() {
        let mut g = SurfaceGraph::new();
        g.insert(main_s("a"));
        // single main → app owns bottom
        assert_eq!(
            g.derive_layout(SizeClass::Compact).bottom_owner,
            BottomOwner::App
        );
        g.insert(main_s("b"));
        // compact has no separate switcher.
        assert_eq!(
            g.derive_layout(SizeClass::Compact).bottom_owner,
            BottomOwner::App
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
    fn same_kind_asides_join_their_slot() {
        let mut g = SurfaceGraph::new();
        g.insert(main_s("home"));
        g.insert(aside_s("a1", Edge::Right));
        // A second lxapp aside joins the ONE lxapp slot as another tab —
        // nothing is evicted, tab order = open order, newest tab is active.
        let (next, decision) = arbitrate(
            &g,
            aside_s("a2", Edge::Right),
            &Policy::default(),
            SizeClass::Expanded,
        );
        assert_eq!(decision, Decision::MergedIntoTabs);
        assert!(next.get("a1").is_some());
        assert!(next.get("a2").is_some());
        let slots = next.aside_slots(SizeClass::Expanded);
        assert_eq!(slots.len(), 1);
        assert_eq!(slots[0].kind, SlotKind::Lxapp);
        assert_eq!(slots[0].children, vec!["a1".to_string(), "a2".to_string()]);
        assert_eq!(slots[0].active_child.as_deref(), Some("a2"));
        assert!(slots[0].visible);
        assert!(next.is_valid());
    }

    #[test]
    fn slots_hide_beyond_admission_never_evict() {
        let mut next = SurfaceGraph::new();
        next.insert(main_s("home"));
        for request in [
            aside_s("chat", Edge::Right),
            terminal_aside_s("terminal", Edge::Bottom),
            web_aside_s("b1", "https://a.example", Edge::Right),
        ] {
            let (n, d) = arbitrate(&next, request, &Policy::default(), SizeClass::Expanded);
            assert_eq!(d, Decision::Accepted);
            next = n;
        }
        // Three kinds → three slots, all admitted on expanded.
        let expanded = next.aside_slots(SizeClass::Expanded);
        assert_eq!(expanded.len(), 3);
        assert!(expanded.iter().all(|slot| slot.visible));
        // Medium admits only the most recently used slot; the others stay
        // alive hidden — the graph itself never shrinks.
        let medium = next.aside_slots(SizeClass::Medium);
        assert_eq!(medium.iter().filter(|slot| slot.visible).count(), 1);
        assert!(
            medium
                .iter()
                .find(|slot| slot.visible)
                .is_some_and(|slot| slot.kind == SlotKind::Browser)
        );
        assert_eq!(next.asides().len(), 3);
        assert!(next.is_valid());
    }

    #[test]
    fn physical_admission_caps_below_the_count_ceiling() {
        // Two right-docked lxapp/browser slots + one bottom terminal slot.
        let mut next = SurfaceGraph::new();
        next.insert(main_s("home"));
        for request in [
            aside_s("chat", Edge::Right),
            web_aside_s("b1", "https://a.example", Edge::Right),
            terminal_aside_s("terminal", Edge::Bottom),
        ] {
            let (n, _) = arbitrate(&next, request, &Policy::default(), SizeClass::Expanded);
            next = n;
        }
        let policy = Policy::default(); // main_min 360, aside_min 240

        // Wide expanded (1200): main 360 + 2×240 = 840 ≤ 1200 → both right
        // slots fit; the bottom terminal never consumes horizontal budget.
        let wide = next.aside_slots_admitted(SizeClass::Expanded, 1200.0, &policy);
        assert_eq!(wide.iter().filter(|s| s.visible).count(), 3);

        // Narrow expanded (850): count ceiling says 3, but 360 + 240 = 600 ≤
        // 850 fits ONE right slot; the second right slot (600 + 240 = 840 —
        // wait, 840 ≤ 850) — pick 700 to force exactly one.
        let narrow = next.aside_slots_admitted(SizeClass::Expanded, 700.0, &policy);
        let visible_right = narrow
            .iter()
            .filter(|s| s.visible && !matches!(s.edge, Some(Edge::Bottom)))
            .count();
        assert_eq!(visible_right, 1, "only one right slot fits at 700pt");
        // The bottom terminal stays visible regardless of horizontal budget.
        assert!(
            narrow
                .iter()
                .any(|s| s.visible && matches!(s.edge, Some(Edge::Bottom)))
        );
        // Nothing was evicted — the graph still holds all three asides.
        assert_eq!(next.asides().len(), 3);
    }

    #[test]
    fn slot_admission_uses_policy_and_true_focus_recency() {
        let mut graph = SurfaceGraph::new();
        graph.insert(main_s("home"));
        graph.insert(aside_s("chat", Edge::Right));
        graph.insert(web_aside_s("browser", "https://example.com", Edge::Right));

        // Browser was opened last, so it initially owns Medium's one slot.
        let medium = graph.aside_slots(SizeClass::Medium);
        assert_eq!(
            medium
                .iter()
                .find(|slot| slot.visible)
                .map(|slot| slot.kind),
            Some(SlotKind::Browser)
        );

        // Focusing an older slot makes it MRU without changing tab/open order.
        assert!(graph.set_focus("chat"));
        let medium = graph.aside_slots(SizeClass::Medium);
        assert_eq!(
            medium
                .iter()
                .find(|slot| slot.visible)
                .map(|slot| slot.kind),
            Some(SlotKind::Lxapp)
        );

        let policy = Policy {
            max_asides_expanded: 1,
            ..Policy::default()
        };
        let expanded = graph.aside_slots_admitted(SizeClass::Expanded, 1200.0, &policy);
        assert_eq!(expanded.iter().filter(|slot| slot.visible).count(), 1);
        assert_eq!(
            expanded
                .iter()
                .find(|slot| slot.visible)
                .map(|slot| slot.kind),
            Some(SlotKind::Lxapp)
        );
    }

    #[test]
    fn physical_admission_keeps_the_most_recent_horizontal_slot() {
        let mut graph = SurfaceGraph::new();
        graph.insert(main_s("home"));
        graph.insert(aside_s("chat", Edge::Right));
        graph.insert(web_aside_s("browser", "https://example.com", Edge::Right));
        graph.insert(terminal_aside_s("terminal", Edge::Right));

        // 700 fits main + one horizontal slot. The newest slot wins, even
        // though returned slots stay in stable first-open order.
        let admitted = graph.aside_slots_admitted(SizeClass::Expanded, 700.0, &Policy::default());
        let visible: Vec<_> = admitted
            .iter()
            .filter(|slot| slot.visible)
            .map(|slot| slot.kind)
            .collect();
        assert_eq!(visible, vec![SlotKind::Native]);
    }

    #[test]
    fn physical_admission_falls_back_to_an_older_fitting_slot() {
        let mut graph = SurfaceGraph::new();
        graph.insert(main_s("home"));
        let mut terminal = terminal_aside_s("terminal", Edge::Top);
        terminal.placement.edge = Some(Edge::Top);
        graph.insert(terminal);
        graph.insert(aside_s("chat", Edge::Right));

        // Medium admits one slot. The MRU right slot cannot fit beside main,
        // so the older top overlay must be considered instead of leaving the
        // entire aside area empty.
        let admitted = graph.aside_slots_admitted(SizeClass::Medium, 500.0, &Policy::default());
        let visible: Vec<_> = admitted
            .iter()
            .filter(|slot| slot.visible)
            .map(|slot| slot.kind)
            .collect();
        assert_eq!(visible, vec![SlotKind::Native]);
    }

    #[test]
    fn web_asides_coexist_as_tabs() {
        let mut g = SurfaceGraph::new();
        g.insert(main_s("home"));
        g.insert(web_aside_s("browser-1", "https://a.example", Edge::Right));
        // A second browser aside for a DIFFERENT url coexists as another tab of
        // the one multi-tab panel (exempt from the generic aside cap), not
        // replacing the first.
        let (next, decision) = arbitrate(
            &g,
            web_aside_s("browser-2", "https://b.example", Edge::Right),
            &Policy::default(),
            SizeClass::Expanded,
        );
        assert_eq!(decision, Decision::MergedIntoTabs);
        assert!(next.get("browser-1").is_some());
        assert!(next.get("browser-2").is_some());
        assert_eq!(
            next.asides()
                .iter()
                .filter(|s| matches!(s.content, SurfaceContent::Web { .. }))
                .count(),
            2
        );
        assert!(next.is_valid());
    }

    #[test]
    fn web_aside_dedups_by_url() {
        let mut g = SurfaceGraph::new();
        g.insert(main_s("home"));
        g.insert(web_aside_s("browser-1", "https://a.example", Edge::Right));
        // Reopening the same url focuses the existing tab instead of adding a
        // duplicate — no new surface is inserted.
        let (next, decision) = arbitrate(
            &g,
            web_aside_s("browser-2", "https://a.example", Edge::Right),
            &Policy::default(),
            SizeClass::Expanded,
        );
        assert_eq!(decision, Decision::MergedIntoTabs);
        assert!(next.get("browser-1").is_some());
        assert!(next.get("browser-2").is_none());
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
    fn browser_asides_never_evict_a_declared_aside() {
        let mut next = SurfaceGraph::new();
        next.insert(main_s("home"));
        next.insert(aside_s("chat", Edge::Right)); // declared lxapp aside
        // Open browser tabs beyond the generic aside cap (expanded=2). The
        // declared `chat` aside must survive — web + non-web budgets are
        // independent (they coexist as separate side panels).
        for (i, url) in [
            "https://a.example",
            "https://b.example",
            "https://c.example",
        ]
        .iter()
        .enumerate()
        {
            let (n, d) = arbitrate(
                &next,
                web_aside_s(&format!("b{i}"), url, Edge::Right),
                &Policy::default(),
                SizeClass::Expanded,
            );
            // The first web aside claims the browser slot; the rest join it.
            let expected = if i == 0 {
                Decision::Accepted
            } else {
                Decision::MergedIntoTabs
            };
            assert_eq!(d, expected);
            next = n;
        }
        assert!(next.get("chat").is_some(), "declared aside must survive");
        assert_eq!(
            next.asides()
                .iter()
                .filter(|s| matches!(s.content, SurfaceContent::Web { .. }))
                .count(),
            3
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
    fn aside_fullscreen_fallback_on_compact() {
        let mut g = SurfaceGraph::new();
        g.insert(main_s("home"));
        let (next, decision) = arbitrate(
            &g,
            aside_s("assistant", Edge::Right),
            &Policy::default(),
            SizeClass::Compact,
        );
        assert_eq!(decision, Decision::FullScreenFallback);
        // promoted to a main, no longer an aside.
        assert_eq!(next.role_of("assistant"), Some(Role::Main));
        assert_eq!(next.active_main_id.as_deref(), Some("assistant"));
        assert_eq!(
            next.presentation_plan(
                SizeClass::Compact,
                390.0,
                &crate::arbitrate::Policy::default()
            )
            .active_main_id
            .as_deref(),
            Some("assistant")
        );
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
