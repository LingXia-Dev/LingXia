use serde::Deserialize;

/// Environment the runtime pushes and the platform cannot measure; each kind keeps its newest revision.
#[derive(Default)]
pub(super) struct HostEnvironment {
    language: Option<(u64, String)>,
    surface: Option<(u64, SurfaceContext)>,
}

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
}

impl HostEnvironment {
    /// True when the update is newer than what this host holds.
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
                newer
            }
        })
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
    }
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
        ] {
            assert!(host.update(bad).is_err(), "{bad}");
        }
    }
}
