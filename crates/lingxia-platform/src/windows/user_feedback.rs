//! User feedback (toast/modal/action sheet) is intentionally unimplemented on
//! Windows: desktop LingXia renders these in the page WebView (the same path
//! macOS uses), so the Logic layer never calls into the platform here. See
//! `lingxia-logic/src/ui/{toast,modal,action_sheet}.rs`, where the
//! `any(macos, windows)` branch routes to `call_view_with("ui.show*")` and the
//! View framework draws the overlay.
//!
//! A native Win32 fallback would also be unreliable: WinRT toasts need package
//! identity this unpackaged process lacks, and `TaskDialogIndirect`/owned
//! popups need shell window handles the platform layer has no access to by
//! design. All four methods therefore report `NotSupported`.

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
