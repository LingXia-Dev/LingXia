use std::future::Future;

use lingxia_surface::LayoutPresentationPlan;

use crate::error::PlatformError;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToastIcon {
    Success,
    Error,
    Loading,
    None,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToastPosition {
    Top,
    Center,
    Bottom,
}

#[derive(Debug, Clone)]
pub struct ToastOptions {
    pub title: String,
    pub icon: ToastIcon,
    pub image: Option<String>,
    pub duration: f64,
    pub mask: bool,
    pub position: ToastPosition,
}

#[derive(Debug, Clone)]
pub struct ModalOptions {
    pub title: String,
    pub content: String,
    pub show_cancel: bool,
    pub cancel_text: String,
    pub cancel_color: Option<String>,
    pub confirm_text: String,
    pub confirm_color: Option<String>,
}

#[repr(i32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SurfaceKind {
    Window = 0,
    Overlay = 1,
}

/// The arbitrated role that drives how the platform presents a surface:
/// `Main` = a top-level window/primary, `Aside` = a docked split companion,
/// `Float` = a positioned popup (it keeps its edge/center placement but never
/// splits the main). Distinguishes a float-popup-at-edge from an aside-dock.
#[repr(i32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SurfaceRole {
    #[default]
    Main = 0,
    Aside = 1,
    Float = 2,
}

#[repr(i32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SurfaceContent {
    #[default]
    Page = 0,
    Url = 1,
}

#[repr(i32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SurfacePosition {
    #[default]
    Center = 0,
    Bottom = 1,
    Left = 2,
    Right = 3,
    Top = 4,
}

#[derive(Debug, Clone)]
pub struct SurfaceRequest {
    pub id: String,
    pub app_id: String,
    pub path: String,
    pub session_id: u64,
    pub page_instance_id: String,
    pub content: SurfaceContent,
    pub kind: SurfaceKind,
    pub width: f64,
    pub height: f64,
    pub width_ratio: f64,
    pub height_ratio: f64,
    pub position: SurfacePosition,
    /// Arbitrated role; the platform uses it to decide dock vs popup vs window.
    pub role: SurfaceRole,
}

pub trait SurfacePresenter: Send + Sync + 'static {
    /// New (Adaptive Surface Layout) contract: the shared core resolves a
    /// `LayoutPresentationPlan` for one window/graph and the platform skin binds
    /// it. This supersedes the per-request `present_surface` path (the legacy
    /// methods below are migrated away in the B-series and then removed).
    fn present_layout(
        &self,
        _window_id: &str,
        _plan: &LayoutPresentationPlan,
    ) -> Result<(), PlatformError> {
        Err(PlatformError::NotSupported(
            "present_layout is not supported on this platform".to_string(),
        ))
    }

    fn present_surface(&self, _request: SurfaceRequest) -> Result<(), PlatformError> {
        Err(PlatformError::NotSupported(
            "surface is not supported on this platform".to_string(),
        ))
    }

    fn close_surface(&self, _app_id: &str, _id: &str, _reason: &str) -> Result<(), PlatformError> {
        Err(PlatformError::NotSupported(
            "surface close is not supported on this platform".to_string(),
        ))
    }

    fn show_surface(&self, _app_id: &str, _id: &str) -> Result<(), PlatformError> {
        Err(PlatformError::NotSupported(
            "surface show is not supported on this platform".to_string(),
        ))
    }

    fn hide_surface(&self, _app_id: &str, _id: &str) -> Result<(), PlatformError> {
        Err(PlatformError::NotSupported(
            "surface hide is not supported on this platform".to_string(),
        ))
    }

    /// Show or hide a top-level surface declared by the host (e.g. the AI-chat
    /// panel or terminal in `ui` config). Only platforms with a host shell that
    /// manages declared surfaces (currently macOS) support it; others have no
    /// such shell and return `NotSupported`.
    fn set_managed_surface_visible(
        &self,
        _id: &str,
        _visible: bool,
    ) -> Result<(), PlatformError> {
        Err(PlatformError::NotSupported(
            "managed surfaces are not supported on this platform".to_string(),
        ))
    }

    /// Toggle a host-declared top-level surface's visibility. See
    /// [`set_managed_surface_visible`](Self::set_managed_surface_visible).
    fn toggle_managed_surface(&self, _id: &str) -> Result<(), PlatformError> {
        Err(PlatformError::NotSupported(
            "managed surfaces are not supported on this platform".to_string(),
        ))
    }
}

pub trait UIUpdate: Send + Sync + 'static {
    fn update_navbar_ui(&self, appid: String) -> Result<(), PlatformError>;
    fn update_tabbar_ui(&self, appid: String) -> Result<(), PlatformError>;

    fn update_orientation_ui(&self, _appid: String) -> Result<(), PlatformError> {
        Err(PlatformError::NotSupported(
            "update_orientation_ui not implemented for this platform".to_string(),
        ))
    }
}

pub trait UserFeedback: Send + Sync + 'static {
    fn show_toast(&self, options: ToastOptions) -> Result<(), PlatformError>;
    fn hide_toast(&self) -> Result<(), PlatformError>;

    fn show_modal(
        &self,
        options: ModalOptions,
    ) -> impl Future<Output = Result<String, PlatformError>> + Send;

    fn show_action_sheet(
        &self,
        options: Vec<String>,
        cancel_text: String,
        item_color: String,
    ) -> impl Future<Output = Result<String, PlatformError>> + Send;
}
