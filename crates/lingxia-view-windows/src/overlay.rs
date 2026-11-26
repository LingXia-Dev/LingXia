//! A window overlay: a see-through page in a borderless popup owned by a host window, covering
//! it and following it. Its swap chain composes with premultiplied alpha over whatever is below;
//! a child window over a page's swap chain could not blend. Everything runs on the thread that
//! made the overlay, which must pump messages.
use crate::{Page, Scheme, Shell, view};
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::{Rc, Weak};
use std::sync::Arc;
use windows::Win32::Foundation::{COLORREF, HWND, RECT};
use windows::Win32::Graphics::Dwm::{
    DWM_WINDOW_CORNER_PREFERENCE, DWMWA_EXTENDED_FRAME_BOUNDS, DWMWA_WINDOW_CORNER_PREFERENCE,
    DWMWCP_DEFAULT, DWMWCP_ROUND, DWMWCP_ROUNDSMALL, DwmGetWindowAttribute,
};
use windows::Win32::System::Threading::GetCurrentProcessId;
use windows::Win32::UI::Accessibility::{HWINEVENTHOOK, SetWinEventHook, UnhookWinEvent};
use windows::Win32::UI::HiDpi::GetDpiForWindow;
use windows::Win32::UI::WindowsAndMessaging::{
    CHILDID_SELF, EVENT_OBJECT_DESTROY, EVENT_OBJECT_HIDE, EVENT_OBJECT_LOCATIONCHANGE,
    EVENT_OBJECT_SHOW, GA_ROOT, GWL_EXSTYLE, GWL_STYLE, GetAncestor, GetForegroundWindow,
    GetWindowLongPtrW, GetWindowRect, GetWindowThreadProcessId, HWND_TOP, IsWindow,
    IsWindowVisible, IsZoomed, LWA_ALPHA, OBJID_WINDOW, SW_HIDE, SW_SHOWNOACTIVATE,
    SWP_FRAMECHANGED, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SetForegroundWindow,
    SetLayeredWindowAttributes, SetWindowLongPtrW, SetWindowPos, ShowWindow, WINEVENT_OUTOFCONTEXT,
    WS_CAPTION, WS_EX_LAYERED, WS_EX_NOACTIVATE, WS_EX_NOREDIRECTIONBITMAP, WS_EX_TOOLWINDOW,
    WS_EX_TRANSPARENT, WS_THICKFRAME,
};

/// The visible top-level window holding `hwnd`.
pub fn root_window(hwnd: isize) -> Option<isize> {
    // SAFETY: plain queries on a window handle that may be stale.
    unsafe {
        let root = GetAncestor(HWND(hwnd as *mut _), GA_ROOT);
        (IsWindow(Some(root)).as_bool() && IsWindowVisible(root).as_bool())
            .then_some(root.0 as isize)
    }
}

/// The foreground window when it is this process's.
pub fn active_window() -> Option<isize> {
    // SAFETY: plain queries.
    unsafe {
        let window = GetForegroundWindow();
        let mut process = 0;
        GetWindowThreadProcessId(window, Some(&mut process as *mut u32));
        (!window.is_invalid() && process == GetCurrentProcessId()).then_some(window.0 as isize)
    }
}

/// Why an overlay can no longer show.
pub enum Gone {
    /// Its owner window closed.
    Owner,
    /// It has no GPU presenter, and a see-through window has no CPU present.
    NoGpu(String),
}

struct Owned {
    page: Weak<Page>,
    closed: Rc<dyn Fn(Gone)>,
}

thread_local! {
    /// Overlays on this thread by owner window, for the owner's move and close events.
    static OWNED: RefCell<HashMap<isize, Owned>> = RefCell::new(HashMap::new());
}

pub struct Overlay {
    page: Rc<Page>,
    owner: isize,
    hook: HWINEVENTHOOK,
}

impl Overlay {
    /// A hidden-until-drawn overlay over `owner`, passing input through and without focus.
    /// `closed` runs on this thread when `owner` goes or the overlay finds it cannot draw.
    pub fn new(
        token: i64,
        shell: Arc<dyn Shell>,
        worker: &str,
        owner: isize,
        closed: Box<dyn Fn(Gone)>,
    ) -> Result<Self, String> {
        let (bounds, radius) = cover(owner);
        let ex_style = WS_EX_NOREDIRECTIONBITMAP
            | WS_EX_LAYERED
            | WS_EX_TRANSPARENT
            | WS_EX_TOOLWINDOW
            | WS_EX_NOACTIVATE;
        let page = Page::with_window(token, shell, worker, true, Scheme::Auto, bounds, || {
            let popup = view::create_popup(owner, ex_style)?;
            // SAFETY: the new window of this thread; a layered window needs its alpha before it shows.
            let _ = unsafe {
                SetLayeredWindowAttributes(HWND(popup as *mut _), COLORREF(0), 255, LWA_ALPHA)
            };
            Ok(popup)
        })?;
        page.set_see_through();
        let [left, top, width, height] = bounds;
        page.set_bounds(left, top, width, height, [radius; 4]);
        let mut process = 0;
        // SAFETY: an out-parameter; the hook calls back on this thread's message loop.
        let hook = unsafe {
            let thread =
                GetWindowThreadProcessId(HWND(owner as *mut _), Some(&mut process as *mut u32));
            SetWinEventHook(
                EVENT_OBJECT_DESTROY,
                EVENT_OBJECT_LOCATIONCHANGE,
                None,
                Some(owner_event),
                process,
                thread,
                WINEVENT_OUTOFCONTEXT,
            )
        };
        OWNED.with(|owned| {
            owned.borrow_mut().insert(
                owner,
                Owned {
                    page: Rc::downgrade(&page),
                    closed: Rc::from(closed),
                },
            )
        });
        Ok(Self { page, owner, hook })
    }

    pub fn page(&self) -> &Page {
        &self.page
    }

    /// Blocking, the overlay takes every click over the owner; otherwise all pass through.
    pub fn block(&self, block: bool) {
        let window = self.window();
        // SAFETY: this thread's window; the frame change applies the new style.
        unsafe {
            let style = GetWindowLongPtrW(window, GWL_EXSTYLE);
            let transparent = WS_EX_TRANSPARENT.0 as isize;
            let style = if block {
                style & !transparent
            } else {
                style | transparent
            };
            SetWindowLongPtrW(window, GWL_EXSTYLE, style);
            let _ = SetWindowPos(
                window,
                Some(HWND_TOP),
                0,
                0,
                0,
                0,
                SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE | SWP_FRAMECHANGED,
            );
        }
    }

    /// Focused, the overlay takes the keyboard; unfocused, the owner gets it back.
    pub fn focus(&self, focused: bool) {
        let window = self.window();
        // SAFETY: this thread's window and its owner.
        unsafe {
            if focused {
                let _ = SetForegroundWindow(window);
                self.page.focus();
            } else if GetForegroundWindow() == window {
                let _ = SetForegroundWindow(HWND(self.owner as *mut _));
            }
        }
    }

    fn window(&self) -> HWND {
        HWND(self.page.hwnd() as *mut _)
    }
}

impl Drop for Overlay {
    fn drop(&mut self) {
        OWNED.with(|owned| owned.borrow_mut().remove(&self.owner));
        // SAFETY: the hook this overlay set.
        let _ = unsafe { UnhookWinEvent(self.hook) };
    }
}

/// The owner's visible frame in screen pixels, and the radius its corners are rounded to.
fn cover(owner: isize) -> ([i32; 4], i32) {
    let window = HWND(owner as *mut _);
    let mut rect = RECT::default();
    // SAFETY: out-parameters on the stack, for a window that may be gone.
    unsafe {
        let framed = DwmGetWindowAttribute(
            window,
            DWMWA_EXTENDED_FRAME_BOUNDS,
            &mut rect as *mut RECT as *mut _,
            std::mem::size_of::<RECT>() as u32,
        );
        if framed.is_err() {
            let _ = GetWindowRect(window, &mut rect);
        }
    }
    let bounds = [
        rect.left,
        rect.top,
        rect.right - rect.left,
        rect.bottom - rect.top,
    ];
    (bounds, corner_radius(window))
}

/// Windows 11 rounds a framed window's corners, 8 pt (4 pt when asked for small ones) unless
/// it is maximized; Windows 10 answers no preference and rounds nothing.
fn corner_radius(window: HWND) -> i32 {
    let mut preference = DWM_WINDOW_CORNER_PREFERENCE(0);
    // SAFETY: out-parameter queries on a window that may be gone.
    unsafe {
        let asked = DwmGetWindowAttribute(
            window,
            DWMWA_WINDOW_CORNER_PREFERENCE,
            &mut preference as *mut DWM_WINDOW_CORNER_PREFERENCE as *mut _,
            std::mem::size_of::<DWM_WINDOW_CORNER_PREFERENCE>() as u32,
        );
        if asked.is_err() || IsZoomed(window).as_bool() {
            return 0;
        }
        let framed =
            GetWindowLongPtrW(window, GWL_STYLE) & (WS_CAPTION.0 | WS_THICKFRAME.0) as isize != 0;
        let points = match preference {
            DWMWCP_ROUND => 8,
            DWMWCP_ROUNDSMALL => 4,
            DWMWCP_DEFAULT if framed => 8,
            _ => return 0,
        };
        let dpi = GetDpiForWindow(window).max(96) as i32;
        points * dpi / 96
    }
}

/// The page window `hwnd` got no GPU presenter: its overlay reports it cannot draw.
pub(crate) fn gpu_unavailable(hwnd: isize, error: &str) {
    let closed = OWNED.with(|owned| {
        owned
            .borrow()
            .values()
            .find(|o| o.page.upgrade().is_some_and(|page| page.hwnd() == hwnd))
            .map(|o| o.closed.clone())
    });
    if let Some(closed) = closed {
        closed(Gone::NoGpu(error.to_owned()));
    }
}

/// The owner moved, showed, hid or went.
unsafe extern "system" fn owner_event(
    _: HWINEVENTHOOK,
    event: u32,
    hwnd: HWND,
    object: i32,
    child: i32,
    _: u32,
    _: u32,
) {
    if object != OBJID_WINDOW.0 || child != CHILDID_SELF as i32 {
        return;
    }
    let owner = hwnd.0 as isize;
    let Some((page, closed)) = OWNED.with(|owned| {
        owned
            .borrow()
            .get(&owner)
            .map(|o| (o.page.upgrade(), o.closed.clone()))
    }) else {
        return;
    };
    match event {
        EVENT_OBJECT_DESTROY => closed(Gone::Owner),
        EVENT_OBJECT_LOCATIONCHANGE => {
            if let Some(page) = page {
                let ([left, top, width, height], radius) = cover(owner);
                page.set_bounds(left, top, width, height, [radius; 4]);
            }
        }
        EVENT_OBJECT_SHOW | EVENT_OBJECT_HIDE => {
            if let Some(page) = page {
                let shown = event == EVENT_OBJECT_SHOW;
                // SAFETY: this thread's window.
                let _ = unsafe {
                    ShowWindow(
                        HWND(page.hwnd() as *mut _),
                        if shown { SW_SHOWNOACTIVATE } else { SW_HIDE },
                    )
                };
            }
        }
        _ => {}
    }
}
