//! Windows shell product layer: custom window chrome.
//!
//! The shell owns all product window-chrome policy on Windows — colors,
//! dimensions, GDI painting of the top bar / sidebar / tab bar / panel
//! decorations, and the mapping from client points to chrome elements.
//! It plugs into `lingxia-webview`'s generic hosting layer through the
//! [`lingxia_webview::platform::windows::WindowsChromeRenderer`] seam.

mod chrome;
pub mod clipboard;
pub mod terminal_grid;
pub mod text_input;

/// Registers the shell window chrome renderer with `lingxia-webview`.
///
/// Called from `register_runtime()`; must run before the first window is
/// created so hosts get the custom (borderless) frame.
pub(crate) fn install() {
    chrome::install();
}
