use crate::I18nKey;

/// Normalize locale string to use hyphens instead of underscores
///
/// Converts platform-specific locale formats:
/// - iOS: "zh_CN" -> "zh-CN"
/// - Android: "zh-rCN" -> "zh-CN"
/// - Standard: "zh-CN" -> "zh-CN"
#[inline]
fn normalize_locale(locale: &str) -> String {
    locale.replace('_', "-").replace("-r", "-")
}

/// Get localized string for a given key
///
/// Automatically retrieves locale from lxapp runtime.
/// This is the recommended way to use i18n in the logic layer.
///
/// # Arguments
/// * `key` - The i18n key to look up
///
/// # Returns
/// The localized string for the given key
///
/// # Example
/// ```ignore
/// let cancel_text = t(I18nKey::CommonCancel);
/// let confirm_text = t(I18nKey::CommonConfirm);
/// ```
#[inline]
pub fn t(key: I18nKey) -> String {
    let locale = lxapp::get_locale();
    let normalized = normalize_locale(&locale);
    key.get(&normalized).to_string()
}
