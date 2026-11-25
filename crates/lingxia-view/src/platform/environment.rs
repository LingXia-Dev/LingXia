//! The View `env` from what an adapter measured. Adapters report OS facts;
//! insets, pointer and motion policy are decided here.
use super::FrameEnvironment;
use crate::platform::calendar;
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

fn sanitize_hinge(hinge: Option<&serde_json::Value>) -> serde_json::Value {
    let Some(raw) = hinge.filter(|v| v.is_object()) else {
        return serde_json::Value::Null;
    };
    let coords_ok = ["x", "y", "width", "height"].iter().all(|k| {
        raw[k]
            .as_f64()
            .is_some_and(|n| n.is_finite() && (0.0..=16384.0).contains(&n))
    });
    let orient_ok = matches!(raw["orientation"].as_str(), Some("vertical" | "horizontal"));
    let posture_ok = matches!(raw["posture"].as_str(), Some("flat" | "half-opened"));
    if coords_ok && orient_ok && posture_ok {
        raw.clone()
    } else {
        serde_json::Value::Null
    }
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
        let edge = |index: usize, extent: u16| {
            let covered = self.system_insets[index].max(self.chrome_insets[index]);
            logical(covered.min(f32::from(extent)))
        };
        let animation_scale = if self.animation_scale.is_finite() {
            self.animation_scale.clamp(0.0, 10.0)
        } else {
            1.0
        };
        let mut value = json!({
            "theme": if self.dark { "dark" } else { "light" },
            "insets": {
                "top": edge(0, height),
                "right": edge(1, width),
                "bottom": edge(2, height),
                "left": edge(3, width),
            },
            "safeArea": {
                "top": logical(self.system_insets[0].min(f32::from(height))),
                "right": logical(self.system_insets[1].min(f32::from(width))),
                "bottom": logical(self.system_insets[2].min(f32::from(height))),
                "left": logical(self.system_insets[3].min(f32::from(width))),
            },
            "hinge": sanitize_hinge(self.hinge.as_ref()),
            "keyboard": {"height": logical(self.keyboard.min(f32::from(height)))},
            "os": self.os,
            "pointer": if self.fine_pointer { "fine" } else { "coarse" },
            "language": self.locale,
            "animatorDurationScale": animation_scale,
            "reducedMotion": self.reduce_motion || animation_scale == 0.0,
            "highContrast": self.high_contrast,
            "hourCycle": self.hour_cycle.as_deref().filter(|cycle| matches!(*cycle, "h12" | "h23"))
                .unwrap_or(calendar::hour_cycle(&self.locale)),
            "firstWeekday": self.first_weekday.filter(|day| (1..=7).contains(day))
                .unwrap_or(calendar::first_weekday(&self.locale)),
            "renderer": {
                "fallback": crate::renderer::sentinel::fallback().map(|_| "driver"),
            },
        });
        if let Some(rows) = &self.text_scale {
            value["textScale"] = json!(rows);
        }
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
pub fn touch_target(env: &serde_json::Value) -> f32 {
    if env["pointer"] == "fine" {
        return 0.0;
    }
    match env["os"].as_str() {
        Some("android") => 48.0,
        Some("ios" | "harmony") => 44.0,
        _ => 0.0,
    }
}

/// The surface edges covered, logical pixels (top, right, bottom, left): by host and system
/// bars and the keyboard, then by system bars and the keyboard alone.
pub fn obscured(env: &serde_json::Value) -> [[f32; 4]; 2] {
    let length = |value: &serde_json::Value| value.as_f64().unwrap_or(0.0) as f32;
    let keyboard = length(&env["keyboard"]["height"]);
    let edges = |insets: &serde_json::Value| {
        [
            length(&insets["top"]),
            length(&insets["right"]),
            length(&insets["bottom"]).max(keyboard),
            length(&insets["left"]),
        ]
    };
    [edges(&env["insets"]), edges(&env["safeArea"])]
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
        assert_eq!(facts.environment().unwrap().value["highContrast"], false);
        facts.high_contrast = true;
        assert_eq!(facts.environment().unwrap().value["highContrast"], true);
    }

    #[test]
    fn insets_take_the_larger_overlap_clamped_to_the_surface_in_logical_pixels() {
        let mut facts = phone();
        facts.keyboard = 4000.0;
        let env = facts.environment().unwrap();
        assert_eq!((env.width, env.height, env.scale), (828, 1792, 2.0));
        assert_eq!(env.value["insets"]["top"], 48.0);
        assert_eq!(
            env.value["insets"]["bottom"], 83.0,
            "chrome covers more than the home indicator"
        );
        assert_eq!(
            env.value["keyboard"]["height"], 896.0,
            "clamped to the surface"
        );
        assert_eq!(env.value["pointer"], "coarse");
        assert_eq!(env.value["theme"], "light");
        assert!(env.value.get("textScale").is_none());
    }

    #[test]
    fn either_motion_signal_reduces_motion_and_a_fine_pointer_is_reported() {
        let mut facts = phone();
        facts.reduce_motion = true;
        assert_eq!(facts.environment().unwrap().value["reducedMotion"], true);
        facts.reduce_motion = false;
        facts.animation_scale = 0.0;
        assert_eq!(facts.environment().unwrap().value["reducedMotion"], true);
        facts.animation_scale = f32::NAN;
        let env = facts.environment().unwrap();
        assert_eq!(env.value["reducedMotion"], false);
        assert_eq!(env.value["animatorDurationScale"], 1.0);
        facts.fine_pointer = true;
        let env = facts.environment().unwrap();
        assert!(env.value.get("formFactor").is_none());
        assert_eq!(env.value["pointer"], "fine");
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
        let env = facts.environment().unwrap();
        let mut value = env.value;
        value["canDock"] = json!(false);
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
