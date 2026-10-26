//! The View `env` from what an adapter measured. Adapters report OS facts;
//! insets, form factor, pointer and motion policy are decided here.
use super::FrameEnvironment;
use serde_json::json;

/// Shorter window side, in logical pixels, from which a device is a tablet.
pub const TABLET_SMALLEST_WIDTH: f32 = 600.0;

/// One frame's OS measurements. Lengths are device pixels unless named logical.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct DeviceFacts {
    pub width: u32,
    pub height: u32,
    /// Device pixels per logical pixel.
    pub scale: f32,
    /// `android`, `ios`, ...
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
    /// `[authored, shown]` font sizes in logical pixels, from `[0, 0]` to `[16384, _]`.
    pub text_scale: Option<Vec<[f32; 2]>>,
    /// Hosted by the desktop runner rather than an app.
    pub runner: bool,
}

impl DeviceFacts {
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
            "keyboard": {"height": logical(self.keyboard.min(f32::from(height)))},
            "os": self.os,
            "pointer": if self.fine_pointer { "fine" } else { "coarse" },
            "formFactor": if self.smallest_width >= TABLET_SMALLEST_WIDTH { "tablet" } else { "phone" },
            "displayLanguage": self.locale,
            "runner": self.runner,
            "animatorDurationScale": animation_scale,
            "reducedMotion": self.reduce_motion || animation_scale == 0.0,
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
        assert_eq!(env.value["formFactor"], "phone");
        assert_eq!(env.value["pointer"], "coarse");
        assert_eq!(env.value["theme"], "light");
        assert!(env.value.get("textScale").is_none());
    }

    #[test]
    fn either_motion_signal_reduces_motion_and_wide_windows_are_tablets() {
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
        facts.smallest_width = 744.0;
        facts.fine_pointer = true;
        let env = facts.environment().unwrap();
        assert_eq!(env.value["formFactor"], "tablet");
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
