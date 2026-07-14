pub(crate) fn load_error_document(url: &str) -> String {
    let title = lingxia_platform::i18n::text("webview.load_error_title", "Couldn't load this page");
    let message = lingxia_platform::i18n::text(
        "webview.load_error_message",
        "Check your connection and try again.",
    );
    let retry_label = lingxia_platform::i18n::text("webview.retry", "Retry");
    lingxia_webview::render_load_error_page(lingxia_webview::LoadErrorPage {
        title: &title,
        message: &message,
        retry_label: &retry_label,
        retry_url: url,
    })
}
