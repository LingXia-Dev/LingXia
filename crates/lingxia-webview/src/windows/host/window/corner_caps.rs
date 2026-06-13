//! Layered anti-aliased corner caps for attached webview surfaces.

use super::*;

/// Corner-cap overlays: attached cards/panels are `WS_CHILD` windows, so
/// the DWM corner rounding used for top-level windows cannot apply, and a
/// GDI window region (`SetWindowRgn`) clips to an aliased staircase edge.
/// Instead, four tiny per-pixel-alpha "cap" child windows are layered over
/// each card's corners, above the card's WebView2 child: each cap paints
/// the renderer's [`card_corner_color`](WindowsChromeRenderer::card_corner_color)
/// outside the rounded-corner arc, anti-aliased coverage along the arc, and
/// full transparency inside, visually rounding the card without clipping
/// it. Caps are input-transparent, created lazily per card window by the
/// attached layout paths, repositioned on every layout, and destroyed when
/// their card hides or goes away.
struct CornerCapSet {
    /// Cap handles ordered top-left, top-right, bottom-left, bottom-right.
    caps: [isize; 4],
    /// Cap side length (the corner radius) the bitmaps were rendered at.
    side: i32,
    /// `COLORREF` value the bitmaps were rendered with.
    color: u32,
}

/// Live corner-cap sets, keyed by the window the caps are children of (an
/// attached card window, or a group host for its own main card).
static CORNER_CAPS: OnceLock<Mutex<HashMap<isize, CornerCapSet>>> = OnceLock::new();

/// Cap windows take no input: `WS_EX_TRANSPARENT` already excludes the
/// layered caps from hit testing, and `HTTRANSPARENT` covers any hit test
/// that still reaches the window.
unsafe extern "system" fn corner_cap_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    if msg == WindowsAndMessaging::WM_NCHITTEST {
        return LRESULT(WindowsAndMessaging::HTTRANSPARENT as isize);
    }
    unsafe { WindowsAndMessaging::DefWindowProcW(hwnd, msg, wparam, lparam) }
}

fn corner_cap_class() -> PCWSTR {
    static REGISTERED: OnceLock<()> = OnceLock::new();
    REGISTERED.get_or_init(|| {
        // Register with the same module handle that cap creation passes to
        // `CreateWindowExW`: window classes are keyed by (name, module), so
        // a mismatched module would make every cap creation fail.
        let module = unsafe { LibraryLoader::GetModuleHandleW(None) }
            .map(|module| HINSTANCE(module.0))
            .unwrap_or_default();
        let class = WNDCLASSW {
            lpfnWndProc: Some(corner_cap_proc),
            hInstance: module,
            lpszClassName: w!("LingXiaCardCornerCap"),
            ..Default::default()
        };
        if unsafe { WindowsAndMessaging::RegisterClassW(&class) } == 0 {
            // A failed registration leaves every later cap creation failing;
            // surface it instead of silently losing the rounded corners.
            log::error!(
                "corner cap class registration failed: {}",
                windows::core::Error::from_thread()
            );
        }
    });
    w!("LingXiaCardCornerCap")
}

/// Creates (lazily) and lays out the four corner caps of one card surface.
/// `card_rect` is in `parent`'s client coordinates: the full client rect
/// for attached card windows, the controller bounds for a group host's own
/// main card. Skipped when the renderer reports no corner color (plain
/// OS-frame fallback) or no corner radius.
pub(crate) fn update_corner_caps(parent: HWND, card_rect: RECT) {
    // Cap windows are children of `parent` and must be owned by the thread
    // that owns `parent`: group layout also runs on short-lived helper
    // threads (chrome-event dispatch, async tasks), and Windows destroys a
    // thread's windows when the thread exits — caps created there silently
    // vanish moments later. Marshal the update onto the parent's UI thread.
    let owner_thread = unsafe { WindowsAndMessaging::GetWindowThreadProcessId(parent, None) };
    if owner_thread != 0 && owner_thread != unsafe { Threading::GetCurrentThreadId() } {
        let parent_handle = hwnd_handle(parent);
        post_to_window_thread(
            parent_handle,
            Box::new(move || update_corner_caps(hwnd_from_handle(parent_handle), card_rect)),
        );
        return;
    }
    let Some(color) = renderer_card_corner_color() else {
        return;
    };
    let side = renderer_panel_radius();
    if side <= 0 {
        return;
    }
    if rect_width(&card_rect) < side * 2 || rect_height(&card_rect) < side * 2 {
        destroy_corner_caps(parent);
        return;
    }

    let sets = CORNER_CAPS.get_or_init(|| Mutex::new(HashMap::new()));
    let existing = sets
        .lock()
        .ok()
        .and_then(|sets| {
            sets.get(&hwnd_handle(parent))
                .map(|set| (set.caps, set.side, set.color))
        })
        .filter(|(caps, cap_side, cap_color)| {
            *cap_side == side
                && *cap_color == color.0
                && caps.iter().all(|cap| is_window_handle_valid(*cap))
        });
    let caps = match existing {
        Some((caps, _, _)) => caps,
        None => {
            destroy_corner_caps(parent);
            let Some(caps) = create_corner_caps(parent, side, color) else {
                return;
            };
            if let Ok(mut sets) = sets.lock() {
                sets.insert(
                    hwnd_handle(parent),
                    CornerCapSet {
                        caps,
                        side,
                        color: color.0,
                    },
                );
            }
            log::debug!(
                "created corner caps for {:?} (side {side}, color #{:06x})",
                parent,
                color.0
            );
            caps
        }
    };

    // A main-card surface flush above a docked bottom panel keeps square
    // bottom corners: its bottom caps would notch the shared dock edge.
    let square_bottom = window_webtag_key(parent)
        .is_some_and(|webtag_key| main_surface_has_docked_bottom_panel(&webtag_key));

    let positions = [
        (card_rect.left, card_rect.top),
        (card_rect.right - side, card_rect.top),
        (card_rect.left, card_rect.bottom - side),
        (card_rect.right - side, card_rect.bottom - side),
    ];
    for (index, (cap, (x, y))) in caps.iter().zip(positions).enumerate() {
        let hide = square_bottom && index >= 2;
        unsafe {
            let _ = WindowsAndMessaging::SetWindowPos(
                hwnd_from_handle(*cap),
                Some(WindowsAndMessaging::HWND_TOP),
                x,
                y,
                side,
                side,
                WindowsAndMessaging::SWP_NOACTIVATE
                    | WindowsAndMessaging::SWP_NOOWNERZORDER
                    | WindowsAndMessaging::SWP_NOCOPYBITS
                    | if hide {
                        WindowsAndMessaging::SWP_HIDEWINDOW
                    } else {
                        WindowsAndMessaging::SWP_SHOWWINDOW
                    },
            );
        }
    }
}

/// Re-asserts the caps of `parent` at the top of its child z-order without
/// moving or resizing them. WebView2 reorders its own `Chrome_WidgetWin`
/// child chain on visibility/focus changes, which can bury the caps under
/// the webview surface; every layout pass re-asserts `HWND_TOP` through
/// [`update_corner_caps`], and the controller visibility flips call this
/// directly. Hidden caps (square-bottom dock seam) stay hidden.
pub(crate) fn raise_corner_caps(parent: HWND) {
    let caps = CORNER_CAPS
        .get()
        .and_then(|sets| sets.lock().ok())
        .and_then(|sets| sets.get(&hwnd_handle(parent)).map(|set| set.caps));
    let Some(caps) = caps else {
        return;
    };
    for cap in caps {
        unsafe {
            let _ = WindowsAndMessaging::SetWindowPos(
                hwnd_from_handle(cap),
                Some(WindowsAndMessaging::HWND_TOP),
                0,
                0,
                0,
                0,
                WindowsAndMessaging::SWP_NOMOVE
                    | WindowsAndMessaging::SWP_NOSIZE
                    | WindowsAndMessaging::SWP_NOACTIVATE
                    | WindowsAndMessaging::SWP_NOOWNERZORDER,
            );
        }
    }
}

/// Creates the four layered cap windows of one card and renders their
/// per-pixel-alpha bitmaps. Returns `None` (destroying any partial set)
/// when a window fails to create.
fn create_corner_caps(parent: HWND, side: i32, color: COLORREF) -> Option<[isize; 4]> {
    let class = corner_cap_class();
    let mut caps = [0isize; 4];
    for corner in 0..4 {
        let result = unsafe {
            WindowsAndMessaging::CreateWindowExW(
                WindowsAndMessaging::WS_EX_LAYERED
                    | WindowsAndMessaging::WS_EX_TRANSPARENT
                    | WindowsAndMessaging::WS_EX_NOACTIVATE,
                class,
                PCWSTR::null(),
                WindowsAndMessaging::WS_CHILD,
                0,
                0,
                side,
                side,
                Some(parent),
                None,
                LibraryLoader::GetModuleHandleW(None)
                    .ok()
                    .map(|module| HINSTANCE(module.0)),
                None,
            )
        };
        let cap = match result {
            Ok(cap) => cap,
            Err(err) => {
                log::warn!("corner cap creation failed for {parent:?}: {err}");
                for created in &caps[..corner] {
                    unsafe {
                        let _ = WindowsAndMessaging::DestroyWindow(hwnd_from_handle(*created));
                    }
                }
                return None;
            }
        };
        paint_corner_cap(cap, corner, side, color);
        caps[corner] = hwnd_handle(cap);
    }
    Some(caps)
}

/// Uploads one cap's premultiplied 32-bit ARGB bitmap via
/// `UpdateLayeredWindow` (`ULW_ALPHA`): opaque `color` outside the
/// quarter-circle arc, anti-aliased coverage along it, transparent inside.
fn paint_corner_cap(cap: HWND, corner: usize, side: i32, color: COLORREF) {
    let pixels = corner_cap_pixels(corner, side, color);
    upload_layered_window_pixels(cap, side, side, &pixels);
}

/// Uploads a premultiplied 32-bit ARGB top-down pixel buffer to a layered
/// window via `UpdateLayeredWindow` (`ULW_ALPHA`).
fn upload_layered_window_pixels(window: HWND, width: i32, height: i32, pixels: &[u32]) {
    unsafe {
        let screen_dc = GetDC(None);
        if screen_dc.is_invalid() {
            return;
        }
        let memory_dc = CreateCompatibleDC(Some(screen_dc));
        if !memory_dc.is_invalid() {
            let info = BITMAPINFO {
                bmiHeader: BITMAPINFOHEADER {
                    biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                    biWidth: width,
                    // Negative height: top-down rows, matching `pixels`.
                    biHeight: -height,
                    biPlanes: 1,
                    biBitCount: 32,
                    biCompression: BI_RGB.0,
                    ..Default::default()
                },
                ..Default::default()
            };
            let mut bits: *mut c_void = std::ptr::null_mut();
            if let Ok(bitmap) =
                CreateDIBSection(Some(screen_dc), &info, DIB_RGB_COLORS, &mut bits, None, 0)
                && !bits.is_null()
            {
                std::ptr::copy_nonoverlapping(pixels.as_ptr(), bits.cast::<u32>(), pixels.len());
                let old_bitmap = SelectObject(memory_dc, HGDIOBJ(bitmap.0));
                let size = SIZE {
                    cx: width,
                    cy: height,
                };
                let origin = POINT { x: 0, y: 0 };
                let blend = BLENDFUNCTION {
                    BlendOp: AC_SRC_OVER as u8,
                    BlendFlags: 0,
                    SourceConstantAlpha: 255,
                    AlphaFormat: AC_SRC_ALPHA as u8,
                };
                let _ = WindowsAndMessaging::UpdateLayeredWindow(
                    window,
                    None,
                    None,
                    Some(&size),
                    Some(memory_dc),
                    Some(&origin),
                    COLORREF(0),
                    Some(&blend),
                    WindowsAndMessaging::ULW_ALPHA,
                );
                if !old_bitmap.is_invalid() {
                    let _ = SelectObject(memory_dc, old_bitmap);
                }
                let _ = DeleteObject(HGDIOBJ(bitmap.0));
            }
            let _ = DeleteDC(memory_dc);
        }
        let _ = ReleaseDC(None, screen_dc);
    }
}

/// Premultiplied ARGB pixels of one corner cap, top-down row order.
/// `corner`: 0 top-left, 1 top-right, 2 bottom-left, 3 bottom-right. Alpha
/// is the 4x4-supersampled coverage of "outside the rounded corner", so
/// the arc edge blends smoothly between the cap color and the webview
/// pixels underneath.
fn corner_cap_pixels(corner: usize, side: i32, color: COLORREF) -> Vec<u32> {
    let radius = side as f32;
    // Arc center in cap-local coordinates: the cap corner that points into
    // the card interior.
    let (center_x, center_y) = match corner {
        0 => (radius, radius),
        1 => (0.0, radius),
        2 => (radius, 0.0),
        _ => (0.0, 0.0),
    };
    let red = color.0 & 0xff;
    let green = (color.0 >> 8) & 0xff;
    let blue = (color.0 >> 16) & 0xff;
    let mut pixels = Vec::with_capacity((side * side) as usize);
    for y in 0..side {
        for x in 0..side {
            let mut outside = 0u32;
            for sub_y in 0..4 {
                for sub_x in 0..4 {
                    let sample_x = x as f32 + (sub_x as f32 + 0.5) / 4.0;
                    let sample_y = y as f32 + (sub_y as f32 + 0.5) / 4.0;
                    let dx = sample_x - center_x;
                    let dy = sample_y - center_y;
                    if dx * dx + dy * dy > radius * radius {
                        outside += 1;
                    }
                }
            }
            let alpha = outside * 255 / 16;
            let premultiply = |channel: u32| channel * alpha / 255;
            pixels.push(
                (alpha << 24)
                    | (premultiply(red) << 16)
                    | (premultiply(green) << 8)
                    | premultiply(blue),
            );
        }
    }
    pixels
}

/// Destroys the caps of one card and forgets its registry entry. Group
/// layout runs on whichever UI thread triggered it, so a cap may belong to
/// a different thread than the caller; `DestroyWindow` fails cross-thread,
/// and those caps are instead closed via `WM_CLOSE` on their owning thread.
pub(crate) fn destroy_corner_caps(parent: HWND) {
    let removed = CORNER_CAPS
        .get()
        .and_then(|sets| sets.lock().ok())
        .and_then(|mut sets| sets.remove(&hwnd_handle(parent)));
    let Some(set) = removed else {
        return;
    };
    log::debug!("destroying corner caps for {parent:?}");
    for cap in set.caps {
        let cap = hwnd_from_handle(cap);
        unsafe {
            if WindowsAndMessaging::DestroyWindow(cap).is_err() {
                let _ = WindowsAndMessaging::PostMessageW(
                    Some(cap),
                    WindowsAndMessaging::WM_CLOSE,
                    WPARAM::default(),
                    LPARAM::default(),
                );
            }
        }
    }
}
