//! Sidebar peek host: a non-activating layered popup owned by the shell
//! window, drawn over the collapsed rail and the WebView2 child beside it.

use super::*;
use std::time::{Duration, Instant};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetAsyncKeyState, VK_LBUTTON, VK_MBUTTON, VK_RBUTTON, VkKeyScanW,
};

/// Owner timer: hovering the rail's expand control this long opens the peek.
const HOVER_TIMER_ID: usize = 0x5A1B;
const HOVER_DELAY_MS: u32 = 300;
/// Peek timer: slide frames, then leave/click-outside polling.
const TICK_TIMER_ID: usize = 0x5A1C;
const TICK_MS: u32 = 16;
const LEAVE_GRACE: Duration = Duration::from_millis(250);
const SLIDE_DURATION: Duration = Duration::from_millis(160);

#[derive(Clone)]
struct SidebarPeek {
    window: isize,
    owner: isize,
    layout: crate::shell::SidebarPeekLayout,
    /// Window bounds on screen.
    screen: RECT,
    /// Cursor in peek coordinates while it is over the peek.
    cursor: Option<(i32, i32)>,
    hover: Option<RECT>,
    tracking_leave: bool,
    opened_at: Instant,
    sliding: bool,
    /// The leave grace starts only once the pointer has been inside; a peek
    /// opened from the keyboard waits for Esc, a click outside, or a visit.
    entered: bool,
    left_at: Option<Instant>,
    /// Pointer buttons at the last tick; only a press that starts outside the
    /// peek dismisses it.
    buttons_down: bool,
    tooltip_shown: bool,
    /// The button whose press started inside the peek (`true` for right);
    /// a release only activates when it matches.
    pressed: Option<bool>,
}

static PEEKS: OnceLock<Mutex<HashMap<isize, SidebarPeek>>> = OnceLock::new();
static HOVER_ARMED: OnceLock<Mutex<HashSet<isize>>> = OnceLock::new();
/// Owners whose Ctrl+\ is still held, keyed to the key to poll. The WebView2
/// accelerator path cannot see auto-repeat flags, so a held key stays latched
/// until it is released.
static SHORTCUT_LATCH: OnceLock<Mutex<HashMap<isize, u32>>> = OnceLock::new();
/// Owner timer: polls a latched shortcut key for its release.
const LATCH_TIMER_ID: usize = 0x5A1D;
const LATCH_POLL_MS: u32 = 50;

fn peeks() -> std::sync::MutexGuard<'static, HashMap<isize, SidebarPeek>> {
    PEEKS
        .get_or_init(|| Mutex::new(HashMap::new()))
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

fn hover_armed() -> std::sync::MutexGuard<'static, HashSet<isize>> {
    HOVER_ARMED
        .get_or_init(|| Mutex::new(HashSet::new()))
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

fn peek_for_owner(owner: HWND) -> Option<SidebarPeek> {
    peeks().get(&hwnd_handle(owner)).cloned()
}

fn peek_for_window(window: HWND) -> Option<SidebarPeek> {
    let handle = hwnd_handle(window);
    peeks().values().find(|peek| peek.window == handle).cloned()
}

fn store(peek: &SidebarPeek) {
    if let Some(slot) = peeks().get_mut(&peek.owner) {
        *slot = peek.clone();
    }
}

fn owner_client_rect(owner: HWND) -> Option<RECT> {
    let mut client = RECT::default();
    unsafe { WindowsAndMessaging::GetClientRect(owner, &mut client) }
        .is_ok()
        .then_some(client)
}

fn client_to_screen(owner: HWND, rect: RECT) -> RECT {
    let mut origin = POINT {
        x: rect.left,
        y: rect.top,
    };
    unsafe {
        let _ = windows::Win32::Graphics::Gdi::ClientToScreen(owner, &mut origin);
    }
    RECT {
        left: origin.x,
        top: origin.y,
        right: origin.x + (rect.right - rect.left),
        bottom: origin.y + (rect.bottom - rect.top),
    }
}

fn owner_peek_layout(owner: HWND) -> Option<crate::shell::SidebarPeekLayout> {
    if !is_window_visible(owner) || is_minimized(owner) {
        return None;
    }
    let webtag_key = active_webtag_key_for_window(owner)?;
    let client = owner_client_rect(owner)?;
    crate::shell::sidebar_peek_layout(client, &current_window_layout(&webtag_key))
}

fn rail_expand_hit_at(owner: HWND, point: (i32, i32)) -> bool {
    let Some(webtag_key) = active_webtag_key_for_window(owner) else {
        return false;
    };
    owner_client_rect(owner).is_some_and(|client| {
        crate::shell::rail_expand_hit(client, &current_window_layout(&webtag_key), point)
    })
}

fn pointer_buttons_down() -> bool {
    [VK_LBUTTON, VK_RBUTTON, VK_MBUTTON]
        .into_iter()
        .any(|key| unsafe { GetAsyncKeyState(i32::from(key.0)) } < 0)
}

fn rail_page_popup_shown(owner: HWND) -> bool {
    sidebar_tabbar_popup_for_owner(owner).is_some_and(|popup| {
        is_window_handle_valid(popup.window)
            && matches!(popup.content, SidebarPopupContent::Tabbar(_))
    })
}

/// A context menu opened from the peek runs a modal loop on this thread; the
/// peek must outlive it even though the pointer is over the menu.
fn menu_active() -> bool {
    let mut info = WindowsAndMessaging::GUITHREADINFO {
        cbSize: std::mem::size_of::<WindowsAndMessaging::GUITHREADINFO>() as u32,
        ..Default::default()
    };
    unsafe { WindowsAndMessaging::GetGUIThreadInfo(GetCurrentThreadId(), &mut info) }.is_ok()
        && info.flags.0
            & (WindowsAndMessaging::GUI_INMENUMODE.0 | WindowsAndMessaging::GUI_POPUPMENUMODE.0)
            != 0
}

/// Arms or disarms the hover-intent timer as the pointer crosses the rail's
/// expand control. Rail items never arm it.
pub(super) fn track_rail_hover(owner: HWND, point: (i32, i32)) {
    let over = peek_for_owner(owner).is_none() && rail_expand_hit_at(owner, point);
    let armed = hover_armed().contains(&hwnd_handle(owner));
    if over && !armed {
        hover_armed().insert(hwnd_handle(owner));
        unsafe {
            let _ = WindowsAndMessaging::SetTimer(
                Some(owner),
                HOVER_TIMER_ID,
                HOVER_DELAY_MS,
                Some(hover_timer_proc),
            );
        }
    } else if !over && armed {
        cancel_hover(owner);
    }
}

pub(super) fn cancel_hover(owner: HWND) {
    if hover_armed().remove(&hwnd_handle(owner)) {
        unsafe {
            let _ = WindowsAndMessaging::KillTimer(Some(owner), HOVER_TIMER_ID);
        }
    }
}

unsafe extern "system" fn hover_timer_proc(owner: HWND, _msg: u32, _id: usize, _time: u32) {
    let _scale = crate::dpi::enter_window_message(owner);
    cancel_hover(owner);
    let mut cursor = POINT::default();
    unsafe {
        if WindowsAndMessaging::GetCursorPos(&mut cursor).is_err()
            || !ScreenToClient(owner, &mut cursor).as_bool()
        {
            return;
        }
    }
    if rail_expand_hit_at(owner, (cursor.x, cursor.y))
        && !pointer_buttons_down()
        && !rail_page_popup_shown(owner)
    {
        open(owner);
    }
}

/// Shows the peek over the owner's collapsed rail. True when one is showing.
pub(super) fn open(owner: HWND) -> bool {
    cancel_hover(owner);
    if peek_for_owner(owner).is_some() {
        return true;
    }
    let Some(layout) = owner_peek_layout(owner) else {
        return false;
    };
    let (width, height) = layout.size();
    if width <= 0 || height <= 0 {
        return false;
    }
    destroy_sidebar_tabbar_popup(owner);
    let window = match unsafe {
        WindowsAndMessaging::CreateWindowExW(
            WindowsAndMessaging::WS_EX_LAYERED
                | WindowsAndMessaging::WS_EX_TOOLWINDOW
                | WindowsAndMessaging::WS_EX_NOACTIVATE,
            peek_class(),
            PCWSTR::null(),
            WS_POPUP,
            0,
            0,
            width,
            height,
            Some(owner),
            None,
            LibraryLoader::GetModuleHandleW(None)
                .ok()
                .map(|module| HINSTANCE(module.0)),
            None,
        )
    } {
        Ok(window) => window,
        Err(error) => {
            log::warn!("sidebar peek creation failed: {error}");
            return false;
        }
    };
    let screen = client_to_screen(owner, layout.frame);
    let peek = SidebarPeek {
        window: hwnd_handle(window),
        owner: hwnd_handle(owner),
        layout,
        screen,
        cursor: None,
        hover: None,
        tracking_leave: false,
        opened_at: Instant::now(),
        sliding: crate::refresh_bar::animations_enabled(),
        entered: false,
        left_at: None,
        buttons_down: pointer_buttons_down(),
        tooltip_shown: false,
        pressed: None,
    };
    peeks().insert(peek.owner, peek.clone());
    unsafe {
        let _ = WindowsAndMessaging::SetWindowPos(
            window,
            Some(WindowsAndMessaging::HWND_TOP),
            screen.left,
            screen.top,
            width,
            height,
            WindowsAndMessaging::SWP_NOACTIVATE | WindowsAndMessaging::SWP_SHOWWINDOW,
        );
        let _ = WindowsAndMessaging::SetTimer(Some(window), TICK_TIMER_ID, TICK_MS, None);
    }
    upload(window, &peek);
    true
}

/// Closes the owner's peek at once. True when there was one.
pub(super) fn close(owner: HWND) -> bool {
    cancel_hover(owner);
    let Some(peek) = peeks().remove(&hwnd_handle(owner)) else {
        return false;
    };
    if peek.tooltip_shown {
        destroy_sidebar_tabbar_popup(owner);
    }
    if is_window_handle_valid(peek.window) {
        unsafe {
            let window = hwnd_from_handle(peek.window);
            let _ = WindowsAndMessaging::KillTimer(Some(window), TICK_TIMER_ID);
            let _ = WindowsAndMessaging::DestroyWindow(window);
        }
    }
    true
}

/// Follows the owner's layout: fresh content repaints in place; a pin, a
/// size-class change, a resize or a hidden owner closes the peek.
pub(super) fn sync(owner: HWND) {
    let Some(mut peek) = peek_for_owner(owner) else {
        return;
    };
    let owner_thread = unsafe { WindowsAndMessaging::GetWindowThreadProcessId(owner, None) };
    if owner_thread != unsafe { GetCurrentThreadId() } {
        let handle = hwnd_handle(owner);
        let _ = post_to_window_thread(handle, Box::new(move || sync(hwnd_from_handle(handle))));
        return;
    }
    let _scale = crate::dpi::enter_window_message(owner);
    let Some(next) = owner_peek_layout(owner).filter(|next| peek.layout.same_placement(next))
    else {
        close(owner);
        return;
    };
    let screen = client_to_screen(owner, next.frame);
    let previous = peek.layout.clone();
    peek.layout.refresh_from(next);
    let moved = screen != peek.screen;
    if !moved && previous == peek.layout {
        return;
    }
    peek.screen = screen;
    peek.hover = peek
        .cursor
        .and_then(|cursor| peek.layout.hover_rect(cursor));
    store(&peek);
    let window = hwnd_from_handle(peek.window);
    if moved {
        unsafe {
            let _ = WindowsAndMessaging::SetWindowPos(
                window,
                None,
                screen.left,
                screen.top,
                0,
                0,
                WindowsAndMessaging::SWP_NOACTIVATE
                    | WindowsAndMessaging::SWP_NOSIZE
                    | WindowsAndMessaging::SWP_NOZORDER,
            );
        }
    }
    upload(window, &peek);
}

/// The WebView2 accelerator path: decide synchronously, act on the owner's
/// queue, since windows must not be created inside that callback.
pub(super) fn handle_key_deferred(owner: HWND, virtual_key: u32) -> bool {
    let handle = hwnd_handle(owner);
    if virtual_key == u32::from(VK_ESCAPE.0) {
        if peek_for_owner(owner).is_none() {
            return false;
        }
        let _ = post_to_window_thread(
            handle,
            Box::new(move || {
                close(hwnd_from_handle(handle));
            }),
        );
        return true;
    }
    let Some(target) = shortcut_target(owner, virtual_key) else {
        return false;
    };
    if latch_shortcut(owner, virtual_key) {
        let _ = post_to_window_thread(
            handle,
            Box::new(move || apply_shortcut(hwnd_from_handle(handle), target)),
        );
    }
    true
}

/// Esc closes an open peek; Ctrl+\ pins/collapses, or toggles the peek where
/// pinning is unavailable. An auto-repeat is consumed without acting. True
/// when the key was consumed.
pub(super) fn handle_key(owner: HWND, virtual_key: u32, repeat: bool) -> bool {
    if virtual_key == u32::from(VK_ESCAPE.0) {
        return close(owner);
    }
    let Some(target) = shortcut_target(owner, virtual_key) else {
        return false;
    };
    if latch_shortcut(owner, virtual_key) && !repeat {
        apply_shortcut(owner, target);
    }
    true
}

/// True for a fresh press; false while the same press is still held.
fn latch_shortcut(owner: HWND, virtual_key: u32) -> bool {
    let fresh = SHORTCUT_LATCH
        .get_or_init(|| Mutex::new(HashMap::new()))
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .insert(hwnd_handle(owner), virtual_key)
        .is_none();
    if fresh {
        unsafe {
            let _ = WindowsAndMessaging::SetTimer(
                Some(owner),
                LATCH_TIMER_ID,
                LATCH_POLL_MS,
                Some(latch_timer_proc),
            );
        }
    }
    fresh
}

unsafe extern "system" fn latch_timer_proc(owner: HWND, _msg: u32, _id: usize, _time: u32) {
    let mut latch = SHORTCUT_LATCH
        .get_or_init(|| Mutex::new(HashMap::new()))
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let released = latch
        .get(&hwnd_handle(owner))
        .is_none_or(|key| unsafe { GetAsyncKeyState(*key as i32) } >= 0);
    if released {
        latch.remove(&hwnd_handle(owner));
        unsafe {
            let _ = WindowsAndMessaging::KillTimer(Some(owner), LATCH_TIMER_ID);
        }
    }
}

fn shortcut_target(owner: HWND, virtual_key: u32) -> Option<crate::shell::SidebarShortcut> {
    let down = |key: u16| unsafe { GetKeyState(i32::from(key)) } < 0;
    if !is_sidebar_shortcut(
        virtual_key,
        backslash_virtual_key(),
        down(VK_CONTROL.0),
        down(VK_SHIFT.0),
        down(VK_MENU.0),
    ) {
        return None;
    }
    let webtag_key = active_webtag_key_for_window(owner)?;
    crate::shell::sidebar_shortcut(&current_window_layout(&webtag_key))
}

fn apply_shortcut(owner: HWND, target: crate::shell::SidebarShortcut) {
    let _scale = crate::dpi::enter_window_message(owner);
    match target {
        crate::shell::SidebarShortcut::Peek => {
            if !close(owner) {
                open(owner);
            }
        }
        crate::shell::SidebarShortcut::Toggle => {
            close(owner);
            if let Some(webtag_key) = active_webtag_key_for_window(owner) {
                invoke_chrome_command(
                    &webtag_key,
                    owner,
                    (0, 0),
                    crate::shell::sidebar_toggle_command(),
                );
            }
        }
    }
}

/// The key that types `\` on the active layout without Shift or AltGr.
fn backslash_virtual_key() -> u32 {
    const VK_OEM_5: u32 = 0xDC;
    let scan = unsafe { VkKeyScanW(u16::from(b'\\')) };
    if scan == -1 || (scan as u16) >> 8 != 0 {
        return VK_OEM_5;
    }
    u32::from(scan as u16 & 0xff)
}

fn is_sidebar_shortcut(
    virtual_key: u32,
    backslash: u32,
    control: bool,
    shift: bool,
    alt: bool,
) -> bool {
    virtual_key == backslash && control && !shift && !alt
}

fn peek_class() -> PCWSTR {
    static REGISTERED: OnceLock<()> = OnceLock::new();
    REGISTERED.get_or_init(|| {
        let module = unsafe { LibraryLoader::GetModuleHandleW(None) }
            .map(|module| HINSTANCE(module.0))
            .unwrap_or_default();
        let cursor =
            unsafe { WindowsAndMessaging::LoadCursorW(None, WindowsAndMessaging::IDC_ARROW) }
                .unwrap_or_default();
        let class = WNDCLASSW {
            lpfnWndProc: Some(peek_proc),
            hInstance: module,
            hCursor: cursor,
            lpszClassName: w!("LingXiaSidebarPeek"),
            ..Default::default()
        };
        if unsafe { WindowsAndMessaging::RegisterClassW(&class) } == 0 {
            log::error!(
                "sidebar peek class registration failed: {}",
                windows::core::Error::from_thread()
            );
        }
    });
    w!("LingXiaSidebarPeek")
}

unsafe extern "system" fn peek_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    let owner = peek_for_window(hwnd).map(|peek| hwnd_from_handle(peek.owner));
    let _scale = owner.map(crate::dpi::enter_window_message);
    match msg {
        WindowsAndMessaging::WM_MOUSEACTIVATE => {
            return LRESULT(WindowsAndMessaging::MA_NOACTIVATE as isize);
        }
        // The shadow is decoration; let the pointer reach what lies under it.
        WindowsAndMessaging::WM_NCHITTEST => {
            if let Some(peek) = peek_for_window(hwnd) {
                let point = lparam_screen_point(lparam);
                let local = (point.x - peek.screen.left, point.y - peek.screen.top);
                if peek.layout.hit(local).is_none() {
                    return LRESULT(WindowsAndMessaging::HTTRANSPARENT as isize);
                }
            }
            return LRESULT(WindowsAndMessaging::HTCLIENT as isize);
        }
        WindowsAndMessaging::WM_ERASEBKGND => return LRESULT(1),
        WindowsAndMessaging::WM_PAINT => {
            unsafe {
                let mut ps = PAINTSTRUCT::default();
                let _ = BeginPaint(hwnd, &mut ps);
                let _ = EndPaint(hwnd, &ps);
            }
            if let Some(peek) = peek_for_window(hwnd) {
                upload(hwnd, &peek);
            }
            return LRESULT(0);
        }
        WindowsAndMessaging::WM_MOUSEMOVE => {
            pointer_moved(hwnd, Some(lparam_client_point(lparam)));
            return LRESULT(0);
        }
        WM_MOUSELEAVE => {
            pointer_moved(hwnd, None);
            return LRESULT(0);
        }
        WindowsAndMessaging::WM_LBUTTONDOWN | WindowsAndMessaging::WM_RBUTTONDOWN => {
            if let Some(mut peek) = peek_for_window(hwnd) {
                peek.pressed = Some(msg == WindowsAndMessaging::WM_RBUTTONDOWN);
                store(&peek);
            }
            return LRESULT(0);
        }
        WindowsAndMessaging::WM_LBUTTONUP | WindowsAndMessaging::WM_RBUTTONUP => {
            let right = msg == WindowsAndMessaging::WM_RBUTTONUP;
            if let Some(mut peek) = peek_for_window(hwnd) {
                let pressed_here = peek.pressed.take() == Some(right);
                store(&peek);
                if pressed_here {
                    activate(hwnd, lparam_client_point(lparam), right);
                }
            }
            return LRESULT(0);
        }
        WindowsAndMessaging::WM_MOUSEWHEEL => {
            if let Some(mut peek) = peek_for_window(hwnd) {
                let point = lparam_screen_point(lparam);
                let local = (point.x - peek.screen.left, point.y - peek.screen.top);
                let delta = ((wparam.0 >> 16) & 0xffff) as u16 as i16;
                if peek.layout.wheel(local, delta) {
                    peek.hover = peek
                        .cursor
                        .and_then(|cursor| peek.layout.hover_rect(cursor));
                    store(&peek);
                    upload(hwnd, &peek);
                }
            }
            return LRESULT(0);
        }
        WindowsAndMessaging::WM_TIMER if wparam.0 == TICK_TIMER_ID => {
            tick(hwnd);
            return LRESULT(0);
        }
        WindowsAndMessaging::WM_DESTROY => {
            let handle = hwnd_handle(hwnd);
            peeks().retain(|_, peek| peek.window != handle);
        }
        _ => {}
    }
    unsafe { WindowsAndMessaging::DefWindowProcW(hwnd, msg, wparam, lparam) }
}

fn pointer_moved(window: HWND, cursor: Option<(i32, i32)>) {
    let Some(mut peek) = peek_for_window(window) else {
        return;
    };
    if cursor.is_some() && !peek.tracking_leave {
        let mut track = TRACKMOUSEEVENT {
            cbSize: std::mem::size_of::<TRACKMOUSEEVENT>() as u32,
            dwFlags: TME_LEAVE,
            hwndTrack: window,
            dwHoverTime: 0,
        };
        peek.tracking_leave = unsafe { TrackMouseEvent(&mut track) }.is_ok();
    } else if cursor.is_none() {
        peek.tracking_leave = false;
    }
    let hover = cursor.and_then(|cursor| peek.layout.hover_rect(cursor));
    let repaint = hover != peek.hover;
    peek.cursor = cursor;
    peek.hover = hover;
    peek.tooltip_shown = sync_expand_tooltip(&peek);
    store(&peek);
    if repaint {
        upload(window, &peek);
    }
}

/// The expand control's label rides the shared rail float, which hides itself
/// once the pointer leaves the control.
fn sync_expand_tooltip(peek: &SidebarPeek) -> bool {
    let owner = hwnd_from_handle(peek.owner);
    let tooltip = peek.cursor.and_then(|cursor| {
        let (anchor, popup, text) = peek.layout.expand_tooltip()?;
        point_in_rect(anchor, cursor).then_some((anchor, popup, text))
    });
    let Some((anchor, popup, text)) = tooltip else {
        if peek.tooltip_shown {
            destroy_sidebar_tabbar_popup(owner);
        }
        return false;
    };
    let to_screen = |rect: RECT| RECT {
        left: rect.left + peek.screen.left,
        top: rect.top + peek.screen.top,
        right: rect.right + peek.screen.left,
        bottom: rect.bottom + peek.screen.top,
    };
    show_sidebar_popup(
        owner,
        to_screen(popup),
        to_screen(anchor),
        SidebarPopupContent::Tooltip(text),
    );
    true
}

fn point_in_rect(rect: RECT, point: (i32, i32)) -> bool {
    point.0 >= rect.left && point.0 < rect.right && point.1 >= rect.top && point.1 < rect.bottom
}

fn activate(window: HWND, point: (i32, i32), context_menu: bool) {
    let Some(peek) = peek_for_window(window) else {
        return;
    };
    if peek.sliding {
        return;
    }
    let owner = hwnd_from_handle(peek.owner);
    let Some(webtag_key) = active_webtag_key_for_window(owner) else {
        return;
    };
    let command = match peek.layout.hit(point) {
        Some(WindowsChromeHit::Command(command)) if !context_menu => command,
        Some(WindowsChromeHit::CommandWithContext {
            command,
            context_menu: menu,
        }) => {
            if context_menu {
                menu
            } else {
                command
            }
        }
        _ => return,
    };
    // Commands carry owner-client points so menus anchor where the click was.
    let owner_point = (
        peek.layout.frame.left + point.0,
        peek.layout.frame.top + point.1,
    );
    // Compact's expand control has nothing to pin; it just closes the peek.
    if crate::shell::is_sidebar_peek_command(&command) {
        close(owner);
        return;
    }
    if crate::shell::sidebar_peek_dismisses(&command) {
        close(owner);
    }
    invoke_chrome_command(&webtag_key, owner, owner_point, command);
}

fn tick(window: HWND) {
    let Some(mut peek) = peek_for_window(window) else {
        unsafe {
            let _ = WindowsAndMessaging::KillTimer(Some(window), TICK_TIMER_ID);
        }
        return;
    };
    let owner = hwnd_from_handle(peek.owner);
    if peek.sliding {
        if peek.opened_at.elapsed() >= SLIDE_DURATION {
            peek.sliding = false;
        }
        upload(window, &peek);
    }
    let buttons_down = pointer_buttons_down();
    let pressed = buttons_down && !peek.buttons_down;
    peek.buttons_down = buttons_down;
    if menu_active() {
        peek.left_at = None;
        store(&peek);
        return;
    }
    let mut cursor = POINT::default();
    let inside = unsafe { WindowsAndMessaging::GetCursorPos(&mut cursor) }.is_ok()
        && peek
            .layout
            .hit((cursor.x - peek.screen.left, cursor.y - peek.screen.top))
            .is_some();
    if inside {
        peek.entered = true;
        peek.left_at = None;
    } else if pressed {
        close(owner);
        return;
    } else if peek.entered {
        let left_at = *peek.left_at.get_or_insert_with(Instant::now);
        if left_at.elapsed() >= LEAVE_GRACE {
            close(owner);
            return;
        }
    }
    store(&peek);
}

fn upload(window: HWND, peek: &SidebarPeek) {
    let (width, height) = peek.layout.size();
    if width <= 0 || height <= 0 {
        return;
    }
    let progress = if peek.sliding {
        peek.opened_at.elapsed().as_secs_f32() / SLIDE_DURATION.as_secs_f32()
    } else {
        1.0
    };
    unsafe {
        let screen = GetDC(None);
        if screen.is_invalid() {
            return;
        }
        let dc = CreateCompatibleDC(Some(screen));
        if dc.is_invalid() {
            let _ = ReleaseDC(None, screen);
            return;
        }
        let info = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: width,
                biHeight: -height,
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB.0,
                ..Default::default()
            },
            ..Default::default()
        };
        let mut bits: *mut c_void = std::ptr::null_mut();
        let Ok(bitmap) = CreateDIBSection(Some(screen), &info, DIB_RGB_COLORS, &mut bits, None, 0)
        else {
            let _ = DeleteDC(dc);
            let _ = ReleaseDC(None, screen);
            return;
        };
        if bits.is_null() {
            let _ = DeleteObject(HGDIOBJ(bitmap.0));
            let _ = DeleteDC(dc);
            let _ = ReleaseDC(None, screen);
            return;
        }
        let old_bitmap = SelectObject(dc, HGDIOBJ(bitmap.0));
        crate::shell::paint_sidebar_peek(dc, &peek.layout, peek.cursor);
        let pixels = std::slice::from_raw_parts_mut(bits.cast::<u32>(), (width * height) as usize);
        crate::shell::compose_sidebar_peek_pixels(
            pixels,
            &peek.layout,
            peek.layout.slide_shift(progress),
        );
        let size = SIZE {
            cx: width,
            cy: height,
        };
        let origin = POINT::default();
        let blend = BLENDFUNCTION {
            BlendOp: AC_SRC_OVER as u8,
            BlendFlags: 0,
            SourceConstantAlpha: 255,
            AlphaFormat: AC_SRC_ALPHA as u8,
        };
        let _ = WindowsAndMessaging::UpdateLayeredWindow(
            window,
            Some(screen),
            None,
            Some(&size),
            Some(dc),
            Some(&origin),
            COLORREF(0),
            Some(&blend),
            WindowsAndMessaging::ULW_ALPHA,
        );
        if !old_bitmap.is_invalid() {
            let _ = SelectObject(dc, old_bitmap);
        }
        let _ = DeleteObject(HGDIOBJ(bitmap.0));
        let _ = DeleteDC(dc);
        let _ = ReleaseDC(None, screen);
    }
}

#[cfg(test)]
mod tests {
    use super::is_sidebar_shortcut;

    #[test]
    fn shortcut_is_plain_ctrl_backslash() {
        const BACKSLASH: u32 = 0xDC;
        assert!(is_sidebar_shortcut(
            BACKSLASH, BACKSLASH, true, false, false
        ));
        assert!(!is_sidebar_shortcut(
            BACKSLASH, BACKSLASH, false, false, false
        ));
        assert!(!is_sidebar_shortcut(
            BACKSLASH, BACKSLASH, true, true, false
        ));
        assert!(!is_sidebar_shortcut(
            BACKSLASH, BACKSLASH, true, false, true
        ));
        assert!(!is_sidebar_shortcut(0x52, BACKSLASH, true, false, false));
    }
}
