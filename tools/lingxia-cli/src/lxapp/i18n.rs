//! An lxapp's `i18n/<tag>.json` catalogues: checked against the languages its lxapp.json declares,
//! then compiled to `i18n/<tag>.js` in its output. A problem stops the build, except that a dev
//! session warns about a translation's key and shows the source message for it.

use anyhow::{Context, Result, anyhow, bail};
use lingxia_i18n::{Catalogue, Problem};
use std::path::Path;

/// `dev` adds `en-XA` (see `lingxia_i18n::pseudo`) and lets a translation fall short per key.
pub fn compile(dir: &Path, output: &Path, dev: bool) -> Result<()> {
    let mut catalogues = check(dir, dev)?;
    // GPU pages bundle the messages from here; a stale copy would outlive a deleted i18n/.
    let generated = dir.join(".lingxia").join("i18n");
    if generated.exists() {
        std::fs::remove_dir_all(&generated)?;
    }
    let types = dir
        .join(".lingxia")
        .join("types")
        .join("i18n.generated.d.ts");
    let Some((source, source_catalogue)) = catalogues.first() else {
        if types.exists() {
            std::fs::remove_file(&types)?;
        }
        return Ok(());
    };
    let source = source.clone();
    let declarations = lingxia_i18n::dts::declarations(source_catalogue);
    let pseudo = dev.then(|| lingxia_i18n::pseudo::catalogue(source_catalogue));
    if std::fs::read_to_string(&types).ok().as_deref() != Some(declarations.as_str()) {
        std::fs::create_dir_all(types.parent().unwrap())?;
        std::fs::write(&types, declarations)?;
    }
    let out = output.join("i18n");
    std::fs::create_dir_all(&out)?;
    std::fs::create_dir_all(&generated)?;
    if let Some(pseudo) = pseudo {
        let json: serde_json::Map<String, serde_json::Value> = pseudo
            .entries
            .iter()
            .map(|(key, entry)| {
                (
                    key.clone(),
                    serde_json::json!({ "defaultMessage": entry.source }),
                )
            })
            .collect();
        std::fs::write(
            out.join(format!("{}.json", lingxia_i18n::pseudo::TAG)),
            serde_json::to_string_pretty(&json)?,
        )?;
        catalogues.push((lingxia_i18n::pseudo::TAG.to_string(), pseudo));
    }
    let mut index = String::new();
    let mut entries = Vec::new();
    for (n, (tag, catalogue)) in catalogues.iter().enumerate() {
        let module = lingxia_i18n::js::module(catalogue);
        std::fs::write(out.join(format!("{tag}.js")), &module)?;
        std::fs::write(generated.join(format!("{tag}.js")), &module)?;
        // Native text (Logic's `lx.app.t`, toasts) formats from the catalogue as checked.
        if tag != lingxia_i18n::pseudo::TAG {
            let json: serde_json::Map<String, serde_json::Value> = catalogue
                .entries
                .iter()
                .map(|(key, entry)| {
                    let mut json = serde_json::json!({ "defaultMessage": entry.source });
                    if let Some(description) = &entry.description {
                        json["description"] = description.clone().into();
                    }
                    (key.clone(), json)
                })
                .collect();
            std::fs::write(
                out.join(format!("{tag}.json")),
                serde_json::to_string_pretty(&json)?,
            )?;
        }
        index.push_str(&format!(
            "import m{n} from './{tag}.js';
"
        ));
        entries.push(format!("{}: m{n}", serde_json::to_string(tag)?));
    }
    index.push_str(&format!(
        "export const source = {};
export const languages = {{ {} }};
",
        serde_json::to_string(&source)?,
        entries.join(", ")
    ));
    let tags: Vec<&str> = catalogues.iter().map(|(tag, _)| tag.as_str()).collect();
    index.push_str(&lingxia_i18n::js::formatter(&tags));
    std::fs::write(generated.join("index.js"), index)?;
    Ok(())
}

/// Appends `tag` to lxapp.json's `languages` and writes an empty `i18n/<tag>.json`.
pub fn add_language(dir: &Path, tag: &str) -> Result<()> {
    let well_formed = !tag.is_empty()
        && tag
            .split('-')
            .all(|part| !part.is_empty() && part.len() <= 8 && part.chars().all(|c| c.is_ascii_alphanumeric()));
    if !well_formed || !tag.as_bytes()[0].is_ascii_alphabetic() {
        bail!("'{tag}' is not a language tag such as `ar-EG`");
    }
    let path = dir.join("lxapp.json");
    let text = std::fs::read_to_string(&path)
        .with_context(|| format!("no lxapp.json in {}", dir.display()))?;
    let mut manifest: serde_json::Value =
        serde_json::from_str(&text).with_context(|| format!("{} is not JSON", path.display()))?;
    let Some(languages) = manifest["languages"].as_array_mut() else {
        bail!("lxapp.json declares no `languages`; add the source language first");
    };
    if languages
        .iter()
        .any(|known| known.as_str().is_some_and(|known| known.eq_ignore_ascii_case(tag)))
    {
        bail!("lxapp.json already declares {tag}");
    }
    let catalogue = dir.join("i18n").join(format!("{tag}.json"));
    if catalogue.exists() {
        bail!("{} already exists", catalogue.display());
    }
    languages.push(tag.into());
    std::fs::create_dir_all(catalogue.parent().unwrap())?;
    std::fs::write(&catalogue, "{}\n")?;
    std::fs::write(&path, serde_json::to_string_pretty(&manifest)? + "\n")?;
    Ok(())
}

/// The first few of `keys`, after their count.
fn untranslated(keys: &[String]) -> String {
    const SHOWN: usize = 5;
    let first = keys[..keys.len().min(SHOWN)].join(", ");
    let more = if keys.len() > SHOWN { ", …" } else { "" };
    format!("untranslated: {} key(s) ({first}{more})", keys.len())
}

/// A dev build's `en-XA` is a language the lxapp has, so the host narrows to it when chosen.
pub fn declare_pseudo(output: &Path) -> Result<()> {
    if !output
        .join("i18n")
        .join(format!("{}.json", lingxia_i18n::pseudo::TAG))
        .is_file()
    {
        return Ok(());
    }
    let path = output.join("lxapp.json");
    let mut manifest: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&path)?)?;
    if let Some(languages) = manifest["languages"].as_array_mut() {
        languages.push(lingxia_i18n::pseudo::TAG.into());
        std::fs::write(&path, serde_json::to_string_pretty(&manifest)?)?;
    }
    Ok(())
}

/// The declared languages' catalogues, the source first, each complete and true to the source.
/// With `dev`, a translation's missing, drifted or unknown key is a warning and takes the source's
/// entry (or is dropped), so every language reads complete.
pub fn check(dir: &Path, dev: bool) -> Result<Vec<(String, Catalogue)>> {
    let i18n = dir.join("i18n");
    let manifest = dir.join("lxapp.json");
    if !manifest.is_file() {
        return Ok(Vec::new());
    }
    let manifest: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&manifest)?)
        .with_context(|| format!("{} is not JSON", manifest.display()))?;
    let mut named = manifest_keys(dir, &manifest)?;
    named.extend(code_keys(dir)?);
    if !i18n.is_dir() {
        if let Some((file, key)) = named.first() {
            bail!("{file} names the message '{key}', but the lxapp has no i18n/ catalogues");
        }
        return Ok(Vec::new());
    }
    let languages: Vec<String> = manifest["languages"]
        .as_array()
        .map(|tags| {
            tags.iter()
                .filter_map(|tag| tag.as_str())
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default();
    let Some(source_tag) = languages.first() else {
        bail!(
            "i18n/ has catalogues but lxapp.json declares no `languages`; the first one is the source"
        );
    };

    let mut problems: Vec<(String, Problem)> = Vec::new();
    let mut catalogues = Vec::new();
    let mut files: Vec<_> = std::fs::read_dir(&i18n)?
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .filter(|path| path.extension().is_some_and(|ext| ext == "json"))
        .collect();
    files.sort();
    for path in files {
        let file = format!(
            "i18n/{}",
            path.file_name().unwrap_or_default().to_string_lossy()
        );
        let stem = path.file_stem().unwrap_or_default().to_string_lossy();
        let Some(tag) = languages.iter().find(|tag| tag.eq_ignore_ascii_case(&stem)) else {
            problems.push((file, note("not one of the languages lxapp.json declares")));
            continue;
        };
        let is_source = tag == source_tag;
        match Catalogue::parse(&std::fs::read_to_string(&path)?, is_source) {
            Ok(catalogue) => catalogues.push((file, tag.clone(), catalogue)),
            Err(found) => problems.extend(found.into_iter().map(|problem| (file.clone(), problem))),
        }
    }

    let Some(source) = catalogues.iter().find(|(_, tag, _)| tag == source_tag) else {
        if problems.is_empty() {
            bail!("i18n/{source_tag}.json is missing: the first declared language is the source");
        }
        return report(problems);
    };
    let source_catalogue = source.2.clone();
    for (file, tag, catalogue) in &mut catalogues {
        if tag == source_tag {
            continue;
        }
        let (found, missing) = lingxia_i18n::check(&source_catalogue, catalogue);
        if dev {
            for problem in &found {
                eprintln!("warning: {file}: {problem}; showing the source message");
            }
            if !missing.is_empty() {
                eprintln!(
                    "warning: {file}: {}; showing the source messages",
                    untranslated(&missing)
                );
            }
            for key in found.iter().filter_map(|problem| problem.key.as_ref()).chain(&missing) {
                match source_catalogue.entries.get(key) {
                    Some(entry) => catalogue.entries.insert(key.clone(), entry.clone()),
                    None => catalogue.entries.remove(key),
                };
            }
            continue;
        }
        problems.extend(found.into_iter().map(|problem| (file.clone(), problem)));
        if !missing.is_empty() {
            problems.push((file.clone(), note(untranslated(&missing))));
        }
    }
    for (file, key) in named {
        if !source_catalogue.entries.contains_key(&key) {
            problems.push((
                file,
                note(format!("names '{key}', which i18n/{source_tag}.json lacks")),
            ));
        }
    }
    if problems.is_empty() {
        let source_tag = source_tag.clone();
        let mut catalogues: Vec<(String, Catalogue)> = catalogues
            .into_iter()
            .map(|(_, tag, catalogue)| (tag, catalogue))
            .collect();
        catalogues.sort_by_key(|(tag, _)| *tag != source_tag);
        return Ok(catalogues);
    }
    report(problems)
}

/// The keys page and Logic code passes to `t` or `lx.app.t` as a literal, with the file naming each.
fn code_keys(dir: &Path) -> Result<Vec<(String, String)>> {
    const SKIP: [&str; 5] = ["node_modules", "dist", "tests", "public", "i18n"];
    let mut named = Vec::new();
    let mut pending = vec![dir.to_path_buf()];
    while let Some(next) = pending.pop() {
        for entry in std::fs::read_dir(&next)?.flatten() {
            let path = entry.path();
            let name = entry.file_name().to_string_lossy().into_owned();
            if path.is_dir() {
                if !name.starts_with('.') && !SKIP.contains(&name.as_str()) {
                    pending.push(path);
                }
            } else if [".ts", ".tsx", ".js", ".jsx"]
                .iter()
                .any(|ext| name.ends_with(ext))
                && !name.ends_with(".d.ts")
            {
                let file = path
                    .strip_prefix(dir)
                    .unwrap_or(&path)
                    .to_string_lossy()
                    .replace('\\', "/");
                let text = std::fs::read_to_string(&path).unwrap_or_default();
                named.extend(
                    literal_keys(&text)
                        .into_iter()
                        .map(|key| (file.clone(), key)),
                );
            }
        }
    }
    named.sort();
    Ok(named)
}

/// `lx.app.t('key'`, and `t('key'` where `t` is the View's: a quoted first argument that is a key.
fn literal_keys(text: &str) -> Vec<String> {
    let views_t = imports_views_t(text);
    let mut keys = Vec::new();
    for (at, _) in text.match_indices("t(") {
        let before = &text[..at];
        let standalone = before
            .chars()
            .next_back()
            .is_none_or(|c| !(c.is_alphanumeric() || c == '_' || c == '$' || c == '.'));
        if !(standalone && views_t || before.ends_with("lx.app.")) {
            continue;
        }
        let rest = &text[at + 2..];
        let Some(quote) = rest.chars().next().filter(|c| *c == '\'' || *c == '"') else {
            continue;
        };
        let Some(end) = rest[1..].find(quote) else {
            continue;
        };
        let key = &rest[1..1 + end];
        if lingxia_i18n::valid_key(key) {
            keys.push(key.to_string());
        }
    }
    keys
}

/// Whether the file imports the lxapp's `t` (`useText` in React); another `t` is not the lxapp's.
fn imports_views_t(text: &str) -> bool {
    [
        ("@lingxia/view", "t"),
        ("@lingxia/bridge", "t"),
        ("@lingxia/react", "useText"),
    ]
    .iter()
    .any(|(module, binding)| {
        text.match_indices(&format!("from '{module}'"))
            .chain(text.match_indices(&format!("from \"{module}\"")))
            .any(|(at, _)| {
                let head = &text[..at];
                let open = head.rfind('{').unwrap_or(at);
                head[open..]
                    .trim_start_matches('{')
                    .split([',', '}'])
                    .map(|name| name.trim().trim_start_matches("type "))
                    .any(|name| name == *binding)
            })
    })
}

/// The catalogue keys tab labels and page titles name, with the file naming each.
fn manifest_keys(dir: &Path, manifest: &serde_json::Value) -> Result<Vec<(String, String)>> {
    let key = |text: &serde_json::Value| text["message"].as_str().map(str::to_string);
    let mut named = Vec::new();
    for item in manifest["tabBar"]["items"].as_array().into_iter().flatten() {
        if let Some(found) = key(&item["text"]) {
            named.push(("lxapp.json".to_string(), found));
        }
    }
    for page in manifest["pages"].as_array().into_iter().flatten() {
        let Some(path) = page["path"].as_str() else {
            continue;
        };
        let file = format!("{}.json", path.trim_end_matches(".js"));
        let Ok(text) = std::fs::read_to_string(dir.join(&file)) else {
            continue;
        };
        let config: serde_json::Value =
            serde_json::from_str(&text).with_context(|| format!("{file} is not JSON"))?;
        if let Some(found) = key(&config["navigationBar"]["title"]) {
            named.push((file, found));
        }
    }
    Ok(named)
}

fn note(reason: impl Into<String>) -> Problem {
    Problem {
        key: None,
        reason: reason.into(),
    }
}

fn report<T>(problems: Vec<(String, Problem)>) -> Result<T> {
    for (file, problem) in &problems {
        eprintln!("{file}: {problem}");
    }
    Err(anyhow!("{} catalogue problem(s)", problems.len()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_catalogue_outside_the_declared_languages_stops_the_build() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        std::fs::create_dir(root.join("i18n")).unwrap();
        std::fs::write(
            root.join("lxapp.json"),
            r#"{ "languages": ["zh-CN", "en-US"] }"#,
        )
        .unwrap();
        let source =
            r#"{ "cart.title": { "defaultMessage": "购物车", "description": "Heading." } }"#;
        std::fs::write(root.join("i18n/zh-CN.json"), source).unwrap();
        std::fs::write(
            root.join("i18n/en-US.json"),
            r#"{ "cart.title": { "defaultMessage": "Cart" } }"#,
        )
        .unwrap();
        check(root, false).unwrap();
        std::fs::write(root.join("i18n/ja-JP.json"), "{}").unwrap();
        assert!(check(root, false).is_err());
    }

    #[test]
    fn literal_keys_are_those_t_and_lx_app_t_are_called_with() {
        let code = "import { t, useView } from '@lingxia/view';
t('cart.title'); lx.app.t(\"cart.items\", { count }); format('not.this'); x.t('nor.this'); t(`a.${b}`); t('Not a key')";
        assert_eq!(literal_keys(code), ["cart.title", "cart.items"]);
        assert_eq!(
            literal_keys("const t = (key) => key; t('own.key'); lx.app.t('app.key')"),
            ["app.key"]
        );
        assert_eq!(
            literal_keys(
                "import { useText } from \"@lingxia/react\";
const t = useText(); t('web.key')"
            ),
            ["web.key"]
        );
    }

    #[test]
    fn a_dev_build_adds_en_xa_true_to_the_source() {
        let dir = tempfile::tempdir().unwrap();
        let (root, output) = (dir.path().join("app"), dir.path().join("dist"));
        std::fs::create_dir_all(root.join("i18n")).unwrap();
        std::fs::write(root.join("lxapp.json"), r#"{ "languages": ["en-US"] }"#).unwrap();
        std::fs::write(
            root.join("i18n/en-US.json"),
            r#"{ "cart.items": { "defaultMessage": "{n, plural, one {# item} other {# items}}", "description": "Count." } }"#,
        )
        .unwrap();
        compile(&root, &output, true).unwrap();
        let json = std::fs::read_to_string(output.join("i18n/en-XA.json")).unwrap();
        let pseudo = lingxia_i18n::Catalogue::parse(&json, false).unwrap();
        let source = lingxia_i18n::Catalogue::parse(
            &std::fs::read_to_string(root.join("i18n/en-US.json")).unwrap(),
            true,
        )
        .unwrap();
        assert_eq!(
            lingxia_i18n::check(&source, &pseudo),
            (Vec::new(), Vec::new())
        );
        let index = std::fs::read_to_string(root.join(".lingxia/i18n/index.js")).unwrap();
        assert!(index.contains("\"en-XA\": m1"), "{index}");

        std::fs::write(output.join("lxapp.json"), r#"{ "languages": ["en-US"] }"#).unwrap();
        declare_pseudo(&output).unwrap();
        let manifest = std::fs::read_to_string(output.join("lxapp.json")).unwrap();
        assert!(manifest.contains("en-XA"));

        compile(&root, &output.join("release"), false).unwrap();
        assert!(!output.join("release/i18n/en-XA.json").exists());
    }

    #[test]
    fn a_tab_label_naming_an_unknown_key_stops_the_build() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        std::fs::create_dir(root.join("i18n")).unwrap();
        let manifest = |key: &str| {
            format!(
                r#"{{ "languages": ["en-US"], "tabBar": {{ "items": [{{ "page": "home", "text": {{ "message": "{key}" }} }}] }} }}"#
            )
        };
        std::fs::write(
            root.join("i18n/en-US.json"),
            r#"{ "tab.home": { "defaultMessage": "Home", "description": "Tab." } }"#,
        )
        .unwrap();
        std::fs::write(root.join("lxapp.json"), manifest("tab.home")).unwrap();
        check(root, false).unwrap();
        std::fs::write(root.join("lxapp.json"), manifest("tab.cart")).unwrap();
        assert!(check(root, true).is_err(), "a dev session has no source to fall back to");
    }

    #[test]
    fn an_added_language_is_declared_and_left_to_translate() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        std::fs::create_dir(root.join("i18n")).unwrap();
        std::fs::write(root.join("lxapp.json"), r#"{ "languages": ["en-US"] }"#).unwrap();
        let keys: String = (1..=7)
            .map(|n| format!(r#""k.m{n}": {{ "defaultMessage": "M{n}", "description": "d" }}"#))
            .collect::<Vec<_>>()
            .join(", ");
        std::fs::write(root.join("i18n/en-US.json"), format!("{{ {keys} }}")).unwrap();
        add_language(root, "ar-EG").unwrap();
        assert!(add_language(root, "AR-eg").is_err(), "already declared");
        assert!(add_language(root, "not a tag").is_err());
        let manifest = std::fs::read_to_string(root.join("lxapp.json")).unwrap();
        assert!(manifest.contains("ar-EG"), "{manifest}");
        assert!(check(root, true).is_ok());
        assert!(check(root, false).is_err());
        let left = untranslated(&(1..=7).map(|n| format!("k.m{n}")).collect::<Vec<_>>());
        assert_eq!(left, "untranslated: 7 key(s) (k.m1, k.m2, k.m3, k.m4, k.m5, …)");
    }

    #[test]
    fn a_dev_session_shows_the_source_for_a_translation_gap_and_a_release_stops() {
        let dir = tempfile::tempdir().unwrap();
        let (root, output) = (dir.path().join("app"), dir.path().join("dist"));
        std::fs::create_dir_all(root.join("i18n")).unwrap();
        std::fs::write(
            root.join("lxapp.json"),
            r#"{ "languages": ["en-US", "de-DE"] }"#,
        )
        .unwrap();
        std::fs::write(
            root.join("i18n/en-US.json"),
            r#"{ "cart.title": { "defaultMessage": "Cart", "description": "Heading." },
                 "cart.items": { "defaultMessage": "{n, plural, one {# item} other {# items}}", "description": "Count." },
                 "cart.empty": { "defaultMessage": "Empty", "description": "No items." } }"#,
        )
        .unwrap();
        std::fs::write(
            root.join("i18n/de-DE.json"),
            r#"{ "cart.title": { "defaultMessage": "Warenkorb" },
                 "cart.items": { "defaultMessage": "{count} Artikel" },
                 "cart.old": { "defaultMessage": "Alt" } }"#,
        )
        .unwrap();
        assert!(compile(&root, &output, false).is_err());

        compile(&root, &output, true).unwrap();
        let json = std::fs::read_to_string(output.join("i18n/de-DE.json")).unwrap();
        let german = lingxia_i18n::Catalogue::parse(&json, false).unwrap();
        let text = |key: &str| german.entries[key].source.as_str();
        assert_eq!(text("cart.title"), "Warenkorb");
        assert_eq!(text("cart.items"), "{n, plural, one {# item} other {# items}}");
        assert_eq!(text("cart.empty"), "Empty");
        assert!(!german.entries.contains_key("cart.old"));
        let module = std::fs::read_to_string(output.join("i18n/de-DE.js")).unwrap();
        assert!(module.contains("Empty") && !module.contains("Alt"), "{module}");
    }
}
