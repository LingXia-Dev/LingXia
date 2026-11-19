//! CPU time of one thread, read from any thread: the View JS budget charges the worker for the
//! time it ran, not for time the host spent on other work.
use std::time::Duration;

#[derive(Clone, Copy, Debug)]
pub(crate) struct ThreadClock(Handle);

#[cfg(any(target_os = "macos", target_os = "ios"))]
type Handle = libc::mach_port_t;
#[cfg(any(target_os = "linux", target_os = "android"))]
type Handle = libc::clockid_t;
/// A real handle to the thread: the watchdog reads it from another thread.
#[cfg(windows)]
type Handle = isize;
#[cfg(not(any(
    target_os = "macos",
    target_os = "ios",
    target_os = "linux",
    target_os = "android",
    windows
)))]
type Handle = ();

#[cfg(any(target_os = "linux", target_os = "android"))]
unsafe extern "C" {
    fn pthread_getcpuclockid(thread: libc::pthread_t, clock: *mut libc::clockid_t) -> libc::c_int;
}

impl ThreadClock {
    /// The calling thread's clock; `None` where the platform has none.
    pub(crate) fn current() -> Option<Self> {
        #[cfg(any(target_os = "macos", target_os = "ios"))]
        {
            // The pthread's own port: no reference to release.
            let port = unsafe { libc::pthread_mach_thread_np(libc::pthread_self()) };
            let clock = Self(port);
            clock.elapsed().map(|_| clock)
        }
        #[cfg(any(target_os = "linux", target_os = "android"))]
        {
            let mut id: libc::clockid_t = 0;
            (unsafe { pthread_getcpuclockid(libc::pthread_self(), &mut id) } == 0)
                .then_some(Self(id))
                .filter(|clock| clock.elapsed().is_some())
        }
        #[cfg(windows)]
        {
            use windows::Win32::Foundation::{CloseHandle, HANDLE};
            use windows::Win32::System::Threading::{
                GetCurrentThreadId, OpenThread, THREAD_QUERY_LIMITED_INFORMATION,
            };
            struct Own(isize);
            impl Drop for Own {
                fn drop(&mut self) {
                    // SAFETY: the handle this thread opened for itself, closed once at its exit.
                    let _ = unsafe { CloseHandle(HANDLE(self.0 as *mut _)) };
                }
            }
            thread_local! {
                static OWN: Option<Own> = unsafe {
                    OpenThread(THREAD_QUERY_LIMITED_INFORMATION, false, GetCurrentThreadId())
                }
                .ok()
                .map(|handle| Own(handle.0 as isize));
            }
            OWN.with(|own| own.as_ref().map(|own| Self(own.0)))
                .filter(|clock| clock.elapsed().is_some())
        }
        #[cfg(not(any(
            target_os = "macos",
            target_os = "ios",
            target_os = "linux",
            target_os = "android",
            windows
        )))]
        None
    }

    /// CPU time the thread has used so far.
    pub(crate) fn elapsed(&self) -> Option<Duration> {
        #[cfg(any(target_os = "macos", target_os = "ios"))]
        {
            let mut info: libc::thread_basic_info = unsafe { std::mem::zeroed() };
            let mut count = libc::THREAD_BASIC_INFO_COUNT;
            let status = unsafe {
                libc::thread_info(
                    self.0,
                    libc::THREAD_BASIC_INFO as _,
                    (&mut info as *mut libc::thread_basic_info).cast(),
                    &mut count,
                )
            };
            let micros = |t: libc::time_value_t| -> Option<u64> {
                Some(
                    u64::try_from(t.seconds).ok()? * 1_000_000
                        + u64::try_from(t.microseconds).ok()?,
                )
            };
            if status != 0 {
                return None;
            }
            Some(Duration::from_micros(
                micros(info.user_time)? + micros(info.system_time)?,
            ))
        }
        #[cfg(any(target_os = "linux", target_os = "android"))]
        {
            let mut time: libc::timespec = unsafe { std::mem::zeroed() };
            (unsafe { libc::clock_gettime(self.0, &mut time) } == 0)
                .then(|| Duration::new(time.tv_sec as u64, time.tv_nsec as u32))
        }
        #[cfg(windows)]
        {
            use windows::Win32::Foundation::{FILETIME, HANDLE};
            use windows::Win32::System::Threading::GetThreadTimes;
            let mut times = [FILETIME::default(); 4];
            let [created, exited, kernel, user] = &mut times;
            // SAFETY: a live thread's handle, kept open for as long as the thread runs.
            unsafe { GetThreadTimes(HANDLE(self.0 as *mut _), created, exited, kernel, user) }
                .ok()?;
            let ticks =
                |t: &FILETIME| u64::from(t.dwHighDateTime) << 32 | u64::from(t.dwLowDateTime);
            // 100 ns ticks.
            Some(Duration::from_nanos((ticks(kernel) + ticks(user)) * 100))
        }
        #[cfg(not(any(
            target_os = "macos",
            target_os = "ios",
            target_os = "linux",
            target_os = "android",
            windows
        )))]
        None
    }
}
