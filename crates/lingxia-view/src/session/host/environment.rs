use serde::Deserialize;

/// Environment the runtime pushes and the platform cannot measure; each kind keeps its newest revision.
#[derive(Default)]
pub(super) struct HostEnvironment {
    /// Revision, display language, and the lxapp's narrowing of it.
    language: Option<(u64, String, Option<String>)>,
    surface: Option<(u64, SurfaceContext)>,
    /// The capsule's box over the page, or `None` under a navigation bar.
    chrome: Option<(u64, Option<Capsule>)>,
    /// The lxapp's theme pack, resolved into palette pushes for light, dark and their
    /// high-contrast variants.
    theme: Option<(u64, [serde_json::Value; 4], serde_json::Value)>,
    /// The View's last run read a host-context key, so a surface push must rebuild it.
    ignores_host_context: bool,
    /// The View's last run did not depend on the appearance, so a theme change only repaints.
    ignores_appearance: bool,
    /// A test run's override (`view.environment`); only automation sets it.
    test: Option<TestOverride>,
    /// The largest factor the system text size may scale text by, as the lxapp and page allow.
    max_text_scale: Option<f32>,
}

/// The text scale held to before the lxapp's limit arrives, and when it names none.
const DEFAULT_MAX_TEXT_SCALE: f32 = 2.0;

/// What a test run pins for a page: its theme, stillness (no motion, a steady caret), the
/// system text size and the contrast setting. `None` leaves a fact as measured.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(super) struct TestOverride {
    /// `Some(true)` dark, `Some(false)` light.
    pub(super) dark: Option<bool>,
    pub(super) still: bool,
    /// Every text size multiplied by this.
    pub(super) text_scale: Option<f32>,
    pub(super) high_contrast: Option<bool>,
}

/// The text sizes a test may pin, as factors of the default.
pub(super) const TEXT_SCALES: std::ops::RangeInclusive<f64> = 0.5..=3.0;

/// Environment keys that only change colours, which the renderer resolves.
const APPEARANCE: [&str; 3] = ["theme", "highContrast", "palette"];

/// Environment keys a surface push sets that only the View reads.
const HOST_CONTEXT: [&str; 2] = ["sizeClass", "canDock"];

/// The lxapp's adaptive context, as Logic's `lx.surface.watchContext()` reports it.
#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct SurfaceContext {
    size_class: SizeClass,
    can_dock: bool,
    width: f64,
    height: f64,
}

/// The host capsule's box in page coordinates, points.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
struct Capsule {
    x: f64,
    y: f64,
    width: f64,
    height: f64,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
enum SizeClass {
    Compact,
    Regular,
}

#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase", deny_unknown_fields)]
enum Update {
    Language {
        revision: u64,
        language: String,
        #[serde(default, rename = "contentLanguage")]
        content: Option<String>,
    },
    Surface {
        revision: u64,
        context: SurfaceContext,
    },
    Theme {
        revision: u64,
        pack: Box<lingxia_theme::Pack>,
    },
    Chrome {
        revision: u64,
        capsule: Option<Capsule>,
    },
    #[serde(rename = "textScaleLimit")]
    TextScaleLimit { max: f32 },
}

impl HostEnvironment {
    /// Seeded with the pack the lxapp of `worker`'s app session registered.
    pub(super) fn for_worker(worker: Option<&str>) -> Self {
        let mut host = Self::default();
        if let Some(pack) = worker.and_then(lingxia_theme::app_pack) {
            match theme(&pack, 0) {
                Ok(theme) => host.theme = Some(theme),
                Err(error) => {
                    log::warn!(target: "lingxia_view", "{error}; the default theme applies")
                }
            }
        }
        host
    }

    /// True when the update is newer than what this host holds and this page must redraw.
    pub(super) fn update(&mut self, update: &str) -> Result<bool, String> {
        let update: Update =
            serde_json::from_str(update).map_err(|e| format!("UI_ENVIRONMENT: {e}"))?;
        let newer = |held: Option<u64>, revision| held.is_none_or(|held| revision > held);
        Ok(match update {
            Update::Language {
                revision,
                language,
                content,
            } => {
                let invalid = |l: &str| l.is_empty() || l.len() > 128;
                if invalid(&language) || content.as_deref().is_some_and(invalid) {
                    return Err("UI_ENVIRONMENT: invalid display language".into());
                }
                let newer = newer(self.language.as_ref().map(|l| l.0), revision);
                if newer {
                    self.language = Some((revision, language, content));
                }
                newer
            }
            Update::Surface { revision, context } => {
                let newer = newer(self.surface.as_ref().map(|s| s.0), revision);
                if newer {
                    self.surface = Some((revision, context));
                }
                // A View that never read the host context waits for its next frame to see it.
                newer && !self.ignores_host_context
            }
            Update::Chrome { revision, capsule } => {
                if capsule.is_some_and(|c| {
                    ![c.x, c.y, c.width, c.height].iter().all(|v| v.is_finite())
                        || c.width < 0.0
                        || c.height < 0.0
                }) {
                    return Err("UI_ENVIRONMENT: invalid capsule".into());
                }
                let newer = newer(self.chrome.as_ref().map(|c| c.0), revision);
                let changed = newer && self.chrome.as_ref().is_none_or(|c| c.1 != capsule);
                if newer {
                    self.chrome = Some((revision, capsule));
                }
                // Not host context: the View's read tracking does not count `capsule`.
                changed
            }
            Update::TextScaleLimit { max } => {
                if !(1.0..=3.0).contains(&max) {
                    return Err("UI_ENVIRONMENT: invalid text scale limit".into());
                }
                self.max_text_scale.replace(max) != Some(max)
            }
            Update::Theme { revision, pack } => {
                let newer = newer(self.theme.as_ref().map(|t| t.0), revision);
                if newer {
                    self.theme = Some(theme(&pack, revision)?);
                }
                newer
            }
        })
    }

    /// Replace the test override; true when it changed.
    pub(super) fn set_test(&mut self, test: Option<TestOverride>) -> bool {
        std::mem::replace(&mut self.test, test) != test
    }

    pub(super) fn test(&self) -> Option<TestOverride> {
        self.test
    }

    pub(super) fn still(&self) -> bool {
        self.test.is_some_and(|test| test.still)
    }

    pub(super) fn set_reads_host_context(&mut self, reads: bool) {
        self.ignores_host_context = !reads;
    }

    pub(super) fn set_reads_appearance(&mut self, reads: bool) {
        self.ignores_appearance = !reads;
    }

    /// `a` and `b` differ only in the appearance and this page's View does not depend on it:
    /// the page repaints in the new colours without running its View.
    pub(super) fn repaints_only(
        &self,
        a: &crate::platform::FrameEnvironment,
        b: &crate::platform::FrameEnvironment,
    ) -> bool {
        if !self.ignores_appearance || (a.width, a.height, a.scale) != (b.width, b.height, b.scale)
        {
            return false;
        }
        let strip = |value: &serde_json::Value| {
            let mut value = value.clone();
            if let Some(object) = value.as_object_mut() {
                for key in APPEARANCE {
                    object.remove(key);
                }
                if self.ignores_host_context {
                    for key in HOST_CONTEXT {
                        object.remove(key);
                    }
                }
            }
            value
        };
        strip(&a.value) == strip(&b.value)
    }

    /// `a` and `b` are the same for this page: host-context keys count only once its View
    /// read one, so a surface push leaves other pages (hidden ones included) as they are.
    pub(super) fn same(
        &self,
        a: &crate::platform::FrameEnvironment,
        b: &crate::platform::FrameEnvironment,
    ) -> bool {
        Self::same_as_read(a, b, !self.ignores_host_context)
    }

    /// `a` and `b` are the same for a View run that read the host context or did not.
    pub(super) fn same_as_read(
        a: &crate::platform::FrameEnvironment,
        b: &crate::platform::FrameEnvironment,
        reads_host_context: bool,
    ) -> bool {
        if a == b {
            return true;
        }
        if reads_host_context || (a.width, a.height, a.scale) != (b.width, b.height, b.scale) {
            return false;
        }
        let strip = |value: &serde_json::Value| {
            let mut value = value.clone();
            if let Some(object) = value.as_object_mut() {
                for key in HOST_CONTEXT {
                    object.remove(key);
                }
            }
            value
        };
        strip(&a.value) == strip(&b.value)
    }

    /// A frame built for `built` still fits a target measuring `now`: same size and scale,
    /// and the same environment as far as this page's View read it.
    pub(super) fn still_fits(
        &self,
        built: Option<&crate::platform::FrameEnvironment>,
        now: Option<crate::platform::FrameEnvironment>,
    ) -> bool {
        let (Some(built), Some(mut now)) = (built, now) else {
            return false;
        };
        self.apply(&mut now.value);
        self.same(built, &now)
    }

    /// Overlays pushed values on the measured `env`; `canDock` is false until a surface push.
    pub(super) fn apply(&self, env: &mut serde_json::Value) {
        if let Some(test) = self.test {
            if let Some(dark) = test.dark
                && (env["theme"] == "dark") != dark
            {
                env["theme"] = if dark { "dark" } else { "light" }.into();
                // A pushed palette was resolved for the measured theme.
                if let Some(object) = env.as_object_mut() {
                    object.remove("palette");
                }
            }
            if let Some(high) = test.high_contrast
                && (env["highContrast"] == true) != high
            {
                env["highContrast"] = high.into();
                if let Some(object) = env.as_object_mut() {
                    object.remove("palette");
                }
            }
            if let Some(scale) = test.text_scale {
                env["textScale"] = serde_json::json!([[0.0, 0.0], [16384.0, 16384.0 * scale]]);
            }
            if test.still {
                env["reducedMotion"] = true.into();
            }
        }
        self.limit_text_scale(env);
        if let Some((_, language, content)) = &self.language {
            env["displayLanguage"] = language.as_str().into();
            env["contentLanguage"] = content.as_deref().unwrap_or(language).into();
        }
        let surface = self.surface.as_ref().map(|s| &s.1);
        if let Some(context) = surface {
            env["sizeClass"] = match context.size_class {
                SizeClass::Compact => "compact",
                SizeClass::Regular => "regular",
            }
            .into();
        }
        env["canDock"] = surface.is_some_and(|c| c.can_dock).into();
        if let Some((_, capsule)) = &self.chrome {
            env["capsule"] = capsule.map_or(
                serde_json::Value::Null,
                |c| serde_json::json!({"x": c.x, "y": c.y, "width": c.width, "height": c.height}),
            );
        }
        // A palette the platform pushes (the host's theme) wins over the lxapp's own pack.
        if let Some((_, palettes, _)) = &self.theme
            && env.get("palette").is_none()
        {
            let look =
                usize::from(env["theme"] == "dark") + 2 * usize::from(env["highContrast"] == true);
            env["palette"] = palettes[look].clone();
        }
        if let Some((_, _, scales)) = &self.theme
            && env.get("scales").is_none()
        {
            env["scales"] = scales.clone();
        }
    }
}

impl HostEnvironment {
    /// Caps the measured text size mapping at the allowed factor. Capped at its points, the
    /// mapping stays rising and stays under the cap between them.
    fn limit_text_scale(&self, env: &mut serde_json::Value) {
        let max = self.max_text_scale.unwrap_or(DEFAULT_MAX_TEXT_SCALE);
        let Some(rows) = env
            .get_mut("textScale")
            .and_then(|rows| rows.as_array_mut())
        else {
            return;
        };
        for row in rows {
            let (Some(x), Some(y)) = (row[0].as_f64(), row[1].as_f64()) else {
                continue;
            };
            let capped = x * f64::from(max);
            if y > capped {
                row[1] = capped.into();
            }
        }
    }
}

/// The pack's palette pushes and its scales push, at `revision`.
fn theme(
    pack: &lingxia_theme::Pack,
    revision: u64,
) -> Result<(u64, [serde_json::Value; 4], serde_json::Value), String> {
    let scales = crate::protocol::scales::Scales::from_pack(pack, revision)?;
    Ok((revision, palettes(pack, revision)?, scales.push()))
}

/// The pack resolved for each look by the resolver host chrome uses, as palette pushes:
/// light, dark, high-contrast light, high-contrast dark.
fn palettes(pack: &lingxia_theme::Pack, revision: u64) -> Result<[serde_json::Value; 4], String> {
    let pack = lingxia_theme::complete(pack.clone());
    if let Some(problem) = lingxia_theme::check(&pack).first() {
        return Err(format!(
            "UI_ENVIRONMENT: theme pack {}: {problem}",
            pack.name
        ));
    }
    let resolve = |appearance, high_contrast| {
        let prefs = lingxia_theme::Prefs {
            appearance,
            high_contrast,
            ..Default::default()
        };
        lingxia_theme::resolve(&pack, &prefs)
            .map(|theme| {
                let colors: Vec<String> = theme.palette.iter().map(|color| color.hex()).collect();
                serde_json::json!({ "revision": revision, "colors": colors })
            })
            .map_err(|error| format!("UI_ENVIRONMENT: {error}"))
    };
    use lingxia_theme::Appearance::{Dark, Light};
    Ok([
        resolve(Light, false)?,
        resolve(Dark, false)?,
        resolve(Light, true)?,
        resolve(Dark, true)?,
    ])
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn a_test_override_pins_the_theme_and_stills_motion_until_cleared() {
        let mut host = HostEnvironment::default();
        let measured = json!({"theme": "light", "reducedMotion": false, "palette": {"revision": 1, "colors": []}});
        let applied = |host: &HostEnvironment| {
            let mut env = measured.clone();
            host.apply(&mut env);
            env
        };
        assert_eq!(applied(&host)["palette"]["revision"], 1);
        let dark = TestOverride {
            dark: Some(true),
            still: true,
            ..Default::default()
        };
        assert!(host.set_test(Some(dark)));
        assert!(!host.set_test(Some(dark)));
        let env = applied(&host);
        assert_eq!(
            (env["theme"].as_str(), env["reducedMotion"].as_bool()),
            (Some("dark"), Some(true))
        );
        // The platform's palette was for light; the dark look resolves its own.
        assert!(env.get("palette").is_none());
        assert!(host.still());
        // Pinning the measured theme keeps its palette.
        host.set_test(Some(TestOverride {
            dark: Some(false),
            ..Default::default()
        }));
        let env = applied(&host);
        assert_eq!(
            (
                env["theme"].as_str(),
                env["reducedMotion"].as_bool(),
                env["palette"]["revision"].as_i64()
            ),
            (Some("light"), Some(false), Some(1))
        );
        assert!(host.set_test(None));
        assert_eq!(applied(&host)["theme"], "light");
        assert!(!host.still());
    }

    #[test]
    fn a_chrome_push_sets_the_capsule_and_refuses_a_malformed_one() {
        let mut host = HostEnvironment::default();
        let chrome = |revision: u64, capsule: serde_json::Value| {
            json!({"kind": "chrome", "revision": revision, "capsule": capsule}).to_string()
        };
        let applied = |host: &HostEnvironment| {
            let mut env = json!({});
            host.apply(&mut env);
            env
        };
        assert!(applied(&host).get("capsule").is_none());
        let rect = json!({"x": 280.0, "y": 50.0, "width": 87.0, "height": 32.0});
        assert!(host.update(&chrome(1, rect.clone())).unwrap());
        assert_eq!(applied(&host)["capsule"], rect);
        assert!(!host.update(&chrome(2, rect.clone())).unwrap(), "unchanged");
        assert!(!host.update(&chrome(1, json!(null))).unwrap(), "older");
        assert!(host.update(&chrome(3, json!(null))).unwrap());
        assert_eq!(applied(&host)["capsule"], json!(null));
        for bad in [
            json!({"x": 0.0, "y": 0.0, "width": -1.0, "height": 32.0}),
            json!({"x": 0.0, "y": 0.0, "width": 87.0}),
        ] {
            assert!(host.update(&chrome(4, bad)).is_err());
        }
    }

    #[test]
    fn pushes_overlay_the_measured_environment_by_revision() {
        let mut host = HostEnvironment::default();
        let mut env = json!({"displayLanguage": "en-US"});
        host.apply(&mut env);
        assert_eq!(env, json!({"displayLanguage": "en-US", "canDock": false}));

        let surface = |revision: u64, size: &str, dock: bool| {
            json!({"kind": "surface", "revision": revision,
                "context": {"sizeClass": size, "canDock": dock, "width": 360.0, "height": 780.0}})
            .to_string()
        };
        assert!(host.update(&surface(2, "regular", true)).unwrap());
        assert!(
            !host.update(&surface(1, "compact", false)).unwrap(),
            "stale"
        );
        assert!(
            host.update(r#"{"kind":"language","revision":5,"language":"zh-Hans-CN"}"#)
                .unwrap()
        );
        host.apply(&mut env);
        assert_eq!(
            env,
            json!({"displayLanguage": "zh-Hans-CN", "contentLanguage": "zh-Hans-CN",
                "sizeClass": "regular", "canDock": true})
        );
        assert!(
            host.update(
                r#"{"kind":"language","revision":6,"language":"zh-Hans-CN","contentLanguage":"en"}"#
            )
            .unwrap()
        );
        host.apply(&mut env);
        assert_eq!(env["displayLanguage"], "zh-Hans-CN");
        assert_eq!(
            env["contentLanguage"], "en",
            "the lxapp's narrowing rides with the language"
        );
    }

    #[test]
    fn a_built_frame_fits_until_an_input_its_view_read_differs() {
        let frame =
            |width: u16, scale: f32, value: serde_json::Value| crate::platform::FrameEnvironment {
                width,
                height: 800,
                scale,
                value,
            };
        let mut host = HostEnvironment::default();
        let mut measured = json!({"theme": "light"});
        host.apply(&mut measured);
        let built = frame(400, 2.0, measured.clone());
        assert!(host.still_fits(
            Some(&built),
            Some(frame(400, 2.0, json!({"theme": "light"})))
        ));
        assert!(!host.still_fits(
            Some(&built),
            Some(frame(401, 2.0, json!({"theme": "light"})))
        ));
        assert!(!host.still_fits(
            Some(&built),
            Some(frame(400, 3.0, json!({"theme": "light"})))
        ));
        assert!(!host.still_fits(
            Some(&built),
            Some(frame(400, 2.0, json!({"theme": "dark"})))
        ));
        assert!(
            !host.still_fits(Some(&built), None),
            "an unmeasured target fits nothing"
        );
        assert!(!host.still_fits(None, Some(frame(400, 2.0, json!({"theme": "light"})))));

        let push = |dock: bool| {
            json!({"kind": "surface", "revision": u64::from(dock) + 1,
                "context": {"sizeClass": "compact", "canDock": dock, "width": 200.0, "height": 400.0}})
            .to_string()
        };
        assert!(host.update(&push(false)).unwrap());
        let now = || Some(frame(400, 2.0, json!({"theme": "light"})));
        assert!(
            !host.still_fits(Some(&built), now()),
            "a View that reads host context needs the pushed values"
        );
        host.set_reads_host_context(false);
        assert!(
            host.still_fits(Some(&built), now()),
            "a push this View never read leaves its frame current"
        );
        host.set_reads_host_context(true);
        let mut pushed = json!({"theme": "light"});
        host.apply(&mut pushed);
        let rebuilt = frame(400, 2.0, pushed);
        assert!(
            host.still_fits(Some(&rebuilt), now()),
            "a frame built with the pushed values stays current"
        );
    }

    #[test]
    fn malformed_pushes_are_rejected() {
        let mut host = HostEnvironment::default();
        for bad in [
            r#"{"kind":"surface","revision":1,"context":{"sizeClass":"huge","canDock":true,"width":0,"height":0}}"#,
            r#"{"kind":"surface","revision":1,"context":{"sizeClass":"compact","width":0,"height":0}}"#,
            r#"{"kind":"language","revision":1,"language":""}"#,
            r#"{"kind":"language","revision":1,"language":"en","contentLanguage":""}"#,
            r#"{"kind":"theme","revision":1}"#,
            r#"{"kind":"theme","revision":1,"pack":{"name":"x","version":2}}"#,
            r##"{"kind":"theme","revision":1,"pack":{"name":"x","version":1,"appearance":{"light":{"text":"#ffffff"}}}}"##,
        ] {
            assert!(host.update(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn an_lxapp_pack_becomes_the_palette_unless_the_platform_pushes_one() {
        let mut host = HostEnvironment::default();
        let pack = json!({"kind": "theme", "revision": 1,
            "pack": {"name": "brand", "version": 1, "accent": {"seed": "#0f9d58", "derive": true}}});
        assert!(host.update(&pack.to_string()).unwrap());
        let slot = lingxia_theme::Role::Accent as usize;
        for theme in ["light", "dark"] {
            let mut env = json!({"theme": theme});
            host.apply(&mut env);
            let palette = crate::theme::Palette::decode(&env["palette"], theme == "dark").unwrap();
            assert_ne!(
                palette.0[slot],
                crate::theme::Palette::builtin(theme == "dark").0[slot]
            );
            assert_eq!(
                palette.role(lingxia_theme::Role::Surface),
                crate::theme::Palette::builtin(theme == "dark").role(lingxia_theme::Role::Surface)
            );
        }
        // The increased-contrast preference picks the pack's high-contrast look.
        let mut base = json!({"theme": "light"});
        let mut high = json!({"theme": "light", "highContrast": true});
        host.apply(&mut base);
        host.apply(&mut high);
        assert_ne!(base["palette"], high["palette"]);
        let mut env = json!({"theme": "light", "palette": {"revision": 9, "colors": ["#000000"]}});
        host.apply(&mut env);
        assert_eq!(env["palette"]["revision"], 9, "the platform's palette wins");
    }

    #[test]
    fn the_text_size_mapping_is_capped_at_the_allowed_scale() {
        let mut host = HostEnvironment::default();
        let measured = json!([[0, 0], [17, 51], [100, 150], [16384, 24576]]);
        let capped = |host: &HostEnvironment| {
            let mut env = json!({"textScale": measured});
            host.apply(&mut env);
            env["textScale"].clone()
        };
        // 2.0 until the lxapp says otherwise; a point under the cap keeps its size.
        assert_eq!(
            capped(&host),
            json!([[0, 0], [17, 34.0], [100, 150], [16384, 24576]])
        );
        let limit = |max: f64| json!({"kind": "textScaleLimit", "max": max}).to_string();
        assert!(host.update(&limit(3.0)).unwrap());
        assert_eq!(capped(&host), measured);
        assert!(
            !host.update(&limit(3.0)).unwrap(),
            "the same limit redraws nothing"
        );
        assert!(host.update(&limit(1.0)).unwrap());
        assert_eq!(
            capped(&host),
            json!([[0, 0], [17, 17.0], [100, 100.0], [16384, 16384.0]])
        );
        assert!(crate::text::scale::TextScale::decode(Some(&capped(&host))).is_ok());
        for invalid in [0.5, 3.5] {
            assert!(host.update(&limit(invalid)).is_err());
        }
    }
}
