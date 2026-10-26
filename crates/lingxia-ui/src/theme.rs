//! The resolved role palette pages paint with; colours never reach the View.

use lingxia_theme::{Appearance, Prefs, ROLE_COUNT, Role};
use std::sync::OnceLock;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Palette(pub [[u8; 4]; ROLE_COUNT]);

impl Palette {
    /// The default pack for `dark`, used until the host pushes a palette.
    pub fn builtin(dark: bool) -> Palette {
        static BUILTIN: OnceLock<[Palette; 2]> = OnceLock::new();
        BUILTIN.get_or_init(|| {
            let pack = lingxia_theme::default_pack();
            [Appearance::Light, Appearance::Dark].map(|appearance| {
                let theme = lingxia_theme::resolve(
                    &pack,
                    &Prefs {
                        appearance,
                        ..Prefs::default()
                    },
                )
                .expect("default pack resolves");
                Palette(theme.palette.map(|color| color.0))
            })
        })[usize::from(dark)]
    }

    /// The environment's `palette` push: `{ revision, colors }`, one hex per slot. A host
    /// built against fewer roles leaves the newer slots to the built-in palette.
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
        Ok(palette)
    }

    pub fn role(&self, role: Role) -> [u8; 4] {
        self.0[role as usize]
    }

    /// A wire colour: `#rrggbb[aa]`, a role, or `role/NN` with NN an alpha percent.
    pub fn resolve(&self, value: &str) -> Result<[u8; 4], String> {
        match parse(value)? {
            WireColor::Literal(rgba) => Ok(rgba),
            WireColor::Role(role, percent) => {
                let [r, g, b, a] = self.role(role);
                Ok([
                    r,
                    g,
                    b,
                    ((u32::from(a) * u32::from(percent) + 50) / 100) as u8,
                ])
            }
        }
    }
}

pub(crate) enum WireColor {
    Literal([u8; 4]),
    Role(Role, u8),
}

pub(crate) fn parse(value: &str) -> Result<WireColor, String> {
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
    let role = Role::from_name(name).ok_or_else(|| format!("UI_COLOR: unknown role {name}"))?;
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
