//! Generated `Settings.bundle`: the app's page in iOS Settings shows the
//! build's version, the way Android's App info shows versionName.

use anyhow::{Context, Result};
use plist::{Dictionary, Value};
use std::fs;
use std::path::Path;

use crate::config::AppEnv;

/// Row titles, localized from `i18n/permission/cli` as `apple.settings_bundle.<title>`.
const TITLES: &[&str] = &["Version", "Environment"];

/// Write `Settings.bundle` into `app_bundle`. Only a `dev` build gets the
/// Environment row: on `prod` it would just read `prod` to every end user.
pub fn write_settings_bundle(app_bundle: &Path, product_version: &str, env: AppEnv) -> Result<()> {
    let bundle = app_bundle.join("Settings.bundle");
    fs::create_dir_all(&bundle)
        .with_context(|| format!("Failed to create {}", bundle.display()))?;

    let version = crate::platform::app_version::os_package_version(product_version)?.marketing;
    Value::Dictionary(root_plist(&version, env))
        .to_file_xml(bundle.join("Root.plist"))
        .context("Failed to write Settings.bundle/Root.plist")?;

    for locale in crate::i18n::supported_locales() {
        let mut strings = String::new();
        for title in TITLES {
            let text = crate::i18n::build_text(locale, &format!("apple.settings_bundle.{title}"))?;
            strings.push_str(&format!("\"{title}\" = \"{text}\";\n"));
        }
        let dir = bundle.join(format!("{locale}.lproj"));
        fs::create_dir_all(&dir).with_context(|| format!("Failed to create {}", dir.display()))?;
        let path = dir.join("Root.strings");
        fs::write(&path, strings).with_context(|| format!("Failed to write {}", path.display()))?;
    }
    Ok(())
}

fn root_plist(version: &str, env: AppEnv) -> Dictionary {
    let mut rows = vec![title_value("Version", "lingxia_settings_version", version)];
    if env == AppEnv::Dev {
        rows.push(title_value(
            "Environment",
            "lingxia_settings_env",
            env.as_str(),
        ));
    }
    let mut root = Dictionary::new();
    root.insert("StringsTable".into(), "Root".into());
    root.insert("PreferenceSpecifiers".into(), Value::Array(rows));
    root
}

/// Read-only row. Settings shows `DefaultValue` while the key is unset in the
/// app's defaults, and nothing writes these keys.
fn title_value(title: &str, key: &str, value: &str) -> Value {
    let mut row = Dictionary::new();
    row.insert("Type".into(), "PSTitleValueSpecifier".into());
    row.insert("Title".into(), title.into());
    row.insert("Key".into(), key.into());
    row.insert("DefaultValue".into(), value.into());
    Value::Dictionary(row)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rows(app: &Path) -> Vec<(String, String)> {
        let root = Value::from_file(app.join("Settings.bundle/Root.plist")).unwrap();
        root.as_dictionary().unwrap()["PreferenceSpecifiers"]
            .as_array()
            .unwrap()
            .iter()
            .map(|row| {
                let row = row.as_dictionary().unwrap();
                (
                    row["Title"].as_string().unwrap().to_string(),
                    row["DefaultValue"].as_string().unwrap().to_string(),
                )
            })
            .collect()
    }

    #[test]
    fn dev_build_shows_version_and_env() {
        let dir = tempfile::tempdir().unwrap();
        write_settings_bundle(dir.path(), "1.2.3", AppEnv::Dev).unwrap();
        assert_eq!(
            rows(dir.path()),
            [
                ("Version".to_string(), "1.2.3".to_string()),
                ("Environment".to_string(), "dev".to_string()),
            ]
        );
        let zh = fs::read_to_string(
            dir.path()
                .join("Settings.bundle/zh-Hans.lproj/Root.strings"),
        )
        .unwrap();
        assert!(zh.contains("\"Environment\" = \"环境\";"));
    }

    #[test]
    fn prod_build_shows_only_version() {
        let dir = tempfile::tempdir().unwrap();
        write_settings_bundle(dir.path(), "2.0.1", AppEnv::Prod).unwrap();
        assert_eq!(
            rows(dir.path()),
            [("Version".to_string(), "2.0.1".to_string())]
        );
    }
}
