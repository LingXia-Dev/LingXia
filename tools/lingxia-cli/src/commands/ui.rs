//! `lingxia ui`: find a UI part in the component catalogues the project's direct dependencies
//! declare with `"lingxia": { "catalog": "./catalog.json" }` in their package.json.
//!
//! Stdout carries data only (one tab-separated line per part, or `--json`); every message goes
//! to stderr. Exit 0: found; 1: nothing matches; 2: no catalogue to search.

use anyhow::{Context, Result};
use clap::Subcommand;
use serde_json::{Value, json};
use std::io::Write;
use std::path::{Path, PathBuf};

/// Version of the `--json` output; bump when its shape changes.
const JSON_SCHEMA: u64 = 1;
const DEFAULT_LIMIT: usize = 10;
/// A type longer than this is cut in text output; `--json` keeps it whole.
const MAX_TYPE_CHARS: usize = 90;

pub const EXIT_NOT_FOUND: i32 = 1;
pub const EXIT_NO_CATALOGUE: i32 = 2;

#[derive(Subcommand, Debug, Clone)]
pub enum UiAction {
    /// Find parts by what the user sees ("a row with a status"); no words lists every part.
    /// Prints `Name<TAB>import<TAB>summary`, best match first
    Search {
        /// Words to match against names, keywords, summaries and prop names
        query: Vec<String>,

        /// How many parts to print (default 10; every part when no words are given)
        #[arg(long)]
        limit: Option<usize>,

        /// Print `{ "schema", "results": [...] }`
        #[arg(long)]
        json: bool,
    },
    /// Show one part: its import, props (required first) and where its doc with examples is
    Info {
        /// Part name or one of its exports, any case: `Button`, `avatargroup`
        name: String,

        /// Print `{ "schema", "components": [catalogue entry + package + doc] }`
        #[arg(long)]
        json: bool,
    },
}

/// One catalogue a dependency ships.
#[derive(Debug)]
struct Catalogue {
    package: String,
    dir: PathBuf,
    components: Vec<Value>,
}

/// What searching the project found before any query runs.
#[derive(Debug)]
enum Found {
    Catalogues(Vec<Catalogue>),
    /// No catalogue; carries the one-line reason.
    None(String),
}

pub fn execute(action: UiAction) -> Result<()> {
    let cwd = std::env::current_dir()?;
    let stdout = std::io::stdout();
    let stderr = std::io::stderr();
    let code = match run(&cwd, &action, &mut stdout.lock(), &mut stderr.lock()) {
        Ok(code) => code,
        // A reader that stops early (`| head`) is not a failure of the lookup.
        Err(error)
            if error
                .downcast_ref::<std::io::Error>()
                .is_some_and(|e| e.kind() == std::io::ErrorKind::BrokenPipe) =>
        {
            return Ok(());
        }
        Err(error) => return Err(error),
    };
    if code != 0 {
        std::process::exit(code);
    }
    Ok(())
}

fn run(cwd: &Path, action: &UiAction, out: &mut dyn Write, err: &mut dyn Write) -> Result<i32> {
    let catalogues = match discover(cwd)? {
        Found::Catalogues(catalogues) => catalogues,
        Found::None(reason) => {
            writeln!(err, "{reason}")?;
            return Ok(EXIT_NO_CATALOGUE);
        }
    };
    match action {
        UiAction::Search { query, limit, json } => {
            search(&catalogues, query, *limit, *json, out, err)
        }
        UiAction::Info { name, json } => info(&catalogues, name, *json, out, err),
    }
}

// ---- discovery ----

/// The project whose dependencies are searched: the nearest package.json at or above `cwd`,
/// or a host project's `lxapp/package.json`.
fn project_manifest(cwd: &Path) -> Option<PathBuf> {
    cwd.ancestors().find_map(|dir| {
        [
            dir.join("package.json"),
            dir.join("lxapp").join("package.json"),
        ]
        .into_iter()
        .find(|path| path.is_file())
    })
}

fn discover(cwd: &Path) -> Result<Found> {
    let Some(manifest) = project_manifest(cwd) else {
        return Ok(Found::None(format!(
            "no component catalogue: no package.json at or above {}",
            cwd.display()
        )));
    };
    let project = manifest.parent().unwrap_or(cwd);
    let json = read_json(&manifest)?;
    let mut missing = Vec::new();
    let mut catalogues = Vec::new();
    for name in direct_dependencies(&json) {
        let Some(dir) = installed(project, &name) else {
            missing.push(name);
            continue;
        };
        if let Some(catalogue) = catalogue_of(&name, &dir)? {
            catalogues.push(catalogue);
        }
    }
    if !catalogues.is_empty() {
        return Ok(Found::Catalogues(catalogues));
    }
    let hint = if missing.is_empty() {
        String::new()
    } else {
        format!(
            "; {} of its dependencies {} not installed, run npm install",
            missing.len(),
            if missing.len() == 1 { "is" } else { "are" }
        )
    };
    Ok(Found::None(format!(
        "no component catalogue: no dependency in {} declares lingxia.catalog{hint}",
        manifest.display()
    )))
}

fn read_json(path: &Path) -> Result<Value> {
    let text = std::fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;
    serde_json::from_str(&text).with_context(|| format!("{} is not JSON", path.display()))
}

/// `dependencies` then `devDependencies`, each name once, in the manifest's order.
pub(crate) fn direct_dependencies(manifest: &Value) -> Vec<String> {
    let mut names: Vec<String> = Vec::new();
    for field in ["dependencies", "devDependencies"] {
        if let Some(map) = manifest.get(field).and_then(Value::as_object) {
            for name in map.keys() {
                if !names.contains(name) {
                    names.push(name.clone());
                }
            }
        }
    }
    names
}

/// Where Node would resolve `name` from `project`: the nearest `node_modules/<name>` above it.
pub(crate) fn installed(project: &Path, name: &str) -> Option<PathBuf> {
    project
        .ancestors()
        .map(|dir| dir.join("node_modules").join(name))
        .find(|dir| dir.join("package.json").is_file())
        .and_then(|dir| dunce::canonicalize(dir).ok())
}

/// `relative` inside the package at `dir` (already canonical), refusing anything that leaves it.
pub(crate) fn inside_package(dir: &Path, relative: &str) -> Option<PathBuf> {
    let path = Path::new(relative);
    if relative.is_empty() || path.is_absolute() {
        return None;
    }
    let resolved = dunce::canonicalize(dir.join(path)).ok()?;
    resolved.starts_with(dir).then_some(resolved)
}

fn catalogue_of(name: &str, dir: &Path) -> Result<Option<Catalogue>> {
    let manifest = read_json(&dir.join("package.json"))?;
    let Some(relative) = manifest.pointer("/lingxia/catalog").and_then(Value::as_str) else {
        return Ok(None);
    };
    let Some(path) = inside_package(dir, relative) else {
        anyhow::bail!("{name}: lingxia.catalog \"{relative}\" is not a file inside the package");
    };
    let catalogue = read_json(&path)?;
    let components = catalogue
        .get("components")
        .and_then(Value::as_array)
        .with_context(|| format!("{}: no \"components\" list", path.display()))?
        .clone();
    Ok(Some(Catalogue {
        package: name.to_string(),
        dir: dir.to_path_buf(),
        components,
    }))
}

// ---- search ----

/// Words that say nothing about a part.
const STOP_WORDS: &[&str] = &[
    "a", "an", "and", "as", "at", "by", "for", "from", "in", "into", "is", "it", "of", "on", "or",
    "that", "the", "to", "with", "without",
];

fn words(text: &str) -> Vec<String> {
    text.split(|c: char| !c.is_alphanumeric() && c != '-')
        .map(str::to_lowercase)
        .filter(|w| !w.is_empty() && !STOP_WORDS.contains(&w.as_str()))
        .collect()
}

fn text<'a>(entry: &'a Value, field: &str) -> &'a str {
    entry.get(field).and_then(Value::as_str).unwrap_or("")
}

fn strings<'a>(entry: &'a Value, field: &str) -> impl Iterator<Item = &'a str> {
    entry
        .get(field)
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
}

/// How well one query word fits a part: the best field it appears in.
fn word_score(entry: &Value, word: &str) -> u32 {
    let name = text(entry, "name").to_lowercase();
    let mut best = if name == word {
        100
    } else if name.contains(word) {
        40
    } else {
        0
    };
    for export in strings(entry, "exports") {
        let export = export.to_lowercase();
        best = best.max(if export == word {
            60
        } else if export.contains(word) {
            30
        } else {
            0
        });
    }
    for keyword in strings(entry, "keywords") {
        let keyword = keyword.to_lowercase();
        let hit = keyword == word || keyword.split(' ').any(|part| part == word);
        best = best.max(if hit {
            30
        } else if keyword.contains(word) {
            15
        } else {
            0
        });
    }
    if words(text(entry, "summary")).iter().any(|w| w == word) {
        best = best.max(12);
    }
    if words(text(entry, "description")).iter().any(|w| w == word) {
        best = best.max(5);
    }
    let prop_hit = entry
        .get("props")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .any(|prop| text(prop, "name").eq_ignore_ascii_case(word));
    if prop_hit {
        best = best.max(8);
    }
    best
}

struct Hit<'a> {
    catalogue: &'a Catalogue,
    entry: &'a Value,
    matched: usize,
    score: u32,
}

fn search(
    catalogues: &[Catalogue],
    query: &[String],
    limit: Option<usize>,
    json: bool,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> Result<i32> {
    let query_words = words(&query.join(" "));
    let mut hits: Vec<Hit> = Vec::new();
    for catalogue in catalogues {
        for entry in &catalogue.components {
            let scores: Vec<u32> = query_words.iter().map(|w| word_score(entry, w)).collect();
            let matched = scores.iter().filter(|s| **s > 0).count();
            if !query_words.is_empty() && matched == 0 {
                continue;
            }
            hits.push(Hit {
                catalogue,
                entry,
                matched,
                score: scores.iter().sum(),
            });
        }
    }
    hits.sort_by(|a, b| {
        b.matched
            .cmp(&a.matched)
            .then(b.score.cmp(&a.score))
            .then(a.catalogue.package.cmp(&b.catalogue.package))
            .then(text(a.entry, "name").cmp(text(b.entry, "name")))
    });
    let limit = limit.unwrap_or(if query_words.is_empty() {
        usize::MAX
    } else {
        DEFAULT_LIMIT
    });
    hits.truncate(limit);

    if json {
        let results: Vec<Value> = hits
            .iter()
            .map(|hit| {
                json!({
                    "name": text(hit.entry, "name"),
                    "package": hit.catalogue.package,
                    "import": text(hit.entry, "import"),
                    "summary": text(hit.entry, "summary"),
                    "score": hit.score,
                })
            })
            .collect();
        writeln!(
            out,
            "{}",
            json!({ "schema": JSON_SCHEMA, "results": results })
        )?;
    } else {
        for hit in &hits {
            writeln!(
                out,
                "{}\t{}\t{}",
                text(hit.entry, "name"),
                text(hit.entry, "import"),
                text(hit.entry, "summary")
            )?;
        }
    }
    if hits.is_empty() {
        writeln!(
            err,
            "no component matches \"{}\"; `lingxia ui search` lists every part",
            query.join(" ")
        )?;
        return Ok(EXIT_NOT_FOUND);
    }
    Ok(0)
}

// ---- info ----

/// A part found by `info`: its entry, and the export asked for when that was a sub-part.
struct Match<'a> {
    catalogue: &'a Catalogue,
    entry: &'a Value,
    sub_part: Option<String>,
}

fn find<'a>(catalogues: &'a [Catalogue], name: &str) -> Vec<Match<'a>> {
    let mut exact = Vec::new();
    let mut by_export = Vec::new();
    for catalogue in catalogues {
        for entry in &catalogue.components {
            if text(entry, "name").eq_ignore_ascii_case(name) {
                exact.push(Match {
                    catalogue,
                    entry,
                    sub_part: None,
                });
            } else if let Some(export) =
                strings(entry, "exports").find(|e| e.eq_ignore_ascii_case(name))
            {
                by_export.push(Match {
                    catalogue,
                    entry,
                    sub_part: Some(export.to_string()),
                });
            }
        }
    }
    if exact.is_empty() { by_export } else { exact }
}

fn doc_path(catalogue: &Catalogue, name: &str) -> Option<PathBuf> {
    let path = catalogue.dir.join("docs").join(format!("{name}.md"));
    path.is_file().then_some(path)
}

fn info(
    catalogues: &[Catalogue],
    name: &str,
    json: bool,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> Result<i32> {
    let matches = find(catalogues, name);
    if json {
        let components: Vec<Value> = matches
            .iter()
            .map(|m| {
                let mut entry = m.entry.clone();
                if let Some(object) = entry.as_object_mut() {
                    object.insert("package".into(), json!(m.catalogue.package));
                    let doc = doc_path(m.catalogue, text(m.entry, "name"));
                    object.insert("doc".into(), json!(doc.map(|p| p.display().to_string())));
                }
                entry
            })
            .collect();
        writeln!(
            out,
            "{}",
            json!({ "schema": JSON_SCHEMA, "components": components })
        )?;
    } else {
        for (index, m) in matches.iter().enumerate() {
            if index > 0 {
                writeln!(out)?;
            }
            write_info(m, out)?;
        }
    }
    if matches.is_empty() {
        let nearest = nearest(catalogues, name);
        let hint = if nearest.is_empty() {
            "`lingxia ui search <words>` finds one by what it shows".to_string()
        } else {
            format!("nearest: {}", nearest.join(", "))
        };
        writeln!(err, "no component \"{name}\"; {hint}")?;
        return Ok(EXIT_NOT_FOUND);
    }
    Ok(0)
}

fn write_info(m: &Match, out: &mut dyn Write) -> Result<()> {
    let name = text(m.entry, "name");
    let shown = m.sub_part.as_deref().unwrap_or(name);
    let import = text(m.entry, "import");
    let status = text(m.entry, "status");
    writeln!(
        out,
        "{shown}\t{import}{}",
        if status.is_empty() {
            String::new()
        } else {
            format!("\t{status}")
        }
    )?;
    writeln!(out, "import {{ {shown} }} from '{import}';")?;
    let props: Vec<&Value> = match &m.sub_part {
        None => array(m.entry.get("props")),
        Some(sub) => array(m.entry.pointer(&format!("/otherProps/{sub}"))),
    };
    match &m.sub_part {
        None => {
            for line in [text(m.entry, "summary"), text(m.entry, "description")] {
                if !line.is_empty() {
                    writeln!(out, "{line}")?;
                }
            }
        }
        Some(_) => writeln!(out, "Part of {name}: {}", text(m.entry, "summary"))?,
    }
    if props.is_empty() {
        if m.sub_part.is_some() {
            writeln!(out, "props: listed with {name} (`lingxia ui info {name}`)")?;
        }
    } else {
        writeln!(out, "props:")?;
        let (required, optional): (Vec<&Value>, Vec<&Value>) = props
            .into_iter()
            .partition(|p| p.get("required").and_then(Value::as_bool).unwrap_or(false));
        for prop in required.into_iter().chain(optional) {
            writeln!(out, "  {}", prop_line(prop))?;
        }
    }
    if m.sub_part.is_none() {
        let mut parts: Vec<String> = strings(m.entry, "exports")
            .filter(|e| *e != name)
            .map(str::to_string)
            .collect();
        if let Some(other) = m.entry.get("otherProps").and_then(Value::as_object) {
            for key in other.keys() {
                if !parts.contains(key) {
                    parts.push(key.clone());
                }
            }
        }
        if !parts.is_empty() {
            writeln!(
                out,
                "parts: {} (`lingxia ui info <part>`)",
                parts.join(", ")
            )?;
        }
    }
    let related: Vec<&str> = strings(m.entry, "related").collect();
    if !related.is_empty() {
        writeln!(out, "related: {}", related.join(", "))?;
    }
    if let Some(doc) = doc_path(m.catalogue, name) {
        writeln!(out, "doc: {}", doc.display())?;
    }
    Ok(())
}

fn array(value: Option<&Value>) -> Vec<&Value> {
    value
        .and_then(Value::as_array)
        .map(|a| a.iter().collect())
        .unwrap_or_default()
}

/// `name?: Type ('a' | 'b') = default — description`; `values` (a string-literal union the
/// catalogue spelled out) is shown when the type is only an alias.
fn prop_line(prop: &Value) -> String {
    let required = prop
        .get("required")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let mut line = format!(
        "{}{}: ",
        text(prop, "name"),
        if required { "" } else { "?" }
    );
    line.push_str(&cut(text(prop, "type")));
    let values: Vec<String> = array(prop.get("values"))
        .into_iter()
        .map(|v| match v {
            Value::String(s) => format!("'{s}'"),
            other => other.to_string(),
        })
        .collect();
    if !values.is_empty() {
        line.push_str(&format!(" ({})", cut(&values.join(" | "))));
    }
    if let Some(default) = prop.get("default").and_then(Value::as_str) {
        line.push_str(&format!(" = {default}"));
    }
    if required {
        line.push_str(" (required)");
    }
    let description = text(prop, "description");
    if !description.is_empty() {
        line.push_str(" — ");
        line.push_str(description);
    }
    line
}

fn cut(text: &str) -> String {
    if text.chars().count() <= MAX_TYPE_CHARS {
        return text.to_string();
    }
    let head: String = text.chars().take(MAX_TYPE_CHARS).collect();
    format!("{head}… (--json for all)")
}

/// Up to three part names closest to `name`: shared prefix, then containment.
fn nearest(catalogues: &[Catalogue], name: &str) -> Vec<String> {
    let lower = name.to_lowercase();
    let mut scored: Vec<(usize, String)> = catalogues
        .iter()
        .flat_map(|c| c.components.iter())
        .flat_map(|entry| strings(entry, "exports").chain(std::iter::once(text(entry, "name"))))
        .filter(|candidate| !candidate.is_empty())
        .filter_map(|candidate| {
            let other = candidate.to_lowercase();
            let prefix = lower
                .chars()
                .zip(other.chars())
                .take_while(|(a, b)| a == b)
                .count();
            let contains = other.contains(&lower) || lower.contains(&other);
            let score = prefix * 2 + if contains { 3 } else { 0 };
            (score >= 3).then(|| (score, candidate.to_string()))
        })
        .collect();
    scored.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
    scored.dedup_by(|a, b| a.1 == b.1);
    scored.into_iter().take(3).map(|(_, name)| name).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn write(path: &Path, text: &str) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, text).unwrap();
    }

    const CATALOGUE: &str = r#"{"components":[
      {"name":"Button","import":"@kit/core/Button","exports":["Button"],"status":"preview",
       "summary":"Runs an action when pressed","description":"Starts an action.",
       "keywords":["button","submit","press"],
       "props":[
         {"name":"variant","type":"ButtonVariant","required":false,"description":"Emphasis.","default":"'secondary'","values":["primary","secondary"]},
         {"name":"label","type":"string","required":true,"description":"The visible text."}
       ],"related":["IconButton"]},
      {"name":"Item","import":"@kit/core/Item","exports":["Item","ItemList"],
       "summary":"A row with a title, a value and a status","description":"A list row.",
       "keywords":["row","list item","status"],
       "props":[{"name":"title","type":"string","required":true,"description":"Row title."}],
       "otherProps":{"ItemList":[{"name":"items","type":"Item[]","required":true,"description":"Rows."}]}}
    ]}"#;

    /// A project depending on `@kit/core` (with a catalogue and a doc) and `left-pad` (none).
    fn project() -> tempfile::TempDir {
        let root = tempfile::tempdir().unwrap();
        let r = root.path();
        write(
            &r.join("package.json"),
            r#"{"dependencies":{"@kit/core":"1","left-pad":"1"},"devDependencies":{"typescript":"5"}}"#,
        );
        write(
            &r.join("node_modules/@kit/core/package.json"),
            r#"{"name":"@kit/core","lingxia":{"catalog":"./catalog.json"}}"#,
        );
        write(&r.join("node_modules/@kit/core/catalog.json"), CATALOGUE);
        write(
            &r.join("node_modules/@kit/core/docs/Button.md"),
            "# Button\n",
        );
        write(
            &r.join("node_modules/left-pad/package.json"),
            r#"{"name":"left-pad"}"#,
        );
        root
    }

    fn run_in(dir: &Path, action: UiAction) -> (i32, String, String) {
        let mut out = Vec::new();
        let mut err = Vec::new();
        let code = run(dir, &action, &mut out, &mut err).unwrap();
        (
            code,
            String::from_utf8(out).unwrap(),
            String::from_utf8(err).unwrap(),
        )
    }

    fn search_for(dir: &Path, words: &str, json: bool) -> (i32, String, String) {
        let query = words.split_whitespace().map(str::to_string).collect();
        run_in(
            dir,
            UiAction::Search {
                query,
                limit: None,
                json,
            },
        )
    }

    #[test]
    fn search_ranks_by_what_the_user_sees() {
        let p = project();
        let (code, out, err) = search_for(p.path(), "a row with a status", false);
        assert_eq!(code, 0, "{err}");
        let first = out.lines().next().unwrap();
        assert_eq!(
            first,
            "Item\t@kit/core/Item\tA row with a title, a value and a status"
        );
        assert!(err.is_empty());
    }

    #[test]
    fn empty_search_lists_every_part() {
        let p = project();
        let (code, out, _) = search_for(p.path(), "", false);
        assert_eq!(code, 0);
        assert_eq!(out.lines().count(), 2);
    }

    #[test]
    fn search_json_carries_schema_and_package() {
        let p = project();
        let (_, out, _) = search_for(p.path(), "press", true);
        let value: Value = serde_json::from_str(&out).unwrap();
        assert_eq!(value["schema"], 1);
        assert_eq!(value["results"][0]["name"], "Button");
        assert_eq!(value["results"][0]["package"], "@kit/core");
    }

    #[test]
    fn a_miss_exits_one_with_data_on_stdout_only() {
        let p = project();
        let (code, out, err) = search_for(p.path(), "spaceship", false);
        assert_eq!(code, EXIT_NOT_FOUND);
        assert!(out.is_empty());
        assert!(
            err.starts_with("no component matches \"spaceship\""),
            "{err}"
        );
        let (code, out, _) = search_for(p.path(), "spaceship", true);
        assert_eq!(code, EXIT_NOT_FOUND);
        assert_eq!(
            serde_json::from_str::<Value>(&out).unwrap()["results"],
            json!([])
        );
    }

    #[test]
    fn info_lists_required_props_first_with_values_and_doc() {
        let p = project();
        let (code, out, _) = run_in(
            p.path(),
            UiAction::Info {
                name: "button".into(),
                json: false,
            },
        );
        assert_eq!(code, 0);
        let lines: Vec<&str> = out.lines().collect();
        assert_eq!(lines[0], "Button\t@kit/core/Button\tpreview");
        assert_eq!(lines[1], "import { Button } from '@kit/core/Button';");
        let label = lines
            .iter()
            .position(|l| l.starts_with("  label:"))
            .unwrap();
        let variant = lines
            .iter()
            .position(|l| l.starts_with("  variant?:"))
            .unwrap();
        assert!(label < variant);
        assert_eq!(
            lines[variant],
            "  variant?: ButtonVariant ('primary' | 'secondary') = 'secondary' — Emphasis."
        );
        assert!(out.contains("related: IconButton\n"));
        assert!(
            lines.last().unwrap().starts_with("doc: ")
                && lines.last().unwrap().ends_with("Button.md")
        );
    }

    #[test]
    fn info_on_a_sub_part_shows_its_own_props() {
        let p = project();
        let (_, main, _) = run_in(
            p.path(),
            UiAction::Info {
                name: "Item".into(),
                json: false,
            },
        );
        assert!(
            main.contains("parts: ItemList (`lingxia ui info <part>`)"),
            "{main}"
        );
        let (code, out, _) = run_in(
            p.path(),
            UiAction::Info {
                name: "itemlist".into(),
                json: false,
            },
        );
        assert_eq!(code, 0);
        assert!(out.starts_with("ItemList\t@kit/core/Item\n"), "{out}");
        assert!(out.contains("  items: Item[] (required) — Rows."));
    }

    #[test]
    fn info_json_is_the_entry_with_package_and_doc() {
        let p = project();
        let (_, out, _) = run_in(
            p.path(),
            UiAction::Info {
                name: "Button".into(),
                json: true,
            },
        );
        let value: Value = serde_json::from_str(&out).unwrap();
        assert_eq!(value["schema"], 1);
        let entry = &value["components"][0];
        assert_eq!(entry["package"], "@kit/core");
        assert_eq!(entry["props"][1]["name"], "label");
        assert!(entry["doc"].as_str().unwrap().ends_with("docs/Button.md"));
    }

    #[test]
    fn unknown_info_names_the_nearest() {
        let p = project();
        let (code, out, err) = run_in(
            p.path(),
            UiAction::Info {
                name: "Buton".into(),
                json: false,
            },
        );
        assert_eq!(code, EXIT_NOT_FOUND);
        assert!(out.is_empty());
        assert_eq!(err.trim(), "no component \"Buton\"; nearest: Button");
    }

    #[test]
    fn no_catalogue_exits_two_and_says_why() {
        let root = tempfile::tempdir().unwrap();
        write(
            &root.path().join("package.json"),
            r#"{"dependencies":{"@kit/core":"1"}}"#,
        );
        let (code, out, err) = search_for(root.path(), "button", false);
        assert_eq!(code, EXIT_NO_CATALOGUE);
        assert!(out.is_empty());
        assert!(
            err.contains(
                "declares lingxia.catalog; 1 of its dependencies is not installed, run npm install"
            ),
            "{err}"
        );
        assert_eq!(err.lines().count(), 1);
    }

    #[test]
    fn found_from_a_subdirectory_and_through_a_host_lxapp() {
        let p = project();
        let pages = p.path().join("pages/home");
        fs::create_dir_all(&pages).unwrap();
        assert_eq!(search_for(&pages, "button", false).0, 0);

        let host = tempfile::tempdir().unwrap();
        let lxapp = host.path().join("lxapp");
        fs::create_dir_all(&lxapp).unwrap();
        fs::rename(p.path().join("package.json"), lxapp.join("package.json")).unwrap();
        fs::rename(p.path().join("node_modules"), lxapp.join("node_modules")).unwrap();
        assert_eq!(search_for(host.path(), "button", false).0, 0);
    }

    #[test]
    fn a_catalogue_path_leaving_the_package_is_refused() {
        let p = project();
        write(
            &p.path().join("node_modules/@kit/core/package.json"),
            r#"{"name":"@kit/core","lingxia":{"catalog":"../../../package.json"}}"#,
        );
        let mut out = Vec::new();
        let mut err = Vec::new();
        let action = UiAction::Search {
            query: vec![],
            limit: None,
            json: false,
        };
        let error = run(p.path(), &action, &mut out, &mut err).unwrap_err();
        assert!(format!("{error:#}").contains("is not a file inside the package"));
    }
}
