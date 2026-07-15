//! The Surface Graph: single source of truth, invariants, state transitions,
//! and the two-axis derivation into `DerivedLayout`.

use serde::{Deserialize, Serialize};

use crate::layout::{
    Axis, BottomOwner, DerivedLayout, LayoutPresentationPlan, LayoutTree, PlanAside, SizeClass,
    SplitForm, SwitcherForm,
};
use crate::model::{Role, SlotKind, Surface, SurfaceId, SurfaceState};

/// One window's graph. Surfaces are kept in insertion order so that
/// "adjacent main" succession and "oldest aside" replacement are deterministic.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SurfaceGraph {
    surfaces: Vec<Surface>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub active_main_id: Option<SurfaceId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub focused_surface_id: Option<SurfaceId>,
    /// Focus snapshots pushed when a modal float opens, popped on its close.
    #[serde(default, skip)]
    modal_focus_stack: Vec<Option<SurfaceId>>,
    /// Aside slot kinds in least- to most-recently-used order. Kept separate
    /// from `surfaces` so focus/reopen affects admission without reordering tabs.
    #[serde(default)]
    aside_slot_mru: Vec<SlotKind>,
    /// Aside children in least- to most-recently-used order. This is separate
    /// from insertion order so hiding an active child can reveal the most
    /// recently used sibling without reordering the slot's tabs.
    #[serde(default)]
    aside_child_mru: Vec<SurfaceId>,
}

impl SurfaceGraph {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn surfaces(&self) -> &[Surface] {
        &self.surfaces
    }

    pub fn get(&self, id: &str) -> Option<&Surface> {
        self.surfaces.iter().find(|s| s.id == id)
    }

    pub fn role_of(&self, id: &str) -> Option<Role> {
        self.get(id).map(|s| s.role)
    }

    pub fn mains(&self) -> Vec<&Surface> {
        self.by_role(Role::Main)
    }
    pub fn asides(&self) -> Vec<&Surface> {
        self.by_role(Role::Aside)
    }
    pub fn floats(&self) -> Vec<&Surface> {
        self.by_role(Role::Float)
    }

    fn by_role(&self, role: Role) -> Vec<&Surface> {
        self.surfaces.iter().filter(|s| s.role == role).collect()
    }

    fn main_ids(&self) -> Vec<SurfaceId> {
        self.surfaces
            .iter()
            .filter(|s| s.role == Role::Main)
            .map(|s| s.id.clone())
            .collect()
    }

    /// Insert (or replace by id) a surface, then re-converge invariants.
    pub fn insert(&mut self, surface: Surface) {
        let surface_id = surface.id.clone();
        let modal = surface.is_modal_float();
        let previous_slot = self
            .get(&surface.id)
            .filter(|existing| existing.role == Role::Aside)
            .map(|existing| existing.content.slot_kind());
        let next_slot = (surface.role == Role::Aside).then(|| surface.content.slot_kind());
        if modal {
            self.modal_focus_stack.push(self.focused_surface_id.clone());
        }
        if let Some(existing) = self.surfaces.iter_mut().find(|s| s.id == surface.id) {
            *existing = surface;
        } else {
            self.surfaces.push(surface);
        }
        if let Some(previous) = previous_slot
            && Some(previous) != next_slot
            && !self
                .asides()
                .iter()
                .any(|aside| aside.content.slot_kind() == previous)
        {
            self.aside_slot_mru.retain(|kind| *kind != previous);
        }
        if let Some(kind) = next_slot {
            self.touch_aside_slot(kind);
            self.touch_aside_child(&surface_id);
        }
        self.converge_after_insert();
    }

    fn touch_aside_child(&mut self, id: &str) {
        self.aside_child_mru.retain(|entry| entry != id);
        self.aside_child_mru.push(id.to_string());
    }

    fn touch_aside_slot(&mut self, kind: SlotKind) {
        self.aside_slot_mru.retain(|entry| *entry != kind);
        self.aside_slot_mru.push(kind);
    }

    fn prune_aside_slot_mru(&mut self) {
        let live: std::collections::HashSet<SlotKind> = self
            .asides()
            .iter()
            .map(|aside| aside.content.slot_kind())
            .collect();
        self.aside_slot_mru.retain(|kind| live.contains(kind));
        let live_children: std::collections::HashSet<SurfaceId> = self
            .asides()
            .into_iter()
            .map(|surface| surface.id.clone())
            .collect();
        self.aside_child_mru.retain(|id| live_children.contains(id));
    }

    fn converge_after_insert(&mut self) {
        // First main becomes active + focused.
        if self.active_main_id.is_none()
            && let Some(first) = self.main_ids().first().cloned()
        {
            self.active_main_id = Some(first.clone());
            if self.focused_surface_id.is_none() {
                self.focused_surface_id = Some(first);
            }
        }
        // A freshly inserted last surface still focuses if nothing else did.
        if self.focused_surface_id.is_none()
            && let Some(s) = self.surfaces.last()
        {
            self.focused_surface_id = Some(s.id.clone());
        }
    }

    /// Remove a surface and re-converge per the transition rules.
    /// Returns the ids actually removed (the target, plus cascaded asides).
    pub fn remove(&mut self, id: &str) -> Vec<SurfaceId> {
        let Some(pos) = self.surfaces.iter().position(|s| s.id == id) else {
            return Vec::new();
        };
        let removed = self.surfaces.remove(pos);
        let removed_aside_kind = (removed.role == Role::Aside).then(|| removed.content.slot_kind());
        let mut removed_ids = vec![removed.id.clone()];

        // Succession for active main.
        if self.active_main_id.as_deref() == Some(id) {
            self.active_main_id = pick_successor_main(&self.surfaces, pos);
        }

        // Last main gone ⇒ all asides close (no primary, no companion).
        if self.mains().is_empty() {
            let aside_ids: Vec<SurfaceId> = self
                .surfaces
                .iter()
                .filter(|s| s.role == Role::Aside)
                .map(|s| s.id.clone())
                .collect();
            self.surfaces.retain(|s| s.role != Role::Aside);
            removed_ids.extend(aside_ids);
        }

        // Modal float closing: restore the pre-popup focus snapshot.
        if removed.is_modal_float()
            && let Some(snapshot) = self.modal_focus_stack.pop()
        {
            self.focused_surface_id = snapshot;
        }

        // Focus fallback if the focused surface is gone.
        if self
            .focused_surface_id
            .as_deref()
            .is_none_or(|f| self.get(f).is_none())
        {
            self.focused_surface_id = removed_aside_kind
                .and_then(|kind| {
                    self.aside_child_mru.iter().rev().find_map(|candidate| {
                        self.get(candidate).and_then(|surface| {
                            (surface.state == SurfaceState::Mounted
                                && surface.role == Role::Aside
                                && surface.content.slot_kind() == kind)
                                .then(|| surface.id.clone())
                        })
                    })
                })
                .or_else(|| self.focus_fallback());
        }
        self.prune_aside_slot_mru();
        removed_ids
    }

    /// Focus fallback order: active main → an aside owned by it → none.
    fn focus_fallback(&self) -> Option<SurfaceId> {
        if let Some(active) = &self.active_main_id
            && self.get(active).is_some()
        {
            return Some(active.clone());
        }
        self.asides().first().map(|s| s.id.clone())
    }

    /// Switch which main is primary. No-op if `id` is not an existing main.
    pub fn set_active_main(&mut self, id: &str) -> bool {
        if self.role_of(id) == Some(Role::Main) {
            self.active_main_id = Some(id.to_string());
            true
        } else {
            false
        }
    }

    /// Focus any surface (any role).
    pub fn set_focus(&mut self, id: &str) -> bool {
        let aside_kind = self
            .get(id)
            .filter(|surface| surface.role == Role::Aside)
            .map(|surface| surface.content.slot_kind());
        if self.get(id).is_some() {
            if let Some(surface) = self.surfaces.iter_mut().find(|surface| surface.id == id) {
                surface.state = SurfaceState::Mounted;
            }
            self.focused_surface_id = Some(id.to_string());
            if let Some(kind) = aside_kind {
                self.touch_aside_slot(kind);
                self.touch_aside_child(id);
            }
            true
        } else {
            false
        }
    }

    /// Show a live surface without changing its identity. Main selection is
    /// handled separately; aside/float visibility is represented in the graph.
    pub fn show(&mut self, id: &str) -> bool {
        let Some(role) = self.role_of(id) else {
            return false;
        };
        if let Some(surface) = self.surfaces.iter_mut().find(|surface| surface.id == id) {
            surface.state = SurfaceState::Mounted;
        }
        match role {
            Role::Main => self.set_active_main(id),
            Role::Aside | Role::Float => self.set_focus(id),
        }
    }

    /// Hide a live aside/float while retaining it. Hiding the active child of
    /// an aside slot selects that slot's most-recent visible sibling; when no
    /// sibling remains the slot disappears and focus returns to the main.
    pub fn hide(&mut self, id: &str) -> bool {
        let Some(surface) = self.get(id) else {
            return false;
        };
        if surface.role == Role::Main {
            return false;
        }
        let role = surface.role;
        let slot_kind = (role == Role::Aside).then(|| surface.content.slot_kind());
        if let Some(surface) = self.surfaces.iter_mut().find(|surface| surface.id == id) {
            surface.state = SurfaceState::Hidden;
        }
        if self.focused_surface_id.as_deref() == Some(id) {
            let sibling = slot_kind.and_then(|kind| {
                self.aside_child_mru.iter().rev().find_map(|candidate| {
                    self.get(candidate).and_then(|surface| {
                        (surface.id != id
                            && surface.role == Role::Aside
                            && surface.state == SurfaceState::Mounted
                            && surface.content.slot_kind() == kind)
                            .then(|| surface.id.clone())
                    })
                })
            });
            self.focused_surface_id = sibling.or_else(|| self.active_main_id.clone());
        }
        true
    }

    /// Group the asides into per-kind slots (lxapp / browser / native), in
    /// first-open order, with tab order = open order. Admission marks the
    /// most recently used slots visible — the size class caps the count
    /// (expanded 3 / medium 1 / compact 0), and physical width caps it
    /// further so the main keeps its minimum. Hidden slots stay alive and
    /// are never evicted; widening the container brings them back.
    ///
    /// `width` is the container's workspace width (window minus sidebar) and
    /// `policy` carries the min-size tokens. See [`Self::aside_slots`] for the
    /// size-class-only convenience used by tests.
    pub fn aside_slots_admitted(
        &self,
        size_class: SizeClass,
        width: f64,
        policy: &crate::arbitrate::Policy,
    ) -> Vec<crate::layout::PlanAsideSlot> {
        let max_visible = policy.max_asides(size_class);
        let mut slots = self.aside_slots_recency(usize::MAX);
        for slot in &mut slots {
            slot.visible = false;
        }
        // §3.3 physical admission: reserve the main's minimum, then admit
        // slots greedily in MRU order until the count ceiling is full.
        // Left/right slots must keep their minimum width; top/bottom slots
        // overlay the main's width and do not consume horizontal budget. A
        // candidate that does not fit must not prevent an older fitting slot
        // from being considered.
        let candidates = self.slot_indices_by_recency(&slots);
        let mut horizontal_used = policy.main_min_width;
        let mut admitted = 0;
        for i in candidates {
            if admitted == max_visible {
                break;
            }
            if slots[i].active_child.is_none() {
                continue;
            }
            if size_class == SizeClass::Compact {
                slots[i].visible = true;
                admitted += 1;
                continue;
            }
            let horizontal = !matches!(
                slots[i].edge,
                Some(crate::model::Edge::Top) | Some(crate::model::Edge::Bottom)
            );
            if horizontal && horizontal_used + policy.aside_min_width > width {
                continue;
            }
            if horizontal {
                horizontal_used += policy.aside_min_width;
            }
            slots[i].visible = true;
            admitted += 1;
        }
        slots
    }

    /// Size-class-only admission (count ceiling, no physical width). Retained
    /// for callers/tests that don't have a container width.
    pub fn aside_slots(&self, size_class: SizeClass) -> Vec<crate::layout::PlanAsideSlot> {
        self.aside_slots_recency(crate::arbitrate::Policy::default().max_asides(size_class))
    }

    /// Shared slot grouping + count-based (recency) admission. Both public
    /// entry points build on this; `aside_slots_admitted` then layers the
    /// physical width check on top.
    fn aside_slots_recency(&self, max_visible: usize) -> Vec<crate::layout::PlanAsideSlot> {
        let asides = self.asides();
        let mut slots: Vec<crate::layout::PlanAsideSlot> = Vec::new();
        // `asides()` preserves insertion order, so child pushes keep tab
        // order == open order and slot order == first-open order.
        for surface in &asides {
            let kind = surface.content.slot_kind();
            let slot = match slots.iter_mut().find(|slot| slot.kind == kind) {
                Some(slot) => slot,
                None => {
                    slots.push(crate::layout::PlanAsideSlot {
                        kind,
                        edge: None,
                        children: Vec::new(),
                        active_child: None,
                        visible: false,
                        overlay: false,
                    });
                    slots.last_mut().expect("just pushed")
                }
            };
            slot.children.push(surface.id.clone());
            // The most recently placed child's explicit edge wins.
            if surface.placement.edge.is_some() {
                slot.edge = surface.placement.edge;
            }
        }
        for slot in &mut slots {
            // Active child: the focused surface when it lives in this slot,
            // else the newest child.
            slot.active_child = self
                .focused_surface_id
                .as_ref()
                .filter(|id| {
                    slot.children.contains(id)
                        && self
                            .get(id)
                            .is_some_and(|surface| surface.state == SurfaceState::Mounted)
                })
                .cloned()
                .or_else(|| {
                    self.aside_child_mru.iter().rev().find_map(|id| {
                        (slot.children.contains(id)
                            && self
                                .get(id)
                                .is_some_and(|surface| surface.state == SurfaceState::Mounted))
                        .then(|| id.clone())
                    })
                });
        }
        for slot_index in self
            .slot_indices_by_recency(&slots)
            .into_iter()
            .take(max_visible)
        {
            slots[slot_index].visible = true;
        }
        slots
    }

    /// Slot indices from most to least recently used. Current graphs always
    /// have `aside_slot_mru`; the child insertion fallback keeps older
    /// serialized graphs deterministic on first load.
    fn slot_indices_by_recency(&self, slots: &[crate::layout::PlanAsideSlot]) -> Vec<usize> {
        let asides = self.asides();
        let mut indices: Vec<usize> = (0..slots.len()).collect();
        indices.sort_by_key(|index| {
            let slot = &slots[*index];
            let explicit = self
                .aside_slot_mru
                .iter()
                .position(|kind| *kind == slot.kind);
            let fallback = asides
                .iter()
                .rposition(|surface| slot.children.contains(&surface.id))
                .unwrap_or(0);
            std::cmp::Reverse((explicit.is_some(), explicit.unwrap_or(fallback), fallback))
        });
        indices
    }

    /// Check the invariants. Returns the list of violations (empty = ok).
    pub fn check_invariants(&self) -> Vec<String> {
        let mut v = Vec::new();
        let mains = self.mains().len();
        let asides = self.asides().len();
        if asides > 0 && mains == 0 {
            v.push("asides>0 but mains==0 (no primary, no companion)".into());
        }
        match &self.active_main_id {
            Some(id) if self.role_of(id) != Some(Role::Main) => {
                v.push(format!("activeMainId '{id}' is not a main"));
            }
            None if mains > 0 => v.push("mains exist but activeMainId is None".into()),
            _ => {}
        }
        if let Some(f) = &self.focused_surface_id
            && self.get(f).is_none()
        {
            v.push(format!("focusedSurfaceId '{f}' does not exist"));
        }
        // Unique ids.
        let mut seen = std::collections::HashSet::new();
        for s in &self.surfaces {
            if !seen.insert(&s.id) {
                v.push(format!("duplicate surface id '{}'", s.id));
            }
        }
        v
    }

    pub fn is_valid(&self) -> bool {
        self.check_invariants().is_empty()
    }

    /// Two-axis derivation: produce the platform-agnostic `DerivedLayout`.
    pub fn derive_layout(&self, size_class: SizeClass) -> DerivedLayout {
        let main_count = self.mains().len();
        let aside_count = self.asides().len();
        let switcher_form = if size_class != SizeClass::Compact && main_count > 1 {
            match size_class {
                SizeClass::Expanded => SwitcherForm::Sidebar,
                SizeClass::Medium => SwitcherForm::Rail,
                SizeClass::Compact => SwitcherForm::None,
            }
        } else {
            SwitcherForm::None
        };

        let split_form = if aside_count > 0 {
            match size_class {
                SizeClass::Expanded => SplitForm::Split,
                SizeClass::Medium => SplitForm::Collapsible,
                // compact has no side-by-side: asides present full-screen.
                SizeClass::Compact => SplitForm::FullScreen,
            }
        } else {
            SplitForm::None
        };

        DerivedLayout {
            size_class,
            switcher_form,
            split_form,
            bottom_owner: BottomOwner::App,
            layout_tree: self.canonical_layout(size_class),
        }
    }

    /// Flatten the graph + derivation into the stable, skin-bindable
    /// [`LayoutPresentationPlan`]: the primary mains, asides (with edge +
    /// preferred size), floats, and the full tree. `width` is the container
    /// workspace width and `policy` the admission tokens, so slot visibility
    /// respects both the size-class ceiling and the physical fit (§3.3).
    pub fn presentation_plan(
        &self,
        size_class: SizeClass,
        width: f64,
        policy: &crate::arbitrate::Policy,
    ) -> LayoutPresentationPlan {
        let derived = self.derive_layout(size_class);

        let asides: Vec<PlanAside> = self
            .asides()
            .iter()
            .filter(|s| s.state == SurfaceState::Mounted)
            .map(|s| PlanAside {
                id: s.id.clone(),
                edge: s.placement.edge,
                preferred_size: s.placement.preferred_size,
            })
            .collect();
        let aside_slots = self.aside_slots_admitted(size_class, width, policy);

        LayoutPresentationPlan {
            size_class: derived.size_class,
            bottom_owner: derived.bottom_owner,
            switcher_form: derived.switcher_form,
            split_form: derived.split_form,
            mains: self.main_ids(),
            // The active main the skin should attach to the primary content
            // area. Mirrors `canonical_layout`'s `Tabs.activeId` fallback so the
            // plan and the tree always agree on which main is primary.
            active_main_id: self
                .active_main_id
                .clone()
                .or_else(|| self.main_ids().first().cloned()),
            asides,
            aside_slots,
            // Floats are popups above the layout and are valid at every size
            // class (no compact gating), so they always appear in the plan. Each
            // carries the float's render-relevant `FloatSpec`; a float surface
            // missing its spec falls back to the default behavior.
            floats: self
                .floats()
                .iter()
                .filter(|s| s.state == SurfaceState::Mounted)
                .map(|s| {
                    let spec = s.float.clone().unwrap_or_default();
                    crate::layout::PlanFloat {
                        id: s.id.clone(),
                        anchor: spec.anchor,
                        dismiss: spec.dismiss,
                        modal: spec.modal,
                    }
                })
                .collect(),
            tree: derived.layout_tree,
        }
    }

    /// Build the canonical authoritative `LayoutTree` from current state:
    /// mains → tabs when needed, asides → split. Compact has no side-by-side
    /// dock; asides share the main tree. Floats are never in the tree.
    pub fn canonical_layout(&self, size_class: SizeClass) -> Option<LayoutTree> {
        let main_ids = self.main_ids();
        if main_ids.is_empty() {
            return None;
        }
        let aside_ids: Vec<SurfaceId> = self
            .asides()
            .iter()
            .filter(|surface| surface.state == SurfaceState::Mounted)
            .map(|s| s.id.clone())
            .collect();
        let active = self
            .active_main_id
            .clone()
            .unwrap_or_else(|| main_ids[0].clone());

        let tabs_of = |ids: Vec<SurfaceId>| {
            if ids.len() == 1 {
                LayoutTree::Leaf {
                    surface_id: ids[0].clone(),
                }
            } else {
                LayoutTree::Tabs {
                    active_id: active.clone(),
                    children: ids,
                }
            }
        };

        // Compact: no split; asides share one full-screen tree with mains.
        if size_class == SizeClass::Compact {
            let mut ids = main_ids;
            ids.extend(aside_ids);
            return Some(tabs_of(ids));
        }

        let main_node = tabs_of(main_ids);
        if aside_ids.is_empty() {
            return Some(main_node);
        }
        let mut children = vec![main_node];
        children.extend(
            aside_ids
                .into_iter()
                .map(|id| LayoutTree::Leaf { surface_id: id }),
        );
        let n = children.len();
        Some(LayoutTree::Split {
            axis: Axis::Horizontal,
            weights: vec![1.0 / n as f64; n],
            children,
        })
    }
}

/// Pick the main that should become active after the one at `removed_pos` is
/// gone: prefer the next main, else the previous, else none.
fn pick_successor_main(surfaces: &[Surface], removed_pos: usize) -> Option<SurfaceId> {
    surfaces
        .iter()
        .skip(removed_pos)
        .find(|s| s.role == Role::Main)
        .or_else(|| {
            surfaces
                .iter()
                .take(removed_pos)
                .rev()
                .find(|s| s.role == Role::Main)
        })
        .map(|s| s.id.clone())
}
