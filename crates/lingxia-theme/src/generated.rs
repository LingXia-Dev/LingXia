// Generated from contracts/view/schema.json; do not edit.
//! Roles are palette slots in order; append-only.

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[repr(u8)]
pub enum Role {
    /// Window and page floor.
    Canvas = 0,
    /// Cards, cells and sheets.
    Surface = 1,
    /// Raised cards and panels that stay in the page.
    SurfaceRaised = 2,
    /// Inset fields and grouped backgrounds.
    SurfaceSunken = 3,
    /// Decorative separators and card borders; interactive boundaries use line-strong.
    Line = 4,
    /// Body text.
    Text = 5,
    /// Secondary text.
    TextMuted = 6,
    /// Hints and disabled text.
    TextFaint = 7,
    /// Primary actions, selection, links and text buttons: reads as text on canvas and surface.
    Accent = 8,
    /// Text and icons on accent.
    OnAccent = 9,
    /// Tinted containers and selected rows.
    AccentSoft = 10,
    /// Text on accent-soft.
    OnAccentSoft = 11,
    /// Destructive actions and errors.
    Danger = 12,
    /// Text and icons on danger.
    OnDanger = 13,
    /// Error containers; text on it uses text.
    DangerSoft = 14,
    /// Success state.
    Success = 15,
    /// Text and icons on success.
    OnSuccess = 16,
    /// Success containers; text on it uses text.
    SuccessSoft = 17,
    /// Warning state.
    Warning = 18,
    /// Text and icons on warning.
    OnWarning = 19,
    /// Warning containers; text on it uses text.
    WarningSoft = 20,
    /// Focus ring.
    Focus = 21,
    /// Modal backdrop, translucent; a pack without it takes a near-black of its own neutral.
    Scrim = 22,
    /// First categorical series.
    Chart1 = 23,
    /// Second categorical series.
    Chart2 = 24,
    /// Third categorical series.
    Chart3 = 25,
    /// Fourth categorical series.
    Chart4 = 26,
    /// Fifth categorical series.
    Chart5 = 27,
    /// Interactive boundaries: editor borders and control outlines.
    LineStrong = 28,
    /// Selected-text background, from the accent's soft tone.
    Selection = 29,
    /// Elevation shadow ink; `shadow-*` layers take it at their alpha.
    Shadow = 30,
    /// Translucent dock over video and images; the same in both appearances; a pack without it takes a near-black of its own neutral.
    Media = 31,
    /// Text and icons over media; tracks and buffered ranges take it at an alpha (`on-media/25`).
    OnMedia = 32,
    /// Anything floating over the page: menus, popovers, toasts, dialogs and sheets; surface-raised, a step lighter in dark.
    SurfaceOverlay = 33,
    /// The thumb of a switch or slider, drawn on an accent track; white in both appearances by default.
    Knob = 34,
    /// Success as a dot or shape, never text.
    SuccessMark = 35,
    /// Warning as a dot or shape, never text.
    WarningMark = 36,
    /// Danger as a dot or shape, never text.
    DangerMark = 37,
    /// The whole screen behind a full-screen picture or video: opaque, near-black in both appearances; a pack without it takes a near-black of its own neutral.
    MediaCanvas = 38,
    /// Information that is neither good nor bad: notices and hints; a fixed blue at the accent's strength, or the accent family when the accent is that blue.
    Info = 39,
    /// Text and icons on info.
    OnInfo = 40,
    /// Information containers; text on it uses text.
    InfoSoft = 41,
    /// Information as a dot or shape, never text.
    InfoMark = 42,
    /// First categorical hue, for tags, avatars and series past the chart colours: a dot or shape, never text.
    Hue1 = 43,
    /// Second categorical hue, for tags, avatars and series past the chart colours: a dot or shape, never text.
    Hue2 = 44,
    /// Third categorical hue, for tags, avatars and series past the chart colours: a dot or shape, never text.
    Hue3 = 45,
    /// Fourth categorical hue, for tags, avatars and series past the chart colours: a dot or shape, never text.
    Hue4 = 46,
    /// Fifth categorical hue, for tags, avatars and series past the chart colours: a dot or shape, never text.
    Hue5 = 47,
    /// Sixth categorical hue, for tags, avatars and series past the chart colours: a dot or shape, never text.
    Hue6 = 48,
    /// Seventh categorical hue, for tags, avatars and series past the chart colours: a dot or shape, never text.
    Hue7 = 49,
    /// Eighth categorical hue, for tags, avatars and series past the chart colours: a dot or shape, never text.
    Hue8 = 50,
    /// Fill behind the first hue; text on it uses text.
    Hue1Soft = 51,
    /// Fill behind the second hue; text on it uses text.
    Hue2Soft = 52,
    /// Fill behind the third hue; text on it uses text.
    Hue3Soft = 53,
    /// Fill behind the fourth hue; text on it uses text.
    Hue4Soft = 54,
    /// Fill behind the fifth hue; text on it uses text.
    Hue5Soft = 55,
    /// Fill behind the sixth hue; text on it uses text.
    Hue6Soft = 56,
    /// Fill behind the seventh hue; text on it uses text.
    Hue7Soft = 57,
    /// Fill behind the eighth hue; text on it uses text.
    Hue8Soft = 58,
    /// Sixth categorical series.
    Chart6 = 59,
    /// Seventh categorical series.
    Chart7 = 60,
    /// Eighth categorical series.
    Chart8 = 61,
    /// Fill of a control that is pressed or switched: primary buttons, checked boxes and radios, switch tracks, a selected day, steppers, floating buttons; the accent in light, a deeper accent in dark, under white marks. A light seed, such as a yellow, stays the fill in both, under dark marks. Accent text, icons, the focus ring and filled labels such as badges keep accent.
    AccentSolid = 62,
    /// Text and icons on accent-solid.
    OnAccentSolid = 63,
    /// Fill of a destructive control that is pressed and carries white marks; danger in light, a deeper danger in dark. A danger badge or label keeps danger.
    DangerSolid = 64,
    /// Text and icons on danger-solid.
    OnDangerSolid = 65,
    /// accent-solid under a hovering pointer: a visible step deeper, or lighter where deeper would make its label hard to read; hue kept. Always derived; a pack does not write it.
    AccentSolidHover = 66,
    /// accent-solid while pressed: the same step again past hover, in the same direction. Always derived; a pack does not write it.
    AccentSolidPressed = 67,
    /// danger-solid under a hovering pointer: a visible step deeper, or lighter where deeper would make its label hard to read; hue kept. Always derived; a pack does not write it.
    DangerSolidHover = 68,
    /// danger-solid while pressed: the same step again past hover, in the same direction. Always derived; a pack does not write it.
    DangerSolidPressed = 69,
    /// Translucent fill of the theme's own neutral behind initials and placeholders, dark in light and light in dark, raised in alpha only as far as it shows as an edge on every floor, 3:1 under high contrast; text on it uses text.
    NeutralSoft = 70,
}

pub const ROLE_COUNT: usize = 71;

impl Role {
    pub const ALL: [Role; ROLE_COUNT] = [
        Role::Canvas,
        Role::Surface,
        Role::SurfaceRaised,
        Role::SurfaceSunken,
        Role::Line,
        Role::Text,
        Role::TextMuted,
        Role::TextFaint,
        Role::Accent,
        Role::OnAccent,
        Role::AccentSoft,
        Role::OnAccentSoft,
        Role::Danger,
        Role::OnDanger,
        Role::DangerSoft,
        Role::Success,
        Role::OnSuccess,
        Role::SuccessSoft,
        Role::Warning,
        Role::OnWarning,
        Role::WarningSoft,
        Role::Focus,
        Role::Scrim,
        Role::Chart1,
        Role::Chart2,
        Role::Chart3,
        Role::Chart4,
        Role::Chart5,
        Role::LineStrong,
        Role::Selection,
        Role::Shadow,
        Role::Media,
        Role::OnMedia,
        Role::SurfaceOverlay,
        Role::Knob,
        Role::SuccessMark,
        Role::WarningMark,
        Role::DangerMark,
        Role::MediaCanvas,
        Role::Info,
        Role::OnInfo,
        Role::InfoSoft,
        Role::InfoMark,
        Role::Hue1,
        Role::Hue2,
        Role::Hue3,
        Role::Hue4,
        Role::Hue5,
        Role::Hue6,
        Role::Hue7,
        Role::Hue8,
        Role::Hue1Soft,
        Role::Hue2Soft,
        Role::Hue3Soft,
        Role::Hue4Soft,
        Role::Hue5Soft,
        Role::Hue6Soft,
        Role::Hue7Soft,
        Role::Hue8Soft,
        Role::Chart6,
        Role::Chart7,
        Role::Chart8,
        Role::AccentSolid,
        Role::OnAccentSolid,
        Role::DangerSolid,
        Role::OnDangerSolid,
        Role::AccentSolidHover,
        Role::AccentSolidPressed,
        Role::DangerSolidHover,
        Role::DangerSolidPressed,
        Role::NeutralSoft,
    ];

    pub const fn name(self) -> &'static str {
        match self {
            Role::Canvas => "canvas",
            Role::Surface => "surface",
            Role::SurfaceRaised => "surface-raised",
            Role::SurfaceSunken => "surface-sunken",
            Role::Line => "line",
            Role::Text => "text",
            Role::TextMuted => "text-muted",
            Role::TextFaint => "text-faint",
            Role::Accent => "accent",
            Role::OnAccent => "on-accent",
            Role::AccentSoft => "accent-soft",
            Role::OnAccentSoft => "on-accent-soft",
            Role::Danger => "danger",
            Role::OnDanger => "on-danger",
            Role::DangerSoft => "danger-soft",
            Role::Success => "success",
            Role::OnSuccess => "on-success",
            Role::SuccessSoft => "success-soft",
            Role::Warning => "warning",
            Role::OnWarning => "on-warning",
            Role::WarningSoft => "warning-soft",
            Role::Focus => "focus",
            Role::Scrim => "scrim",
            Role::Chart1 => "chart-1",
            Role::Chart2 => "chart-2",
            Role::Chart3 => "chart-3",
            Role::Chart4 => "chart-4",
            Role::Chart5 => "chart-5",
            Role::LineStrong => "line-strong",
            Role::Selection => "selection",
            Role::Shadow => "shadow",
            Role::Media => "media",
            Role::OnMedia => "on-media",
            Role::SurfaceOverlay => "surface-overlay",
            Role::Knob => "knob",
            Role::SuccessMark => "success-mark",
            Role::WarningMark => "warning-mark",
            Role::DangerMark => "danger-mark",
            Role::MediaCanvas => "media-canvas",
            Role::Info => "info",
            Role::OnInfo => "on-info",
            Role::InfoSoft => "info-soft",
            Role::InfoMark => "info-mark",
            Role::Hue1 => "hue-1",
            Role::Hue2 => "hue-2",
            Role::Hue3 => "hue-3",
            Role::Hue4 => "hue-4",
            Role::Hue5 => "hue-5",
            Role::Hue6 => "hue-6",
            Role::Hue7 => "hue-7",
            Role::Hue8 => "hue-8",
            Role::Hue1Soft => "hue-1-soft",
            Role::Hue2Soft => "hue-2-soft",
            Role::Hue3Soft => "hue-3-soft",
            Role::Hue4Soft => "hue-4-soft",
            Role::Hue5Soft => "hue-5-soft",
            Role::Hue6Soft => "hue-6-soft",
            Role::Hue7Soft => "hue-7-soft",
            Role::Hue8Soft => "hue-8-soft",
            Role::Chart6 => "chart-6",
            Role::Chart7 => "chart-7",
            Role::Chart8 => "chart-8",
            Role::AccentSolid => "accent-solid",
            Role::OnAccentSolid => "on-accent-solid",
            Role::DangerSolid => "danger-solid",
            Role::OnDangerSolid => "on-danger-solid",
            Role::AccentSolidHover => "accent-solid-hover",
            Role::AccentSolidPressed => "accent-solid-pressed",
            Role::DangerSolidHover => "danger-solid-hover",
            Role::DangerSolidPressed => "danger-solid-pressed",
            Role::NeutralSoft => "neutral-soft",
        }
    }

    pub fn from_name(name: &str) -> Option<Role> {
        Role::ALL.into_iter().find(|role| role.name() == name)
    }

    pub const fn tier(self) -> Tier {
        match self {
            Role::Canvas
            | Role::Surface
            | Role::SurfaceRaised
            | Role::SurfaceSunken
            | Role::Line
            | Role::Text
            | Role::TextMuted
            | Role::TextFaint
            | Role::Danger
            | Role::Success
            | Role::Warning
            | Role::LineStrong => Tier::Authored,
            Role::Scrim
            | Role::Shadow
            | Role::Media
            | Role::OnMedia
            | Role::Knob
            | Role::MediaCanvas => Tier::Fixed,
            _ => Tier::Derived,
        }
    }
}

/// Who gives a role its colour.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tier {
    /// The pack writes it.
    Authored,
    /// The same in every pack unless a pack sets it.
    Fixed,
    /// Computed from the pack's authored roles unless a pack sets it.
    Derived,
}

/// (foreground, background, minimum WCAG contrast) every resolved palette meets. A translucent
/// foreground is measured laid over its background.
pub const CONTRAST: &[(Role, Role, f32)] = &[
    (Role::Line, Role::Surface, 1.2),
    (Role::Line, Role::Canvas, 1.1),
    (Role::Line, Role::SurfaceSunken, 1.1),
    (Role::Line, Role::SurfaceRaised, 1.1),
    (Role::Text, Role::Canvas, 4.5),
    (Role::Text, Role::Surface, 4.5),
    (Role::Text, Role::SurfaceRaised, 4.5),
    (Role::Text, Role::SurfaceSunken, 4.5),
    (Role::Text, Role::Selection, 4.5),
    (Role::Text, Role::SurfaceOverlay, 4.5),
    (Role::Text, Role::Hue1Soft, 4.5),
    (Role::Text, Role::Hue2Soft, 4.5),
    (Role::Text, Role::Hue3Soft, 4.5),
    (Role::Text, Role::Hue4Soft, 4.5),
    (Role::Text, Role::Hue5Soft, 4.5),
    (Role::Text, Role::Hue6Soft, 4.5),
    (Role::Text, Role::Hue7Soft, 4.5),
    (Role::Text, Role::Hue8Soft, 4.5),
    (Role::TextMuted, Role::Canvas, 4.5),
    (Role::TextMuted, Role::Surface, 4.5),
    (Role::TextMuted, Role::SurfaceOverlay, 4.5),
    (Role::TextFaint, Role::Surface, 3.0),
    (Role::TextFaint, Role::SurfaceOverlay, 3.0),
    (Role::Accent, Role::Surface, 4.5),
    (Role::Accent, Role::Canvas, 4.5),
    (Role::OnAccent, Role::Accent, 4.5),
    (Role::OnAccentSoft, Role::AccentSoft, 4.5),
    (Role::Danger, Role::Surface, 3.0),
    (Role::OnDanger, Role::Danger, 4.5),
    (Role::Success, Role::Surface, 3.0),
    (Role::OnSuccess, Role::Success, 4.5),
    (Role::Warning, Role::Surface, 3.0),
    (Role::OnWarning, Role::Warning, 4.5),
    (Role::Focus, Role::Surface, 3.0),
    (Role::Focus, Role::Canvas, 3.0),
    (Role::Chart1, Role::Surface, 3.0),
    (Role::Chart1, Role::Canvas, 3.0),
    (Role::Chart2, Role::Surface, 3.0),
    (Role::Chart2, Role::Canvas, 3.0),
    (Role::Chart3, Role::Surface, 3.0),
    (Role::Chart3, Role::Canvas, 3.0),
    (Role::Chart4, Role::Surface, 3.0),
    (Role::Chart4, Role::Canvas, 3.0),
    (Role::Chart5, Role::Surface, 3.0),
    (Role::Chart5, Role::Canvas, 3.0),
    (Role::LineStrong, Role::Surface, 3.0),
    (Role::LineStrong, Role::Canvas, 3.0),
    (Role::Selection, Role::Surface, 3.0),
    (Role::Selection, Role::Canvas, 3.0),
    (Role::OnMedia, Role::Media, 4.5),
    (Role::OnMedia, Role::MediaCanvas, 4.5),
    (Role::SuccessMark, Role::Surface, 3.0),
    (Role::SuccessMark, Role::SurfaceSunken, 3.0),
    (Role::WarningMark, Role::Surface, 3.0),
    (Role::WarningMark, Role::SurfaceSunken, 3.0),
    (Role::DangerMark, Role::Surface, 3.0),
    (Role::DangerMark, Role::SurfaceSunken, 3.0),
    (Role::Info, Role::Surface, 3.0),
    (Role::OnInfo, Role::Info, 4.5),
    (Role::InfoMark, Role::Surface, 3.0),
    (Role::InfoMark, Role::SurfaceSunken, 3.0),
    (Role::Hue1, Role::Surface, 3.0),
    (Role::Hue1, Role::SurfaceSunken, 3.0),
    (Role::Hue1, Role::Hue1Soft, 3.0),
    (Role::Hue2, Role::Surface, 3.0),
    (Role::Hue2, Role::SurfaceSunken, 3.0),
    (Role::Hue2, Role::Hue2Soft, 3.0),
    (Role::Hue3, Role::Surface, 3.0),
    (Role::Hue3, Role::SurfaceSunken, 3.0),
    (Role::Hue3, Role::Hue3Soft, 3.0),
    (Role::Hue4, Role::Surface, 3.0),
    (Role::Hue4, Role::SurfaceSunken, 3.0),
    (Role::Hue4, Role::Hue4Soft, 3.0),
    (Role::Hue5, Role::Surface, 3.0),
    (Role::Hue5, Role::SurfaceSunken, 3.0),
    (Role::Hue5, Role::Hue5Soft, 3.0),
    (Role::Hue6, Role::Surface, 3.0),
    (Role::Hue6, Role::SurfaceSunken, 3.0),
    (Role::Hue6, Role::Hue6Soft, 3.0),
    (Role::Hue7, Role::Surface, 3.0),
    (Role::Hue7, Role::SurfaceSunken, 3.0),
    (Role::Hue7, Role::Hue7Soft, 3.0),
    (Role::Hue8, Role::Surface, 3.0),
    (Role::Hue8, Role::SurfaceSunken, 3.0),
    (Role::Hue8, Role::Hue8Soft, 3.0),
    (Role::Chart6, Role::Surface, 3.0),
    (Role::Chart6, Role::Canvas, 3.0),
    (Role::Chart7, Role::Surface, 3.0),
    (Role::Chart7, Role::Canvas, 3.0),
    (Role::Chart8, Role::Surface, 3.0),
    (Role::Chart8, Role::Canvas, 3.0),
    (Role::OnAccentSolid, Role::AccentSolid, 4.5),
    (Role::OnAccentSolid, Role::AccentSolidHover, 4.5),
    (Role::OnAccentSolid, Role::AccentSolidPressed, 4.5),
    (Role::OnDangerSolid, Role::DangerSolid, 4.5),
    (Role::OnDangerSolid, Role::DangerSolidHover, 4.5),
    (Role::OnDangerSolid, Role::DangerSolidPressed, 4.5),
    (Role::NeutralSoft, Role::Surface, 1.3),
    (Role::NeutralSoft, Role::SurfaceSunken, 1.3),
    (Role::NeutralSoft, Role::SurfaceRaised, 1.3),
    (Role::NeutralSoft, Role::Canvas, 1.3),
];

/// (foreground, ground, the ground's alpha percent, what it is laid over, minimum WCAG contrast).
pub const COMPOSITE_CONTRAST: &[(Role, Role, u8, Role, f32)] = &[];
