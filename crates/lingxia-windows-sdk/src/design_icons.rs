//! Shared Windows rendering for LingXia design icons.
//!
//! The source of truth is `design/icons/svg`. The CLI renders
//! Windows PNGs into each app's generated assets, and runtime code draws those
//! assets instead of hand-maintaining a Windows-only icon set.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};

use windows::Win32::Foundation::RECT;
use windows::Win32::Graphics::Gdi::{
    AC_SRC_ALPHA, AC_SRC_OVER, AlphaBlend, BI_RGB, BITMAPINFO, BITMAPINFOHEADER, BLENDFUNCTION,
    CreateCompatibleDC, CreateDIBSection, DIB_RGB_COLORS, DeleteDC, DeleteObject, HDC, HGDIOBJ,
    SelectObject,
};

type PixelCache = HashMap<(WindowsDesignIcon, u32, Option<u32>), Arc<Vec<u32>>>;

/// Decoded-icon memo: PNG/SVG decode + Lanczos resize is far too expensive to
/// run per paint (the top bar repaints on every hover mouse-move). Keyed by
/// (icon, size, tint) — a naturally small set, so no eviction is needed.
static ICON_PIXELS: OnceLock<Mutex<PixelCache>> = OnceLock::new();
static WINDOWS_DESIGN_ICON_DIR: OnceLock<PathBuf> = OnceLock::new();

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum WindowsDesignIcon {
    Back,
    Forward,
    BrowserRefresh,
    BrowserTabs,
    Bookmark,
    BookmarkFilled,
    Bookmarks,
    Pin,
    PinFilled,
    Unpin,
    Link,
    External,
    Globe,
    CloseOtherTabs,
    CloseTabsBelow,
    History,
    ClearData,
    PageMenu,
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
    Rotate,
    CapsuleMenu,
    CapsuleClose,
    CleanCache,
    Restart,
    Uninstall,
}

impl WindowsDesignIcon {
    fn file_name(self) -> &'static str {
        match self {
            Self::Back => "icon_back.png",
            Self::Forward => "icon_forward.png",
            Self::BrowserRefresh => "icon_browser_refresh.png",
            Self::BrowserTabs => "icon_browser_tabs.png",
            Self::Bookmark => "icon_bookmark.png",
            Self::BookmarkFilled => "icon_bookmark_filled.png",
            Self::Bookmarks => "icon_bookmarks.png",
            Self::Pin => "icon_pin.png",
            Self::PinFilled => "icon_pin_filled.png",
            Self::Unpin => "icon_unpin.png",
            Self::Link => "icon_link.png",
            Self::External => "icon_external.png",
            Self::Globe => "icon_globe.png",
            Self::CloseOtherTabs => "icon_close_other_tabs.png",
            Self::CloseTabsBelow => "icon_close_tabs_below.png",
            Self::History => "icon_history.png",
            Self::ClearData => "icon_clear_data.png",
            Self::PageMenu => "icon_page_menu.png",
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
            Self::Rotate => "icon_rotate.png",
            Self::CapsuleMenu => "icon_capsule_menu.png",
            Self::CapsuleClose => "icon_capsule_close.png",
            Self::CleanCache => "icon_clean_cache.png",
            Self::Restart => "icon_restart.png",
            Self::Uninstall => "icon_uninstall.png",
        }
    }

    #[cfg(feature = "shell-chrome")]
    fn embedded_svg(self) -> Option<&'static [u8]> {
        Some(match self {
            Self::Back => include_bytes!("../../../design/icons/svg/icon_back.svg"),
            Self::Forward => include_bytes!("../../../design/icons/svg/icon_forward.svg"),
            Self::BrowserRefresh => {
                include_bytes!("../../../design/icons/svg/icon_browser_refresh.svg")
            }
            Self::BrowserTabs => include_bytes!("../../../design/icons/svg/icon_browser_tabs.svg"),
            Self::Bookmark => include_bytes!("../../../design/icons/svg/icon_bookmark.svg"),
            Self::BookmarkFilled => {
                include_bytes!("../../../design/icons/svg/icon_bookmark_filled.svg")
            }
            Self::Bookmarks => include_bytes!("../../../design/icons/svg/icon_bookmarks.svg"),
            Self::Pin => include_bytes!("../../../design/icons/svg/icon_pin.svg"),
            Self::PinFilled => include_bytes!("../../../design/icons/svg/icon_pin_filled.svg"),
            Self::Unpin => include_bytes!("../../../design/icons/svg/icon_unpin.svg"),
            Self::Link => include_bytes!("../../../design/icons/svg/icon_link.svg"),
            Self::External => include_bytes!("../../../design/icons/svg/icon_external.svg"),
            Self::Globe => include_bytes!("../../../design/icons/svg/icon_globe.svg"),
            Self::CloseOtherTabs => {
                include_bytes!("../../../design/icons/svg/icon_close_other_tabs.svg")
            }
            Self::CloseTabsBelow => {
                include_bytes!("../../../design/icons/svg/icon_close_tabs_below.svg")
            }
            Self::History => include_bytes!("../../../design/icons/svg/icon_history.svg"),
            Self::ClearData => include_bytes!("../../../design/icons/svg/icon_clear_data.svg"),
            Self::PageMenu => include_bytes!("../../../design/icons/svg/icon_page_menu.svg"),
            _ => return None,
        })
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
    let width = (rect.right - rect.left).max(1);
    let height = (rect.bottom - rect.top).max(1);
    let size = width.max(height) as u32;
    let Some(pixels) = design_icon_argb_premultiplied(icon, size, tint) else {
        return false;
    };
    unsafe {
        let memory = CreateCompatibleDC(Some(hdc));
        if memory.is_invalid() {
            return false;
        }
        let info = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: size as i32,
                biHeight: -(size as i32),
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB.0,
                ..Default::default()
            },
            ..Default::default()
        };
        let mut bits = std::ptr::null_mut();
        let Ok(bitmap) = CreateDIBSection(Some(hdc), &info, DIB_RGB_COLORS, &mut bits, None, 0)
        else {
            let _ = DeleteDC(memory);
            return false;
        };
        if bits.is_null() {
            let _ = DeleteObject(HGDIOBJ(bitmap.0));
            let _ = DeleteDC(memory);
            return false;
        }
        std::ptr::copy_nonoverlapping(pixels.as_ptr(), bits.cast::<u32>(), pixels.len());
        let previous = SelectObject(memory, HGDIOBJ(bitmap.0));
        let blend = BLENDFUNCTION {
            BlendOp: AC_SRC_OVER as u8,
            BlendFlags: 0,
            SourceConstantAlpha: 255,
            AlphaFormat: AC_SRC_ALPHA as u8,
        };
        let ok = AlphaBlend(
            hdc,
            rect.left,
            rect.top,
            width,
            height,
            memory,
            0,
            0,
            size as i32,
            size as i32,
            blend,
        )
        .as_bool();
        let _ = SelectObject(memory, previous);
        let _ = DeleteObject(HGDIOBJ(bitmap.0));
        let _ = DeleteDC(memory);
        ok
    }
}

fn design_icon_path(icon: WindowsDesignIcon) -> Option<PathBuf> {
    if let Some(root) = WINDOWS_DESIGN_ICON_DIR.get() {
        return Some(root.join(icon.file_name()));
    }
    None
}

/// Loads a design icon as premultiplied ARGB pixels (top-down, row-major,
/// `size * size` entries) for compositing directly onto a per-pixel-alpha
/// layered surface, where `DrawIconEx` does not reliably write the alpha
/// channel. `tint` recolors the icon (the PNG is a black silhouette + alpha).
/// Decodes are memoized in [`ICON_PIXELS`]; only successes are cached so a
/// not-yet-registered icon dir does not pin a permanent miss.
pub fn design_icon_argb_premultiplied(
    icon: WindowsDesignIcon,
    size: u32,
    tint: Option<u32>,
) -> Option<Arc<Vec<u32>>> {
    let key = (icon, size, tint);
    let cache = ICON_PIXELS.get_or_init(|| Mutex::new(HashMap::new()));
    let Ok(mut cache) = cache.lock() else {
        return decode_icon_argb_premultiplied(icon, size, tint).map(Arc::new);
    };
    if let Some(pixels) = cache.get(&key) {
        return Some(pixels.clone());
    }
    let pixels = Arc::new(decode_icon_argb_premultiplied(icon, size, tint)?);
    cache.insert(key, pixels.clone());
    Some(pixels)
}

fn decode_icon_argb_premultiplied(
    icon: WindowsDesignIcon,
    size: u32,
    tint: Option<u32>,
) -> Option<Vec<u32>> {
    let image = design_icon_path(icon)
        .and_then(|path| image::open(path).ok())
        .map(|image| {
            image
                .resize_exact(size, size, image::imageops::FilterType::Lanczos3)
                .into_rgba8()
        })
        .or_else(|| embedded_svg_image(icon, size))?;
    let mut out = Vec::with_capacity((size * size) as usize);
    for pixel in image.pixels() {
        let [mut r, mut g, mut b, a] = pixel.0;
        if let Some(rgb) = tint {
            r = ((rgb >> 16) & 0xff) as u8;
            g = ((rgb >> 8) & 0xff) as u8;
            b = (rgb & 0xff) as u8;
        }
        let pm = |c: u8| c as u32 * a as u32 / 255;
        out.push(((a as u32) << 24) | (pm(r) << 16) | (pm(g) << 8) | pm(b));
    }
    Some(out)
}

#[cfg(feature = "shell-chrome")]
fn embedded_svg_image(icon: WindowsDesignIcon, size: u32) -> Option<image::RgbaImage> {
    let svg = std::str::from_utf8(icon.embedded_svg()?).ok()?;
    let tree = usvg::Tree::from_str(svg, &usvg::Options::default()).ok()?;
    let svg_size = tree.size();
    let max_side = svg_size.width().max(svg_size.height());
    if max_side <= 0.0 {
        return None;
    }
    let scale = size as f32 / max_side;
    let mut pixmap = tiny_skia::Pixmap::new(size, size)?;
    let transform = tiny_skia::Transform::from_translate(
        (size as f32 - svg_size.width() * scale) / 2.0,
        (size as f32 - svg_size.height() * scale) / 2.0,
    )
    .post_scale(scale, scale);
    resvg::render(&tree, transform, &mut pixmap.as_mut());
    let mut image = image::RgbaImage::new(size, size);
    for (pixel, out) in pixmap.pixels().iter().zip(image.pixels_mut()) {
        let color = pixel.demultiply();
        *out = image::Rgba([color.red(), color.green(), color.blue(), color.alpha()]);
    }
    Some(image)
}

#[cfg(not(feature = "shell-chrome"))]
fn embedded_svg_image(_icon: WindowsDesignIcon, _size: u32) -> Option<image::RgbaImage> {
    None
}
