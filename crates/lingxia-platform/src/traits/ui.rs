use std::future::Future;

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
    Popup = 1,
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
}

pub trait SurfacePresenter: Send + Sync + 'static {
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
