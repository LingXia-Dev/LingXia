//! Theme roles for native chrome and WebView pages, from the same resolver View pages use.

use lingxia_theme::{ROLE_COUNT, Rgba, Role, ThemeLooks, looks};
use std::sync::{Arc, OnceLock};

/// Whether the product is in high contrast as far as the runtime knows: the user's `more`.
pub fn high_contrast() -> bool {
    super::host_contrast_update().preference == super::ContrastPreference::More
}

/// A role of the host's theme, for product chrome; `None` when the product ships no theme, so
/// desktop shells keep their native colours.
pub fn host_role(role: Role, dark: bool) -> Option<Rgba> {
    Some(host_looks()?.role(role, dark, high_contrast()))
}

/// The host theme's looks; the pack never changes once the config is set.
pub fn host_looks() -> Option<Arc<ThemeLooks>> {
    static LOOKS: OnceLock<Arc<ThemeLooks>> = OnceLock::new();
    let pack = lingxia_app_context::host_theme_pack()?;
    Some(LOOKS.get_or_init(|| looks(Some(pack))).clone())
}

/// A role of the theme `appid` renders with (its own, else the host's, else the default).
pub fn app_role(appid: &str, role: Role, dark: bool) -> Rgba {
    let looks = match super::try_get(appid) {
        Some(app) => app.theme_looks(),
        None => host_looks().unwrap_or_else(|| looks(None)),
    };
    looks.role(role, dark, high_contrast())
}

/// A role as `#rrggbb` for native chrome: `appid`'s theme, or with an empty `appid` the host's
/// (`None` without a host theme). `None` for an unknown role name.
pub fn theme_role_hex(appid: &str, role: &str, dark: bool) -> Option<String> {
    let role = Role::from_name(role)?;
    let color = if appid.is_empty() {
        host_role(role, dark)?
    } else {
        app_role(appid, role, dark)
    };
    Some(color.hex())
}

/// `--lx-<role>` for every role of `palette`, as CSS declarations.
fn declarations(palette: &[Rgba; ROLE_COUNT]) -> String {
    let mut out = String::new();
    for role in Role::ALL {
        out.push_str("--lx-");
        out.push_str(role.name());
        out.push(':');
        out.push_str(&palette[role as usize].hex());
        out.push(';');
    }
    out
}

/// The role variables a WebView page reads, for one look. Plain `:root`, injected first in
/// `<head>`: a page stylesheet declaring the same name comes later and wins.
pub fn role_stylesheet(looks: &ThemeLooks, dark: bool, high_contrast: bool) -> String {
    format!(
        ":root{{{}}}",
        declarations(looks.palette(dark, high_contrast))
    )
}

/// The id of the `<style>` element holding [`role_stylesheet`].
const ROLE_STYLE_ID: &str = "lx-roles";

impl super::LxApp {
    /// The looks of the theme this lxapp renders with.
    pub fn theme_looks(&self) -> Arc<ThemeLooks> {
        if let Some(looks) = self.state.lock().unwrap().theme_looks.clone() {
            return looks;
        }
        let resolved = looks(self.theme_pack().as_deref());
        self.state.lock().unwrap().theme_looks = Some(resolved.clone());
        resolved
    }

    fn role_stylesheet(&self, dark: bool) -> String {
        role_stylesheet(&self.theme_looks(), dark, high_contrast())
    }

    /// The `<style>` a WebView page's HTML carries, so the roles are there before first paint.
    pub(crate) fn role_style_tag(&self) -> String {
        let dark = self.appearance_state().resolved.is_dark();
        format!(
            "<style id=\"{ROLE_STYLE_ID}\">{}</style>",
            self.role_stylesheet(dark)
        )
    }

    /// Replaces a loaded page's role variables in place after an appearance or contrast change.
    pub(crate) fn role_style_script(&self, dark: bool) -> String {
        let css = serde_json::to_string(&self.role_stylesheet(dark)).unwrap_or_default();
        format!(
            "(function(){{var s=document.getElementById('{ROLE_STYLE_ID}');if(s)s.textContent={css};}})();"
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_role_is_a_root_variable_of_the_look() {
        let looks = looks(None);
        let css = role_stylesheet(&looks, true, false);
        let canvas_dark = looks.role(Role::Canvas, true, false).hex();
        assert!(
            css.starts_with(&format!(":root{{--lx-canvas:{canvas_dark};")),
            "{css}"
        );
        assert_eq!(css.matches("--lx-accent:").count(), 1);
        assert_eq!(css.matches('{').count(), 1);
    }

    #[test]
    fn a_pack_that_does_not_parse_renders_with_the_default() {
        let default = looks(None);
        let broken = looks(Some("{"));
        assert_eq!(broken.palette(false, false), default.palette(false, false));
    }
}
