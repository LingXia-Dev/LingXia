//! The View `env` from what an adapter measured. Adapters report OS facts;
//! insets, pointer and motion policy are decided here.
use super::FrameEnvironment;
use crate::platform::calendar;
use crate::protocol::env;
use serde_json::json;

/// One frame's OS measurements. Lengths are device pixels unless named logical.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct DeviceFacts {
    pub width: u32,
    pub height: u32,
    /// Device pixels per logical pixel.
    pub scale: f32,
    /// `ios`, `macos`, `android`, `windows` or `harmony`.
    pub os: String,
    pub dark: bool,
    /// System bars, cutouts and home indicators over the surface: top, right, bottom, left.
    pub system_insets: [f32; 4],
    /// Host chrome (navigation and tab bars) over the surface, same order.
    pub chrome_insets: [f32; 4],
    /// The software keyboard's overlap with the surface's bottom edge.
    pub keyboard: f32,
    /// The window's shorter side, logical pixels.
    pub smallest_width: f32,
    /// BCP 47 display language.
    pub locale: String,
    /// A mouse or trackpad drives the page, so hover styles apply.
    pub fine_pointer: bool,
    /// System animation speed; 0 turns animations off.
    pub animation_scale: f32,
    /// The OS's explicit reduce-motion switch.
    pub reduce_motion: bool,
    /// The OS asks for increased contrast.
    pub high_contrast: bool,
    /// `[authored, shown]` font sizes in logical pixels, from `[0, 0]` to `[16384, _]`.
    pub text_scale: Option<Vec<[f32; 2]>>,
    /// The user's clock setting (`h12` or `h23`); `None` follows the display language.
    pub hour_cycle: Option<String>,
    /// The user's first day of the week, 1 = Monday to 7 = Sunday; `None` follows the display language.
    pub first_weekday: Option<u8>,
    /// Physical display hinge/crease intersecting the page canvas in local logical pixels, if any.
    pub hinge: Option<serde_json::Value>,
}

fn sanitize_hinge(hinge: Option<&serde_json::Value>) -> Option<env::Hinge> {
    let hinge: env::Hinge = serde_json::from_value(hinge?.clone()).ok()?;
    [hinge.x, hinge.y, hinge.width, hinge.height]
        .iter()
        .all(|n| n.is_finite() && (0.0..=16384.0).contains(n))
        .then_some(hinge)
}

impl DeviceFacts {
    /// `first_weekday` from a platform that counts 1 = Sunday to 7 = Saturday (Android, Apple, HarmonyOS).
    pub fn weekday_from_sunday_first(day: u64) -> Option<u8> {
        (1..=7).contains(&day).then(|| ((day + 5) % 7 + 1) as u8)
    }

    /// `None` until the surface has a size the wire can carry.
    pub fn environment(&self) -> Option<FrameEnvironment> {
        let width = u16::try_from(self.width).ok().filter(|w| *w > 0)?;
        let height = u16::try_from(self.height).ok().filter(|h| *h > 0)?;
        if !(self.scale.is_finite() && self.scale > 0.0) {
            return None;
        }
        let logical = |px: f32| (px.max(0.0) / self.scale) as f64;
        let animation_scale = if self.animation_scale.is_finite() {
            self.animation_scale.clamp(0.0, 10.0)
        } else {
            1.0
        };
        let edges = |extent: [f32; 4]| env::Edges {
            top: logical(extent[0].min(f32::from(height))),
            right: logical(extent[1].min(f32::from(width))),
            bottom: logical(extent[2].min(f32::from(height))),
            left: logical(extent[3].min(f32::from(width))),
        };
        let covered: [f32; 4] =
            std::array::from_fn(|i| self.system_insets[i].max(self.chrome_insets[i]));
        let value = env::Environment {
            appearance: if self.dark {
                env::Appearance::Dark
            } else {
                env::Appearance::Light
            },
            insets: edges(covered),
            safe_area: Some(edges(self.system_insets)),
            hinge: sanitize_hinge(self.hinge.as_ref()),
            keyboard: env::Keyboard {
                height: logical(self.keyboard.min(f32::from(height))),
            },
            capsule: None,
            pointer: if self.fine_pointer {
                env::Pointer::Fine
            } else {
                env::Pointer::Coarse
            },
            os: env::Os::parse(&self.os).unwrap_or(env::Os::Unknown),
            size_class: None,
            language: self.locale.clone(),
            reduced_motion: Some(self.reduce_motion || animation_scale == 0.0),
            high_contrast: Some(self.high_contrast),
            animator_duration_scale: Some(f64::from(animation_scale)),
            hour_cycle: self
                .hour_cycle
                .as_deref()
                .and_then(env::HourCycle::parse)
                .or_else(|| env::HourCycle::parse(calendar::hour_cycle(&self.locale))),
            first_weekday: Some(
                self.first_weekday
                    .filter(|day| (1..=7).contains(day))
                    .unwrap_or(calendar::first_weekday(&self.locale)),
            ),
            can_dock: false,
            renderer: Some(env::Renderer {
                fallback: crate::renderer::sentinel::fallback()
                    .map(|_| env::RendererFallback::Driver),
            }),
            text_scale: self.text_scale.as_ref().map(|rows| json!(rows)),
            palette: None,
            scales: None,
        };
        Some(FrameEnvironment {
            width,
            height,
            scale: self.scale,
            value,
        })
    }
}

/// The smallest box a finger press reaches on this platform, logical pixels: the platform's
/// minimum touch target (Android 48 dp, iOS and HarmonyOS 44 pt); none for a fine pointer.
pub fn touch_target(env: &env::Environment) -> f32 {
    if env.pointer == env::Pointer::Fine {
        return 0.0;
    }
    match env.os {
        env::Os::Android => 48.0,
        env::Os::Ios | env::Os::Harmony => 44.0,
        _ => 0.0,
    }
}

/// The surface edges covered, logical pixels (top, right, bottom, left): by host and system
/// bars and the keyboard, then by system bars and the keyboard alone.
pub fn obscured(env: &env::Environment) -> [[f32; 4]; 2] {
    let keyboard = env.keyboard.height as f32;
    let edges = |insets: env::Edges| {
        [
            insets.top as f32,
            insets.right as f32,
            (insets.bottom as f32).max(keyboard),
            insets.left as f32,
        ]
    };
    [
        edges(env.insets),
        edges(env.safe_area.unwrap_or_default()),
    ]
}

/// A well-formed environment with `fields` over a plain light phone's.
#[cfg(test)]
pub(crate) fn test_environment(fields: serde_json::Value) -> env::Environment {
    let mut value = json!({"appearance": "light", "insets": {"top": 0.0, "right": 0.0, "bottom": 0.0, "left": 0.0},
        "keyboard": {"height": 0.0}, "os": "unknown", "language": "en", "canDock": false});
    for (key, field) in fields.as_object().expect("fields are an object") {
        value[key] = field.clone();
    }
    serde_json::from_value(value).expect("a well-formed environment")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn phone() -> DeviceFacts {
        DeviceFacts {
            width: 828,
            height: 1792,
            scale: 2.0,
            os: "ios".into(),
            system_insets: [96.0, 0.0, 68.0, 0.0],
            chrome_insets: [0.0, 0.0, 166.0, 0.0],
            keyboard: 0.0,
            smallest_width: 414.0,
            locale: "zh-Hans-CN".into(),
            animation_scale: 1.0,
            ..Default::default()
        }
    }

    #[test]
    fn the_increased_contrast_switch_reaches_the_environment() {
        let mut facts = phone();
        assert_eq!(facts.environment().unwrap().value.high_contrast, Some(false));
        facts.high_contrast = true;
        assert_eq!(facts.environment().unwrap().value.high_contrast, Some(true));
    }

    #[test]
    fn insets_take_the_larger_overlap_clamped_to_the_surface_in_logical_pixels() {
        let mut facts = phone();
        facts.keyboard = 4000.0;
        let env = facts.environment().unwrap();
        assert_eq!((env.width, env.height, env.scale), (828, 1792, 2.0));
        assert_eq!(env.value.insets.top, 48.0);
        assert_eq!(
            env.value.insets.bottom, 83.0,
            "chrome covers more than the home indicator"
        );
        assert_eq!(env.value.keyboard.height, 896.0, "clamped to the surface");
        assert_eq!(env.value.pointer, env::Pointer::Coarse);
        assert_eq!(env.value.appearance, env::Appearance::Light);
        assert!(env.value.text_scale.is_none());
    }

    #[test]
    fn either_motion_signal_reduces_motion_and_a_fine_pointer_is_reported() {
        let mut facts = phone();
        facts.reduce_motion = true;
        assert_eq!(facts.environment().unwrap().value.reduced_motion, Some(true));
        facts.reduce_motion = false;
        facts.animation_scale = 0.0;
        assert_eq!(facts.environment().unwrap().value.reduced_motion, Some(true));
        facts.animation_scale = f32::NAN;
        let env = facts.environment().unwrap();
        assert_eq!(env.value.reduced_motion, Some(false));
        assert_eq!(env.value.animator_duration_scale, Some(1.0));
        facts.fine_pointer = true;
        assert_eq!(facts.environment().unwrap().value.pointer, env::Pointer::Fine);
    }

    #[test]
    fn an_unmeasured_surface_has_no_environment() {
        for facts in [
            DeviceFacts {
                width: 0,
                ..phone()
            },
            DeviceFacts {
                height: 70_000,
                ..phone()
            },
            DeviceFacts {
                scale: 0.0,
                ..phone()
            },
        ] {
            assert!(facts.environment().is_none());
        }
    }

    #[test]
    fn a_built_environment_is_one_the_session_accepts() {
        let mut facts = phone();
        facts.text_scale = Some(vec![[0.0, 0.0], [17.0, 20.0], [16384.0, 19275.0]]);
        let value = facts.environment().unwrap().value.to_json();
        let module = crate::session::tests::source("function render(){return h('text',{},'ok')}");
        let mut page = crate::ViewSession::new(&module).unwrap();
        page.set_environment(value).unwrap();
        assert_eq!(page.layout(414, 896, 2.0).unwrap().nodes[0].text, "ok");
    }
}

#[cfg(test)]
mod weekday_tests {
    use super::DeviceFacts;

    #[test]
    fn a_sunday_first_platform_day_becomes_iso() {
        let iso: Vec<_> = (0..=8)
            .map(DeviceFacts::weekday_from_sunday_first)
            .collect();
        assert_eq!(
            iso,
            [
                None,
                Some(7),
                Some(1),
                Some(2),
                Some(3),
                Some(4),
                Some(5),
                Some(6),
                None
            ]
        );
    }
}
