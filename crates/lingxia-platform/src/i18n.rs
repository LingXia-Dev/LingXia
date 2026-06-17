//! Localized strings for native platform UI (file/media dialog titles).
//!
//! `lingxia-platform` sits *below* the i18n string table (which lives in
//! `lingxia-logic`, and `lingxia-logic` depends on this crate), so it cannot
//! call the table directly. Instead the logic layer installs a translator
//! hook here at startup ([`set_dialog_translator`]); the platform code looks
//! strings up by key via [`dialog_title`], falling back to the bundled
//! English literal when no translator is installed.

use std::sync::OnceLock;

type DialogTranslator = Box<dyn Fn(&str) -> Option<String> + Send + Sync>;

static DIALOG_TRANSLATOR: OnceLock<DialogTranslator> = OnceLock::new();

/// Installs the locale-aware translator used for native dialog titles. Called
/// once by the logic layer (which owns the i18n table) at startup; later calls
/// are ignored.
pub fn set_dialog_translator(translator: impl Fn(&str) -> Option<String> + Send + Sync + 'static) {
    let _ = DIALOG_TRANSLATOR.set(Box::new(translator));
}

/// Localized title for `key` (e.g. `"file_chooser.select_folder"`), falling
/// back to `fallback` when no translator is installed or the key is unknown.
pub fn dialog_title(key: &str, fallback: &str) -> String {
    DIALOG_TRANSLATOR
        .get()
        .and_then(|translate| translate(key))
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| fallback.to_string())
}
