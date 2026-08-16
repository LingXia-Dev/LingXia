//! Shared shape for user-dismissable APIs: every result is discriminated on
//! `canceled`, so a caller must branch before reading the payload, and a
//! rejection always means the operation failed rather than that the user
//! said no.

use rong::{JSContext, JSObject, JSResult};

/// The business code every platform reports when the user dismissed the UI
/// rather than when anything failed. It is the whole contract behind this
/// module: an adapter that sends it for a real failure turns a crash into
/// "the user said no", silently and in the direction that keeps going. Kept
/// named on both sides of the boundary so the two uses stay greppable — see
/// `LxAppDismissal.userDismissedCode` (Apple), `LxAppDismissal.USER_DISMISSED`
/// (Android), and `USER_DISMISSED_CODE` (Harmony).
pub(crate) const USER_DISMISSED: u32 = 2000;

/// `{ canceled: true }` — the user dismissed the operation.
pub(crate) fn canceled(ctx: &JSContext) -> JSResult<JSObject> {
    let result = JSObject::new(ctx);
    result.set("canceled", true)?;
    Ok(result)
}

/// `{ canceled: false }` — set the payload fields on the returned object.
pub(crate) fn completed(ctx: &JSContext) -> JSResult<JSObject> {
    let result = JSObject::new(ctx);
    result.set("canceled", false)?;
    Ok(result)
}
