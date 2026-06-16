//! Layout output types: sizeClass, LayoutTree, and the per-platform
//! `DerivedLayout` that skins bind to (§2 and §6 of the spec).

use serde::{Deserialize, Serialize};

use crate::model::SurfaceId;

/// Available-width band. Aligned to Material breakpoints (§6.1) and computed
/// from the *container* width, not the physical screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SizeClass {
    Compact,
    Medium,
    Expanded,
}

/// Compact `< 600`, Medium `600..=840`, Expanded `> 840`.
pub const COMPACT_MAX: f64 = 600.0;
pub const MEDIUM_MAX: f64 = 840.0;
/// Default hysteresis margin to avoid breakpoint thrashing (§6.1).
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
        // Within the hysteresis band around a boundary, stick with prev.
        let near_lower = (width - COMPACT_MAX).abs() < margin;
        let near_upper = (width - MEDIUM_MAX).abs() < margin;
        if near_lower || near_upper {
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
    /// Collect every `surfaceId` referenced by the tree (for invariant checks).
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

    /// Structural invariants (§2): tabs.activeId ∈ children, split weights match
    /// children and are positive, splits have ≥2 children.
    pub fn validate(&self) -> Result<(), String> {
        match self {
            LayoutTree::Leaf { .. } | LayoutTree::Freeform { .. } => Ok(()),
            LayoutTree::Tabs { active_id, children } => {
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
    Drawer,
    Chip,
}

/// How asides (the split axis) render. `sheet` belongs to `float`, not here.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SplitForm {
    None,
    Split,
    Collapsible,
    PeerFallback,
}

/// Who owns the bottom bar in compact (§6.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum BottomOwner {
    Host,
    App,
}

/// Shared-core output bound by each platform skin (§6).
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
