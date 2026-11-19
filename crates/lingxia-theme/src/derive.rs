//! Derived roles: computed from a pack's authored roles, so a seed and a few surfaces give a
//! whole theme. A role the pack wrote is never replaced.

use crate::ALIKE;
use crate::color::{Oklch, Rgba, contrast, distance, ensure_contrast};
use crate::generated::{ROLE_COUNT, Role};

const WHITE: Rgba = Rgba([255, 255, 255, 255]);

/// A categorical colour, light then dark: hue angle, chroma, and contrast with the surface.
/// Only lightness is solved per pack, so the set keeps its character on any surface.
#[derive(Clone, Copy)]
struct Swatch {
    angle: [f64; 2],
    chroma: [f64; 2],
    ratio: [f64; 2],
}

const fn swatch(angle: [f64; 2], chroma: [f64; 2], ratio: [f64; 2]) -> Swatch {
    Swatch {
        angle,
        chroma,
        ratio,
    }
}

/// `hue-2` to `hue-7`: teal, brown, olive, pink, purple, cerulean.
const HUES: [Swatch; 6] = [
    swatch([189.0, 185.0], [0.088, 0.103], [5.25, 12.25]),
    swatch([57.0, 60.0], [0.094, 0.1], [7.65, 5.3]),
    swatch([110.0, 115.0], [0.114, 0.126], [4.85, 10.5]),
    swatch([357.0, 3.0], [0.184, 0.132], [8.25, 8.15]),
    swatch([308.0, 310.0], [0.189, 0.189], [6.4, 5.15]),
    swatch([236.0, 221.0], [0.094, 0.128], [7.35, 7.4]),
];
/// `hue-1` at the accent's angle and `hue-8` at the muted text's.
const HUE_ACCENT: Swatch = swatch([0.0; 2], [0.175, 0.086], [3.7, 9.9]);
const HUE_NEUTRAL: Swatch = swatch([0.0; 2], [0.031, 0.035], [3.7, 5.4]);
/// `chart-2` to `chart-8`: orange, green, magenta, cyan, then the brown and pink of the hues and
/// their olive, a little yellower in light to stand off the green.
const SERIES: [Swatch; 7] = [
    swatch([38.0, 47.0], [0.19, 0.161], [4.3, 7.2]),
    swatch([147.0, 159.0], [0.143, 0.136], [4.35, 7.6]),
    swatch([319.0, 317.0], [0.203, 0.158], [5.8, 7.1]),
    swatch([215.0, 208.0], [0.087, 0.117], [5.6, 8.3]),
    HUES[1],
    HUES[3],
    swatch([100.0, 115.0], HUES[2].chroma, HUES[2].ratio),
];
/// Where a crowded hue goes: blue, green, amber or vermilion. Blue is the default accent's
/// place, which another accent leaves empty.
const HUE_SPARES: [Swatch; 4] = [
    swatch([267.0; 2], HUE_ACCENT.chroma, HUE_ACCENT.ratio),
    swatch([150.0; 2], [0.13, 0.13], [7.5, 6.0]),
    swatch([80.0, 88.0], [0.14, 0.16], [3.7, 7.6]),
    swatch([38.0, 30.0], [0.17, 0.17], [4.3, 6.2]),
];
/// Where a crowded series goes: blue, teal, violet, gold, lime, sky, red or rose. Never a grey: a series
/// must read as a colour beside the neutral hue and the page's text, so an accent that crowds two
/// series may leave the second a little nearer its neighbours than the rest.
const SERIES_SPARES: [Swatch; 8] = [
    swatch([267.0; 2], [0.202, 0.156], [5.6, 6.2]),
    swatch([180.0; 2], [0.11, 0.115], [4.6, 9.0]),
    swatch([290.0; 2], [0.19, 0.17], [9.5, 3.5]),
    swatch([85.0; 2], [0.13, 0.14], [3.4, 9.5]),
    swatch([125.0; 2], [0.15, 0.15], [3.6, 9.0]),
    swatch([240.0; 2], [0.13, 0.13], [4.0, 5.0]),
    swatch([25.0; 2], [0.19, 0.17], [6.5, 4.0]),
    swatch([340.0; 2], [0.17, 0.15], [4.0, 4.5]),
];
/// Hues, then series, with a smaller [`gap`] than this crowd each other. Series are read side
/// by side in one chart, so they keep further apart.
const CROWDED: [f64; 2] = [10.0, 12.0];
/// What a step in the chroma plane counts for in a [`gap`]: half of `CROWDED` there is crowded
/// at any lightness.
const PLANE: f64 = 2.0;
/// Angles closer than this read as one colour.
const NEAR: f64 = 20.0;
const INFO_HUE: f64 = 250.0;
/// Below this chroma a colour has no hue to speak of.
const GREY: f64 = 0.02;

/// Chroma of a soft fill, light then dark.
const HUE_TINT: [f64; 2] = [0.024, 0.045];
const NEUTRAL_TINT: [f64; 2] = [0.013, 0.019];
const STATUS_TINT: [f64; 2] = [0.02, 0.047];
/// A soft fill's share of its seed's chroma, light then dark.
const SEED_TINT: [f64; 2] = [0.25, 0.35];
/// How far a soft fill sits from the surface in lightness, light then dark.
const SOFT_STEP: [f64; 2] = [-0.045, 0.07];
const STATUS_STEP: [f64; 2] = [-0.045, 0.05];
/// Yellow is the lightest hue, so the warning fill sits lighter than the other statuses'.
const WARNING_STEP: [f64; 2] = [-0.037, 0.067];
/// The warning fill is held in this yellow band: outside it the pale tint of an amber or ochre
/// seed reads salmon or olive.
const WARNING_BAND: [f64; 2] = [75.0, 95.0];
/// Its share of the chroma sRGB shows at its lightness and angle, light then dark. The gamut
/// widens fast across the band, and a pale yellow at a set chroma turns beige where it is wide.
const WARNING_SHARE: [f64; 2] = [0.83, 0.7];
/// Ink must beat white by this much to be the text on a fill: on a mid tone white reads as
/// the label of a coloured control, and the fill darkens a little to carry it.
const WHITE_BIAS: f64 = 1.25;
/// How far overlays lift off `surface-raised` in dark, where a shadow alone does not separate them.
const OVERLAY_LIFT: f64 = 0.045;
/// White on a dark-appearance solid fill: past the 4.5 it needs, so rounding never takes it under.
const SOLID_WHITE: f64 = 4.8;

const HUE_ROLES: [(Role, Role); 8] = [
    (Role::Hue1, Role::Hue1Soft),
    (Role::Hue2, Role::Hue2Soft),
    (Role::Hue3, Role::Hue3Soft),
    (Role::Hue4, Role::Hue4Soft),
    (Role::Hue5, Role::Hue5Soft),
    (Role::Hue6, Role::Hue6Soft),
    (Role::Hue7, Role::Hue7Soft),
    (Role::Hue8, Role::Hue8Soft),
];
const SERIES_ROLES: [Role; 7] = [
    Role::Chart2,
    Role::Chart3,
    Role::Chart4,
    Role::Chart5,
    Role::Chart6,
    Role::Chart7,
    Role::Chart8,
];
const STATUSES: [[Role; 4]; 3] = [
    [
        Role::Danger,
        Role::OnDanger,
        Role::DangerSoft,
        Role::DangerMark,
    ],
    [
        Role::Success,
        Role::OnSuccess,
        Role::SuccessSoft,
        Role::SuccessMark,
    ],
    [
        Role::Warning,
        Role::OnWarning,
        Role::WarningSoft,
        Role::WarningMark,
    ],
];

/// Degrees between two hue angles, 0–180.
pub(crate) fn apart(a: f64, b: f64) -> f64 {
    ((a - b + 540.0).rem_euclid(360.0) - 180.0).abs()
}

/// The hue angle of a colour that has one.
fn hue_of(color: Rgba) -> Option<f64> {
    let tone = color.oklch();
    (tone.c >= GREY).then_some(tone.h)
}

/// `h` if it lies between the two angles, else the nearer of them.
fn held(h: f64, [low, high]: [f64; 2]) -> f64 {
    if (low..=high).contains(&h) {
        h
    } else if apart(h, low) <= apart(h, high) {
        low
    } else {
        high
    }
}

/// How strong a soft fill is.
#[derive(Clone, Copy)]
enum Tint {
    Chroma(f64),
    /// A share of the most chroma sRGB shows at the fill's lightness and angle.
    Share(f64),
}

/// What a pack's two appearances lend to the placing of its categorical colours, light then
/// dark, so both move the same ones.
pub(crate) struct Sides {
    pub surface: [Rgba; 2],
    pub canvas: [Rgba; 2],
    pub muted: [Rgba; 2],
    /// Danger, success and warning.
    pub statuses: [[Rgba; 2]; 3],
}

/// A swatch on each appearance's surface.
fn tones(swatch: &Swatch, surface: [Rgba; 2]) -> [Rgba; 2] {
    [0, 1].map(|side| {
        at_contrast(
            swatch.angle[side],
            swatch.chroma[side],
            swatch.ratio[side].max(3.0),
            surface[side],
            side == 1,
        )
    })
}

/// [`distance`] with lightness left out: how far apart two colours are in hue and chroma. Two
/// greys are together here and a grey is apart from any strong colour, which an angle cannot say.
pub(crate) fn plane(a: Rgba, b: Rgba) -> f64 {
    let (x, y) = (a.oklch(), b.oklch());
    let turn = libm::cos((x.h - y.h).to_radians());
    let square = x.c * x.c + y.c * y.c - 2.0 * x.c * y.c * turn;
    100.0 * square.max(0.0).sqrt()
}

/// How far apart two colours of one family read: their [`distance`], but no more than `PLANE`
/// times their distance in the chroma plane. Lightness is not a category cue: a teal and a
/// deep teal are one colour in a legend.
fn gap(a: Rgba, b: Rgba) -> f64 {
    distance(a, b).min(PLANE * plane(a, b))
}

/// How close the nearest of `others` comes to `color` in either appearance.
fn clearance(color: [Rgba; 2], others: &[[Rgba; 2]]) -> f64 {
    let gaps = others
        .iter()
        .flat_map(|other| [0, 1].map(|side| gap(color[side], other[side])));
    gaps.fold(f64::INFINITY, f64::min)
}

/// The swatch each colour of a family uses in a pack. The family's own swatches stand clear of
/// each other; what a pack brings is `fixed`: the accent's colour, the neutral. A swatch one of
/// those crowds in either appearance gives way, in both, to the first spare that stands clear of
/// the rest of the family, or to the one that stands farthest when none does. Clear is a
/// [`gap`], so neither a crowded swatch nor a spare is saved by lightness alone. A swatch that
/// reads as one of `statuses` in either appearance gives way too, after the crowded ones have
/// chosen, to the first clear spare that reads as none of them, and stays when there is none: a
/// category should not look like danger, success or warning, but must not crowd its family.
fn spread(
    family: &[Swatch],
    spares: &[Swatch],
    fixed: &[[Rgba; 2]],
    statuses: &[[Rgba; 2]],
    surface: [Rgba; 2],
    crowded: f64,
) -> Vec<Swatch> {
    let alike = |color: [Rgba; 2]| {
        statuses.iter().any(|status| {
            [0, 1]
                .iter()
                .any(|&side| distance(color[side], status[side]) < ALIKE)
        })
    };
    let mut chosen = family.to_vec();
    let mut colors = fixed.to_vec();
    colors.extend(family.iter().map(|swatch| tones(swatch, surface)));
    for for_status in [false, true] {
        for (index, choice) in chosen.iter_mut().enumerate() {
            let slot = fixed.len() + index;
            let gives_way = if for_status {
                alike(colors[slot])
            } else {
                clearance(colors[slot], fixed) < crowded
            };
            if !gives_way {
                continue;
            }
            let home = colors.remove(slot);
            // Leaving a status must not crowd the family: only a clear spare is worth the move.
            let stay = if for_status {
                crowded
            } else {
                clearance(home, &colors)
            };
            let (mut best, mut placed) = (stay, home);
            for spare in spares {
                let color = tones(spare, surface);
                if alike(color) {
                    continue;
                }
                let clear = clearance(color, &colors);
                if clear > best || (for_status && clear >= crowded) {
                    (best, placed, *choice) = (clear, color, *spare);
                }
                if clear >= crowded {
                    break;
                }
            }
            colors.insert(slot, placed);
        }
    }
    chosen
}

/// The accent a seed gives before its text is fitted: its own tone, moved to stand `off` the page.
fn seeded(seed: Rgba, surface: Rgba, canvas: Rgba, off: f64, dark: bool) -> Rgba {
    let tone = seed.oklch();
    let l = if dark {
        tone.l.max(0.7)
    } else {
        tone.l.min(0.6)
    };
    let start = Oklch { l, ..tone }.rgba(255);
    ensure_contrast(ensure_contrast(start, surface, off), canvas, off)
}

/// Where overlays float: `raised`, a step lighter in dark.
pub(crate) fn overlay(raised: Rgba, dark: bool) -> Rgba {
    if !dark {
        return raised;
    }
    let tone = raised.oklch();
    Oklch {
        l: tone.l + OVERLAY_LIFT,
        ..tone
    }
    .rgba(255)
}

/// The lightest tone of `source`'s hue and chroma that white stands `ratio` off.
fn carrying_white(source: Rgba, ratio: f64) -> Rgba {
    let Oklch { c, h, .. } = source.oklch();
    let (mut low, mut high) = (0.0, 1.0);
    for _ in 0..40 {
        let mid = (low + high) / 2.0;
        if contrast(WHITE, Oklch { l: mid, c, h }.rgba(255)) >= ratio {
            low = mid;
        } else {
            high = mid;
        }
    }
    Oklch { l: low, c, h }.rgba(255)
}

/// The tone of this hue and chroma that stands `ratio` off `surface`, by bisection in lightness.
fn at_contrast(h: f64, c: f64, ratio: f64, surface: Rgba, dark: bool) -> Rgba {
    let edge = surface.oklch().l;
    let (mut low, mut high) = if dark { (edge, 1.0) } else { (0.0, edge) };
    for _ in 0..40 {
        let mid = (low + high) / 2.0;
        let passes = contrast(Oklch { l: mid, c, h }.rgba(255), surface) >= ratio;
        if dark == passes {
            high = mid;
        } else {
            low = mid;
        }
    }
    Oklch {
        l: if dark { high } else { low },
        c,
        h,
    }
    .rgba(255)
}

/// One palette being filled in. `written` marks what the pack set; `strong` builds against the
/// high-contrast thresholds; `movable` lets a status colour move to carry its text.
pub(crate) struct Build<'a> {
    pub palette: &'a mut [Rgba; ROLE_COUNT],
    pub written: [bool; ROLE_COUNT],
    pub dark: bool,
    pub strong: bool,
    pub movable: bool,
}

impl Build<'_> {
    fn get(&self, role: Role) -> Rgba {
        self.palette[role as usize]
    }

    fn has(&self, role: Role) -> bool {
        self.written[role as usize]
    }

    fn put(&mut self, role: Role, color: Rgba) {
        if !self.has(role) {
            self.palette[role as usize] = color;
        }
    }

    fn need(&self, ratio: f32) -> f64 {
        if self.strong {
            crate::boosted(ratio)
        } else {
            f64::from(ratio)
        }
    }

    fn pick(&self, values: [f64; 2]) -> f64 {
        values[usize::from(self.dark)]
    }

    /// Every derived role the pack left out.
    pub(crate) fn run(&mut self, seed: Rgba, sides: &Sides) {
        self.accent(seed);
        self.overlay();
        let tint = Tint::Chroma(self.pick(STATUS_TINT));
        for [status, on, soft, mark] in STATUSES {
            self.family(status, on, soft, mark, tint, self.movable);
        }
        self.solid(
            [Role::Accent, Role::OnAccent],
            [Role::AccentSolid, Role::OnAccentSolid],
            seed,
        );
        let danger = self.get(Role::Danger);
        self.solid(
            [Role::Danger, Role::OnDanger],
            [Role::DangerSolid, Role::OnDangerSolid],
            danger,
        );
        self.info(seed);
        self.categorical(seed, sides);
    }

    fn accent(&mut self, seed: Rgba) {
        let tone = seed.oklch();
        let (surface, canvas) = (self.get(Role::Surface), self.get(Role::Canvas));
        let off = self.need(3.0);
        if self.has(Role::Accent) {
            let (_, on) = self.pair(self.get(Role::Accent), Role::OnAccent, &[], false);
            self.put(Role::OnAccent, on);
        } else {
            let start = seeded(seed, surface, canvas, off, self.dark);
            let (accent, on) = self.pair(start, Role::OnAccent, &[surface, canvas], true);
            self.put(Role::Accent, accent);
            self.put(Role::OnAccent, on);
        }
        self.put(
            Role::AccentSoft,
            self.soft(
                tone.h,
                Tint::Chroma(tone.c * self.pick(SEED_TINT)),
                SOFT_STEP,
            ),
        );
        let on_soft = Oklch {
            l: if self.dark { 0.88 } else { 0.4 },
            c: tone.c * 0.9,
            h: tone.h,
        }
        .rgba(255);
        self.put(
            Role::OnAccentSoft,
            ensure_contrast(on_soft, self.get(Role::AccentSoft), self.need(4.5)),
        );
        self.put(Role::Focus, self.get(Role::Accent));
    }

    /// `fill` and the text that reads on it: white or a near-black ink of the fill's hue,
    /// whichever stands further off. A fill that may move does so until the text reads and it
    /// still stands off `grounds`; one that may not leaves the text to move.
    fn pair(&self, fill: Rgba, on: Role, grounds: &[Rgba], movable: bool) -> (Rgba, Rgba) {
        let need = self.need(4.5);
        if self.has(on) {
            let text = self.get(on);
            let fill = if movable {
                ensure_contrast(fill, text, need)
            } else {
                fill
            };
            return (fill, text);
        }
        let tone = fill.oklch();
        let ink = Oklch {
            l: 0.16,
            c: tone.c.min(0.03),
            h: tone.h,
        }
        .rgba(255);
        let mut texts = [WHITE, ink];
        if contrast(ink, fill) > contrast(WHITE, fill) * WHITE_BIAS {
            texts.swap(0, 1);
        }
        if !movable {
            let moved = texts.map(|text| ensure_contrast(text, fill, need));
            let reads = moved.into_iter().find(|&text| contrast(text, fill) >= need);
            return (fill, reads.unwrap_or(moved[0]));
        }
        let off = self.need(3.0);
        let tries = texts.map(|text| (ensure_contrast(fill, text, need), text));
        tries
            .into_iter()
            .find(|&(moved, text)| {
                contrast(text, moved) >= need
                    && grounds.iter().all(|&ground| contrast(moved, ground) >= off)
            })
            .unwrap_or(tries[0])
    }

    /// The fill of a control that carries white marks and its text. In light the plain role is
    /// already that; in dark, where the plain role is a pastel for text and rings, a fill at
    /// `source`'s hue and chroma, as light as white on it allows.
    fn solid(&mut self, [plain, on_plain]: [Role; 2], [solid, on_solid]: [Role; 2], source: Rgba) {
        if !self.dark && !self.has(solid) && !self.has(on_solid) {
            self.put(solid, self.get(plain));
            self.put(on_solid, self.get(on_plain));
            return;
        }
        let fill = if self.has(solid) {
            self.get(solid)
        } else if self.dark {
            carrying_white(source, self.need(4.5).max(SOLID_WHITE))
        } else {
            self.get(plain)
        };
        let (fill, text) = self.pair(fill, on_solid, &[], false);
        self.put(solid, fill);
        self.put(on_solid, text);
    }

    /// A tint `step` off the surface in lightness, moved back toward it until text reads on it.
    fn soft(&self, h: f64, tint: Tint, step: [f64; 2]) -> Rgba {
        let (text, need) = (self.get(Role::Text), self.need(4.5));
        let at = |l: f64| {
            let c = match tint {
                Tint::Chroma(c) => c,
                Tint::Share(share) => share * Oklch { l, c: 0.4, h }.rgba(255).oklch().c,
            };
            Oklch { l, c, h }.rgba(255)
        };
        let mut l = self.get(Role::Surface).oklch().l + self.pick(step);
        let mut color = at(l);
        for _ in 0..100 {
            if contrast(text, color) >= need {
                break;
            }
            l -= self.pick(step).signum() * 0.005;
            color = at(l);
        }
        color
    }

    /// A status colour as a dot or shape. In light a fuller, lighter tone, so it reads as its
    /// colour and not as brown; in dark the status tone is already that.
    fn mark(&self, status: Rgba) -> Rgba {
        let (surface, off) = (self.get(Role::Surface), self.need(3.0));
        let tone = status.oklch();
        let start = if self.dark {
            ensure_contrast(status, surface, off)
        } else {
            let chroma = (tone.c * 1.05).max(0.145);
            at_contrast(tone.h, chroma, off.max(3.7), surface, false)
        };
        ensure_contrast(start, self.get(Role::SurfaceSunken), off)
    }

    /// The text, soft fill and mark of a status colour. The warning fill is a yellow whatever
    /// the seed.
    fn family(
        &mut self,
        status: Role,
        on: Role,
        soft: Role,
        mark: Role,
        tint: Tint,
        movable: bool,
    ) {
        let (fill, text) = self.pair(self.get(status), on, &[self.get(Role::Surface)], movable);
        self.palette[status as usize] = fill;
        self.put(on, text);
        let h = fill.oklch().h;
        let tinted = if status == Role::Warning {
            self.soft(
                held(h, WARNING_BAND),
                Tint::Share(self.pick(WARNING_SHARE)),
                WARNING_STEP,
            )
        } else {
            self.soft(h, tint, STATUS_STEP)
        };
        self.put(soft, tinted);
        self.put(mark, self.mark(fill));
    }

    /// A fixed blue at the accent's lightness and chroma; an accent that is itself that blue
    /// lends its family, since two blues would not read as two meanings.
    fn info(&mut self, seed: Rgba) {
        let (surface, off) = (self.get(Role::Surface), self.need(3.0));
        if !self.has(Role::Info) {
            if hue_of(seed).is_some_and(|hue| apart(hue, INFO_HUE) < NEAR) {
                let accent = self.get(Role::Accent);
                self.put(Role::Info, accent);
                self.put(Role::OnInfo, self.get(Role::OnAccent));
                self.put(Role::InfoSoft, self.get(Role::AccentSoft));
                let mark = ensure_contrast(accent, surface, off);
                self.put(
                    Role::InfoMark,
                    ensure_contrast(mark, self.get(Role::SurfaceSunken), off),
                );
                return;
            }
            let accent = self.get(Role::Accent).oklch();
            let blue = Oklch {
                // A grey accent would leave a grey notice.
                c: accent.c.max(0.1),
                h: INFO_HUE,
                ..accent
            }
            .rgba(255);
            self.palette[Role::Info as usize] = ensure_contrast(blue, surface, off);
        }
        let tint = Tint::Chroma(self.get(Role::Info).oklch().c * self.pick(SEED_TINT));
        let movable = self.movable || !self.has(Role::Info);
        self.family(
            Role::Info,
            Role::OnInfo,
            Role::InfoSoft,
            Role::InfoMark,
            tint,
            movable,
        );
    }

    /// The overlay moves away from the text read on it until that text reads.
    fn overlay(&mut self) {
        let mut overlay = self.get(Role::SurfaceOverlay);
        for (text, ratio) in [
            (Role::Text, 4.5),
            (Role::TextMuted, 4.5),
            (Role::TextFaint, 3.0),
        ] {
            let (ink, need) = (self.get(text), self.need(ratio));
            let mut tone = overlay.oklch();
            let step = if overlay.luminance() >= ink.luminance() {
                0.005
            } else {
                -0.005
            };
            while contrast(ink, overlay) < need && tone.l > 0.0 && tone.l < 1.0 {
                tone.l = (tone.l + step).clamp(0.0, 1.0);
                overlay = tone.rgba(255);
            }
        }
        self.put(Role::SurfaceOverlay, overlay);
    }

    fn categorical(&mut self, seed: Rgba, sides: &Sides) {
        let (surface, off) = (self.get(Role::Surface), self.need(3.0));
        let accents = [0, 1].map(|side| {
            seeded(
                seed,
                sides.surface[side],
                sides.canvas[side],
                3.0,
                side == 1,
            )
        });
        let seed = seed.oklch();
        // The first hue is no stronger than the accent it follows.
        let first = swatch(
            [seed.h; 2],
            HUE_ACCENT.chroma.map(|chroma| chroma.min(seed.c)),
            HUE_ACCENT.ratio,
        );
        let neutral = swatch(
            sides.muted.map(|muted| muted.oklch().h),
            HUE_NEUTRAL.chroma,
            HUE_NEUTRAL.ratio,
        );
        let fixed = [&first, &neutral].map(|swatch| tones(swatch, sides.surface));
        let hues = spread(
            &HUES,
            &HUE_SPARES,
            &fixed,
            &sides.statuses,
            sides.surface,
            CROWDED[0],
        );
        // Series do not give way to a status: a chart is its own context.
        let series = spread(
            &SERIES,
            &SERIES_SPARES,
            &[accents],
            &[],
            sides.surface,
            CROWDED[1],
        );
        let muted = self.get(Role::TextMuted).oklch().h;
        for (index, (hue, soft)) in HUE_ROLES.into_iter().enumerate() {
            let (angle, swatch, tint) = match index {
                0 => (seed.h, &first, HUE_TINT),
                7 => (muted, &HUE_NEUTRAL, NEUTRAL_TINT),
                _ => (self.pick(hues[index - 1].angle), &hues[index - 1], HUE_TINT),
            };
            let chroma = self.pick(swatch.chroma);
            // A hue the pack wrote tints its own fill.
            let tinted = if self.has(hue) {
                self.get(hue).oklch().h
            } else {
                angle
            };
            self.put(
                soft,
                self.soft(tinted, Tint::Chroma(self.pick(tint).min(chroma)), SOFT_STEP),
            );
            let ratio = self.pick(swatch.ratio).max(off);
            let color = at_contrast(angle, chroma, ratio, surface, self.dark);
            let color = ensure_contrast(color, self.get(Role::SurfaceSunken), off);
            self.put(hue, ensure_contrast(color, self.get(soft), off));
        }
        self.put(Role::Chart1, self.get(Role::Accent));
        for (index, role) in SERIES_ROLES.into_iter().enumerate() {
            let swatch = series[index];
            let ratio = self.pick(swatch.ratio).max(off);
            let (angle, chroma) = (self.pick(swatch.angle), self.pick(swatch.chroma));
            let color = at_contrast(angle, chroma, ratio, surface, self.dark);
            self.put(role, ensure_contrast(color, self.get(Role::Canvas), off));
        }
    }
}
