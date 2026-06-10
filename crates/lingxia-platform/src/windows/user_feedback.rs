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
