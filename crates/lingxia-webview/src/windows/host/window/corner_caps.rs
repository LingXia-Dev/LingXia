//! Seam for rounded-corner decoration of attached webview cards.
//!
//! Attached cards/panels are `WS_CHILD` windows, so DWM corner rounding does
//! not apply and a GDI window region clips to an aliased staircase. Rounding
//! them is shell *chrome decoration*, not webview mechanics, so the
//! per-pixel-alpha cap windows that draw the rounded corners are owned by the
//! Windows UI layer (`lingxia-windows`) through [`WindowsCardDecorator`].
//!
//! lingxia-webview only decides *when* to update the caps and supplies the
//! resolved inputs the decorator cannot see: the corner color and radius from
//! the chrome renderer, and the docked-bottom flag from group state. When no
//! decorator is registered (standalone `lingxia-webview`), cards keep plain
//! square corners.

use super::*;

/// Draws rounded-corner overlays on attached webview cards. Every method runs
/// on the thread that owns `parent` (lingxia-webview marshals before calling).
pub trait WindowsCardDecorator: Send + Sync {
    /// Create (lazily) and lay out the four corner caps of `parent`'s card.
    ///
    /// `card` is in `parent`'s client coordinates; `color` and `side` (the
    /// corner radius) come from the chrome renderer; `square_bottom` keeps the
    /// bottom corners square for a main card laid out flush above a docked
    /// bottom panel.
    fn update(&self, parent: HWND, card: RECT, color: COLORREF, side: i32, square_bottom: bool);

    /// Re-assert `parent`'s caps at the top of its child z-order without
    /// moving or resizing them (WebView2 reorders its own child chain).
    fn raise(&self, parent: HWND);

    /// Destroy `parent`'s caps and forget them.
    fn destroy(&self, parent: HWND);
}

static WINDOWS_CARD_DECORATOR: OnceLock<Mutex<Option<Arc<dyn WindowsCardDecorator>>>> =
    OnceLock::new();

/// Registers the process-wide card decorator. Called once by the Windows UI
/// layer at startup; without it, attached cards keep square corners.
pub fn set_windows_card_decorator(decorator: Arc<dyn WindowsCardDecorator>) {
    let slot = WINDOWS_CARD_DECORATOR.get_or_init(|| Mutex::new(None));
    if let Ok(mut slot) = slot.lock() {
        *slot = Some(decorator);
    }
}

fn windows_card_decorator() -> Option<Arc<dyn WindowsCardDecorator>> {
    WINDOWS_CARD_DECORATOR
        .get()
        .and_then(|slot| slot.lock().ok())
        .and_then(|slot| slot.clone())
}

/// Updates (lazily creating) the corner caps of one card surface. `card_rect`
/// is in `parent`'s client coordinates. Skipped when the renderer reports no
/// corner color (plain OS-frame fallback) or no radius, or when no decorator
/// is registered.
pub(crate) fn update_corner_caps(parent: HWND, card_rect: RECT) {
    // Cap windows are children of `parent` and must be owned by the thread
    // that owns `parent`: group layout also runs on short-lived helper threads
    // (chrome-event dispatch, async tasks), and Windows destroys a thread's
    // windows when the thread exits. Marshal onto the parent's UI thread.
    let owner_thread = unsafe { WindowsAndMessaging::GetWindowThreadProcessId(parent, None) };
    if owner_thread != 0 && owner_thread != unsafe { Threading::GetCurrentThreadId() } {
        let parent_handle = hwnd_handle(parent);
        post_to_window_thread(
            parent_handle,
            Box::new(move || update_corner_caps(hwnd_from_handle(parent_handle), card_rect)),
        );
        return;
    }
    let Some(decorator) = windows_card_decorator() else {
        return;
    };
    let Some(color) = renderer_card_corner_color() else {
        return;
    };
    let side = renderer_panel_radius();
    if side <= 0 {
        return;
    }
    // A main-card surface flush above a docked bottom panel keeps square bottom
    // corners: its bottom caps would notch the shared dock edge.
    let square_bottom = window_webtag_key(parent)
        .is_some_and(|webtag_key| main_surface_has_docked_bottom_panel(&webtag_key));
    decorator.update(parent, card_rect, color, side, square_bottom);
}

/// Re-asserts the caps of `parent` at the top of its child z-order.
pub(crate) fn raise_corner_caps(parent: HWND) {
    if let Some(decorator) = windows_card_decorator() {
        decorator.raise(parent);
    }
}

/// Destroys the caps of `parent`.
pub(crate) fn destroy_corner_caps(parent: HWND) {
    if let Some(decorator) = windows_card_decorator() {
        decorator.destroy(parent);
    }
}
