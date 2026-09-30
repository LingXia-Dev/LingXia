use super::tsfn::call_arkts;

/// Keeps the app window's screen on while held. A screen that turns off stops
/// ArkUI building new pages and pauses media, so a run would stall until someone
/// woke the phone.
pub struct DisplayAwake(());

impl Drop for DisplayAwake {
    fn drop(&mut self) {
        if let Err(err) = call_arkts("setKeepScreenOn", &["false"]) {
            log::warn!("Failed to release keep-screen-on: {err}");
        }
    }
}

/// Hold the screen on until the returned guard drops. It does not wake a
/// screen that is already off, and it does not unlock one.
pub fn keep_display_awake() -> DisplayAwake {
    if let Err(err) = call_arkts("setKeepScreenOn", &["true"]) {
        log::warn!("Failed to keep the screen on: {err}");
    }
    DisplayAwake(())
}
