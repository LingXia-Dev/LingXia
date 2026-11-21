//! Checks an lxapp's `i18n/<tag>.json` catalogues against the languages its lxapp.json declares;
//! a build stops on any problem.

use anyhow::{Context, Result, anyhow, bail};
use lingxia_i18n::{Catalogue, Problem};
use std::path::Path;

pub fn check(dir: &Path) -> Result<()> {
    let i18n = dir.join("i18n");
    let manifest = dir.join("lxapp.json");
    if !i18n.is_dir() || !manifest.is_file() {
        return Ok(());
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
        return Ok(());
    }
    report(problems)
}

fn note(reason: impl Into<String>) -> Problem {
    Problem {
        key: None,
        reason: reason.into(),
    }
}

fn report(problems: Vec<(String, Problem)>) -> Result<()> {
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
