//! The host chrome repaints the moment the scheme changes while the page needs a frame, so
//! the page column holds its own last frame until the page presents in the new scheme
//! (300 ms at most), then fades it over 200 ms; under reduced motion it goes at once. All of
//! it runs on the page's thread, which pumps messages.
use std::cell::RefCell;
use std::collections::HashMap;
use std::time::{Duration, Instant};
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, RECT, SIZE, WPARAM};
use windows::Win32::Graphics::Gdi::{
    AC_SRC_OVER, BI_RGB, BITMAPINFO, BITMAPINFOHEADER, BLENDFUNCTION, CreateCompatibleDC,
    CreateDIBSection, DIB_RGB_COLORS, DeleteDC, DeleteObject, GetDC, ReleaseDC, SelectObject,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, GA_ROOT, GetAncestor, GetWindowRect, KillTimer,
    RegisterClassW, SW_SHOWNOACTIVATE, SetTimer, ShowWindow, ULW_ALPHA, UpdateLayeredWindow,
    WM_DESTROY, WM_TIMER, WNDCLASSW, WS_EX_LAYERED, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW,
    WS_EX_TRANSPARENT, WS_POPUP,
};
use windows::core::w;

const TIMEOUT: Duration = Duration::from_millis(300);
const DURATION: Duration = Duration::from_millis(200);
const TIMEOUT_TIMER: usize = 1;
const FADE_TIMER: usize = 2;

struct Picture {
    page: isize,
    began: Instant,
    fading: Option<Instant>,
}

thread_local! {
    /// The pictures on this thread, by picture window.
    static PICTURES: RefCell<HashMap<isize, Picture>> = RefCell::new(HashMap::new());
}

fn picture_of(page: isize) -> Option<isize> {
    PICTURES.with(|p| {
        p.borrow()
            .iter()
            .find(|(_, picture)| picture.page == page && picture.fading.is_none())
            .map(|(window, _)| *window)
    })
}

/// Whether page window `page` holds a picture that waits for its next frame.
pub(crate) fn holds(page: isize) -> bool {
    picture_of(page).is_some()
}

/// Shows `frame` (width, height, straight RGBA rows) over page window `page`; false when it
/// cannot, so nothing waits for the page.
pub(crate) fn begin(page: isize, frame: (u32, u32, Vec<u8>)) -> bool {
    if holds(page) {
        return true;
    }
    match show_picture(page, frame) {
        Ok(picture) => {
            log::debug!(target: "lingxia_view", "scheme fade: began");
            PICTURES.with(|p| {
                p.borrow_mut().insert(
                    picture,
                    Picture {
                        page,
                        began: Instant::now(),
                        fading: None,
                    },
                )
            });
            // SAFETY: the window just made on this thread.
            unsafe {
                SetTimer(
                    Some(HWND(picture as *mut _)),
                    TIMEOUT_TIMER,
                    TIMEOUT.as_millis() as u32,
                    None,
                )
            };
            true
        }
        Err(error) => {
            log::debug!(target: "lingxia_view", "scheme fade: no picture: {error}");
            false
        }
    }
}

/// Page window `page` presented in the new scheme.
pub(crate) fn shown(page: isize) {
    if let Some(picture) = picture_of(page) {
        end(HWND(picture as *mut _), false);
    }
}

/// Opaque BGRA rows for a GDI bitmap from straight RGBA ones.
fn bgra(rgba: &[u8]) -> Vec<u8> {
    rgba.chunks_exact(4)
        .flat_map(|px| [px[2], px[1], px[0], 255])
        .collect()
}

/// A layered popup over the page window, owned by its top-level window, showing the frame.
fn show_picture(page: isize, (width, height, rgba): (u32, u32, Vec<u8>)) -> Result<isize, String> {
    let page = HWND(page as *mut _);
    let mut rect = RECT::default();
    // SAFETY: plain queries on this thread's page window.
    let owner = unsafe {
        GetWindowRect(page, &mut rect).map_err(|e| e.to_string())?;
        GetAncestor(page, GA_ROOT)
    };
    let (w, h) = (rect.right - rect.left, rect.bottom - rect.top);
    // A frame of another size is not what the column shows now.
    if w <= 0 || (w, h) != (width as i32, height as i32) || rgba.len() != (w * h * 4) as usize {
        return Err(format!("frame {width}x{height} for a {w}x{h} page"));
    }
    let class = w!("LingXiaSchemeFade");
    // SAFETY: plain Win32 calls; a repeat class registration fails harmlessly. Every GDI object
    // made here is released before returning.
    unsafe {
        let instance = GetModuleHandleW(None).map_err(|e| e.to_string())?;
        RegisterClassW(&WNDCLASSW {
            lpfnWndProc: Some(proc),
            hInstance: instance.into(),
            lpszClassName: class,
            ..Default::default()
        });
        let picture = CreateWindowExW(
            WS_EX_LAYERED | WS_EX_TRANSPARENT | WS_EX_NOACTIVATE | WS_EX_TOOLWINDOW,
            class,
            None,
            WS_POPUP,
            rect.left,
            rect.top,
            w,
            h,
            Some(owner),
            None,
            Some(instance.into()),
            None,
        )
        .map_err(|e| format!("scheme fade window: {e}"))?;

        let screen = GetDC(None);
        let memory = CreateCompatibleDC(Some(screen));
        let header = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: w,
                biHeight: -h,
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB.0,
                ..Default::default()
            },
            ..Default::default()
        };
        let mut bits: *mut std::ffi::c_void = std::ptr::null_mut();
        let bitmap =
            match CreateDIBSection(Some(memory), &header, DIB_RGB_COLORS, &mut bits, None, 0) {
                Ok(bitmap) if !bits.is_null() => bitmap,
                result => {
                    if let Ok(bitmap) = result {
                        let _ = DeleteObject(bitmap.into());
                    }
                    let _ = DeleteDC(memory);
                    ReleaseDC(None, screen);
                    let _ = DestroyWindow(picture);
                    return Err("scheme fade bitmap".into());
                }
            };
        let pixels = bgra(&rgba);
        std::ptr::copy_nonoverlapping(pixels.as_ptr(), bits.cast::<u8>(), pixels.len());
        let previous = SelectObject(memory, bitmap.into());
        let origin = POINT {
            x: rect.left,
            y: rect.top,
        };
        let size = SIZE { cx: w, cy: h };
        let source = POINT::default();
        let shown = UpdateLayeredWindow(
            picture,
            Some(screen),
            Some(&origin as *const POINT),
            Some(&size as *const SIZE),
            Some(memory),
            Some(&source as *const POINT),
            windows::Win32::Foundation::COLORREF(0),
            Some(&blend(255) as *const BLENDFUNCTION),
            ULW_ALPHA,
        )
        .is_ok();
        SelectObject(memory, previous);
        let _ = DeleteObject(bitmap.into());
        let _ = DeleteDC(memory);
        ReleaseDC(None, screen);
        if !shown {
            let _ = DestroyWindow(picture);
            return Err("scheme fade picture not shown".into());
        }
        let _ = ShowWindow(picture, SW_SHOWNOACTIVATE);
        Ok(picture.0 as isize)
    }
}

fn blend(alpha: u8) -> BLENDFUNCTION {
    BLENDFUNCTION {
        BlendOp: AC_SRC_OVER as u8,
        BlendFlags: 0,
        SourceConstantAlpha: alpha,
        AlphaFormat: 0,
    }
}

fn end(picture: HWND, timed_out: bool) {
    let key = picture.0 as isize;
    let began = PICTURES.with(|p| {
        let mut pictures = p.borrow_mut();
        let entry = pictures.get_mut(&key)?;
        if entry.fading.is_some() {
            return None;
        }
        entry.fading = Some(Instant::now());
        Some(entry.began)
    });
    let Some(began) = began else { return };
    log::debug!(
        target: "lingxia_view",
        "scheme fade: {} after {} ms",
        if timed_out { "timed out" } else { "page shown" },
        began.elapsed().as_millis()
    );
    // SAFETY: timers and the window are this thread's.
    unsafe {
        let _ = KillTimer(Some(picture), TIMEOUT_TIMER);
        if crate::environment::reduce_motion() {
            let _ = DestroyWindow(picture);
        } else {
            SetTimer(Some(picture), FADE_TIMER, 16, None);
        }
    }
}

/// Opacity `elapsed` into the fade: ease-out from 255 to 0; `None` once it is over.
fn fade_alpha(elapsed: Duration) -> Option<u8> {
    let progress = elapsed.as_secs_f32() / DURATION.as_secs_f32();
    if progress >= 1.0 {
        return None;
    }
    let eased = 1.0 - (1.0 - progress) * (1.0 - progress);
    Some(((1.0 - eased) * 255.0).round() as u8)
}

fn step(picture: HWND) {
    let key = picture.0 as isize;
    let Some(since) = PICTURES.with(|p| p.borrow().get(&key).and_then(|entry| entry.fading)) else {
        return;
    };
    let Some(alpha) = fade_alpha(since.elapsed()) else {
        // SAFETY: this thread's window.
        let _ = unsafe { DestroyWindow(picture) };
        return;
    };
    // SAFETY: an opacity change only; the window keeps the picture it was given.
    let _ = unsafe {
        UpdateLayeredWindow(
            picture,
            None,
            None,
            None,
            None,
            None,
            windows::Win32::Foundation::COLORREF(0),
            Some(&blend(alpha) as *const BLENDFUNCTION),
            ULW_ALPHA,
        )
    };
}

unsafe extern "system" fn proc(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match message {
        WM_TIMER if wparam.0 == TIMEOUT_TIMER => end(hwnd, true),
        WM_TIMER if wparam.0 == FADE_TIMER => step(hwnd),
        WM_DESTROY => {
            PICTURES.with(|p| p.borrow_mut().remove(&(hwnd.0 as isize)));
        }
        // SAFETY: the default handling for this window's message.
        _ => return unsafe { DefWindowProcW(hwnd, message, wparam, lparam) },
    }
    LRESULT(0)
}

#[cfg(test)]
mod tests {
    use super::{bgra, fade_alpha};
    use std::time::Duration;

    #[test]
    fn frame_rows_become_opaque_bgra() {
        assert_eq!(
            bgra(&[1, 2, 3, 0, 9, 8, 7, 128]),
            [3, 2, 1, 255, 7, 8, 9, 255]
        );
    }

    #[test]
    fn the_fade_eases_out_and_ends_at_its_duration() {
        assert_eq!(fade_alpha(Duration::ZERO), Some(255));
        let half = fade_alpha(Duration::from_millis(100)).unwrap();
        assert!(half < 128, "ease-out is past halfway at half time: {half}");
        assert_eq!(fade_alpha(Duration::from_millis(200)), None);
    }
}
