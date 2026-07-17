//! Layout output types: sizeClass, LayoutTree, and the per-platform
//! `DerivedLayout` that skins bind to.

use serde::{Deserialize, Serialize};

use crate::model::SurfaceId;

/// Available-width band. Aligned to Material breakpoints and computed from the
/// full client-area width, not the physical screen. `Ord` follows the declared
/// order (Compact < Medium < Expanded).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SizeClass {
    Compact,
    Medium,
    Expanded,
}

/// Compact `< 600`, Medium `600..=840`, Expanded `> 840`.
pub const COMPACT_MAX: f64 = 600.0;
pub const MEDIUM_MAX: f64 = 840.0;
/// Default hysteresis margin to avoid breakpoint thrashing.
pub const DEFAULT_HYSTERESIS: f64 = 24.0;

impl SizeClass {
    pub fn from_width(width: f64) -> Self {
        if width < COMPACT_MAX {
            SizeClass::Compact
        } else if width <= MEDIUM_MAX {
            SizeClass::Medium
        } else {
            SizeClass::Expanded
        }
    }

    /// Resolve with hysteresis: only switch class when `width` clears the
    /// boundary by `margin`, otherwise keep `prev` (prevents edge flicker).
    pub fn resolve(prev: Option<SizeClass>, width: f64, margin: f64) -> Self {
        let raw = SizeClass::from_width(width);
        let Some(prev) = prev else { return raw };
        if prev == raw {
            return prev;
        }
        // Only an adjacent transition shares a hysteresis boundary. A jump
        // across two classes must converge immediately.
        let boundary =
            match (prev, raw) {
                (SizeClass::Compact, SizeClass::Medium)
                | (SizeClass::Medium, SizeClass::Compact) => Some(COMPACT_MAX),
                (SizeClass::Medium, SizeClass::Expanded)
                | (SizeClass::Expanded, SizeClass::Medium) => Some(MEDIUM_MAX),
                _ => None,
            };
        if boundary.is_some_and(|boundary| (width - boundary).abs() < margin) {
            prev
        } else {
            raw
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Axis {
    Horizontal,
    Vertical,
}

/// Authoritative layout produced by the Host (output), referencing surfaces
/// only by id. Overlay/float surfaces never appear here.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum LayoutTree {
    Leaf {
        surface_id: SurfaceId,
    },
    Split {
        axis: Axis,
        children: Vec<LayoutTree>,
        weights: Vec<f64>,
    },
    Tabs {
        active_id: SurfaceId,
        children: Vec<SurfaceId>,
    },
    /// Desktop free-floating pane that still lives inside this window's graph.
    Freeform {
        surface_id: SurfaceId,
    },
}

impl LayoutTree {
    /// Collect every `surfaceId` referenced by the tree.
    pub fn surface_ids(&self) -> Vec<SurfaceId> {
        let mut out = Vec::new();
        self.collect_ids(&mut out);
        out
    }

    fn collect_ids(&self, out: &mut Vec<SurfaceId>) {
        match self {
            LayoutTree::Leaf { surface_id } | LayoutTree::Freeform { surface_id } => {
                out.push(surface_id.clone());
            }
            LayoutTree::Tabs { children, .. } => out.extend(children.iter().cloned()),
            LayoutTree::Split { children, .. } => {
                for c in children {
                    c.collect_ids(out);
                }
            }
        }
    }

    /// Structural invariants: tabs.activeId ∈ children, split weights match
    /// children and are positive, splits have ≥2 children.
    pub fn validate(&self) -> Result<(), String> {
        match self {
            LayoutTree::Leaf { .. } | LayoutTree::Freeform { .. } => Ok(()),
            LayoutTree::Tabs {
                active_id,
                children,
            } => {
                if children.is_empty() {
                    return Err("tabs node has no children".into());
                }
                if !children.contains(active_id) {
                    return Err(format!("tabs.activeId '{active_id}' not in children"));
                }
                Ok(())
            }
            LayoutTree::Split {
                children, weights, ..
            } => {
                if children.len() < 2 {
                    return Err("split node needs >= 2 children".into());
                }
                if weights.len() != children.len() {
                    return Err("split weights length != children length".into());
                }
                if weights.iter().any(|w| *w <= 0.0) {
                    return Err("split weights must be > 0".into());
                }
                for c in children {
                    c.validate()?;
                }
                Ok(())
            }
        }
    }
}

/// How the main-switcher renders on this platform/size.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SwitcherForm {
    None,
    Sidebar,
    Rail,
}

/// How asides (the split axis) render. `sheet` belongs to `float`, not here.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SplitForm {
    None,
    Split,
    Collapsible,
    FullScreen,
}

/// Who owns the bottom bar in compact.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum BottomOwner {
    App,
}

/// Shared-core output bound by each platform skin. The pure core view: the
/// resolved `sizeClass`/forms/`bottomOwner` and the authoritative
/// `layoutTree`. The renderable, skin-bindable contract is
/// [`LayoutPresentationPlan`] (derived from this graph), not this type.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DerivedLayout {
    pub size_class: SizeClass,
    pub switcher_form: SwitcherForm,
    pub split_form: SplitForm,
    pub bottom_owner: BottomOwner,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub layout_tree: Option<LayoutTree>,
}

/// One aside in the [`LayoutPresentationPlan`]: the surface id, the requested
/// edge, and its preferred size. Skins read `split_form` to decide whether
/// these asides dock beside the main or present full-screen on compact.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanAside {
    pub id: SurfaceId,
    /// Edge the aside docks to; `None` when no edge was placed (skin default).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub edge: Option<crate::model::Edge>,
    /// Preferred dock size in logical px; `None` lets the skin pick a default.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub preferred_size: Option<f64>,
}

/// One aside slot in the [`LayoutPresentationPlan`]: the aside area holds at
/// most one region per content kind (lxapp / browser / native); the region's
/// contents are its tabs, in open order. Skins render ONE docked panel per
/// slot, with a header tab strip when `children` has more than one entry.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanAsideSlot {
    pub kind: crate::model::SlotKind,
    /// Edge the slot docks to (the most recently placed child's edge wins).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub edge: Option<crate::model::Edge>,
    /// Tab order = open order; never reordered.
    pub children: Vec<SurfaceId>,
    /// The child the slot currently shows.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub active_child: Option<SurfaceId>,
    /// Admitted visible at this size class. Hidden slots stay alive and
    /// reappear when the container widens — they are never evicted.
    pub visible: bool,
    /// Temporarily covers the main instead of consuming dock space. This is
    /// the compact projection and the fallback for an explicitly opened aside
    /// that physical admission cannot fit.
    #[serde(default)]
    pub overlay: bool,
}

/// One float in the [`LayoutPresentationPlan`]: the surface id plus the
/// render-relevant `FloatSpec` semantics (anchor, dismiss, modal). Floats are
/// popups above the layout and are never in the tree, so the skin reads this
/// list to know which popups to show and how each behaves. The reconciler is
/// the single authority for float visibility.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanFloat {
    pub id: SurfaceId,
    /// Where the popup anchors (screen vs. another surface).
    pub anchor: crate::model::FloatAnchor,
    /// How the popup is dismissed (tap-outside vs. manual).
    pub dismiss: crate::model::FloatDismiss,
    /// Whether the popup blocks input to layers below.
    pub modal: bool,
}

/// The stable, complete render contract a skin binds. Unlike the pure-core
/// [`DerivedLayout`], this flattens the graph into the renderable
/// view any skin needs: ordered `mains`, `asides` (with edge + preferred size),
/// `floats`, and the full id-only `tree`. Derived from
/// the graph so the shared core output isn't bound to one skin's needs.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LayoutPresentationPlan {
    pub size_class: SizeClass,
    pub bottom_owner: BottomOwner,
    pub switcher_form: SwitcherForm,
    pub split_form: SplitForm,
    /// Main surface ids, in stable order.
    pub mains: Vec<SurfaceId>,
    /// The currently-active main (the one occupying the primary content area).
    /// Skins drive the active-main switch from this rather than inferring it
    /// from the tree's `Tabs.activeId`. `None` only when there are no mains.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub active_main_id: Option<SurfaceId>,
    /// Asides currently in the layout. `split_form` decides whether they dock
    /// beside the main or present full-screen on compact.
    pub asides: Vec<PlanAside>,
    /// Asides grouped into per-kind slots (lxapp / browser / native) with tab
    /// order and admission visibility. Supersedes per-aside handling; `asides`
    /// stays for skins that have not migrated yet.
    #[serde(default)]
    pub aside_slots: Vec<PlanAsideSlot>,
    /// Floats currently open: popups above the layout (never in the tree),
    /// each carrying its render-relevant `FloatSpec` semantics.
    pub floats: Vec<PlanFloat>,
    /// The full authoritative layout tree (ids only).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tree: Option<LayoutTree>,
}
