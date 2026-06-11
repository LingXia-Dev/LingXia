//! User feedback (toast/modal/action sheet) is intentionally unimplemented on
//! Windows for now:
//!
//! - Toasts: WinRT `ToastNotificationManager` only delivers notifications for
//!   callers with package identity (MSIX) or an explicit AUMID that is backed
//!   by a registered Start Menu shortcut. This process runs unpackaged with
//!   neither, so `CreateToastNotifierWithId`/`Show` either fails with
//!   "element not found" or silently drops the toast depending on the OS
//!   build — too unreliable to ship. An in-process popup fallback would mean
//!   building UI inside the platform layer, which is owned by the product
//!   shell instead.
//! - Modals/action sheets: `TaskDialogIndirect`/owned popups need the host
//!   HWND for correct ownership and modality, and the platform layer has no
//!   access to shell window handles by design.
//!
//! All four methods therefore report `NotSupported` honestly rather than
//! pretending to display feedback.

use std::future::Future;

use super::{Platform, not_supported};
use crate::error::PlatformError;
use crate::traits::ui::{ModalOptions, ToastOptions, UserFeedback};

impl UserFeedback for Platform {
    fn show_toast(&self, _options: ToastOptions) -> Result<(), PlatformError> {
        not_supported("show_toast")
    }

    fn hide_toast(&self) -> Result<(), PlatformError> {
        not_supported("hide_toast")
    }

    fn show_modal(
        &self,
        _options: ModalOptions,
    ) -> impl Future<Output = Result<String, PlatformError>> + Send {
        async { not_supported("show_modal") }
    }

    fn show_action_sheet(
        &self,
        _options: Vec<String>,
        _cancel_text: String,
        _item_color: String,
    ) -> impl Future<Output = Result<String, PlatformError>> + Send {
        async { not_supported("show_action_sheet") }
    }
}
