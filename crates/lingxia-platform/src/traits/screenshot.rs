use async_trait::async_trait;
use serde::{Deserialize, Serialize};

/// Description of a top-level window belonging to the host app.
///
/// Returned by [`AppScreenshot::list_app_windows`]. The `id` is opaque
/// platform-specific (macOS NSWindow.windowNumber, Windows HWND, etc.) and
/// is the value to pass back to [`AppScreenshot::take_app_screenshot`] to
/// target that specific window.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WindowInfo {
    /// Opaque platform-specific identifier (stringified for portability).
    pub id: String,
    /// Window title (may be empty if the platform / app does not set one).
    pub title: String,
    /// `true` if this window currently has keyboard focus / is "key".
    pub focused: bool,
    /// `true` if this window is the app's main window (macOS concept).
    pub main: bool,
    /// `true` if the window is currently on-screen / not minimized.
    pub visible: bool,
    /// Width in the platform's window-content coordinate unit (points on
    /// Apple platforms, client pixels on Windows).
    pub width: u32,
    /// Height in the platform's window-content coordinate unit.
    pub height: u32,
}

/// Capture a PNG snapshot of the host app's window(s).
///
/// Conceptually one level above `WebViewController::take_screenshot`: the
/// WebView API only sees web content, while this captures the entire window
/// the host renders — host-drawn navigation bars, native overlays, multiple
/// WebViews composited together, etc.
#[async_trait]
pub trait AppScreenshot: Send + Sync {
    /// Enumerate the app's top-level windows.
    ///
    /// Mobile platforms typically return a single "main" entry. Desktop
    /// platforms return one entry per open NSWindow / HWND.
    async fn list_app_windows(&self) -> Result<Vec<WindowInfo>, Error> {
        Err(Error::not_supported(
            "list_app_windows is not implemented for this platform".to_string(),
        ))
    }

    /// Resolve an optional id using the same default-window policy as capture
    /// and input, returning the concrete target for automation metadata.
    async fn resolve_app_window(&self, window_id: Option<&str>) -> Result<WindowInfo, Error> {
        let windows = self.list_app_windows().await?;
        if let Some(window_id) = window_id {
            return windows
                .into_iter()
                .find(|window| window.id == window_id)
                .ok_or_else(|| {
                    Error::invalid_arg(format!(
                        "window id does not belong to this app: {window_id}"
                    ))
                });
        }
        windows
            .iter()
            .find(|window| window.focused && window.visible)
            .or_else(|| windows.iter().find(|window| window.main && window.visible))
            .or_else(|| windows.iter().find(|window| window.visible))
            .or_else(|| windows.first())
            .cloned()
            .ok_or_else(|| Error::internal("no app window is available".to_string()))
    }

    /// Capture and return PNG-encoded bytes of the app's window.
    ///
    /// When `window_id` is `None`, the platform picks a sensible default
    /// (key/focused window on desktop; the sole window on mobile).
    async fn take_app_screenshot(&self, window_id: Option<&str>) -> Result<Vec<u8>, Error> {
        let _ = window_id;
        Err(Error::not_supported(
            "app screenshot is not implemented for this platform".to_string(),
        ))
    }

    /// Bring one of the app's own windows (the same default as capture) in
    /// front of other apps' windows. `focus` also activates the app and makes
    /// the window key; without it keyboard focus stays where the user left it.
    ///
    /// The app acts on its own windows, so no Accessibility permission is
    /// involved. Returns the window as it is afterwards.
    async fn bring_app_window_to_front(
        &self,
        window_id: Option<&str>,
        focus: bool,
    ) -> Result<WindowInfo, Error> {
        let _ = (window_id, focus);
        Err(Error::not_supported(
            "raising an app window is not implemented for this platform".to_string(),
        ))
    }

    /// Order the app's page window in front of other apps' windows only when it is minimized or
    /// nothing of it can be seen, without activating the app. Returns why it raised, or `None`.
    async fn raise_app_window_if_hidden(&self) -> Result<Option<&'static str>, Error> {
        Err(Error::not_supported(
            "raising an app window is not implemented for this platform".to_string(),
        ))
    }
}

#[cfg(any(target_os = "android", target_env = "ohos", test))]
use image::ImageEncoder as _;
use lingxia_error::Error;

/// A GPU surface inside a window capture, in the capture's pixels. Window captures leave the
/// surface out (it is composited by the system), so its newest frame is drawn in beneath.
#[cfg(any(target_os = "android", target_env = "ohos", test))]
#[derive(Debug, Clone, Deserialize)]
pub(crate) struct CapturedSurface {
    /// Names the surface to the installed [`SurfaceFrames`] source.
    pub handle: i64,
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

/// The newest frame a GPU surface presented: width, height and straight-alpha RGBA rows;
/// `None` when it has none or is not on screen.
pub type SurfaceFrames = fn(handle: i64) -> Option<(u32, u32, Vec<u8>)>;

static SURFACE_FRAMES: std::sync::OnceLock<SurfaceFrames> = std::sync::OnceLock::new();

/// Installed by the host runtime, which owns the GPU presenters.
pub fn set_surface_frames(source: SurfaceFrames) {
    let _ = SURFACE_FRAMES.set(source);
}

/// `capture` (PNG) with each surface's newest frame drawn beneath it.
#[cfg(any(target_os = "android", target_env = "ohos", test))]
pub(crate) fn composite_surfaces(
    capture: Vec<u8>,
    surfaces: &[CapturedSurface],
) -> Result<Vec<u8>, Error> {
    let Some(source) = SURFACE_FRAMES.get() else {
        return Ok(capture);
    };
    let frames: Vec<_> = surfaces
        .iter()
        .filter(|surface| surface.width > 0 && surface.height > 0)
        .filter_map(|surface| source(surface.handle).map(|frame| (surface, frame)))
        .collect();
    if frames.is_empty() {
        return Ok(capture);
    }
    let window = image::load_from_memory_with_format(&capture, image::ImageFormat::Png)
        .map_err(|e| Error::internal(format!("app screenshot decode: {e}")))?
        .into_rgba8();
    let mut composed = image::RgbaImage::new(window.width(), window.height());
    for (surface, (width, height, rgba)) in frames {
        let Some(frame) = image::RgbaImage::from_raw(width, height, rgba) else {
            continue;
        };
        // A surface scaled by the system shows its frame at the size it was laid out.
        let frame = if (width, height) == (surface.width, surface.height) {
            frame
        } else {
            image::imageops::resize(
                &frame,
                surface.width,
                surface.height,
                image::imageops::FilterType::Triangle,
            )
        };
        image::imageops::replace(&mut composed, &frame, surface.x.into(), surface.y.into());
    }
    image::imageops::overlay(&mut composed, &window, 0, 0);
    let mut png = Vec::new();
    image::codecs::png::PngEncoder::new(&mut png)
        .write_image(
            composed.as_raw(),
            composed.width(),
            composed.height(),
            image::ExtendedColorType::Rgba8,
        )
        .map_err(|e| Error::internal(format!("app screenshot encode: {e}")))?;
    Ok(png)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn png(image: &image::RgbaImage) -> Vec<u8> {
        let mut bytes = Vec::new();
        image::codecs::png::PngEncoder::new(&mut bytes)
            .write_image(
                image.as_raw(),
                image.width(),
                image.height(),
                image::ExtendedColorType::Rgba8,
            )
            .unwrap();
        bytes
    }

    #[test]
    fn a_surface_frame_shows_through_the_window_capture_and_under_its_chrome() {
        set_surface_frames(|handle| {
            (handle == 7).then(|| (2, 2, [[0u8, 0, 255, 255]; 4].concat()))
        });
        // A 4x4 window: an opaque red bar on the top row, a clear hole elsewhere.
        let mut window = image::RgbaImage::new(4, 4);
        for x in 0..4 {
            window.put_pixel(x, 0, image::Rgba([255, 0, 0, 255]));
        }
        let surfaces = [
            CapturedSurface {
                handle: 7,
                x: 0,
                y: 0,
                width: 4,
                height: 4,
            },
            CapturedSurface {
                handle: 8,
                x: 0,
                y: 0,
                width: 4,
                height: 4,
            },
        ];
        let out = composite_surfaces(png(&window), &surfaces).unwrap();
        let out = image::load_from_memory(&out).unwrap().into_rgba8();
        assert_eq!(
            out.get_pixel(1, 0).0,
            [255, 0, 0, 255],
            "chrome stays on top"
        );
        assert_eq!(
            out.get_pixel(2, 3).0,
            [0, 0, 255, 255],
            "the scaled frame fills the hole"
        );
        assert_eq!(
            composite_surfaces(png(&window), &surfaces[1..]).unwrap(),
            png(&window),
            "no frame leaves the capture as it was"
        );
    }
}
