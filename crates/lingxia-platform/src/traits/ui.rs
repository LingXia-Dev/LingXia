use std::future::Future;
use std::pin::Pin;

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

impl SurfaceRole {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Main => "main",
            Self::Aside => "aside",
            Self::Float => "float",
        }
    }
}

impl From<lingxia_surface::Role> for SurfaceRole {
    fn from(role: lingxia_surface::Role) -> Self {
        match role {
            lingxia_surface::Role::Main => Self::Main,
            lingxia_surface::Role::Aside => Self::Aside,
            lingxia_surface::Role::Float => Self::Float,
        }
    }
}

impl From<SurfaceRole> for lingxia_surface::Role {
    fn from(role: SurfaceRole) -> Self {
        match role {
            SurfaceRole::Main => Self::Main,
            SurfaceRole::Aside => Self::Aside,
            SurfaceRole::Float => Self::Float,
        }
    }
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

/// Window decoration for `SurfaceKind::Window`.
///
/// `Full` extends the page to the window edge while keeping the system
/// minimize, maximize, resize, and drag affordances. The runtime owns a native
/// drag strip across the top and publishes its height to the page, so a page
/// that does nothing to opt in still cannot trap the user — which is what sank
/// the earlier edge-to-edge attempt.
#[repr(i32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum WindowChrome {
    #[default]
    System = 0,
    Full = 1,
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
    /// Resolved interaction contract. Platforms render this verbatim.
    pub interaction: lingxia_surface::SurfaceInteraction,
    /// Window decoration. Ignored unless `kind` is `Window`.
    pub chrome: WindowChrome,
    /// `Url` content only: isolate the WebView's cookies/site storage from
    /// shared persistent data and discard them when the surface closes (auth
    /// handoffs). `Page` content ignores it.
    pub ephemeral_web_data: bool,
    /// `Url` content only: navigation is paired with a native callback
    /// interception channel. Platforms use this to keep local-file access out
    /// of callback surfaces without restricting ordinary browser surfaces.
    pub url_callback: bool,
}

/// Callback adapter used by platform SDK handlers that cannot return a Rust
/// future directly. `SurfacePresenter` exposes only `ManagedSurfaceFuture`.
pub type ManagedSurfaceCompletion = Box<dyn FnOnce(Result<(), PlatformError>) + Send + 'static>;

pub type ManagedSurfaceFuture =
    Pin<Box<dyn Future<Output = Result<(), PlatformError>> + Send + 'static>>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ManagedSurfaceProvider {
    Declared,
    Native {
        capability: String,
        instance_key: Option<String>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManagedSurfaceProviderRequest {
    pub surface_id: String,
    pub provider: ManagedSurfaceProvider,
    pub role: Option<SurfaceRole>,
    pub edge: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManagedSurfaceProviderDestroyRequest {
    pub surface_id: String,
    pub provider: ManagedSurfaceProvider,
    pub role: Option<SurfaceRole>,
}

pub trait SurfacePresenter: Send + Sync + 'static {
    /// The shared core resolves a `LayoutPresentationPlan` for one window/graph
    /// and the platform skin binds it. The per-surface methods below present a
    /// single surface at a time.
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

    /// Ensure the platform provider exists for a core-owned Surface. Identity,
    /// role, visibility, focus, and menu policy remain in the shared graph;
    /// `present_layout` projects that state after this future succeeds.
    fn ensure_managed_surface_provider(
        &self,
        _request: ManagedSurfaceProviderRequest,
    ) -> ManagedSurfaceFuture {
        Box::pin(async {
            Err(PlatformError::NotSupported(
                "managed surface providers are not supported on this platform".to_string(),
            ))
        })
    }

    /// Destroy provider state after the core removes a non-root Surface.
    fn destroy_managed_surface_provider(
        &self,
        _request: ManagedSurfaceProviderDestroyRequest,
    ) -> ManagedSurfaceFuture {
        Box::pin(async {
            Err(PlatformError::NotSupported(
                "managed surface providers are not supported on this platform".to_string(),
            ))
        })
    }
}

pub trait UIUpdate: Send + Sync + 'static {
    fn update_navbar_ui(&self, appid: String) -> Result<(), PlatformError>;
    fn update_tabbar_ui(&self, appid: String) -> Result<(), PlatformError>;

    fn update_tabbar_ui_async(
        &self,
        appid: String,
    ) -> impl Future<Output = Result<(), PlatformError>> + Send {
        async move { self.update_tabbar_ui(appid) }
    }

    fn update_orientation_ui(&self, _appid: String) -> Result<(), PlatformError> {
        Err(PlatformError::NotSupported(
            "update_orientation_ui not implemented for this platform".to_string(),
        ))
    }

    /// Effective host/Runner scheme used when an lxapp preference is `auto`.
    fn host_appearance_dark(&self) -> bool {
        false
    }

    /// Apply an lxapp-scoped scheme to native Page Chrome and every matching
    /// WebView. Shared shell and unrelated browser/lxapp WebViews are excluded.
    fn apply_lxapp_appearance(&self, _appid: &str, _dark: bool) -> Result<(), PlatformError> {
        Ok(())
    }

    /// Clear platform state retained for a closed lxapp session.
    fn clear_lxapp_appearance(&self, _appid: &str) {}

    /// The home lxapp's entry page finished its first render (fired at most
    /// once per process). Hosts dismiss the startup splash overlay on it.
    fn notify_home_first_ready(&self) {}

    /// Measure the visible capsule after native Page Chrome has laid out.
    /// The JSON payload is an internal transport; app code only sees the
    /// revisioned View snapshot assembled by `lingxia-lxapp`.
    fn measure_page_chrome_capsule(
        &self,
        _appid: String,
    ) -> impl Future<Output = Result<Option<String>, PlatformError>> + Send {
        async { Ok(None) }
    }

    /// Acknowledge one Page Chrome revision after native layout and visuals
    /// have been applied. Platforms with an asynchronous UI thread override
    /// this; the default preserves the existing synchronous update path.
    fn apply_page_chrome_revision(
        &self,
        appid: String,
        _revision: u64,
    ) -> impl Future<Output = Result<(), PlatformError>> + Send {
        async move {
            self.update_navbar_ui(appid.clone())?;
            self.update_tabbar_ui_async(appid).await
        }
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
