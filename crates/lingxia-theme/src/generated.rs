// Generated from packages/lingxia-ui/contracts/schema.json; do not edit.
//! Roles are palette slots in order; append-only.

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[repr(u8)]
pub enum Role {
    /// Window and page floor.
    Canvas = 0,
    /// Cards, cells and sheets.
    Surface = 1,
    /// Menus, popovers and elevated sheets.
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
    /// Primary actions, selection and links.
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
    /// Modal backdrop, translucent.
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
}

pub const ROLE_COUNT: usize = 29;

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
        }
    }

    pub fn from_name(name: &str) -> Option<Role> {
        Role::ALL.into_iter().find(|role| role.name() == name)
    }
}

/// (foreground, background, minimum WCAG contrast) every resolved palette meets.
pub const CONTRAST: &[(Role, Role, f32)] = &[
    (Role::Line, Role::Surface, 1.2),
    (Role::Line, Role::Canvas, 1.1),
    (Role::Text, Role::Canvas, 4.5),
    (Role::Text, Role::Surface, 4.5),
    (Role::Text, Role::SurfaceRaised, 4.5),
    (Role::Text, Role::SurfaceSunken, 4.5),
    (Role::TextMuted, Role::Canvas, 4.5),
    (Role::TextMuted, Role::Surface, 4.5),
    (Role::TextFaint, Role::Surface, 3.0),
    (Role::Accent, Role::Surface, 3.0),
    (Role::Accent, Role::Canvas, 3.0),
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
];
