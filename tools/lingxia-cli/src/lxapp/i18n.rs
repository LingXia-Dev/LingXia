//! An lxapp's `i18n/<tag>.json` catalogues: checked against the languages its lxapp.json declares,
//! then compiled to `i18n/<tag>.js` in its output. A problem stops the build, except that a dev
//! session warns about a translation's key and shows the source message for it.

use anyhow::{Context, Result, anyhow, bail};
use lingxia_i18n::{Catalogue, Problem};
use std::path::{Path, PathBuf};

/// `dev` adds `en-XA` and `ar-XB` (see `lingxia_i18n::pseudo`) and lets a translation fall short
/// per key. Returns the compiled `index.js` GPU pages bundle, `None` without catalogues.
pub fn compile(dir: &Path, output: &Path, dev: bool) -> Result<Option<PathBuf>> {
    let mut catalogues = check(dir, dev)?;
    let types = dir
        .join(".lingxia")
        .join("types")
        .join("i18n.generated.d.ts");
    let Some((source, source_catalogue)) = catalogues.first() else {
        if types.exists() {
            std::fs::remove_file(&types)?;
        }
        generated::clear(&generated::mode_dir(dir, dev))?;
        return Ok(None);
    };
    let source = source.clone();
    let declarations = lingxia_i18n::dts::declarations(source_catalogue);
    let pseudo = if dev {
        vec![
            (
                lingxia_i18n::pseudo::TAG,
                lingxia_i18n::pseudo::catalogue(source_catalogue),
            ),
            (
                lingxia_i18n::pseudo::RTL_TAG,
                lingxia_i18n::pseudo::rtl_catalogue(source_catalogue),
            ),
        ]
    } else {
        Vec::new()
    };
    if std::fs::read_to_string(&types).ok().as_deref() != Some(declarations.as_str()) {
        std::fs::create_dir_all(types.parent().unwrap())?;
        std::fs::write(&types, declarations)?;
    }
    let out = output.join("i18n");
    std::fs::create_dir_all(&out)?;
    // The modules GPU pages bundle, published whole once all are known.
    let mut modules: Vec<(String, String)> = Vec::new();
    for (tag, pseudo) in pseudo {
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
            out.join(format!("{tag}.json")),
            serde_json::to_string_pretty(&json)?,
        )?;
        catalogues.push((tag.to_string(), pseudo));
    }
    for (tag, catalogue) in &catalogues {
        let module = lingxia_i18n::js::module(catalogue);
        std::fs::write(out.join(format!("{tag}.js")), &module)?;
        modules.push((format!("{tag}.js"), module));
        // Native text (Logic's `lx.app.t`, toasts) formats from the catalogue as checked.
        if !lingxia_i18n::pseudo::TAGS.contains(&tag.as_str()) {
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
    }
    let tags: Vec<&str> = catalogues.iter().map(|(tag, _)| tag.as_str()).collect();
    let index = lingxia_i18n::js::index(&source, &tags);
    modules.push(("index.js".to_string(), index));
    let published = generated::publish(&generated::mode_dir(dir, dev), &modules)?;
    Ok(Some(published.join("index.js")))
}

/// The compiled `index.js` of the lxapp at `dir` its last build in this mode published.
pub fn compiled_messages(dir: &Path, dev: bool) -> Option<PathBuf> {
    generated::current(&generated::mode_dir(dir, dev)).map(|published| published.join("index.js"))
}

/// One copy of the compiled modules per mode: `<mode>/<content hash>/` and a `current` file naming
/// it. Sessions and builds of one lxapp write concurrently; a content dir is complete once it has
/// its name and the pointer is replaced by a rename, so a reader never sees a partial copy.
mod generated {
    use anyhow::Result;
    use sha2::Digest;
    use std::path::{Path, PathBuf};

    const POINTER: &str = "current";

    pub(super) fn mode_dir(dir: &Path, dev: bool) -> PathBuf {
        dir.join(".lingxia")
            .join("i18n")
            .join(if dev { "dev" } else { "build" })
    }

    pub(super) fn current(mode: &Path) -> Option<PathBuf> {
        let name = std::fs::read_to_string(mode.join(POINTER)).ok()?;
        let published = mode.join(name.trim());
        published.is_dir().then_some(published)
    }

    /// Publish `files` as this mode's copy; nothing is written when it already is.
    pub(super) fn publish(mode: &Path, files: &[(String, String)]) -> Result<PathBuf> {
        let mut hasher = sha2::Sha256::new();
        for (name, text) in files {
            for part in [name.as_bytes(), text.as_bytes()] {
                hasher.update((part.len() as u64).to_le_bytes());
                hasher.update(part);
            }
        }
        let name = hex(&hasher.finalize()[..12]);
        let published = mode.join(&name);
        let previous = std::fs::read_to_string(mode.join(POINTER)).ok();
        write(mode, &published, files)?;
        if previous.as_deref().map(str::trim) != Some(name.as_str()) {
            let temp = mode.join(format!(".{POINTER}-{}", unique()));
            std::fs::write(&temp, &name)?;
            replace(&temp, &mode.join(POINTER))?;
            prune(mode, &name, previous.as_deref().map(str::trim));
            // Another writer's prune may have taken the copy before the pointer named it.
            write(mode, &published, files)?;
        }
        Ok(published)
    }

    /// `published` with `files`, made complete under another name and renamed into place.
    fn write(mode: &Path, published: &Path, files: &[(String, String)]) -> Result<()> {
        if published.is_dir() {
            return Ok(());
        }
        let staging = mode.join(format!(".staging-{}", unique()));
        std::fs::create_dir_all(&staging)?;
        for (file, text) in files {
            std::fs::write(staging.join(file), text)?;
        }
        // Another writer may have published the same content first.
        let renamed = std::fs::rename(&staging, published);
        let _ = std::fs::remove_dir_all(&staging);
        if renamed.is_err() && !published.is_dir() {
            anyhow::bail!("could not publish {}", published.display());
        }
        Ok(())
    }

    /// Without catalogues there is nothing to bundle in this mode.
    pub(super) fn clear(mode: &Path) -> Result<()> {
        match std::fs::remove_file(mode.join(POINTER)) {
            Err(error) if error.kind() != std::io::ErrorKind::NotFound => Err(error.into()),
            _ => Ok(()),
        }
    }

    /// A build that resolved the last pointer may still be reading its copy; older ones go.
    fn prune(mode: &Path, keep: &str, previous: Option<&str>) {
        let Ok(entries) = std::fs::read_dir(mode) else {
            return;
        };
        for entry in entries.flatten() {
            let name = entry.file_name();
            let name = name.to_string_lossy();
            let stale = name != keep && Some(&*name) != previous && !name.starts_with('.');
            if stale && entry.path().is_dir() {
                let _ = std::fs::remove_dir_all(entry.path());
            }
        }
    }

    /// A rename over the pointer; Windows refuses one while a reader has the file open.
    fn replace(from: &Path, to: &Path) -> Result<()> {
        let mut delay = std::time::Duration::from_millis(10);
        loop {
            match std::fs::rename(from, to) {
                Ok(()) => return Ok(()),
                Err(error)
                    if cfg!(windows)
                        && error.kind() == std::io::ErrorKind::PermissionDenied
                        && delay < std::time::Duration::from_secs(1) =>
                {
                    std::thread::sleep(delay);
                    delay *= 2;
                }
                Err(error) => {
                    let _ = std::fs::remove_file(from);
                    return Err(error.into());
                }
            }
        }
    }

    fn unique() -> String {
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        format!(
            "{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        )
    }

    fn hex(bytes: &[u8]) -> String {
        bytes.iter().map(|byte| format!("{byte:02x}")).collect()
    }
}

/// Appends `tag` to lxapp.json's `languages` and writes an empty `i18n/<tag>.json`.
pub fn add_language(dir: &Path, tag: &str) -> Result<()> {
    let well_formed = !tag.is_empty()
        && tag.split('-').all(|part| {
            !part.is_empty() && part.len() <= 8 && part.chars().all(|c| c.is_ascii_alphanumeric())
        });
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
    if languages.iter().any(|known| {
        known
            .as_str()
            .is_some_and(|known| known.eq_ignore_ascii_case(tag))
    }) {
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

/// A dev build's pseudo-languages are languages the lxapp has, so the host narrows to one when chosen.
pub fn declare_pseudo(output: &Path) -> Result<()> {
    let built: Vec<&str> = lingxia_i18n::pseudo::TAGS
        .into_iter()
        .filter(|tag| output.join("i18n").join(format!("{tag}.json")).is_file())
        .collect();
    if built.is_empty() {
        return Ok(());
    }
    let path = output.join("lxapp.json");
    let mut manifest: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&path)?)?;
    if let Some(languages) = manifest["languages"].as_array_mut() {
        languages.extend(built.into_iter().map(serde_json::Value::from));
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
    let packages = package_catalogues(dir)?;
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
            Ok(catalogue) => {
                problems.extend(
                    overrides(&catalogue, &packages)
                        .into_iter()
                        .map(|problem| (file.clone(), problem)),
                );
                catalogues.push((file, tag.clone(), catalogue));
            }
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
        let (found, mut missing) = lingxia_i18n::check(&source_catalogue, catalogue);
        // An override of a package's word is the app's choice per language.
        missing.retain(|key| !key.starts_with('@'));
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
            for key in found
                .iter()
                .filter_map(|problem| problem.key.as_ref())
                .chain(&missing)
            {
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

/// The source catalogue of each installed dependency that declares one
/// (`"lingxia": { "messages": "<path>" }` in its package.json), by its keys' `@namespace`.
fn package_catalogues(dir: &Path) -> Result<std::collections::BTreeMap<String, Catalogue>> {
    let mut found = std::collections::BTreeMap::new();
    let Ok(text) = std::fs::read_to_string(dir.join("package.json")) else {
        return Ok(found);
    };
    let manifest: serde_json::Value =
        serde_json::from_str(&text).context("package.json is not JSON")?;
    let names = ["dependencies", "devDependencies"]
        .iter()
        .filter_map(|field| manifest[field].as_object())
        .flat_map(|deps| deps.keys().cloned());
    for name in names {
        let root = dir.join("node_modules").join(&name);
        let Ok(text) = std::fs::read_to_string(root.join("package.json")) else {
            continue;
        };
        let package: serde_json::Value = serde_json::from_str(&text)
            .with_context(|| format!("node_modules/{name}/package.json is not JSON"))?;
        let Some(messages) = package["lingxia"]["messages"].as_str() else {
            continue;
        };
        let path = root.join(messages);
        let text = std::fs::read_to_string(&path)
            .with_context(|| format!("{name} declares messages at {messages}, which it lacks"))?;
        let catalogue = Catalogue::parse(&text, true)
            .map_err(|problems| anyhow!("{name}'s {messages}: {}", problems[0]))?;
        let Some(namespace) = catalogue
            .entries
            .keys()
            .next()
            .and_then(|key| namespace(key))
        else {
            continue;
        };
        found.insert(namespace.to_string(), catalogue);
    }
    Ok(found)
}

/// `@view` of `@view.picker.done`.
fn namespace(key: &str) -> Option<&str> {
    key.starts_with('@')
        .then(|| key.split('.').next())
        .flatten()
}

/// Each `@namespace.*` key must be a word of an installed package, with its arguments.
fn overrides(
    catalogue: &Catalogue,
    packages: &std::collections::BTreeMap<String, Catalogue>,
) -> Vec<Problem> {
    let mut problems = Vec::new();
    for (key, entry) in &catalogue.entries {
        let Some(space) = namespace(key) else {
            continue;
        };
        let at = |reason: String| Problem {
            key: Some(key.clone()),
            reason,
        };
        let Some(package) = packages.get(space) else {
            let all = packages
                .values()
                .flat_map(|catalogue| catalogue.entries.keys());
            problems.push(at(match nearest(key, all) {
                Some(near) => {
                    format!("no installed package has {space} words; did you mean '{near}'?")
                }
                None => format!("no installed package has {space} words"),
            }));
            continue;
        };
        match package.entries.get(key) {
            Some(word) => problems.extend(
                lingxia_i18n::signature_drift(&word.signature, &entry.signature)
                    .into_iter()
                    .map(|drift| at(format!("{drift} (the package's word)"))),
            ),
            None => problems.push(at(match nearest(key, package.entries.keys()) {
                Some(near) => format!("not a {space} word; did you mean '{near}'?"),
                None => format!("not a {space} word"),
            })),
        }
    }
    problems
}

/// The candidate closest to `key` by edit distance.
fn nearest<'a>(key: &str, candidates: impl Iterator<Item = &'a String>) -> Option<&'a str> {
    let distance = |a: &str, b: &str| {
        let b: Vec<char> = b.chars().collect();
        let mut row: Vec<usize> = (0..=b.len()).collect();
        for (i, ca) in a.chars().enumerate() {
            let mut previous = row[0];
            row[0] = i + 1;
            for (j, cb) in b.iter().enumerate() {
                let next = (row[j + 1] + 1)
                    .min(row[j] + 1)
                    .min(previous + usize::from(ca != *cb));
                previous = row[j + 1];
                row[j + 1] = next;
            }
        }
        row[b.len()]
    };
    candidates
        .min_by_key(|candidate| distance(key, candidate))
        .map(String::as_str)
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
    fn a_dev_build_adds_the_pseudo_languages_true_to_the_source() {
        let dir = tempfile::tempdir().unwrap();
        let (root, output) = (dir.path().join("app"), dir.path().join("dist"));
        std::fs::create_dir_all(root.join("i18n")).unwrap();
        std::fs::write(root.join("lxapp.json"), r#"{ "languages": ["en-US"] }"#).unwrap();
        std::fs::write(
            root.join("i18n/en-US.json"),
            r#"{ "cart.items": { "defaultMessage": "{n, plural, one {# item} other {# items}}", "description": "Count." } }"#,
        )
        .unwrap();
        let messages = compile(&root, &output, true).unwrap().unwrap();
        assert_eq!(compiled_messages(&root, true), Some(messages.clone()));
        let source = lingxia_i18n::Catalogue::parse(
            &std::fs::read_to_string(root.join("i18n/en-US.json")).unwrap(),
            true,
        )
        .unwrap();
        let index = std::fs::read_to_string(&messages).unwrap();
        for (n, tag) in ["en-XA", "ar-XB"].into_iter().enumerate() {
            let json = std::fs::read_to_string(output.join(format!("i18n/{tag}.json"))).unwrap();
            let pseudo = lingxia_i18n::Catalogue::parse(&json, false).unwrap();
            assert_eq!(
                lingxia_i18n::check(&source, &pseudo),
                (Vec::new(), Vec::new()),
                "{tag}"
            );
            assert!(index.contains(&format!("\"{tag}\": m{}", n + 1)), "{index}");
        }

        std::fs::write(output.join("lxapp.json"), r#"{ "languages": ["en-US"] }"#).unwrap();
        declare_pseudo(&output).unwrap();
        let manifest: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(output.join("lxapp.json")).unwrap())
                .unwrap();
        assert_eq!(
            manifest["languages"],
            serde_json::json!(["en-US", "en-XA", "ar-XB"])
        );

        compile(&root, &output.join("release"), false).unwrap();
        assert!(!output.join("release/i18n/en-XA.json").exists());
        assert!(!output.join("release/i18n/ar-XB.json").exists());
    }

    /// An lxapp at `root` whose source catalogue says `title`.
    fn catalogued(root: &Path, title: &str) {
        std::fs::create_dir_all(root.join("i18n")).unwrap();
        std::fs::write(root.join("lxapp.json"), r#"{ "languages": ["en-US"] }"#).unwrap();
        std::fs::write(
            root.join("i18n/en-US.json"),
            format!(
                r#"{{ "app.title": {{ "defaultMessage": "{title}", "description": "Title." }} }}"#
            ),
        )
        .unwrap();
    }

    /// A dev session compiling the same lxapp before and after a build leaves the build's
    /// modules and output free of the pseudo-languages.
    #[test]
    fn a_build_never_bundles_or_emits_the_pseudo_languages() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("app");
        catalogued(&root, "Shop");
        compile(&root, &dir.path().join("session"), true).unwrap();
        let built = compile(&root, &dir.path().join("dist"), false)
            .unwrap()
            .unwrap();
        compile(&root, &dir.path().join("session"), true).unwrap();
        for module in [built.clone(), compiled_messages(&root, false).unwrap()] {
            let index = std::fs::read_to_string(&module).unwrap();
            assert!(
                !index.contains("en-XA") && !index.contains("ar-XB"),
                "{index}"
            );
            assert!(!module.with_file_name("en-XA.js").exists());
        }
        let emitted: Vec<String> = std::fs::read_dir(dir.path().join("dist/i18n"))
            .unwrap()
            .flatten()
            .map(|entry| entry.file_name().to_string_lossy().into_owned())
            .collect();
        assert!(
            emitted.iter().all(|name| name.starts_with("en-US.")),
            "{emitted:?}"
        );
    }

    #[test]
    fn unchanged_messages_are_not_written_again() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("app");
        catalogued(&root, "Shop");
        let first = compile(&root, &dir.path().join("dist"), false)
            .unwrap()
            .unwrap();
        let pointer = root.join(".lingxia/i18n/build/current");
        let stamp = |path: &Path| std::fs::metadata(path).unwrap().modified().unwrap();
        let (module, current) = (stamp(&first), stamp(&pointer));
        std::thread::sleep(std::time::Duration::from_millis(20));
        assert_eq!(
            compile(&root, &dir.path().join("dist"), false)
                .unwrap()
                .unwrap(),
            first
        );
        assert_eq!((stamp(&first), stamp(&pointer)), (module, current));

        catalogued(&root, "Store");
        let second = compile(&root, &dir.path().join("dist"), false)
            .unwrap()
            .unwrap();
        assert_ne!(second, first);
        // The copy a build may still be reading stays until the next change.
        assert!(first.is_file());
        catalogued(&root, "Market");
        compile(&root, &dir.path().join("dist"), false).unwrap();
        assert!(!first.exists() && second.is_file());
    }

    /// Two sessions publishing different messages while a third reads: the reader always finds
    /// one complete copy.
    #[test]
    fn two_writers_and_a_reader_never_meet_a_partial_copy() {
        let dir = tempfile::tempdir().unwrap();
        let mode = dir.path().join("dev");
        let copy = |text: &str| -> Vec<(String, String)> {
            (0..8)
                .map(|n| (format!("m{n}.js"), text.repeat(4096)))
                .chain([("index.js".to_string(), text.to_string())])
                .collect()
        };
        let copies = [copy("a"), copy("b")];
        generated::publish(&mode, &copies[0]).unwrap();
        let start = std::sync::Barrier::new(3);
        std::thread::scope(|scope| {
            for writer in 0..2 {
                let (mode, copies, start) = (&mode, &copies, &start);
                scope.spawn(move || {
                    start.wait();
                    for round in 0..40 {
                        generated::publish(mode, &copies[(writer + round) % 2]).unwrap();
                    }
                });
            }
            let (mode, start) = (&mode, &start);
            scope.spawn(move || {
                start.wait();
                for _ in 0..200 {
                    // A copy pruned under the reader is resolved again, as a build retrying would.
                    let Some(published) = generated::current(mode) else {
                        panic!("no current copy");
                    };
                    let Ok(index) = std::fs::read_to_string(published.join("index.js")) else {
                        continue;
                    };
                    let Ok(module) = std::fs::read_to_string(published.join("m7.js")) else {
                        continue;
                    };
                    assert_eq!(module, index.repeat(4096), "a torn copy");
                }
            });
        });
        let leftovers: Vec<_> = std::fs::read_dir(&mode)
            .unwrap()
            .flatten()
            .map(|entry| entry.file_name().to_string_lossy().into_owned())
            .filter(|name| name.starts_with('.'))
            .collect();
        assert!(leftovers.is_empty(), "{leftovers:?}");
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
        assert!(
            check(root, true).is_err(),
            "a dev session has no source to fall back to"
        );
    }

    /// An app at `root` with one installed package whose words are `@kit.ok` and `@kit.pageOf`.
    fn app_with_package(root: &Path) {
        std::fs::create_dir_all(root.join("i18n")).unwrap();
        std::fs::create_dir_all(root.join("node_modules/@acme/kit/messages")).unwrap();
        std::fs::write(
            root.join("package.json"),
            r#"{ "dependencies": { "@acme/kit": "1.0.0" } }"#,
        )
        .unwrap();
        std::fs::write(
            root.join("node_modules/@acme/kit/package.json"),
            r#"{ "name": "@acme/kit", "lingxia": { "messages": "./messages/en.json" } }"#,
        )
        .unwrap();
        std::fs::write(
            root.join("node_modules/@acme/kit/messages/en.json"),
            r#"{ "@kit.ok": { "defaultMessage": "OK", "description": "d" },
                 "@kit.pageOf": { "defaultMessage": "{page} of {count}", "description": "d" } }"#,
        )
        .unwrap();
        std::fs::write(
            root.join("lxapp.json"),
            r#"{ "languages": ["en-US", "de-DE"] }"#,
        )
        .unwrap();
        std::fs::write(root.join("i18n/de-DE.json"), "{}").unwrap();
    }

    fn source(root: &Path, entries: &str) {
        std::fs::write(root.join("i18n/en-US.json"), format!("{{ {entries} }}")).unwrap();
    }

    #[test]
    fn a_package_word_override_is_checked_against_the_installed_package() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        app_with_package(root);
        let word = |key: &str, text: &str| {
            format!(r#""{key}": {{ "defaultMessage": "{text}", "description": "d" }}"#)
        };
        source(root, &word("@kit.ok", "Got it"));
        assert!(
            check(root, false).is_ok(),
            "an override in one language only"
        );
        source(root, &word("@kit.okay", "Got it"));
        let error = format!("{:#}", check(root, false).unwrap_err());
        assert!(error.contains("problem"), "{error}");
        assert_eq!(
            overrides(
                &Catalogue::parse(&format!("{{ {} }}", word("@kit.okay", "x")), true).unwrap(),
                &package_catalogues(root).unwrap()
            )[0]
            .reason,
            "not a @kit word; did you mean '@kit.ok'?"
        );
        let packages = package_catalogues(root).unwrap();
        let problems = |entries: &str| {
            overrides(
                &Catalogue::parse(&format!("{{ {entries} }}"), true).unwrap(),
                &packages,
            )
            .into_iter()
            .map(|problem| problem.reason)
            .collect::<Vec<_>>()
        };
        assert_eq!(
            problems(&word("@kat.ok", "x")),
            ["no installed package has @kat words; did you mean '@kit.ok'?"]
        );
        assert_eq!(
            problems(&word("@kit.pageOf", "{n} of {count}")),
            [
                "drops `page` (the package's word)",
                "adds `n`, which the source does not take (the package's word)"
            ]
        );
        assert!(problems(&word("@kit.pageOf", "{count}: {page}")).is_empty());
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
        assert_eq!(
            left,
            "untranslated: 7 key(s) (k.m1, k.m2, k.m3, k.m4, k.m5, …)"
        );
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
        assert_eq!(
            text("cart.items"),
            "{n, plural, one {# item} other {# items}}"
        );
        assert_eq!(text("cart.empty"), "Empty");
        assert!(!german.entries.contains_key("cart.old"));
        let module = std::fs::read_to_string(output.join("i18n/de-DE.js")).unwrap();
        assert!(
            module.contains("Empty") && !module.contains("Alt"),
            "{module}"
        );
    }
}
