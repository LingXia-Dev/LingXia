//! Display refreshes for pages that asked for one: a single thread waits on DWM's next
//! composition while any request is pending, then posts each page its vsync.
use crate::dispatch::Dispatcher;
use std::collections::HashMap;
use std::sync::{Condvar, Mutex, OnceLock};
use std::time::Duration;
use windows::Win32::Foundation::HWND;
use windows::Win32::Graphics::Dwm::{DWM_TIMING_INFO, DwmFlush, DwmGetCompositionTimingInfo};
use windows::Win32::System::Performance::{QueryPerformanceCounter, QueryPerformanceFrequency};

/// Called on the page's thread with the frame's time to screen and the refresh interval (µs).
pub(crate) type Tick = Box<dyn FnOnce(i64, u64) + Send>;

struct Source {
    pending: Mutex<HashMap<i64, (Dispatcher, Tick)>>,
    wake: Condvar,
}

fn source() -> &'static Source {
    static SOURCE: OnceLock<&'static Source> = OnceLock::new();
    SOURCE.get_or_init(|| {
        let source: &'static Source = Box::leak(Box::new(Source {
            pending: Mutex::new(HashMap::new()),
            wake: Condvar::new(),
        }));
        let _ = std::thread::Builder::new()
            .name("lingxia-view-vsync".into())
            .spawn(move || source.run());
        source
    })
}

/// The next refresh calls `tick` on `dispatcher`'s thread; a newer request from the same page
/// replaces an unanswered one.
pub(crate) fn request(token: i64, dispatcher: Dispatcher, tick: Tick) {
    let source = source();
    source
        .pending
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .insert(token, (dispatcher, tick));
    source.wake.notify_one();
}

pub(crate) fn cancel(token: i64) {
    source()
        .pending
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .remove(&token);
}

impl Source {
    fn run(&self) {
        loop {
            {
                let mut pending = self.pending.lock().unwrap_or_else(|p| p.into_inner());
                while pending.is_empty() {
                    pending = self.wake.wait(pending).unwrap_or_else(|p| p.into_inner());
                }
            }
            // SAFETY: no arguments; fails without desktop composition (a locked session).
            if unsafe { DwmFlush() }.is_err() {
                std::thread::sleep(Duration::from_millis(16));
            }
            let (present_in, interval) = timing();
            let due = std::mem::take(&mut *self.pending.lock().unwrap_or_else(|p| p.into_inner()));
            for (_, (dispatcher, tick)) in due {
                dispatcher.post(Box::new(move || tick(present_in, interval)));
            }
        }
    }
}

/// Microseconds to the next vblank, and the refresh period.
fn timing() -> (i64, u64) {
    let fallback = (16_667, 16_667);
    let mut info = DWM_TIMING_INFO {
        cbSize: std::mem::size_of::<DWM_TIMING_INFO>() as u32,
        ..Default::default()
    };
    let (mut now, mut frequency) = (0i64, 0i64);
    // SAFETY: out-parameters on the stack.
    let ok = unsafe {
        DwmGetCompositionTimingInfo(HWND::default(), &mut info).is_ok()
            && QueryPerformanceCounter(&mut now).is_ok()
            && QueryPerformanceFrequency(&mut frequency).is_ok()
    };
    if !ok || frequency <= 0 || info.qpcRefreshPeriod == 0 {
        return fallback;
    }
    let micros = |ticks: i64| ticks.saturating_mul(1_000_000) / frequency;
    let period = info.qpcRefreshPeriod as i64;
    let mut next = info.qpcVBlank as i64;
    while next <= now {
        next += period;
    }
    (micros(next - now), micros(period).max(1) as u64)
}
