//! Shared Windows rendering for LingXia design icons.
//!
//! The source of truth is `design/icons/svg`. The CLI renders
//! Windows PNGs into each app's generated assets, and runtime code draws those
//! assets instead of hand-maintaining a Windows-only icon set.

use std::collections::HashMap;
use std::ffi::c_void;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

use windows::Win32::Foundation::RECT;
use windows::Win32::Graphics::Gdi::{CreateBitmap, DeleteObject, HDC, HGDIOBJ};
use windows::Win32::UI::WindowsAndMessaging::{self, HICON, ICONINFO};
use windows::core::BOOL;

type IconCache = HashMap<(WindowsDesignIcon, u32, Option<u32>), Option<isize>>;

static ICON_HANDLES: OnceLock<Mutex<IconCache>> = OnceLock::new();
static WINDOWS_DESIGN_ICON_DIR: OnceLock<PathBuf> = OnceLock::new();

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum WindowsDesignIcon {
    Back,
    Forward,
    BrowserRefresh,
    CloseX,
    Home,
    Settings,
    Downloads,
    Lock,
    Warning,
    Play,
    Pause,
    VolumeOn,
    VolumeOff,
    FullscreenEnter,
    FullscreenExit,
    SidebarCollapse,
    SidebarExpand,
}

impl WindowsDesignIcon {
    fn file_name(self) -> &'static str {
        match self {
            Self::Back => "icon_back.png",
            Self::Forward => "icon_forward.png",
            Self::BrowserRefresh => "icon_browser_refresh.png",
            Self::CloseX => "icon_close_x.png",
            Self::Home => "icon_home.png",
            Self::Settings => "icon_settings.png",
            Self::Downloads => "icon_download.png",
            Self::Lock => "icon_lock.png",
            Self::Warning => "icon_warning.png",
            Self::Play => "icon_play.png",
            Self::Pause => "icon_pause.png",
            Self::VolumeOn => "icon_volume_on.png",
            Self::VolumeOff => "icon_volume_off.png",
            Self::FullscreenEnter => "icon_fullscreen_enter.png",
            Self::FullscreenExit => "icon_fullscreen_exit.png",
            Self::SidebarCollapse => "icon_sidebar_collapse.png",
            Self::SidebarExpand => "icon_sidebar_expand.png",
        }
    }
}

pub fn set_windows_design_icon_dir(path: impl AsRef<Path>) {
    let _ = WINDOWS_DESIGN_ICON_DIR.set(path.as_ref().to_path_buf());
}

pub fn draw_windows_design_icon(hdc: HDC, icon: WindowsDesignIcon, rect: RECT) -> bool {
    draw_windows_design_icon_inner(hdc, icon, rect, None)
}

pub fn draw_windows_design_icon_with_color(
    hdc: HDC,
    icon: WindowsDesignIcon,
    rect: RECT,
    rgb: u32,
) -> bool {
    draw_windows_design_icon_inner(hdc, icon, rect, Some(rgb))
}

fn draw_windows_design_icon_inner(
    hdc: HDC,
    icon: WindowsDesignIcon,
    rect: RECT,
    tint: Option<u32>,
) -> bool {
    let size = (rect.right - rect.left).max(rect.bottom - rect.top).max(1) as u32;
    let Some(handle) = cached_icon_handle(icon, size, tint) else {
        return false;
    };
    unsafe {
        WindowsAndMessaging::DrawIconEx(
            hdc,
            rect.left,
            rect.top,
            HICON(handle as *mut c_void),
            (rect.right - rect.left).max(1),
            (rect.bottom - rect.top).max(1),
            0,
            None,
            WindowsAndMessaging::DI_NORMAL,
        )
        .is_ok()
    }
}

fn cached_icon_handle(icon: WindowsDesignIcon, size: u32, tint: Option<u32>) -> Option<isize> {
    let key = (icon, size, tint);
    let handles = ICON_HANDLES.get_or_init(|| Mutex::new(HashMap::new()));
    if let Ok(mut handles) = handles.lock() {
        if let Some(handle) = handles.get(&key) {
            return *handle;
        }
        let handle = create_icon_from_png(&design_icon_path(icon)?, size, tint).ok();
        if let Some(Some(previous)) = handles.insert(key, handle)
            && Some(previous) != handle
        {
            destroy_icon_handle(previous);
        }
        return handle;
    }
    create_icon_from_png(&design_icon_path(icon)?, size, tint).ok()
}

fn design_icon_path(icon: WindowsDesignIcon) -> Option<PathBuf> {
    if let Some(root) = WINDOWS_DESIGN_ICON_DIR.get() {
        return Some(root.join(icon.file_name()));
    }
    if let Some(root) = std::env::var_os("LINGXIA_ASSET_DIR") {
        return Some(
            PathBuf::from(root)
                .join("icons")
                .join("design")
                .join(icon.file_name()),
        );
    }
    None
}

fn create_icon_from_png(path: &Path, size: u32, tint: Option<u32>) -> Result<isize, String> {
    let image = image::open(path)
        .map_err(|err| {
            format!(
                "Failed to load Windows design icon {}: {err}",
                path.display()
            )
        })?
        .resize_exact(size, size, image::imageops::FilterType::Lanczos3)
        .into_rgba8();

    let mut bgra = Vec::with_capacity(image.len());
    for pixel in image.pixels() {
        let [mut r, mut g, mut b, a] = pixel.0;
        if let Some(rgb) = tint {
            r = ((rgb >> 16) & 0xff) as u8;
            g = ((rgb >> 8) & 0xff) as u8;
            b = (rgb & 0xff) as u8;
        }
        bgra.extend_from_slice(&[b, g, r, a]);
    }

    unsafe {
        let width = size as i32;
        let height = size as i32;
        let color = CreateBitmap(width, height, 1, 32, Some(bgra.as_ptr().cast()));
        if color.is_invalid() {
            return Err("Failed to create generated design icon color bitmap".to_string());
        }

        let mask = CreateBitmap(width, height, 1, 1, None);
        if mask.is_invalid() {
            let _ = DeleteObject(HGDIOBJ(color.0));
            return Err("Failed to create generated design icon mask bitmap".to_string());
        }

        let info = ICONINFO {
            fIcon: BOOL(1),
            xHotspot: 0,
            yHotspot: 0,
            hbmMask: mask,
            hbmColor: color,
        };
        let icon = WindowsAndMessaging::CreateIconIndirect(&info)
            .map_err(|err| format!("Failed to create generated design icon: {err}"))?;
        let _ = DeleteObject(HGDIOBJ(color.0));
        let _ = DeleteObject(HGDIOBJ(mask.0));
        Ok(icon.0 as isize)
    }
}

fn destroy_icon_handle(handle: isize) {
    if handle != 0 {
        unsafe {
            let _ = WindowsAndMessaging::DestroyIcon(HICON(handle as *mut c_void));
        }
    }
}
