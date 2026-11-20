//! The interactive session's lock state.

use windows::Win32::System::RemoteDesktop::{
    ProcessIdToSessionId, WTS_CURRENT_SERVER_HANDLE, WTS_SESSIONSTATE_LOCK,
    WTS_SESSIONSTATE_UNLOCK, WTSFreeMemory, WTSINFOEXW, WTSQuerySessionInformationW,
    WTSSessionInfoEx,
};
use windows::Win32::System::Threading::GetCurrentProcessId;
use windows::core::PWSTR;

/// Whether this process's session has its screen locked, or `None` when Windows
/// does not say. The input desktop's name stays `Default` while locked, so ask the
/// session itself.
///
/// A locked session keeps running, but Windows refuses its processes the clipboard.
pub fn screen_locked() -> Option<bool> {
    // SAFETY: the buffer the call allocates is read as the struct it returns for this
    // class, after checking its size, and freed below.
    unsafe {
        let mut session = 0u32;
        ProcessIdToSessionId(GetCurrentProcessId(), &mut session).ok()?;
        let mut buffer = PWSTR::null();
        let mut bytes = 0u32;
        WTSQuerySessionInformationW(
            Some(WTS_CURRENT_SERVER_HANDLE),
            session,
            WTSSessionInfoEx,
            &mut buffer,
            &mut bytes,
        )
        .ok()?;
        let info = buffer.0 as *const WTSINFOEXW;
        let flags = (bytes as usize >= std::mem::size_of::<WTSINFOEXW>() && (*info).Level == 1)
            .then(|| (*info).Data.WTSInfoExLevel1.SessionFlags as u32);
        WTSFreeMemory(buffer.0.cast());
        match flags? {
            WTS_SESSIONSTATE_LOCK => Some(true),
            WTS_SESSIONSTATE_UNLOCK => Some(false),
            _ => None,
        }
    }
}
