//! An lxapp's `i18n/<tag>.json` catalogues: checked against the languages its lxapp.json declares,
//! where any problem stops the build, then compiled to `i18n/<tag>.js` in its output.

use anyhow::{Context, Result, anyhow, bail};
use lingxia_i18n::{Catalogue, Problem};
use std::path::Path;

pub fn compile(dir: &Path, output: &Path) -> Result<()> {
    let catalogues = check(dir)?;
    // GPU pages bundle the messages from here; a stale copy would outlive a deleted i18n/.
    let generated = dir.join(".lingxia").join("i18n");
    if generated.exists() {
        std::fs::remove_dir_all(&generated)?;
    }
    let Some((source, _)) = catalogues.first() else {
        return Ok(());
    };
    let source = source.clone();
    let out = output.join("i18n");
    std::fs::create_dir_all(&out)?;
    std::fs::create_dir_all(&generated)?;
    let mut index = String::new();
    let mut entries = Vec::new();
    for (n, (tag, catalogue)) in catalogues.iter().enumerate() {
        let module = lingxia_i18n::js::module(catalogue);
        std::fs::write(out.join(format!("{tag}.js")), &module)?;
        std::fs::write(generated.join(format!("{tag}.js")), &module)?;
        // Native text (Logic's `lx.app.t`, toasts) formats from the catalogue itself.
        std::fs::copy(
            dir.join("i18n").join(format!("{tag}.json")),
            out.join(format!("{tag}.json")),
        )?;
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
    index.push_str(lingxia_i18n::js::FORMATTER);
    std::fs::write(generated.join("index.js"), index)?;
    Ok(())
}

/// The declared languages' catalogues, the source first, each complete and true to the source.
pub fn check(dir: &Path) -> Result<Vec<(String, Catalogue)>> {
    let i18n = dir.join("i18n");
    let manifest = dir.join("lxapp.json");
    if !i18n.is_dir() || !manifest.is_file() {
        return Ok(Vec::new());
    }
    let manifest: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&manifest)?)
        .with_context(|| format!("{} is not JSON", manifest.display()))?;
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
    for (file, tag, catalogue) in &catalogues {
        if tag == source_tag {
            continue;
        }
        let (found, missing) = lingxia_i18n::check(&source.2, catalogue);
        problems.extend(found.into_iter().map(|problem| (file.clone(), problem)));
        if !missing.is_empty() {
            problems.push((
                file.clone(),
                note(format!("untranslated: {}", missing.join(", "))),
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
        check(root).unwrap();
        std::fs::write(root.join("i18n/ja-JP.json"), "{}").unwrap();
        assert!(check(root).is_err());
    }
}
