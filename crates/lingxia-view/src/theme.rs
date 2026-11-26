//! The resolved role palette pages paint with; colours never reach the View.

use lingxia_theme::{Appearance, Prefs, ROLE_COUNT, Role};
use std::collections::BTreeMap;
use std::sync::{Arc, OnceLock};

/// The role slots, and the app's own colours by name with `on-<name>` for each fill's label.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Palette(pub [[u8; 4]; ROLE_COUNT], Arc<BTreeMap<String, [u8; 4]>>);

impl Palette {
    /// A palette of roles alone.
    pub fn from_roles(roles: [[u8; 4]; ROLE_COUNT]) -> Palette {
        Palette(roles, Arc::default())
    }

    /// `theme` as painted: its roles and its app colours.
    pub fn from_theme(theme: &lingxia_theme::ResolvedTheme) -> Palette {
        let app = theme
            .app
            .iter()
            .map(|(name, color)| (name.clone(), color.0))
            .collect();
        Palette(theme.palette.map(|color| color.0), Arc::new(app))
    }

    /// The default pack for `dark`, used until the host pushes a palette.
    pub fn builtin(dark: bool) -> Palette {
        Self::builtin_for(dark, false)
    }

    /// The default pack for `dark` and the system's increased-contrast preference.
    pub fn builtin_for(dark: bool, high_contrast: bool) -> Palette {
        static BUILTIN: OnceLock<[Palette; 4]> = OnceLock::new();
        BUILTIN.get_or_init(|| {
            let pack = lingxia_theme::default_pack();
            [(false, false), (true, false), (false, true), (true, true)].map(
                |(dark, high_contrast)| {
                    let prefs = Prefs {
                        appearance: if dark {
                            Appearance::Dark
                        } else {
                            Appearance::Light
                        },
                        high_contrast,
                        ..Prefs::default()
                    };
                    let theme =
                        lingxia_theme::resolve(&pack, &prefs).expect("default pack resolves");
                    Palette::from_theme(&theme)
                },
            )
        })[usize::from(dark) + 2 * usize::from(high_contrast)]
        .clone()
    }

    /// The environment's `palette` push: `{ revision, colors, app? }`, one hex per slot and one
    /// per app colour. A host built against fewer roles leaves the newer slots to the built-in
    /// palette.
    pub fn decode(value: &serde_json::Value, dark: bool) -> Result<Palette, String> {
        let invalid = || "UI_ENVIRONMENT: invalid palette".to_string();
        let colors = value["colors"]
            .as_array()
            .filter(|c| !c.is_empty() && c.len() <= 256)
            .ok_or_else(invalid)?;
        if !value["revision"].is_u64() {
            return Err(invalid());
        }
        let mut palette = Palette::builtin(dark);
        for (slot, color) in colors.iter().enumerate().take(ROLE_COUNT) {
            palette.0[slot] = color
                .as_str()
                .and_then(lingxia_theme::Rgba::parse)
                .ok_or_else(invalid)?
                .0;
        }
        let mut app = BTreeMap::new();
        if let Some(pushed) = value.get("app") {
            let pushed = pushed
                .as_object()
                .filter(|colors| colors.len() <= 2 * lingxia_theme::APP_COLOR_LIMIT)
                .ok_or_else(invalid)?;
            for (name, color) in pushed {
                let color = color
                    .as_str()
                    .and_then(lingxia_theme::Rgba::parse)
                    .ok_or_else(invalid)?;
                app.insert(name.clone(), color.0);
            }
        }
        palette.1 = Arc::new(app);
        Ok(palette)
    }

    /// The app colour `name`, if the pushed theme has it.
    pub fn app(&self, name: &str) -> Option<[u8; 4]> {
        self.1.get(name).copied()
    }

    pub fn role(&self, role: Role) -> [u8; 4] {
        self.0[role as usize]
    }

    /// This palette as drawn inside `surface`: a lifted surface hands its contents its own lines.
    pub fn under(&self, surface: Role) -> Palette {
        let mut scoped = self.clone();
        for role in Role::ALL {
            scoped.0[role as usize] = self.role(role.under(surface));
        }
        scoped
    }

    /// A wire colour: `#rrggbb[aa]`, a role or app colour, either with `/NN` alpha percent.
    pub fn resolve(&self, value: &str) -> Result<[u8; 4], String> {
        let ([r, g, b, a], percent) = match parse(value)? {
            WireColor::Literal(rgba) => return Ok(rgba),
            WireColor::Role(role, percent) => (self.role(role), percent),
            WireColor::App(name, percent) => (self.app_or_neutral(name)?, percent),
        };
        Ok([
            r,
            g,
            b,
            ((u32::from(a) * u32::from(percent) + 50) / 100) as u8,
        ])
    }

    /// A name the pushed theme lacks fails a debug build; a release one paints muted text,
    /// never transparent, and logs it once.
    fn app_or_neutral(&self, name: &str) -> Result<[u8; 4], String> {
        if let Some(color) = self.app(name) {
            return Ok(color);
        }
        if cfg!(debug_assertions) {
            return Err(format!("UI_COLOR: app.{name} is not in this app's theme"));
        }
        static LOGGED: OnceLock<()> = OnceLock::new();
        if LOGGED.set(()).is_ok() {
            log::warn!("UI_COLOR: app.{name} is not in this app's theme; painting text-muted");
        }
        Ok(self.role(Role::TextMuted))
    }
}

pub(crate) enum WireColor<'a> {
    Literal([u8; 4]),
    Role(Role, u8),
    /// An app colour by name; which names exist is the pushed theme's question.
    App(&'a str, u8),
}

pub(crate) fn parse(value: &str) -> Result<WireColor<'_>, String> {
    if value.starts_with('#') {
        return lingxia_theme::Rgba::parse(value)
            .map(|color| WireColor::Literal(color.0))
            .ok_or_else(|| "UI_COLOR: expected #rrggbb or #rrggbbaa".into());
    }
    let (name, percent) = match value.split_once('/') {
        Some((name, alpha)) => (
            name,
            alpha
                .parse::<u8>()
                .ok()
                .filter(|p| *p <= 100 && !alpha.starts_with('+'))
                .ok_or("UI_COLOR: a role alpha is 0-100")?,
        ),
        None => (value, 100),
    };
    if let Some(app) = name.strip_prefix("app.") {
        let kebab = app.split('-').all(|part| {
            !part.is_empty()
                && part
                    .bytes()
                    .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit())
        }) && app.starts_with(|first: char| first.is_ascii_lowercase());
        return if kebab {
            Ok(WireColor::App(app, percent))
        } else {
            Err(format!("UI_COLOR: {name} is not an app colour name"))
        };
    }
    let role = Role::from_name(name)
        .filter(|role| role.authorable())
        .ok_or_else(|| format!("UI_COLOR: unknown role {name}"))?;
    Ok(WireColor::Role(role, percent))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roles_resolve_per_appearance_with_alpha_and_hex_passes_through() {
        let (light, dark) = (Palette::builtin(false), Palette::builtin(true));
        assert_ne!(light.resolve("canvas"), dark.resolve("canvas"));
        let text = light.role(Role::Text);
        assert_eq!(
            light.resolve("text/8").unwrap(),
            [text[0], text[1], text[2], 20]
        );
        assert_eq!(
            light.resolve("#12345680").unwrap(),
            [0x12, 0x34, 0x56, 0x80]
        );
        for bad in ["brand", "text/101", "text/x", "#1234", "Surface", "text/+5"] {
            assert!(light.resolve(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn the_increased_contrast_preference_resolves_the_high_contrast_variant() {
        for dark in [false, true] {
            let (base, high) = (Palette::builtin(dark), Palette::builtin_for(dark, true));
            let ratio = |p: &Palette| {
                lingxia_theme::contrast(
                    lingxia_theme::Rgba(p.role(Role::TextMuted)),
                    lingxia_theme::Rgba(p.role(Role::Surface)),
                )
            };
            assert!(ratio(&high) > ratio(&base), "dark={dark}");
            assert!(ratio(&high) >= 7.0, "AAA for body-like text, dark={dark}");
        }
        assert_eq!(
            Palette::builtin(false).resolve("shadow/10").unwrap(),
            [0, 0, 0, 26]
        );
    }

    #[test]
    fn app_colours_resolve_by_name_with_alpha_from_the_push() {
        let pushed = serde_json::json!({
            "revision": 1,
            "colors": ["#000000"],
            "app": {"trend-up": "#0f7b3f", "on-brand-band": "#ffffff"},
        });
        let palette = Palette::decode(&pushed, false).unwrap();
        assert_eq!(
            palette.resolve("app.trend-up").unwrap(),
            [0x0f, 0x7b, 0x3f, 255]
        );
        assert_eq!(
            palette.resolve("app.on-brand-band/40").unwrap(),
            [255, 255, 255, 102]
        );
        for bad in ["app.Trend", "app.", "app.trend-up/101", "app.-x"] {
            assert!(palette.resolve(bad).is_err(), "{bad}");
        }
        if cfg!(debug_assertions) {
            assert!(palette.resolve("app.trend-down").is_err());
        } else {
            assert_eq!(
                palette.resolve("app.trend-down").unwrap(),
                palette.role(Role::TextMuted)
            );
        }
        let bad = serde_json::json!({"revision": 1, "colors": ["#000000"], "app": {"x": "red"}});
        assert!(Palette::decode(&bad, false).is_err());
    }

    #[test]
    fn a_pushed_palette_fills_slots_it_does_not_know() {
        let pushed = serde_json::json!({"revision": 3, "colors": ["#000000", "#ff000080"]});
        let palette = Palette::decode(&pushed, false).unwrap();
        assert_eq!(palette.role(Role::Canvas), [0, 0, 0, 255]);
        assert_eq!(palette.role(Role::Surface), [255, 0, 0, 128]);
        assert_eq!(
            palette.role(Role::Text),
            Palette::builtin(false).role(Role::Text)
        );
        for bad in [
            serde_json::json!({"colors": ["#000000"]}),
            serde_json::json!({"revision": 1, "colors": []}),
            serde_json::json!({"revision": 1, "colors": ["red"]}),
        ] {
            assert!(Palette::decode(&bad, false).is_err());
        }
    }
}
