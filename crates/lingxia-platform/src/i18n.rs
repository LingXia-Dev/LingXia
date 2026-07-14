//! Localized strings for platform-owned UI.
//!
//! `lingxia-platform` sits *below* the i18n string table (which lives in
//! `lingxia-logic`, and `lingxia-logic` depends on this crate), so it cannot
//! call the table directly. Instead the logic layer installs a translator
//! hook here once at startup; platform scenes look strings up by stable key,
//! falling back to their bundled English literal before runtime initialization.

use std::sync::OnceLock;

type Localizer = Box<dyn Fn(&str) -> Option<String> + Send + Sync>;

static LOCALIZER: OnceLock<Localizer> = OnceLock::new();

/// Installs the locale-aware lookup owned by the logic layer. Later calls are
/// ignored; scenes never supply localized strings per invocation.
pub fn set_localizer(localizer: impl Fn(&str) -> Option<String> + Send + Sync + 'static) {
    let _ = LOCALIZER.set(Box::new(localizer));
}

/// Resolves `key`, falling back when runtime localization is unavailable.
pub fn text(key: &str, fallback: &str) -> String {
    LOCALIZER
        .get()
        .and_then(|translate| translate(key))
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| fallback.to_string())
}
