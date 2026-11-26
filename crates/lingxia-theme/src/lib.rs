//! Theme packs: colour roles per appearance plus numeric scales, resolved for user
//! preferences into one palette every renderer and host chrome paints from.

mod color;
mod derive;
mod generated;

use std::collections::{BTreeMap, HashMap};
use std::sync::{LazyLock, Mutex};

use serde::{Deserialize, Serialize};

pub use color::{Oklch, Rgba, contrast, distance, ensure_contrast, over};
pub use generated::{COMPOSITE_CONTRAST, CONTRAST, ROLE_COUNT, Role, Tier};

/// The pack shipped when a host names none.
pub fn default_pack() -> Pack {
    serde_json::from_str(include_str!("../packs/default.json")).expect("default pack parses")
}

static APP_PACKS: LazyLock<Mutex<HashMap<String, Pack>>> = LazyLock::new(Default::default);

/// Sets or clears the pack an lxapp bundles, keyed by its app session's View worker key,
/// before its pages are created; page hosts read it at creation.
pub fn set_app_pack(key: &str, pack: Option<Pack>) {
    let mut packs = APP_PACKS.lock().unwrap_or_else(|p| p.into_inner());
    match pack {
        Some(pack) => packs.insert(key.to_owned(), pack),
        None => packs.remove(key),
    };
}

/// The pack registered for `key`, if any.
pub fn app_pack(key: &str) -> Option<Pack> {
    APP_PACKS
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .get(key)
        .cloned()
}

/// `pack` with every authored and fixed role, the accent seed and every scale step it leaves
/// out taken from the default pack, so an lxapp pack may change only its accent. Derived roles
/// and the near-blacks are never copied: [`resolve`] computes them from the pack's own colours.
pub fn complete(pack: Pack) -> Pack {
    complete_from(pack, &default_pack())
}

/// [`complete`] from `base`, itself complete, in place of the default pack.
pub fn complete_from(mut pack: Pack, base: &Pack) -> Pack {
    let own_accent = [&pack.appearance.light, &pack.appearance.dark]
        .iter()
        .any(|colors| colors.contains_key(Role::Accent.name()));
    if pack.accent.is_none() && !own_accent {
        pack.accent = base.accent;
    }
    for (colors, defaults) in [
        (&mut pack.appearance.light, &base.appearance.light),
        (&mut pack.appearance.dark, &base.appearance.dark),
    ] {
        for (role, color) in defaults {
            if Role::from_name(role).is_some_and(|role| {
                role.tier() != Tier::Derived && !NEAR_BLACKS.contains(&role) && role != Role::Knob
            }) {
                colors.entry(role.clone()).or_insert(*color);
            }
        }
    }
    for (values, defaults) in [
        (&mut pack.typography.scale, &base.typography.scale),
        (&mut pack.typography.leading, &base.typography.leading),
        (&mut pack.shape.radius, &base.shape.radius),
    ] {
        for (key, value) in defaults {
            values.entry(key.clone()).or_insert(*value);
        }
    }
    pack
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Pack {
    pub name: String,
    pub version: u32,
    #[serde(default)]
    pub appearance: Appearances,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub colors: BTreeMap<String, AppColor>,
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

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Appearances {
    #[serde(default)]
    pub light: Colors,
    #[serde(default)]
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

/// At most this many app colours, so each look's map stays small.
pub const APP_COLOR_LIMIT: usize = 32;

/// A colour an app names for itself, proven against the surface roles it is drawn on.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct AppColor {
    pub light: Rgba,
    pub dark: Rgba,
    /// Without one, high contrast boosts the colour as it does an authored role.
    #[serde(
        default,
        rename = "high-contrast-light",
        skip_serializing_if = "Option::is_none"
    )]
    pub high_contrast_light: Option<Rgba>,
    #[serde(
        default,
        rename = "high-contrast-dark",
        skip_serializing_if = "Option::is_none"
    )]
    pub high_contrast_dark: Option<Rgba>,
    #[serde(rename = "use")]
    pub usage: ColorUse,
    /// The surface roles it appears on; a fill is held to its label and advised on these.
    #[serde(default = "default_grounds")]
    pub on: Vec<String>,
    /// A fill's label colour; without one, the look's `text` or `on-accent`, whichever reads more.
    #[serde(default, rename = "on-color", skip_serializing_if = "Option::is_none")]
    pub on_color: Option<Rgba>,
}

fn default_grounds() -> Vec<String> {
    vec![Role::Surface.name().to_owned()]
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ColorUse {
    /// Marks and edges, 3:1 against each surface it is on.
    Graphic,
    /// Text, 4.5:1 against each surface it is on.
    Text,
    /// A solid that carries a label, named `on-<name>`, at 4.5:1.
    Fill,
}

impl ColorUse {
    fn ratio(self) -> f32 {
        match self {
            ColorUse::Graphic => 3.0,
            ColorUse::Text | ColorUse::Fill => 4.5,
        }
    }
}

/// The colour every derived role starts from. Accent roles the pack wrote are used as written and
/// the rest come from the seed. With `derive`, a user accent preference replaces the seed and the
/// whole accent family then comes from it.
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

/// Durations in milliseconds, three easings as CSS timing functions (`easing` is the
/// standard one, for a change in place; `enter` and `exit` for what arrives and leaves) and
/// the bounce of the pack's spring. Pages name them: `duration-fast`, `ease-enter`.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields, default)]
pub struct Motion {
    pub fast: u32,
    pub base: u32,
    pub slow: u32,
    pub easing: String,
    pub enter: String,
    pub exit: String,
    pub spring: f32,
}

impl Default for Motion {
    fn default() -> Self {
        Motion {
            fast: 150,
            base: 200,
            slow: 300,
            easing: "cubic-bezier(0, 0, 0.2, 1)".into(),
            enter: "cubic-bezier(0, 0, 0.2, 1)".into(),
            exit: "cubic-bezier(0.4, 0, 1, 1)".into(),
            spring: 0.0,
        }
    }
}

/// Whether `value` is a timing function a transition takes: a keyword, or a cubic bezier
/// whose two x coordinates lie in 0–1.
fn is_easing(value: &str) -> bool {
    if matches!(
        value,
        "linear" | "ease" | "ease-in" | "ease-out" | "ease-in-out"
    ) {
        return true;
    }
    let Some(points) = value
        .strip_prefix("cubic-bezier(")
        .and_then(|rest| rest.strip_suffix(')'))
    else {
        return false;
    };
    let points: Vec<Option<f64>> = points
        .split(',')
        .map(|point| point.trim().parse().ok().filter(|n: &f64| n.is_finite()))
        .collect();
    matches!(points[..], [Some(x1), Some(_), Some(x2), Some(_)]
        if (0.0..=1.0).contains(&x1) && (0.0..=1.0).contains(&x2))
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
    /// App colours by name, with `on-<name>` for each fill's label.
    pub app: BTreeMap<String, Rgba>,
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

const ACCENT_FAMILY: [Role; 7] = [
    Role::Accent,
    Role::OnAccent,
    Role::AccentSoft,
    Role::OnAccentSoft,
    Role::Focus,
    Role::AccentSolid,
    Role::OnAccentSolid,
];

/// Fixed roles a pack may leave out, taken from its own dark canvas.
const NEAR_BLACKS: [Role; 3] = [Role::Scrim, Role::Media, Role::MediaCanvas];

/// The pack resolved for `prefs`: its authored and fixed roles as written, boosted under high
/// contrast, then every derived role it left out, built to pass its pairs. A colour the pack
/// wrote is proven by [`check`], not here.
pub fn resolve(pack: &Pack, prefs: &Prefs) -> Result<ResolvedTheme, String> {
    let (base, explicit_high_contrast) = colors_for(pack, prefs.appearance, prefs.high_contrast);
    let seeded = pack.accent.filter(|accent| accent.derive);
    // Only a user accent overrides what the pack wrote for its own seed.
    let picked = seeded.is_some_and(|accent| prefs.accent.is_some_and(|user| user != accent.seed));
    let mut palette = [Rgba([0, 0, 0, 255]); ROLE_COUNT];
    let mut written = [false; ROLE_COUNT];
    for role in Role::ALL {
        // The knob follows the track, so a user accent drops a written one with the family.
        if picked && (ACCENT_FAMILY.contains(&role) || role == Role::Knob) {
            continue;
        }
        let slot = role as usize;
        if derive::is_shade(role) {
            continue;
        }
        if let Some(color) = base.get(role.name()) {
            palette[slot] = *color;
            written[slot] = true;
            continue;
        }
        palette[slot] = match role {
            _ if role.tier() == Tier::Derived => continue,
            Role::Knob => Some(Rgba([255, 255, 255, 255])),
            _ if NEAR_BLACKS.contains(&role) => near_black(pack, base, role, prefs.appearance),
            _ => None,
        }
        .ok_or_else(|| format!("THEME: {} has no {}", pack.name, role.name()))?;
    }
    let seed = match seeded {
        Some(accent) => prefs.accent.unwrap_or(accent.seed),
        None if written[Role::Accent as usize] => palette[Role::Accent as usize],
        None => pack
            .accent
            .map(|accent| accent.seed)
            .ok_or_else(|| format!("THEME: {} has no accent", pack.name))?,
    };
    // The overlay is in place before the boost, so text is boosted on it too.
    let overlay = Role::SurfaceOverlay as usize;
    let mut placed = written;
    if !written[overlay] {
        palette[overlay] = derive::overlay(
            palette[Role::SurfaceRaised as usize],
            prefs.appearance == Appearance::Dark,
        );
        placed[overlay] = true;
    }
    let boost = prefs.high_contrast && !explicit_high_contrast;
    if boost {
        for _ in 0..2 {
            for &(foreground, background, ratio) in CONTRAST {
                let (fg, bg) = (foreground as usize, background as usize);
                if keeps_base_ratio(foreground, background) || !placed[fg] || !placed[bg] {
                    continue;
                }
                let target = boosted(ratio);
                palette[fg] = ensure_contrast(palette[fg], palette[bg], target);
                // A foreground already at black or white leaves only the background to move.
                if contrast(palette[fg], palette[bg]) < target {
                    palette[bg] = ensure_contrast(palette[bg], palette[fg], target);
                }
            }
        }
    }
    // Both appearances move the same categorical colours, so each lends its side.
    let both = [&pack.appearance.light, &pack.appearance.dark];
    let side =
        |role: Role| both.map(|colors| *colors.get(role.name()).unwrap_or(&palette[role as usize]));
    let sides = derive::Sides {
        surface: side(Role::Surface),
        canvas: side(Role::Canvas),
        muted: side(Role::TextMuted),
        statuses: [Role::Danger, Role::Success, Role::Warning].map(side),
    };
    derive::Build {
        palette: &mut palette,
        written,
        dark: prefs.appearance == Appearance::Dark,
        strong: prefs.high_contrast,
        movable: boost,
    }
    .run(seed, &sides);
    if !written[Role::Selection as usize] {
        palette[Role::Selection as usize] = derive_selection(&palette);
    }
    let app = resolve_app_colors(pack, prefs, &palette);
    Ok(ResolvedTheme {
        appearance: prefs.appearance,
        high_contrast: prefs.high_contrast,
        palette,
        app,
        typography: pack.typography.clone(),
        shape: pack.shape.clone(),
        space: pack.space.clone(),
        motion: pack.motion.clone(),
        text_scale: prefs.text_scale,
    })
}

/// Each app colour for `prefs`, boosted under high contrast unless it wrote its own value there,
/// and each fill's label chosen after the boost.
fn resolve_app_colors(
    pack: &Pack,
    prefs: &Prefs,
    palette: &[Rgba; ROLE_COUNT],
) -> BTreeMap<String, Rgba> {
    let mut app = BTreeMap::new();
    for (name, color) in &pack.colors {
        let written = match (prefs.appearance, prefs.high_contrast) {
            (Appearance::Light, true) => color.high_contrast_light,
            (Appearance::Dark, true) => color.high_contrast_dark,
            _ => None,
        };
        let mut value = written.unwrap_or(match prefs.appearance {
            Appearance::Light => color.light,
            Appearance::Dark => color.dark,
        });
        let boost = prefs.high_contrast && written.is_none();
        if color.usage == ColorUse::Fill {
            let ink = color.on_color.unwrap_or_else(|| auto_ink(value, palette));
            if boost {
                value = ensure_contrast(value, ink, boosted(4.5));
            }
            app.insert(name.clone(), value);
            app.insert(format!("on-{name}"), ink);
            continue;
        }
        if boost {
            for ground in grounds(color, palette) {
                value = ensure_contrast(value, ground, boosted(color.usage.ratio()));
            }
        }
        app.insert(name.clone(), value);
    }
    app
}

/// The palette's dark or light ink, whichever stands further off `fill`.
fn auto_ink(fill: Rgba, palette: &[Rgba; ROLE_COUNT]) -> Rgba {
    let [text, on_accent] = [Role::Text, Role::OnAccent].map(|role| palette[role as usize]);
    if contrast(text, fill) >= contrast(on_accent, fill) {
        text
    } else {
        on_accent
    }
}

/// The resolved surfaces an app colour declares; unknown names are left to [`check`].
fn grounds(color: &AppColor, palette: &[Rgba; ROLE_COUNT]) -> Vec<Rgba> {
    color
        .on
        .iter()
        .filter_map(|name| Role::from_name(name))
        .map(|role| palette[role as usize])
        .collect()
}

fn is_kebab(name: &str) -> bool {
    name.split('-').all(|part| {
        !part.is_empty()
            && part
                .bytes()
                .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit())
    }) && name.starts_with(|first: char| first.is_ascii_lowercase())
}

/// Every reason the pack's app colours would not stand as declared: their names and surfaces,
/// and each pair in light, dark and both high-contrast looks.
fn check_app_colors(pack: &Pack, problems: &mut Vec<Problem>) {
    let mut problem = |appearance, message: String| {
        problems.push(Problem {
            appearance,
            message,
        })
    };
    if pack.colors.len() > APP_COLOR_LIMIT {
        problem(
            "pack",
            format!(
                "{} app colours; a theme names at most {APP_COLOR_LIMIT}",
                pack.colors.len()
            ),
        );
    }
    for (name, color) in &pack.colors {
        if !is_kebab(name) {
            problem("pack", format!("colour {name} must be a kebab-case name"));
        } else if Role::from_name(name).is_some() {
            problem(
                "pack",
                format!("colour {name} is a role; name it for what it marks"),
            );
        } else if name.starts_with("on-") {
            problem(
                "pack",
                format!("colour {name}: on- names a fill's label and is made for each fill"),
            );
        }
        for ground in &color.on {
            if Role::from_name(ground).is_none() {
                problem(
                    "pack",
                    format!("colour {name} is on {ground}, which is not a role"),
                );
            }
        }
        if color.on_color.is_some() && color.usage != ColorUse::Fill {
            problem(
                "pack",
                format!("colour {name}: on-color labels a fill only"),
            );
        }
    }
    if pack.colors.is_empty() {
        return;
    }
    for (look, appearance, high_contrast) in [
        ("light", Appearance::Light, false),
        ("dark", Appearance::Dark, false),
        ("high-contrast-light", Appearance::Light, true),
        ("high-contrast-dark", Appearance::Dark, true),
    ] {
        let prefs = Prefs {
            appearance,
            high_contrast,
            ..Prefs::default()
        };
        // A pack that does not resolve is reported by the role checks.
        let Ok(theme) = resolve(pack, &prefs) else {
            continue;
        };
        for (name, color) in &pack.colors {
            let value = theme.app[name];
            let base = color.usage.ratio();
            let needed = if high_contrast {
                boosted(base)
            } else {
                f64::from(base)
            };
            if color.usage == ColorUse::Fill {
                let label = theme.app[&format!("on-{name}")];
                let actual = contrast(label, value);
                if actual + 1e-9 < needed {
                    let near = ensure_contrast(value, label, needed);
                    let nearest = if contrast(near, label) + 1e-9 >= needed {
                        format!("; {name} {} is the nearest that passes", near.hex())
                    } else {
                        String::new()
                    };
                    problem(
                        look,
                        format!(
                            "on-{name} {} on {name} {} is {actual:.2}:1, needs {needed:.1}:1{nearest}",
                            label.hex(),
                            value.hex()
                        ),
                    );
                }
                continue;
            }
            for ground_name in &color.on {
                let Some(ground) = Role::from_name(ground_name) else {
                    continue;
                };
                let ground = theme.color(ground);
                let ink = if value.0[3] < 255 {
                    over(value, ground)
                } else {
                    value
                };
                let actual = contrast(ink, ground);
                if actual + 1e-9 >= needed {
                    continue;
                }
                let near = ensure_contrast(value, ground, needed);
                let nearest = if value.0[3] == 255 && contrast(near, ground) + 1e-9 >= needed {
                    format!("; {name} {} is the nearest that passes", near.hex())
                } else {
                    String::new()
                };
                let graphic = if color.usage == ColorUse::Text {
                    "; if it carries no text, declare use: graphic (3:1)"
                } else {
                    ""
                };
                problem(
                    look,
                    format!(
                        "{name} on {ground_name} is {actual:.2}:1, needs {needed:.1}:1{nearest}{graphic}"
                    ),
                );
            }
        }
    }
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
pub(crate) fn boosted(ratio: f32) -> f64 {
    match ratio {
        r if r >= 4.5 => 7.0,
        r if r >= 3.0 => 4.5,
        _ => 3.0,
    }
}

/// A scrim, media dock or media canvas the pack left out: its dark canvas held to a near-black,
/// so a warm pack does not dim to the default's blue-black. The dark scrim is plain black.
fn near_black(pack: &Pack, colors: &Colors, role: Role, appearance: Appearance) -> Option<Rgba> {
    if role == Role::Scrim && appearance == Appearance::Dark {
        return Some(Rgba([0, 0, 0, 0xa0]));
    }
    let canvas = Role::Canvas.name();
    let tone = pack
        .appearance
        .dark
        .get(canvas)
        .or(colors.get(canvas))?
        .oklch();
    let (l, alpha) = match role {
        Role::Scrim => (0.19, 0x66),
        Role::Media => (0.165, 0xb8),
        _ => (0.155, 255),
    };
    let ink = Oklch {
        l: tone.l.min(l),
        c: tone.c.min(0.02),
        h: tone.h,
    };
    Some(ink.rgba(alpha))
}

/// Selection pairs keep their base ratios under high contrast: text must stay readable on the
/// highlight, and a highlight boosted past 3:1 off the page would leave no room for that.
fn keeps_base_ratio(foreground: Role, background: Role) -> bool {
    foreground == Role::Selection || background == Role::Selection
}

/// The contrast a pair is held to: a translucent foreground is measured laid over its background.
pub fn pair_contrast(theme: &ResolvedTheme, foreground: Role, background: Role) -> f64 {
    let ground = theme.color(background);
    let ink = theme.color(foreground);
    let ink = if ink.0[3] < 255 {
        over(ink, ground)
    } else {
        ink
    };
    contrast(ink, ground)
}

/// Each composite pair `theme` misses: the foreground on a role at an alpha laid over another.
fn composite_misses(
    theme: &ResolvedTheme,
    pairs: &[(Role, Role, u8, Role, f32)],
    high_contrast: bool,
) -> Vec<String> {
    let mut misses = Vec::new();
    for &(foreground, ground, percent, under, ratio) in pairs {
        let needed = if high_contrast {
            boosted(ratio)
        } else {
            f64::from(ratio)
        };
        let mut tint = theme.color(ground);
        tint.0[3] = (f64::from(tint.0[3]) * f64::from(percent) / 100.0).round() as u8;
        let actual = contrast(theme.color(foreground), over(tint, theme.color(under)));
        if actual + 1e-9 < needed {
            misses.push(format!(
                "{} on {}/{percent} over {} is {actual:.2}:1, needs {needed:.1}:1",
                foreground.name(),
                ground.name(),
                under.name()
            ));
        }
    }
    misses
}

/// The accent's soft tone moved in OKLCH lightness, by bisection, just far enough to stand 3:1
/// off surface and canvas, so text keeps as much contrast on it as the background allows.
fn derive_selection(palette: &[Rgba; ROLE_COUNT]) -> Rgba {
    let soft = palette[Role::AccentSoft as usize];
    let backgrounds = [
        palette[Role::Surface as usize],
        palette[Role::Canvas as usize],
    ];
    let passes = |color: Rgba| backgrounds.iter().all(|&bg| contrast(color, bg) >= 3.0);
    if passes(soft) {
        return soft;
    }
    let tone = soft.oklch();
    // Away from the backgrounds: darker on a light page, lighter on a dark one.
    let target = if palette[Role::Surface as usize].luminance() > 0.18 {
        0.0
    } else {
        1.0
    };
    let (mut near, mut far) = (tone.l, target);
    for _ in 0..40 {
        let mid = (near + far) / 2.0;
        if passes(Oklch { l: mid, ..tone }.rgba(255)) {
            far = mid;
        } else {
            near = mid;
        }
    }
    Oklch { l: far, ..tone }.rgba(255)
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
            match Role::from_name(key) {
                None => problem(name, format!("unknown role {key}")),
                Some(role) if derive::is_shade(role) => problem(
                    name,
                    format!("{key} is derived from its solid fill; a pack does not write it"),
                ),
                Some(_) => {}
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
        let need = if high_contrast { boosted(4.5) } else { 4.5 };
        let grounds = [theme.color(Role::Surface), theme.color(Role::Canvas)];
        let mut refused = Vec::new();
        for (solid, hover, pressed) in derive::SHADES {
            let label = if solid == Role::AccentSolid {
                Role::OnAccentSolid
            } else {
                Role::OnDangerSolid
            };
            let (fill, ink) = (theme.color(solid), theme.color(label));
            let Err(why) = derive::shade_states(fill, ink, &grounds, need) else {
                continue;
            };
            refused.extend([hover, pressed]);
            let lighter = match why.ground {
                Some(ground) if why.lighter >= need => {
                    format!("lighter, it stands {ground:.2}:1 off the surface behind it")
                }
                _ => format!("lighter, {:.2}:1", why.lighter),
            };
            let nearest = derive::nearest_passing(fill, ink, &grounds, need)
                .map_or(String::new(), |near| {
                    format!("; {} is the nearest that passes", near.hex())
                });
            problem(
                name,
                format!(
                    "{} {}: its pressed state does not read under {} {} either way (deeper, {:.2}:1; {lighter}; needs {need:.1}:1){nearest}",
                    solid.name(),
                    fill.hex(),
                    label.name(),
                    ink.hex(),
                    why.deeper,
                ),
            );
        }
        for &(foreground, background, ratio) in CONTRAST {
            if refused.contains(&background) {
                continue;
            }
            let needed = if high_contrast && !keeps_base_ratio(foreground, background) {
                boosted(ratio)
            } else {
                f64::from(ratio)
            };
            let (ink, ground) = (theme.color(foreground), theme.color(background));
            let actual = pair_contrast(&theme, foreground, background);
            if actual + 1e-9 < needed {
                // A derived foreground follows its ground, so the ground is the colour to move.
                let (moved, near, against) = if foreground.tier() == Tier::Derived {
                    (background, ensure_contrast(ground, ink, needed), ink)
                } else {
                    (foreground, ensure_contrast(ink, ground, needed), ground)
                };
                // A translucent colour's nearest value would also be a question of alpha.
                let nearest = if ink.0[3] == 255 && contrast(near, against) + 1e-9 >= needed {
                    format!(
                        "; {} {} is the nearest that passes",
                        moved.name(),
                        near.hex()
                    )
                } else {
                    String::new()
                };
                problem(
                    name,
                    format!(
                        "{} on {} is {actual:.2}:1, needs {needed:.1}:1{nearest}",
                        foreground.name(),
                        background.name()
                    ),
                );
            }
        }
        for miss in composite_misses(&theme, COMPOSITE_CONTRAST, high_contrast) {
            problem(name, miss);
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
    let motion = &pack.motion;
    for (key, duration) in [
        ("fast", motion.fast),
        ("base", motion.base),
        ("slow", motion.slow),
    ] {
        if duration > 10_000 {
            problem("pack", format!("motion.{key} must be 0-10000 ms"));
        }
    }
    for (key, easing) in [
        ("easing", &motion.easing),
        ("enter", &motion.enter),
        ("exit", &motion.exit),
    ] {
        if !is_easing(easing) {
            problem(
                "pack",
                format!(
                    "motion.{key} must be linear, ease, ease-in, ease-out, ease-in-out or cubic-bezier(x1, y1, x2, y2) with x in 0-1"
                ),
            );
        }
    }
    if !(0.0..=0.9).contains(&motion.spring) {
        problem("pack", "motion.spring is a bounce of 0-0.9".into());
    }
    check_app_colors(pack, &mut problems);
    problems
}

/// Under this distance two colours read as one.
pub(crate) const ALIKE: f64 = 5.0;

/// Colours that pass every pair yet read alike or poorly: a status colour beside a hue or the
/// accent, the accent as link text on sunken and tinted grounds, a solid fill lost on the surface.
/// Advice for the pack's author, not a failure.
pub fn advise(pack: &Pack) -> Vec<Problem> {
    let mut advice = Vec::new();
    for (name, appearance) in [("light", Appearance::Light), ("dark", Appearance::Dark)] {
        let prefs = Prefs {
            appearance,
            ..Prefs::default()
        };
        let Ok(theme) = resolve(pack, &prefs) else {
            continue;
        };
        for status in [Role::Danger, Role::Success, Role::Warning] {
            let others = Role::ALL.into_iter().filter(|role| {
                *role == Role::Accent
                    || (role.name().starts_with("hue-") && !role.name().ends_with("-soft"))
            });
            for other in others {
                let apart = distance(theme.color(status), theme.color(other));
                if apart < ALIKE {
                    advice.push(Problem {
                        appearance: name,
                        message: format!(
                            "{} is {apart:.1} from {}; under {ALIKE:.0} they read as one colour",
                            status.name(),
                            other.name()
                        ),
                    });
                }
            }
        }
        let mut weak = |foreground: Role, background: Role, needed: f64, use_: &str| {
            let ratio = contrast(theme.color(foreground), theme.color(background));
            if ratio + 1e-9 < needed {
                advice.push(Problem {
                    appearance: name,
                    message: format!(
                        "{} is {ratio:.2}:1 on {}; {use_} needs {needed:.1}:1",
                        foreground.name(),
                        background.name()
                    ),
                });
            }
        };
        // Canvas and surface are proven by the accent's contract.
        for ground in [Role::SurfaceSunken, Role::AccentSoft] {
            weak(Role::Accent, ground, 4.5, "as link text it");
        }
        for solid in [Role::AccentSolid, Role::DangerSolid] {
            weak(solid, Role::Surface, 3.0, "a filled control's edge");
        }
        for (color_name, color) in &pack.colors {
            if color.usage != ColorUse::Fill {
                continue;
            }
            let fill = theme.app[color_name];
            for ground_name in &color.on {
                let Some(ground) = Role::from_name(ground_name) else {
                    continue;
                };
                let ratio = contrast(fill, theme.color(ground));
                if ratio + 1e-9 < 3.0 {
                    advice.push(Problem {
                        appearance: name,
                        message: format!(
                            "{color_name} is {ratio:.2}:1 on {ground_name}; a filled control's edge needs 3.0:1"
                        ),
                    });
                }
            }
        }
    }
    advice
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
