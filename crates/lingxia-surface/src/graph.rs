//! The Surface Graph: single source of truth, invariants (§1.3), state
//! transitions (§1.5), and the two-axis derivation into `DerivedLayout` (§2/§6).

use serde::{Deserialize, Serialize};

use crate::layout::{
    Axis, BottomOwner, DerivedLayout, LayoutTree, SizeClass, SplitForm, SwitcherForm,
};
use crate::model::{Role, Surface, SurfaceId};

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

    /// Insert (or replace by id) a surface, then re-converge invariants (§1.5).
    pub fn insert(&mut self, surface: Surface) {
        let modal = surface.is_modal_float();
        if modal {
            self.modal_focus_stack.push(self.focused_surface_id.clone());
        }
        if let Some(existing) = self.surfaces.iter_mut().find(|s| s.id == surface.id) {
            *existing = surface;
        } else {
            self.surfaces.push(surface);
        }
        self.converge_after_insert();
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

    /// Remove a surface and re-converge per the §1.5 transition rules.
    /// Returns the ids actually removed (the target, plus cascaded asides).
    pub fn remove(&mut self, id: &str) -> Vec<SurfaceId> {
        let Some(pos) = self.surfaces.iter().position(|s| s.id == id) else {
            return Vec::new();
        };
        let removed = self.surfaces.remove(pos);
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
            self.focused_surface_id = self.focus_fallback();
        }
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
        if self.get(id).is_some() {
            self.focused_surface_id = Some(id.to_string());
            true
        } else {
            false
        }
    }

    /// Check the §1.3 invariants. Returns the list of violations (empty = ok).
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

    /// Two-axis derivation (§2/§6): produce the platform-agnostic `DerivedLayout`.
    pub fn derive_layout(&self, size_class: SizeClass) -> DerivedLayout {
        let main_count = self.mains().len();
        let aside_count = self.asides().len();
        // On compact, asides peer-fall-back into the switcher, so they count as
        // switchable items; elsewhere only mains drive the switcher.
        let switchable = if size_class == SizeClass::Compact {
            main_count + aside_count
        } else {
            main_count
        };

        let switcher_form = if switchable > 1 {
            match size_class {
                SizeClass::Expanded => SwitcherForm::Sidebar,
                SizeClass::Medium => SwitcherForm::Rail,
                SizeClass::Compact => SwitcherForm::Drawer,
            }
        } else {
            SwitcherForm::None
        };

        let split_form = if aside_count > 0 {
            match size_class {
                SizeClass::Expanded => SplitForm::Split,
                SizeClass::Medium => SplitForm::Collapsible,
                // compact has no side-by-side: asides fall back to mains (§3.4/§6).
                SizeClass::Compact => SplitForm::PeerFallback,
            }
        } else {
            SplitForm::None
        };

        // Bottom belongs to the Host switcher only when there's a switcher in
        // compact; a lone switchable item gives the bottom back to the app (§6.2).
        let bottom_owner = if size_class == SizeClass::Compact && switchable > 1 {
            BottomOwner::Host
        } else {
            BottomOwner::App
        };

        // Asides are docked beside the main only outside compact; on compact
        // they peer-fall-back into the switcher (see `canonical_layout`) and are
        // not separately docked, so the dock list is empty there.
        let asides = if size_class == SizeClass::Compact {
            Vec::new()
        } else {
            self.asides()
                .iter()
                .map(|s| crate::layout::AsideEntry {
                    id: s.id.clone(),
                    edge: s.placement.edge,
                })
                .collect()
        };

        DerivedLayout {
            size_class,
            switcher_form,
            split_form,
            bottom_owner,
            layout_tree: self.canonical_layout(size_class),
            asides,
        }
    }

    /// Build the canonical authoritative `LayoutTree` from current state:
    /// mains → switcher (tabs), asides → split. On compact asides fold into the
    /// switcher (peer-fall-back). Floats are never in the tree.
    pub fn canonical_layout(&self, size_class: SizeClass) -> Option<LayoutTree> {
        let main_ids = self.main_ids();
        if main_ids.is_empty() {
            return None;
        }
        let aside_ids: Vec<SurfaceId> = self.asides().iter().map(|s| s.id.clone()).collect();
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

        // Compact: asides fold into the main switcher (peer-fall-back), one tree.
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
        children.extend(aside_ids.into_iter().map(|id| LayoutTree::Leaf { surface_id: id }));
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
        .or_else(|| surfaces.iter().take(removed_pos).rev().find(|s| s.role == Role::Main))
        .map(|s| s.id.clone())
}
