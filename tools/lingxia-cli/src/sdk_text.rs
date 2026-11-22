//! A host project's `i18n/<tag>.yaml`: its own words for LingXia's SDK strings, or a language
//! LingXia does not ship. Keys are the SDK's (`file_chooser.filter_label`); a key LingXia no
//! longer has stops the build, and a missing one reads in English.

use crate::r#gen::i18n::{ArgForm, SOURCE_LOCALE, argument_order, flatten_yaml, render};
use anyhow::{Context, Result, bail};
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

/// The SDK's source strings, every scope.
const SDK_SOURCE: &[&str] = &[
    include_str!("../../../i18n/shared/en-US.yaml"),
    include_str!("../../../i18n/permission/runtime/en-US.yaml"),
    include_str!("../../../i18n/error/en-US.yaml"),
    include_str!("../../../i18n/desktop/en-US.yaml"),
    include_str!("../../../i18n/android/en-US.yaml"),
    include_str!("../../../i18n/apple/en-US.yaml"),
];
/// The languages LingXia translates its own strings into.
const SDK_LANGUAGES: &[&str] = &["en-US", "zh-CN"];

/// Language → SDK key (flat, `file_chooser_filter_label`) → ICU text.
#[derive(Debug, Default)]
pub struct SdkText {
    pub languages: BTreeMap<String, BTreeMap<String, String>>,
}

fn sdk_source() -> Result<BTreeMap<String, String>> {
    let mut keys = BTreeMap::new();
    for yaml in SDK_SOURCE {
        let value: serde_yaml_ng::Value = serde_yaml_ng::from_str(yaml)?;
        for (key, value) in flatten_yaml(&value, None) {
            keys.insert(key, value.default);
        }
    }
    Ok(keys)
}

/// The project's overrides, checked against the SDK and `languages`; `None` without `i18n/`.
pub fn load(project_root: &Path, languages: &[String]) -> Result<Option<SdkText>> {
    let dir = project_root.join("i18n");
    if !dir.is_dir() {
        return Ok(None);
    }
    let source = sdk_source()?;
    let mut text = SdkText::default();
    let mut problems = Vec::new();
    for entry in fs::read_dir(&dir)? {
        let path = entry?.path();
        if path.extension().and_then(|e| e.to_str()) != Some("yaml") {
            continue;
        }
        let tag = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or_default();
        if !languages.iter().any(|language| language == tag) {
            problems.push(format!("{tag}: not one of lingxia.yaml's languages"));
            continue;
        }
        let yaml: serde_yaml_ng::Value = serde_yaml_ng::from_str(&fs::read_to_string(&path)?)
            .with_context(|| format!("Failed to read {}", path.display()))?;
        let mut words = BTreeMap::new();
        for (key, value) in flatten_yaml(&yaml, None) {
            let Some(sdk) = source.get(&key) else {
                problems.push(format!("{tag} {key}: LingXia has no such string"));
                continue;
            };
            let signature = |text: &str| {
                lingxia_i18n::parse(text)
                    .map_err(|e| e.to_string())
                    .and_then(|m| m.signature())
            };
            match (signature(sdk), signature(&value.default)) {
                (Ok(want), Ok(got)) => {
                    for drift in lingxia_i18n::signature_drift(&want, &got) {
                        problems.push(format!("{tag} {key}: {drift}"));
                    }
                }
                (_, Err(error)) | (Err(error), _) => problems.push(format!("{tag} {key}: {error}")),
            }
            words.insert(key, value.default);
        }
        text.languages.insert(tag.to_string(), words);
    }
    if !problems.is_empty() {
        bail!("i18n/ overrides:\n  {}", problems.join("\n  "));
    }
    for tag in languages {
        if SDK_LANGUAGES.contains(&tag.as_str()) {
            continue;
        }
        let given = text.languages.get(tag);
        let missing: Vec<&String> = source
            .keys()
            .filter(|key| given.is_none_or(|words| !words.contains_key(*key)))
            .collect();
        if !missing.is_empty() {
            let shown: Vec<&str> = missing.iter().take(5).map(|key| key.as_str()).collect();
            eprintln!(
                "  {tag}: {} LingXia strings missing → shown in {SOURCE_LOCALE}: {}{}",
                missing.len(),
                shown.join(", "),
                if missing.len() > shown.len() {
                    ", …"
                } else {
                    ""
                }
            );
        }
    }
    Ok(Some(text))
}

impl SdkText {
    fn printf(&self, conversion: &'static str) -> Result<BTreeMap<&str, BTreeMap<&str, String>>> {
        let source = sdk_source()?;
        let mut out = BTreeMap::new();
        for (tag, words) in &self.languages {
            let mut converted = BTreeMap::new();
            for (key, text) in words {
                let order = argument_order(&source[key])?;
                converted.insert(
                    key.as_str(),
                    render(text, &order, ArgForm::Printf(conversion))?,
                );
            }
            out.insert(tag.as_str(), converted);
        }
        Ok(out)
    }

    /// `sdkText` in app.json: Logic's and the Windows SDK's strings read from it.
    pub fn app_json(&self) -> Result<serde_json::Value> {
        let source = sdk_source()?;
        let mut out = serde_json::Map::new();
        for (tag, words) in &self.languages {
            let mut converted = serde_json::Map::new();
            for (key, text) in words {
                let order = argument_order(&source[key])?;
                converted.insert(key.clone(), render(text, &order, ArgForm::Icu)?.into());
            }
            out.insert(tag.clone(), converted.into());
        }
        Ok(out.into())
    }

    /// App resources win over the SDK library's of the same name.
    pub fn write_android_overlay(&self, res_dir: &Path) -> Result<()> {
        for (tag, words) in self.printf("s")? {
            let dir = res_dir.join(crate::product_i18n::android_values_dir(tag));
            fs::create_dir_all(&dir)?;
            let strings: String = words
                .iter()
                .map(|(key, text)| {
                    format!(
                        "    <string name=\"lx_{key}\">{}</string>\n",
                        text.replace('\\', "\\\\")
                            .replace('\'', "\\'")
                            .replace('"', "\\\"")
                            .replace('&', "&amp;")
                            .replace('<', "&lt;")
                            .replace('>', "&gt;")
                    )
                })
                .collect();
            fs::write(
                dir.join("lingxia_text.xml"),
                format!(
                    // The default string is the SDK library's, which lint does not see from the app.
                    "<?xml version=\"1.0\" encoding=\"utf-8\"?>\n<resources xmlns:tools=\"http://schemas.android.com/tools\" tools:ignore=\"ExtraTranslation\">\n{strings}</resources>\n"
                ),
            )?;
        }
        Ok(())
    }

    /// The entry module's resources win over the SDK HAR's of the same name.
    pub fn write_harmony_overlay(&self, staging: &Path) -> Result<()> {
        for (tag, words) in self.printf("s")? {
            let locale = crate::product_i18n::harmony_locale_dir(tag);
            let path = staging.join(format!(
                "entry/src/main/resources/{locale}/element/string.json"
            ));
            for (key, text) in words {
                crate::product_i18n::upsert_harmony_string(&path, &format!("lx_{key}"), &text)?;
            }
        }
        Ok(())
    }

    /// Merged into the app's copy of the SDK bundle. A language the SDK lacks starts from its
    /// English strings, as a missing `.strings` key would otherwise read as the key.
    pub fn write_apple_overlay(&self, app_bundle: &Path) -> Result<()> {
        let Some(bundle) = find_dir(app_bundle, "lingxia_lingxia.bundle") else {
            bail!(
                "{} has no lingxia_lingxia.bundle for i18n/ overrides",
                app_bundle.display()
            );
        };
        let resources = if bundle.join("Contents/Resources").is_dir() {
            bundle.join("Contents/Resources")
        } else {
            bundle
        };
        for (tag, words) in self.printf("@")? {
            let Some(lproj) = crate::product_i18n::apple_lproj_dir(tag) else {
                continue;
            };
            let path = resources.join(&lproj).join("Localizable.strings");
            let mut content = if path.is_file() {
                fs::read_to_string(&path)?
            } else {
                fs::read_to_string(resources.join("en.lproj/Localizable.strings"))
                    .unwrap_or_default()
            };
            for (key, text) in words {
                let line = format!(
                    "\"lx_{key}\" = \"{}\";",
                    text.replace('\\', "\\\\")
                        .replace('"', "\\\"")
                        .replace('\n', "\\n")
                );
                let prefix = format!("\"lx_{key}\" =");
                content = match content.lines().position(|l| l.starts_with(&prefix)) {
                    Some(at) => {
                        let mut lines: Vec<&str> = content.lines().collect();
                        lines[at] = &line;
                        lines.join("\n") + "\n"
                    }
                    None => format!("{content}{line}\n"),
                };
            }
            fs::create_dir_all(path.parent().expect("an lproj"))?;
            fs::write(&path, content)?;
        }
        Ok(())
    }
}

fn find_dir(root: &Path, name: &str) -> Option<std::path::PathBuf> {
    for entry in fs::read_dir(root).ok()?.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        if path.file_name().is_some_and(|n| n == name) {
            return Some(path);
        }
        if let Some(found) = find_dir(&path, name) {
            return Some(found);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_override_must_name_a_lingxia_string_and_keep_its_arguments() {
        let dir = tempfile::tempdir().unwrap();
        let languages = vec!["en-US".to_string(), "ja-JP".to_string()];
        fs::create_dir_all(dir.path().join("i18n")).unwrap();
        let write = |yaml: &str| fs::write(dir.path().join("i18n/ja-JP.yaml"), yaml).unwrap();

        write("file_chooser:\n  filter_label: \"表示中：{filter}\"\n");
        let text = load(dir.path(), &languages).unwrap().unwrap();
        let res = dir.path().join("res");
        text.write_android_overlay(&res).unwrap();
        let xml = fs::read_to_string(res.join("values-ja-rJP/lingxia_text.xml")).unwrap();
        assert!(
            xml.contains(r#"<string name="lx_file_chooser_filter_label">表示中：%1$s</string>"#),
            "{xml}"
        );

        write("file_chooser:\n  filter_label: \"表示中\"\n");
        let error = load(dir.path(), &languages).unwrap_err().to_string();
        assert!(error.contains("ja-JP file_chooser_filter_label"), "{error}");

        write("file_chooser:\n  no_such_word: \"x\"\n");
        let error = load(dir.path(), &languages).unwrap_err().to_string();
        assert!(error.contains("LingXia has no such string"), "{error}");
    }
}
