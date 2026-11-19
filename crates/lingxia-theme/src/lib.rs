//! Theme packs: colour roles per appearance plus numeric scales, resolved for user
//! preferences into one palette every renderer and host chrome paints from.

mod color;
mod derive;
mod generated;

use std::collections::{BTreeMap, HashMap};
use std::sync::{LazyLock, Mutex};

use serde::{Deserialize, Serialize};

pub use color::{Oklch, Rgba, contrast, distance, ensure_contrast};
pub use generated::{CONTRAST, ROLE_COUNT, Role, Tier};

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
pub fn complete(mut pack: Pack) -> Pack {
    let base = default_pack();
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
            if Role::from_name(role)
                .is_some_and(|role| role.tier() != Tier::Derived && !NEAR_BLACKS.contains(&role))
            {
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

/// The colour every derived role starts from. With `derive`, the accent family (accent,
/// on-accent, accent-soft, on-accent-soft, focus) always comes from the seed, and a user accent
/// preference replaces the seed; without it, accent roles the pack wrote are used as written.
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
    let mut palette = [Rgba([0, 0, 0, 255]); ROLE_COUNT];
    let mut written = [false; ROLE_COUNT];
    for role in Role::ALL {
        if seeded.is_some() && ACCENT_FAMILY.contains(&role) {
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
                if involves_selection(foreground, background) || !placed[fg] || !placed[bg] {
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
fn involves_selection(foreground: Role, background: Role) -> bool {
    foreground == Role::Selection || background == Role::Selection
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
        for &(foreground, background, ratio) in CONTRAST {
            let needed = if high_contrast && !involves_selection(foreground, background) {
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
