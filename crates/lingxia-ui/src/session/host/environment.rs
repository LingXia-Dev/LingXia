use serde::Deserialize;

/// Environment the runtime pushes and the platform cannot measure; each kind keeps its newest revision.
#[derive(Default)]
pub(super) struct HostEnvironment {
    language: Option<(u64, String)>,
    surface: Option<(u64, SurfaceContext)>,
    /// The lxapp's theme pack, resolved into palette pushes for light, dark and their
    /// high-contrast variants.
    theme: Option<(u64, [serde_json::Value; 4])>,
    /// The View's last run read a host-context key, so a surface push must rebuild it.
    ignores_host_context: bool,
}

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
    },
    Surface {
        revision: u64,
        context: SurfaceContext,
    },
    Theme {
        revision: u64,
        pack: Box<lingxia_theme::Pack>,
    },
}

impl HostEnvironment {
    /// Seeded with the pack the lxapp of `worker`'s app session registered.
    pub(super) fn for_worker(worker: Option<&str>) -> Self {
        let mut host = Self::default();
        if let Some(pack) = worker.and_then(lingxia_theme::app_pack) {
            match palettes(&pack, 0) {
                Ok(palettes) => host.theme = Some((0, palettes)),
                Err(error) => {
                    log::warn!(target: "lingxia_ui", "{error}; the default palette applies")
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
            Update::Language { revision, language } => {
                if language.is_empty() || language.len() > 128 {
                    return Err("UI_ENVIRONMENT: invalid display language".into());
                }
                let newer = newer(self.language.as_ref().map(|l| l.0), revision);
                if newer {
                    self.language = Some((revision, language));
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
            Update::Theme { revision, pack } => {
                let newer = newer(self.theme.as_ref().map(|t| t.0), revision);
                if newer {
                    self.theme = Some((revision, palettes(&pack, revision)?));
                }
                newer
            }
        })
    }

    pub(super) fn set_reads_host_context(&mut self, reads: bool) {
        self.ignores_host_context = !reads;
    }

    /// `a` and `b` are the same for this page: host-context keys count only once its View
    /// read one, so a surface push leaves other pages (hidden ones included) as they are.
    pub(super) fn same(
        &self,
        a: &crate::platform::FrameEnvironment,
        b: &crate::platform::FrameEnvironment,
    ) -> bool {
        if a == b {
            return true;
        }
        if !self.ignores_host_context
            || (a.width, a.height, a.scale) != (b.width, b.height, b.scale)
        {
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

    /// Overlays pushed values on the measured `env`; `canDock` is false until a surface push.
    pub(super) fn apply(&self, env: &mut serde_json::Value) {
        if let Some((_, language)) = &self.language {
            env["displayLanguage"] = language.as_str().into();
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
        // A palette the platform pushes (the host's theme) wins over the lxapp's own pack.
        if let Some((_, palettes)) = &self.theme
            && env.get("palette").is_none()
        {
            let look =
                usize::from(env["theme"] == "dark") + 2 * usize::from(env["highContrast"] == true);
            env["palette"] = palettes[look].clone();
        }
    }
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
            json!({"displayLanguage": "zh-Hans-CN", "sizeClass": "regular", "canDock": true})
        );
    }

    #[test]
    fn malformed_pushes_are_rejected() {
        let mut host = HostEnvironment::default();
        for bad in [
            r#"{"kind":"surface","revision":1,"context":{"sizeClass":"huge","canDock":true,"width":0,"height":0}}"#,
            r#"{"kind":"surface","revision":1,"context":{"sizeClass":"compact","width":0,"height":0}}"#,
            r#"{"kind":"language","revision":1,"language":""}"#,
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
}
