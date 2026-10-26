//! Theme packs: colour roles per appearance plus numeric scales, resolved for user
//! preferences into one palette every renderer and host chrome paints from.

mod color;
mod generated;

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

pub use color::{Oklch, Rgba, contrast, ensure_contrast};
pub use generated::{CONTRAST, ROLE_COUNT, Role};

/// The pack shipped when a host names none.
pub fn default_pack() -> Pack {
    serde_json::from_str(include_str!("../packs/default.json")).expect("default pack parses")
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Pack {
    pub name: String,
    pub version: u32,
    pub appearance: Appearances,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub accent: Option<Accent>,
    #[serde(default, rename = "type")]
    pub typography: Typography,
    #[serde(default)]
    pub shape: Shape,
    #[serde(default)]
    pub space: Space,
    #[serde(default)]
    pub motion: Motion,
}

pub type Colors = BTreeMap<String, Rgba>;

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Appearances {
    pub light: Colors,
    pub dark: Colors,
    #[serde(
        default,
        rename = "high-contrast-light",
        skip_serializing_if = "Option::is_none"
    )]
    pub high_contrast_light: Option<Colors>,
    #[serde(
        default,
        rename = "high-contrast-dark",
        skip_serializing_if = "Option::is_none"
    )]
    pub high_contrast_dark: Option<Colors>,
}

/// With `derive`, the accent family (accent, on-accent, accent-soft, on-accent-soft, focus)
/// comes from the seed, and a user accent preference replaces the seed.
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Accent {
    pub seed: Rgba,
    #[serde(default)]
    pub derive: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields, default)]
pub struct Typography {
    pub family: Vec<String>,
    pub scale: BTreeMap<String, f32>,
    pub leading: BTreeMap<String, f32>,
}

impl Default for Typography {
    fn default() -> Self {
        Typography {
            family: vec!["system-ui".into()],
            scale: BTreeMap::new(),
            leading: BTreeMap::new(),
        }
    }
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields, default)]
pub struct Shape {
    pub radius: BTreeMap<String, f32>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields, default)]
pub struct Space {
    pub unit: f32,
}

impl Default for Space {
    fn default() -> Self {
        Space { unit: 4.0 }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields, default)]
pub struct Motion {
    pub fast: u32,
    pub base: u32,
    pub slow: u32,
    pub easing: String,
}

impl Default for Motion {
    fn default() -> Self {
        Motion {
            fast: 120,
            base: 200,
            slow: 320,
            easing: "standard".into(),
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Appearance {
    #[default]
    Light,
    Dark,
}

/// User preferences; the host resolves `auto` appearance from the OS before this.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Prefs {
    pub appearance: Appearance,
    pub high_contrast: bool,
    pub accent: Option<Rgba>,
    pub text_scale: f32,
}

impl Default for Prefs {
    fn default() -> Self {
        Prefs {
            appearance: Appearance::Light,
            high_contrast: false,
            accent: None,
            text_scale: 1.0,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct ResolvedTheme {
    pub appearance: Appearance,
    pub high_contrast: bool,
    pub palette: [Rgba; ROLE_COUNT],
    pub typography: Typography,
    pub shape: Shape,
    pub space: Space,
    pub motion: Motion,
    pub text_scale: f32,
}

impl ResolvedTheme {
    pub fn color(&self, role: Role) -> Rgba {
        self.palette[role as usize]
    }
}

const ACCENT_FAMILY: [Role; 5] = [
    Role::Accent,
    Role::OnAccent,
    Role::AccentSoft,
    Role::OnAccentSoft,
    Role::Focus,
];

/// The pack resolved for `prefs`. Contrast is proven by [`check`], not here, except that
/// derived accents and the high-contrast boost construct passing pairs.
pub fn resolve(pack: &Pack, prefs: &Prefs) -> Result<ResolvedTheme, String> {
    let (base, explicit_high_contrast) = colors_for(pack, prefs.appearance, prefs.high_contrast);
    let derived = pack.accent.filter(|accent| accent.derive);
    let mut palette = [Rgba([0, 0, 0, 255]); ROLE_COUNT];
    for role in Role::ALL {
        if derived.is_some() && ACCENT_FAMILY.contains(&role) {
            continue;
        }
        palette[role as usize] = *base
            .get(role.name())
            .ok_or_else(|| format!("THEME: {} has no {}", pack.name, role.name()))?;
    }
    if let Some(accent) = derived {
        derive_accent(
            &mut palette,
            prefs.accent.unwrap_or(accent.seed),
            prefs.appearance,
        );
    }
    if prefs.high_contrast && !explicit_high_contrast {
        for _ in 0..2 {
            for &(foreground, background, ratio) in CONTRAST {
                let (fg, bg) = (foreground as usize, background as usize);
                let target = boosted(ratio);
                palette[fg] = ensure_contrast(palette[fg], palette[bg], target);
                // A foreground already at black or white leaves only the background to move.
                if contrast(palette[fg], palette[bg]) < target {
                    palette[bg] = ensure_contrast(palette[bg], palette[fg], target);
                }
            }
        }
    }
    Ok(ResolvedTheme {
        appearance: prefs.appearance,
        high_contrast: prefs.high_contrast,
        palette,
        typography: pack.typography.clone(),
        shape: pack.shape.clone(),
        space: pack.space.clone(),
        motion: pack.motion.clone(),
        text_scale: prefs.text_scale,
    })
}

fn colors_for(pack: &Pack, appearance: Appearance, high_contrast: bool) -> (&Colors, bool) {
    let set = &pack.appearance;
    match (appearance, high_contrast) {
        (Appearance::Light, true) if set.high_contrast_light.is_some() => {
            (set.high_contrast_light.as_ref().unwrap(), true)
        }
        (Appearance::Dark, true) if set.high_contrast_dark.is_some() => {
            (set.high_contrast_dark.as_ref().unwrap(), true)
        }
        (Appearance::Light, _) => (&set.light, false),
        (Appearance::Dark, _) => (&set.dark, false),
    }
}

/// The high-contrast threshold for a base minimum: AA text to AAA, UI to AA text.
fn boosted(ratio: f32) -> f64 {
    match ratio {
        r if r >= 4.5 => 7.0,
        r if r >= 3.0 => 4.5,
        _ => 3.0,
    }
}

fn derive_accent(palette: &mut [Rgba; ROLE_COUNT], seed: Rgba, appearance: Appearance) {
    let dark = appearance == Appearance::Dark;
    let tone = seed.oklch();
    let surface = palette[Role::Surface as usize];
    let canvas = palette[Role::Canvas as usize];
    let start = if dark {
        Oklch {
            l: tone.l.max(0.7),
            ..tone
        }
        .rgba(255)
    } else {
        Oklch {
            l: tone.l.min(0.6),
            ..tone
        }
        .rgba(255)
    };
    let mut accent = ensure_contrast(ensure_contrast(start, surface, 3.0), canvas, 3.0);
    let white = Rgba([255, 255, 255, 255]);
    let ink = Oklch {
        l: 0.16,
        c: tone.c.min(0.03),
        h: tone.h,
    }
    .rgba(255);
    let on_accent = if contrast(white, accent) >= contrast(ink, accent) {
        white
    } else {
        ink
    };
    accent = ensure_contrast(accent, on_accent, 4.5);
    let soft = Oklch {
        l: if dark { 0.3 } else { 0.95 },
        c: tone.c * if dark { 0.35 } else { 0.25 },
        h: tone.h,
    }
    .rgba(255);
    let on_soft = Oklch {
        l: if dark { 0.88 } else { 0.4 },
        c: tone.c * 0.9,
        h: tone.h,
    }
    .rgba(255);
    palette[Role::Accent as usize] = accent;
    palette[Role::OnAccent as usize] = on_accent;
    palette[Role::AccentSoft as usize] = soft;
    palette[Role::OnAccentSoft as usize] = ensure_contrast(on_soft, soft, 4.5);
    palette[Role::Focus as usize] = accent;
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Problem {
    /// `light`, `dark`, `high-contrast-light`, `high-contrast-dark` or `pack`.
    pub appearance: &'static str,
    pub message: String,
}

impl std::fmt::Display for Problem {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{}: {}", self.appearance, self.message)
    }
}

/// Every reason the pack would ship unreadable or incomplete; empty when it is sound.
pub fn check(pack: &Pack) -> Vec<Problem> {
    let mut problems = Vec::new();
    let mut problem = |appearance, message: String| {
        problems.push(Problem {
            appearance,
            message,
        })
    };
    if pack.version != 1 {
        problem(
            "pack",
            format!("version {} is unsupported; use 1", pack.version),
        );
    }
    let set = &pack.appearance;
    let variants = [
        ("light", Some(&set.light), Appearance::Light, false),
        ("dark", Some(&set.dark), Appearance::Dark, false),
        (
            "high-contrast-light",
            set.high_contrast_light.as_ref(),
            Appearance::Light,
            true,
        ),
        (
            "high-contrast-dark",
            set.high_contrast_dark.as_ref(),
            Appearance::Dark,
            true,
        ),
    ];
    for (name, colors, appearance, high_contrast) in variants {
        let Some(colors) = colors else { continue };
        for key in colors.keys() {
            if Role::from_name(key).is_none() {
                problem(name, format!("unknown role {key}"));
            }
        }
        let prefs = Prefs {
            appearance,
            high_contrast,
            ..Prefs::default()
        };
        let theme = match resolve(pack, &prefs) {
            Ok(theme) => theme,
            Err(error) => {
                problem(name, error);
                continue;
            }
        };
        for &(foreground, background, ratio) in CONTRAST {
            let needed = if high_contrast {
                boosted(ratio)
            } else {
                f64::from(ratio)
            };
            let actual = contrast(theme.color(foreground), theme.color(background));
            if actual + 1e-9 < needed {
                problem(
                    name,
                    format!(
                        "{} on {} is {actual:.2}:1, needs {needed:.1}:1",
                        foreground.name(),
                        background.name()
                    ),
                );
            }
        }
    }
    for (group, values) in [
        ("type.scale", &pack.typography.scale),
        ("type.leading", &pack.typography.leading),
        ("shape.radius", &pack.shape.radius),
    ] {
        for (key, value) in values {
            if !(value.is_finite() && *value >= 0.0 && *value <= 16384.0) {
                problem("pack", format!("{group}.{key} must be 0-16384"));
            }
        }
    }
    if !(pack.space.unit.is_finite() && pack.space.unit > 0.0 && pack.space.unit <= 64.0) {
        problem("pack", "space.unit must be in (0, 64]".into());
    }
    problems
}

impl Pack {
    /// A pack from W3C Design Tokens: `color.<appearance>.<role>.$value` hex colours.
    /// Numeric scales come from `base`, which also supplies roles the tokens omit.
    pub fn from_dtcg(tokens: &serde_json::Value, base: &Pack) -> Result<Pack, String> {
        let groups = tokens
            .get("color")
            .and_then(|value| value.as_object())
            .ok_or("THEME: design tokens need a color group")?;
        let mut pack = base.clone();
        for (appearance, roles) in groups {
            let target = match appearance.as_str() {
                "light" => &mut pack.appearance.light,
                "dark" => &mut pack.appearance.dark,
                "high-contrast-light" => pack
                    .appearance
                    .high_contrast_light
                    .get_or_insert_with(Colors::new),
                "high-contrast-dark" => pack
                    .appearance
                    .high_contrast_dark
                    .get_or_insert_with(Colors::new),
                other => return Err(format!("THEME: unknown appearance group {other}")),
            };
            for (role, token) in roles
                .as_object()
                .ok_or("THEME: an appearance group must be an object")?
            {
                if token.get("$type").is_some_and(|kind| kind != "color") {
                    return Err(format!("THEME: {appearance}.{role} is not a color token"));
                }
                let value = token
                    .get("$value")
                    .and_then(|value| value.as_str())
                    .and_then(Rgba::parse)
                    .ok_or_else(|| format!("THEME: {appearance}.{role} needs a hex $value"))?;
                target.insert(role.clone(), value);
            }
        }
        Ok(pack)
    }
}

#[cfg(test)]
mod tests;
