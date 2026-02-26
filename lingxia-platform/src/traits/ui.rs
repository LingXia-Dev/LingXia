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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PopupPosition {
    Center = 0,
    #[default]
    Bottom = 1,
    Left = 2,
    Right = 3,
}

#[derive(Debug, Clone)]
pub struct PopupRequest {
    pub app_id: String,
    pub path: String,
    pub width_ratio: f64,
    pub height_ratio: f64,
    pub position: PopupPosition,
}

impl PopupRequest {
    pub fn new(app_id: String, path: String) -> Self {
        Self {
            app_id,
            path,
            width_ratio: f64::NAN,
            height_ratio: f64::NAN,
            position: PopupPosition::Bottom,
        }
    }
}

pub trait PopupPresenter: Send + Sync + 'static {
    fn show_popup(&self, request: PopupRequest) -> Result<(), PlatformError>;
    fn hide_popup(&self, app_id: &str) -> Result<(), PlatformError>;
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
    fn show_modal(&self, options: ModalOptions, callback_id: u64) -> Result<(), PlatformError>;
    fn show_action_sheet(
        &self,
        options: Vec<String>,
        cancel_text: String,
        item_color: String,
        callback_id: u64,
    ) -> Result<(), PlatformError>;
}
