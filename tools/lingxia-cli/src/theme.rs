//! `lingxia theme`: start, check, preview and turn into tokens an lxapp's theme.

mod preview;
pub mod tokens;

use anyhow::{Context, Result, bail};
use clap::Subcommand;
use lingxia_theme::Pack;
use serde_json::{Value, json};
use std::fs;
use std::path::{Path, PathBuf};

/// Where an lxapp keeps its own theme; without one the built-in default applies.
pub const THEME_PATH: &str = "theme.json";

#[derive(Subcommand, Debug)]
pub enum ThemeCommand {
    /// Start a theme file: one accent seed, everything else from the theme it extends
    New {
        /// The theme's name
        id: String,
        /// The colour every accent role derives from (#rrggbb)
        #[arg(long)]
        seed: Option<String>,
        /// Start from this theme instead of the built-in one: `default`, a file, or `themes/<id>.json`
        #[arg(long)]
        extends: Option<String>,
        /// The file to write
        #[arg(short, long, default_value = THEME_PATH)]
        out: PathBuf,
    },
    /// Run the build's checks on a theme: every contrast pair in light, dark and high contrast
    Check {
        /// The theme file (default: `theme.json`, else `lxapp/theme.json`)
        file: Option<PathBuf>,
        /// Complete it from this theme instead of the built-in one: `default`, a file, or `themes/<id>.json`
        #[arg(long)]
        extends: Option<String>,
    },
    /// Run the current lxapp in the Runner under its theme, one key per look: light, dark and both high-contrast looks
    Preview,
    /// Write a theme's scales as Tailwind tokens, for Web pages and editor completion
    Tokens(tokens::TokensConfig),
}

pub fn run(command: ThemeCommand) -> Result<()> {
    match command {
        ThemeCommand::New {
            id,
            seed,
            extends,
            out,
        } => new(&id, seed.as_deref(), extends.as_deref(), &out),
        ThemeCommand::Check { file, extends } => {
            let file = match file {
                Some(file) => file,
                None => [THEME_PATH, "lxapp/theme.json"]
                    .into_iter()
                    .map(PathBuf::from)
                    .find(|path| path.is_file())
                    .with_context(|| format!("no {THEME_PATH} here; name the theme file"))?,
            };
            let base = base(extends.as_deref())?;
            let pack = load_from(&file, &base)?;
            println!(
                "{}: {} passes every pair in light, dark and high contrast",
                file.display(),
                pack.name
            );
            for line in app_color_pairs(&pack) {
                println!("  {line}");
            }
            Ok(())
        }
        ThemeCommand::Preview => preview::run(),
        ThemeCommand::Tokens(config) => tokens::run(config),
    }
}

/// The theme at `path` completed from the built-in one, under what it extends; or the built-in
/// one. Fails listing every contrast or schema problem.
pub fn load(path: Option<&Path>) -> Result<Pack> {
    match path {
        Some(path) => {
            let written = written(path, &mut Vec::new())?;
            let pack = serde_json::from_value(written)
                .with_context(|| format!("{} is not a theme", path.display()))?;
            checked(lingxia_theme::complete(pack))
        }
        None => checked(lingxia_theme::default_pack()),
    }
}

/// The theme at `path` completed from `base`, which replaces any `extends` it names.
pub fn load_from(path: &Path, base: &Pack) -> Result<Pack> {
    let (value, _) = read_value(path)?;
    let pack = serde_json::from_value(value)
        .with_context(|| format!("{} is not a theme", path.display()))?;
    checked(lingxia_theme::complete_from(pack, base))
}

/// A theme file as written, without its `$schema`, and the id it `extends`.
fn read_value(path: &Path) -> Result<(Value, Option<String>)> {
    let text = fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;
    let mut value: Value =
        serde_json::from_str(&text).with_context(|| format!("{} is not JSON", path.display()))?;
    let object = value
        .as_object_mut()
        .with_context(|| format!("{} is not a JSON object", path.display()))?;
    object.remove("$schema");
    let extends = match object.remove("extends") {
        None => None,
        Some(Value::String(id)) => Some(id),
        Some(_) => bail!("{}: `extends` is a theme id", path.display()),
    };
    Ok((value, extends))
}

/// What a theme and everything it extends wrote, the theme's own values winning.
fn written(path: &Path, chain: &mut Vec<PathBuf>) -> Result<Value> {
    let key = fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    if chain.contains(&key) {
        bail!(
            "{} extends itself through {}",
            path.display(),
            chain[0].display()
        );
    }
    chain.push(key);
    let (value, extends) = read_value(path)?;
    let Some(id) = extends.filter(|id| id != "default") else {
        return Ok(value);
    };
    let dir = path.parent().unwrap_or(Path::new("."));
    let file = [
        dir.join(format!("{id}.json")),
        dir.join("themes").join(format!("{id}.json")),
    ]
    .into_iter()
    .find(|file| file.is_file())
    .with_context(|| {
        format!(
            "{} extends {id}, but there is no {id}.json beside it",
            path.display()
        )
    })?;
    let mut base = written(&file, chain)?;
    overlay(&mut base, value);
    Ok(base)
}

/// `top` written over `base`: objects merge key by key, anything else replaces.
fn overlay(base: &mut Value, top: Value) {
    match (base, top) {
        (Value::Object(base), Value::Object(top)) => {
            for (key, value) in top {
                match base.get_mut(&key) {
                    Some(slot) => overlay(slot, value),
                    None => {
                        base.insert(key, value);
                    }
                }
            }
        }
        (slot, top) => *slot = top,
    }
}

/// Where a host keeps its themes: `themes/<id>.json`, the file name being the id.
pub const HOST_THEMES_DIR: &str = "themes";

/// The host's theme, completed: `id` names one of `themes/<id>.json`; without one, the only file
/// there. Every file there is checked, so a broken theme fails the build even when unused.
pub fn host_theme(root: &Path, id: Option<&str>) -> Result<Option<Pack>> {
    let dir = root.join(HOST_THEMES_DIR);
    let mut files: Vec<PathBuf> = match fs::read_dir(&dir) {
        Ok(entries) => entries
            .filter_map(|entry| entry.ok().map(|entry| entry.path()))
            .filter(|path| path.extension().is_some_and(|ext| ext == "json"))
            .collect(),
        Err(_) => Vec::new(),
    };
    files.sort();
    let id_of = |path: &Path| path.file_stem().and_then(|s| s.to_str()).map(str::to_owned);
    let mut packs = Vec::new();
    for file in &files {
        let pack = load(Some(file))?;
        packs.push((id_of(file).unwrap_or_default(), pack));
    }
    let ids = packs
        .iter()
        .map(|(id, _)| id.as_str())
        .collect::<Vec<_>>()
        .join(", ");
    match id {
        Some(id) => packs
            .into_iter()
            .find(|(file_id, _)| file_id == id)
            .map(|(_, pack)| Some(pack))
            .with_context(|| match ids.is_empty() {
                true => format!(
                    "lingxia.yaml names theme {id}, but there is no {HOST_THEMES_DIR}/{id}.json"
                ),
                false => {
                    format!("lingxia.yaml names theme {id}, but {HOST_THEMES_DIR}/ holds {ids}")
                }
            }),
        None if packs.len() > 1 => bail!(
            "{HOST_THEMES_DIR}/ holds {ids}; name the product's theme with `theme: <id>` in lingxia.yaml"
        ),
        None => Ok(packs.pop().map(|(_, pack)| pack)),
    }
}

/// Each app colour's pairs with their ratio in every look, one line per pair.
fn app_color_pairs(pack: &Pack) -> Vec<String> {
    use lingxia_theme::Appearance::{Dark, Light};
    let themes: Vec<(&str, lingxia_theme::ResolvedTheme)> = [
        ("light", Light, false),
        ("dark", Dark, false),
        ("high-contrast-light", Light, true),
        ("high-contrast-dark", Dark, true),
    ]
    .into_iter()
    .filter_map(|(look, appearance, high_contrast)| {
        let prefs = lingxia_theme::Prefs {
            appearance,
            high_contrast,
            ..Default::default()
        };
        lingxia_theme::resolve(pack, &prefs)
            .ok()
            .map(|theme| (look, theme))
    })
    .collect();
    let mut lines = Vec::new();
    for (name, color) in &pack.colors {
        let pairs: Vec<(String, String)> = if color.usage == lingxia_theme::ColorUse::Fill {
            vec![(format!("on-{name}"), name.clone())]
        } else {
            color
                .on
                .iter()
                .map(|ground| (name.clone(), ground.clone()))
                .collect()
        };
        for (ink, ground) in pairs {
            let ratios: Vec<String> = themes
                .iter()
                .filter_map(|(look, theme)| {
                    let pick = |key: &str| {
                        theme.app.get(key).copied().or_else(|| {
                            lingxia_theme::Role::from_name(key).map(|role| theme.color(role))
                        })
                    };
                    let ratio = lingxia_theme::contrast(pick(&ink)?, pick(&ground)?);
                    Some(format!("{look} {ratio:.2}:1"))
                })
                .collect();
            lines.push(format!("{ink} on {ground}: {}", ratios.join(", ")));
        }
    }
    lines
}

/// The lxapp theme at `root` in its four looks, as wire colour names to hex, for the page
/// compiler to measure text on gradients; null without a theme or one that does not parse.
pub fn project_looks(root: &Path) -> Value {
    load(Some(&root.join(THEME_PATH))).map_or(Value::Null, |pack| looks(&pack))
}

/// `pack` resolved for light, dark and both high-contrast looks: each role and `app.<name>`.
pub fn looks(pack: &Pack) -> Value {
    use lingxia_theme::Appearance::{Dark, Light};
    let mut looks = serde_json::Map::new();
    for (look, appearance, high_contrast) in [
        ("light", Light, false),
        ("dark", Dark, false),
        ("high-contrast-light", Light, true),
        ("high-contrast-dark", Dark, true),
    ] {
        let prefs = lingxia_theme::Prefs {
            appearance,
            high_contrast,
            ..Default::default()
        };
        let Ok(theme) = lingxia_theme::resolve(pack, &prefs) else {
            continue;
        };
        let mut colors: serde_json::Map<String, Value> = lingxia_theme::Role::ALL
            .into_iter()
            .map(|role| (role.name().to_owned(), theme.color(role).hex().into()))
            .collect();
        for (name, color) in &theme.app {
            colors.insert(format!("app.{name}"), color.hex().into());
        }
        looks.insert(look.to_owned(), colors.into());
    }
    looks.into()
}

fn checked(pack: Pack) -> Result<Pack> {
    let problems = lingxia_theme::check(&pack);
    if !problems.is_empty() {
        let list: Vec<String> = problems.iter().map(|p| format!("  {p}")).collect();
        bail!("theme {} fails its checks:\n{}", pack.name, list.join("\n"));
    }
    for advice in lingxia_theme::advise(&pack) {
        eprintln!("warning: theme {}: {advice}", pack.name);
    }
    Ok(pack)
}

/// The file a theme id names: `default` is the built-in theme.
fn base_file(id: &str) -> Result<Option<PathBuf>> {
    if id == "default" {
        return Ok(None);
    }
    [
        PathBuf::from(id),
        Path::new("themes").join(format!("{id}.json")),
    ]
    .into_iter()
    .find(|path| path.is_file())
    .map(Some)
    .with_context(|| format!("no theme {id}: not `default`, a file, or themes/{id}.json"))
}

/// The complete theme an id names, the built-in one when there is none.
fn base(id: Option<&str>) -> Result<Pack> {
    match id.map(base_file).transpose()?.flatten() {
        Some(path) => load(Some(&path)),
        None => Ok(lingxia_theme::default_pack()),
    }
}

fn new(id: &str, seed: Option<&str>, extends: Option<&str>, out: &Path) -> Result<()> {
    if out.exists() {
        bail!("{} exists; a new theme does not replace it", out.display());
    }
    if let Some(seed) = seed {
        lingxia_theme::Rgba::parse(seed)
            .with_context(|| format!("--seed {seed} is not #rrggbb"))?;
    }
    let base_path = extends.map(base_file).transpose()?.flatten();
    // What the extended theme's author wrote, not its completion: the new file keeps only that.
    let mut theme = match &base_path {
        Some(path) => serde_json::from_str::<Value>(
            &fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?,
        )
        .with_context(|| format!("{} is not a theme", path.display()))?,
        None => json!({ "version": 1 }),
    };
    let object = theme
        .as_object_mut()
        .context("a theme file is a JSON object")?;
    object.insert("name".into(), json!(id));
    let dir = out
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    if let Some(schema) = schema_reference(dir) {
        object.insert("$schema".into(), json!(schema));
    }
    if seed.is_some() || !object.contains_key("accent") {
        let seed = match seed {
            Some(seed) => seed.to_owned(),
            None => lingxia_theme::default_pack()
                .accent
                .context("the built-in theme has an accent seed")?
                .seed
                .hex(),
        };
        object.insert("accent".into(), json!({ "seed": seed, "derive": true }));
    }
    let text = serde_json::to_string_pretty(&theme)? + "\n";
    let base = base(extends)?;
    let mut written = theme.clone();
    if let Some(object) = written.as_object_mut() {
        object.remove("$schema");
        object.remove("extends");
    }
    checked(lingxia_theme::complete_from(
        serde_json::from_value(written).context("the new theme parses")?,
        &base,
    ))?;
    if let Some(parent) = out.parent().filter(|p| !p.as_os_str().is_empty()) {
        fs::create_dir_all(parent)?;
    }
    fs::write(out, text).with_context(|| format!("write {}", out.display()))?;
    println!(
        "Wrote {} ({id}); `lingxia theme check` lists its contrast pairs, `lingxia build` ships it",
        out.display()
    );
    Ok(())
}

/// The theme JSON Schema `@lingxia/view` ships, relative to `dir`, for editor completion.
fn schema_reference(dir: &Path) -> Option<String> {
    const SCHEMA: &str = "node_modules/@lingxia/view/schemas/theme.schema.json";
    ["./", "../", "lxapp/", "../lxapp/"]
        .into_iter()
        .map(|prefix| format!("{prefix}{SCHEMA}"))
        .find(|reference| dir.join(reference).is_file())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("lingxia-theme-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn a_new_theme_is_one_seed_and_passes() {
        let dir = scratch("new");
        let out = dir.join(THEME_PATH);
        new("brand", Some("#0f9d58"), None, &out).unwrap();
        let written: Value = serde_json::from_str(&fs::read_to_string(&out).unwrap()).unwrap();
        assert_eq!(
            written,
            json!({ "name": "brand", "version": 1, "accent": { "seed": "#0f9d58", "derive": true } })
        );
        assert!(load(Some(&out)).is_ok());
        assert!(
            new("brand", None, None, &out)
                .unwrap_err()
                .to_string()
                .contains("exists")
        );
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn a_new_theme_that_extends_keeps_what_its_base_wrote() {
        let dir = scratch("extends");
        let base = dir.join("warm.json");
        fs::write(
            &base,
            include_str!("../../../packages/liuli/themes/warm.json"),
        )
        .unwrap();
        let out = dir.join(THEME_PATH);
        new("brand", None, Some(base.to_str().unwrap()), &out).unwrap();
        let mut written: Value = serde_json::from_str(&fs::read_to_string(&out).unwrap()).unwrap();
        let mut warm: Value = serde_json::from_str(&fs::read_to_string(&base).unwrap()).unwrap();
        written.as_object_mut().unwrap().remove("name");
        warm.as_object_mut().unwrap().remove("name");
        assert_eq!(written, warm);
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn a_failing_theme_names_each_problem_and_its_nearest_passing_value() {
        let dir = scratch("failing");
        let mut pack = lingxia_theme::default_pack();
        pack.appearance.light.insert(
            "text-muted".into(),
            lingxia_theme::Rgba::parse("#dddddd").unwrap(),
        );
        let path = dir.join(THEME_PATH);
        fs::write(&path, serde_json::to_string(&pack).unwrap()).unwrap();
        let error = load(Some(&path)).unwrap_err().to_string();
        assert!(error.contains("light: text-muted on surface is"), "{error}");
        assert!(error.contains("is the nearest that passes"), "{error}");
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn a_theme_file_may_carry_a_schema_and_extend_a_theme_beside_it() {
        let dir = scratch("chain");
        fs::write(
            dir.join("warm.json"),
            include_str!("../../../packages/liuli/themes/warm.json"),
        )
        .unwrap();
        let file = dir.join("ocean.json");
        fs::write(
            &file,
            r#"{ "$schema": "x.json", "name": "ocean", "version": 1, "extends": "warm" }"#,
        )
        .unwrap();
        let pack = load(Some(&file)).unwrap();
        let warm = load(Some(&dir.join("warm.json"))).unwrap();
        let prefs = lingxia_theme::Prefs::default();
        assert_eq!(
            lingxia_theme::resolve(&pack, &prefs).unwrap().palette,
            lingxia_theme::resolve(&warm, &prefs).unwrap().palette
        );
        assert_eq!(pack.name, "ocean");

        fs::write(
            dir.join("warm.json"),
            r#"{ "name": "warm", "version": 1, "extends": "ocean" }"#,
        )
        .unwrap();
        let error = load(Some(&file)).unwrap_err().to_string();
        assert!(error.contains("extends itself"), "{error}");
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn the_host_theme_is_the_named_file_or_the_only_one() {
        let dir = scratch("host");
        assert!(host_theme(&dir, None).unwrap().is_none());
        let themes = dir.join(HOST_THEMES_DIR);
        fs::create_dir_all(&themes).unwrap();
        fs::write(
            themes.join("ocean.json"),
            r#"{ "name": "ocean", "version": 1 }"#,
        )
        .unwrap();
        assert_eq!(host_theme(&dir, None).unwrap().unwrap().name, "ocean");
        fs::write(
            themes.join("forest.json"),
            r#"{ "name": "forest", "version": 1 }"#,
        )
        .unwrap();
        let error = host_theme(&dir, None).unwrap_err().to_string();
        assert!(error.contains("forest, ocean"), "{error}");
        assert_eq!(
            host_theme(&dir, Some("forest")).unwrap().unwrap().name,
            "forest"
        );
        let error = host_theme(&dir, Some("sky")).unwrap_err().to_string();
        assert!(error.contains("themes/ holds forest, ocean"), "{error}");
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn an_unknown_base_says_where_it_looked() {
        let error = base(Some("nowhere")).unwrap_err().to_string();
        assert!(error.contains("themes/nowhere.json"), "{error}");
    }
}
