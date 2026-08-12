//! Shared shape for user-dismissable APIs: every result is discriminated on
//! `canceled`, so a caller must branch before reading the payload, and a
//! rejection always means the operation failed rather than that the user
//! said no.

use rong::{JSContext, JSObject, JSResult};

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
