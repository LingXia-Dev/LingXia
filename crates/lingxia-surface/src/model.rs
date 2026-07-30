//! Surface node data model (§1.1 of the Adaptive Surface Layout spec).
//!
//! A `Surface` is one content unit in the graph. Its `role` expresses the
//! relationship to the main content; `content` is what it shows; `owner`
//! drives lifecycle; `placement` is a non-authoritative hint.

use serde::{Deserialize, Serialize};

use crate::content::SurfaceContent;

/// Stable identifier of a surface within a graph.
pub type SurfaceId = String;

/// Relationship of a surface to the main content. The single core abstraction
/// behind every platform skin (window / panel / sheet / tab …).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    /// Switchable top-level content. Only one is the active `primary` at a time.
    Main,
    /// Companion shown beside the main content (split axis).
    Aside,
    /// Floats above, never occupies the main layout.
    Float,
}

/// Which edge a surface docks to (asides) or anchors from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Edge {
    Left,
    Right,
    Top,
    Bottom,
}

/// Owner scope: decides when the surface is closed (§5).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "scope", rename_all = "camelCase")]
pub enum SurfaceOwner {
    Page { page_instance_id: String },
    Lxapp { app_id: String },
    Host,
}

/// Placement hint (input). Authoritative layout is the `LayoutTree` (output).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Placement {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub edge: Option<Edge>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub preferred_size: Option<f64>,
}

/// Runtime state of a surface.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SurfaceState {
    Mounted,
    Hidden,
    Minimized,
}

/// How a `float` surface anchors.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "to", rename_all = "lowercase")]
pub enum FloatAnchor {
    Screen,
    Surface { surface_id: SurfaceId },
}

/// How a `float` surface is dismissed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum FloatDismiss {
    TapOutside,
    Manual,
}

/// User interaction contract for a dynamically presented surface.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SurfaceInteraction {
    pub close_button: bool,
    pub dismiss: FloatDismiss,
    pub modal: bool,
}

impl SurfaceInteraction {
    pub const fn standard() -> Self {
        Self {
            close_button: false,
            dismiss: FloatDismiss::TapOutside,
            modal: false,
        }
    }

    pub const fn url_callback() -> Self {
        Self {
            close_button: true,
            dismiss: FloatDismiss::Manual,
            modal: true,
        }
    }

    pub const fn window() -> Self {
        Self {
            close_button: false,
            dismiss: FloatDismiss::Manual,
            modal: false,
        }
    }
}

impl Default for SurfaceInteraction {
    fn default() -> Self {
        Self::standard()
    }
}

/// Minimal semantics carried only by `float` surfaces (§1.1).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FloatSpec {
    pub anchor: FloatAnchor,
    pub dismiss: FloatDismiss,
    /// Whether it blocks input to layers below (drives focus-restore, §1.5).
    pub modal: bool,
    /// Whether the platform renders the standard circular close control.
    pub close_button: bool,
}

impl Default for FloatSpec {
    fn default() -> Self {
        Self {
            anchor: FloatAnchor::Screen,
            dismiss: FloatDismiss::TapOutside,
            modal: false,
            close_button: false,
        }
    }
}

/// One node in the Surface Graph.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Surface {
    pub id: SurfaceId,
    pub role: Role,
    pub content: SurfaceContent,
    pub owner: SurfaceOwner,
    #[serde(default)]
    pub placement: Placement,
    pub state: SurfaceState,
    /// Present only for `role == Float`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub float: Option<FloatSpec>,
}

impl Surface {
    pub fn lxapp(id: impl Into<SurfaceId>, role: Role, app_id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            role,
            content: SurfaceContent::Lxapp {
                app_id: app_id.into(),
                path: None,
            },
            owner: SurfaceOwner::Host,
            placement: Placement::default(),
            state: SurfaceState::Mounted,
            float: if role == Role::Float {
                Some(FloatSpec::default())
            } else {
                None
            },
        }
    }

    pub fn native(id: impl Into<SurfaceId>, role: Role, capability: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            role,
            content: SurfaceContent::Native {
                capability: capability.into(),
            },
            owner: SurfaceOwner::Host,
            placement: Placement::default(),
            state: SurfaceState::Mounted,
            float: (role == Role::Float).then(FloatSpec::default),
        }
    }

    pub fn browser(id: impl Into<SurfaceId>, role: Role, initial_url: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            role,
            content: SurfaceContent::Browser {
                initial_url: initial_url.into(),
                reuse_by_url: true,
            },
            owner: SurfaceOwner::Host,
            placement: Placement::default(),
            state: SurfaceState::Mounted,
            float: (role == Role::Float).then(FloatSpec::default),
        }
    }

    pub fn is_modal_float(&self) -> bool {
        self.role == Role::Float && self.float.as_ref().is_some_and(|f| f.modal)
    }
}
