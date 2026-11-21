//! `lingxia api <query>`: the Logic API catalogue (`@lingxia/types` `api.json`), one member,
//! namespace or type at a time.

use anyhow::{Context, Result};
use serde::Deserialize;
use serde_json::Value;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

const EMBEDDED: &str = include_str!("../../../../packages/lingxia-types/api.json");
const SUPPORTED_SCHEMA: u64 = 1;
const MAX_LINES: usize = 60;
const MAX_FIELDS: usize = 10;
const MAX_TYPES: usize = 3;

#[derive(Deserialize)]
struct Member {
    path: String,
    kind: String,
    entry: String,
    summary: String,
    description: String,
    signatures: Vec<Signature>,
    #[serde(rename = "type")]
    value_type: Option<String>,
    types: Vec<String>,
    #[serde(default)]
    tags: Tags,
    #[serde(default)]
    examples: Vec<String>,
    source: Option<String>,
}

#[derive(Deserialize)]
struct Signature {
    text: String,
    parameters: Vec<Parameter>,
}

#[derive(Deserialize)]
struct Parameter {
    name: String,
    #[serde(rename = "type")]
    param_type: String,
    required: bool,
}

#[derive(Deserialize, Default)]
struct Tags {
    #[serde(default)]
    throws: Vec<Throws>,
    #[serde(default)]
    platform: Vec<String>,
    #[serde(default)]
    permission: Vec<String>,
    #[serde(default)]
    supports: Vec<String>,
    default: Option<String>,
}

#[derive(Deserialize)]
struct Throws {
    code: Option<String>,
    when: String,
}

#[derive(Deserialize)]
struct TypeEntry {
    name: String,
    summary: String,
    description: String,
    text: Option<String>,
    fields: Vec<Field>,
    source: Option<String>,
}

#[derive(Deserialize)]
struct Field {
    name: String,
    #[serde(rename = "type")]
    field_type: String,
    required: bool,
    summary: String,
    default: Option<String>,
    #[serde(default)]
    tags: Tags,
}

pub struct Catalogue {
    version: String,
    /// The project file it was read from; `None` for the copy this CLI carries.
    origin: Option<PathBuf>,
    members: Vec<Member>,
    raw_members: Vec<Value>,
    types: Vec<TypeEntry>,
    raw_types: Vec<Value>,
    aliases: BTreeMap<String, Vec<String>>,
}

impl Catalogue {
    fn parse(text: &str, origin: Option<PathBuf>) -> Result<Self> {
        let value: Value = serde_json::from_str(text).context("parse api.json")?;
        let schema = value["schema"].as_u64().unwrap_or(0);
        anyhow::ensure!(
            schema <= SUPPORTED_SCHEMA,
            "api.json schema {schema} is newer than this CLI reads ({SUPPORTED_SCHEMA})"
        );
        let raw_members = value["members"].as_array().cloned().unwrap_or_default();
        let raw_types = value["types"].as_array().cloned().unwrap_or_default();
        Ok(Self {
            version: value["version"].as_str().unwrap_or("unknown").to_string(),
            origin,
            members: raw_members
                .iter()
                .map(|m| serde_json::from_value(m.clone()))
                .collect::<Result<_, _>>()
                .context("read api.json members")?,
            types: raw_types
                .iter()
                .map(|t| serde_json::from_value(t.clone()))
                .collect::<Result<_, _>>()
                .context("read api.json types")?,
            aliases: serde_json::from_value(value["aliases"].clone()).unwrap_or_default(),
            raw_members,
            raw_types,
        })
    }

    pub fn embedded() -> Self {
        Self::parse(EMBEDDED, None).expect("the embedded api.json parses")
    }

    fn member(&self, path: &str) -> Option<&Member> {
        self.members.iter().find(|m| m.path == path)
    }

    fn type_entry(&self, name: &str) -> Option<&TypeEntry> {
        self.types.iter().find(|t| t.name == name)
    }

    fn children(&self, path: &str) -> Vec<&Member> {
        let prefix = format!("{path}.");
        self.members
            .iter()
            .filter(|m| {
                m.path
                    .strip_prefix(&prefix)
                    .is_some_and(|rest| !rest.contains('.'))
            })
            .collect()
    }
}

/// The project's installed catalogue, nearest the working directory first; an in-repo link
/// without a build has only the package-root copy.
fn project_catalogue(start: &Path) -> Option<PathBuf> {
    for dir in start.ancestors() {
        for base in [
            dir.join("node_modules"),
            dir.join("lxapp").join("node_modules"),
        ] {
            let package = base.join("@lingxia").join("types");
            for file in [
                package.join("dist").join("api.json"),
                package.join("api.json"),
            ] {
                if file.is_file() {
                    return Some(file);
                }
            }
        }
    }
    None
}

pub enum Outcome {
    Found(String),
    Miss(String),
}

/// Answer a query; the text is what `lingxia api` prints.
pub fn answer(catalogue: &Catalogue, query: &str, json: bool) -> Outcome {
    let query = query.trim();
    let hits = resolve(catalogue, query);
    if json {
        return match &hits {
            Hits::Miss(_) => Outcome::Miss("[]".to_string()),
            _ => Outcome::Found(
                serde_json::to_string_pretty(&json_entries(catalogue, &hits)).unwrap_or_default(),
            ),
        };
    }
    match hits {
        Hits::Root => Outcome::Found(render_root(catalogue)),
        Hits::Member(i) => {
            let member = &catalogue.members[i];
            if member.kind == "namespace" {
                Outcome::Found(render_namespace(catalogue, member))
            } else {
                Outcome::Found(render_member(catalogue, member))
            }
        }
        Hits::Type(i) => Outcome::Found(render_type(catalogue, &catalogue.types[i])),
        Hits::List(title, paths) => Outcome::Found(render_list(catalogue, &title, &paths)),
        Hits::Miss(nearest) => Outcome::Miss(format!(
            "no member \"{query}\". Nearest: {}",
            nearest.join(", ")
        )),
    }
}

enum Hits {
    Root,
    Member(usize),
    Type(usize),
    List(String, Vec<String>),
    Miss(Vec<String>),
}

fn resolve(catalogue: &Catalogue, query: &str) -> Hits {
    if query.is_empty() {
        return Hits::Root;
    }
    let position = |path: &str| catalogue.members.iter().position(|m| m.path == path);
    if let Some(i) = position(query) {
        return Hits::Member(i);
    }
    if let Some(i) = position(&format!("lx.{query}"))
        && catalogue.members[i].kind == "namespace"
    {
        return Hits::Member(i);
    }
    if let Some(i) = catalogue.types.iter().position(|t| t.name == query) {
        return Hits::Type(i);
    }
    let words: Vec<&str> = query.split_whitespace().collect();
    if words.len() == 1 && !query.contains('.') {
        let named: Vec<String> = catalogue
            .members
            .iter()
            .filter(|m| m.path.rsplit('.').next() == Some(query))
            .map(|m| m.path.clone())
            .collect();
        match named.len() {
            0 => {}
            1 => return Hits::Member(position(&named[0]).unwrap_or_default()),
            _ => return Hits::List(format!("\"{query}\" names {} members", named.len()), named),
        }
    }
    let last = query.rsplit('.').next().unwrap_or(query);
    if words.len() == 1 && alias(catalogue, last).is_some() {
        return Hits::Miss(nearest(catalogue, query, 3));
    }
    let found = free_text(catalogue, &words);
    if !found.is_empty() {
        return Hits::List(format!("best matches for \"{query}\""), found);
    }
    Hits::Miss(nearest(catalogue, query, 3))
}

fn alias<'a>(catalogue: &'a Catalogue, name: &str) -> Option<&'a Vec<String>> {
    let name = name.to_lowercase();
    catalogue
        .aliases
        .iter()
        .find(|(alias, _)| alias.to_lowercase() == name)
        .map(|(_, targets)| targets)
}

const STOP_WORDS: &[&str] = &[
    "a", "an", "the", "to", "of", "and", "or", "for", "in", "on", "with", "from", "is", "it",
];

/// Up to five members whose path, summary or description hold most of the words.
fn free_text(catalogue: &Catalogue, words: &[&str]) -> Vec<String> {
    let words: Vec<String> = words
        .iter()
        .map(|w| w.to_lowercase())
        .filter(|w| !STOP_WORDS.contains(&w.as_str()))
        .collect();
    if words.is_empty() {
        return Vec::new();
    }
    let needed = words.len().div_ceil(2);
    let mut scored: Vec<(usize, &str)> = catalogue
        .members
        .iter()
        .filter_map(|m| {
            let path = m.path.to_lowercase();
            let summary = m.summary.to_lowercase();
            let description = m.description.to_lowercase();
            let mut matched = 0;
            let mut score = 0;
            for word in &words {
                let hit = [(&path, 3), (&summary, 2), (&description, 1)]
                    .iter()
                    .filter(|(text, _)| text.contains(word.as_str()))
                    .map(|(_, weight)| weight)
                    .sum::<usize>();
                if hit > 0 {
                    matched += 1;
                    score += hit;
                }
            }
            (matched >= needed).then_some((score, m.path.as_str()))
        })
        .collect();
    scored.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(b.1)));
    scored
        .into_iter()
        .take(5)
        .map(|(_, p)| p.to_string())
        .collect()
}

fn distance(a: &str, b: &str) -> usize {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    let mut row: Vec<usize> = (0..=b.len()).collect();
    for i in 1..=a.len() {
        let mut previous = row[0];
        row[0] = i;
        for j in 1..=b.len() {
            let current = row[j];
            let cost = usize::from(a[i - 1] != b[j - 1]);
            row[j] = (row[j] + 1).min(row[j - 1] + 1).min(previous + cost);
            previous = current;
        }
    }
    row[b.len()]
}

/// The alias table first, then edit distance on the last segment, as the docs check ranks them.
fn nearest(catalogue: &Catalogue, query: &str, count: usize) -> Vec<String> {
    let (parent, last) = match query.rsplit_once('.') {
        Some((parent, last)) => (parent, last.to_lowercase()),
        None => ("", query.to_lowercase()),
    };
    let aliased = alias(catalogue, &last).cloned().unwrap_or_default();
    let mut ranked: Vec<(isize, &str)> = catalogue
        .members
        .iter()
        .filter(|m| !aliased.contains(&m.path))
        .map(|m| {
            let (candidate_parent, name) = m.path.rsplit_once('.').unwrap_or(("", m.path.as_str()));
            let name = name.to_lowercase();
            let contained =
                name.chars().count() >= 3 && (last.contains(&name) || name.contains(&last));
            let same_parent = parent.is_empty() || candidate_parent == parent;
            let bonus: isize = if contained { 3 } else { 0 };
            let penalty: isize = if same_parent { 0 } else { 3 };
            let score = distance(&last, &name) as isize - bonus + penalty;
            (score, m.path.as_str())
        })
        .collect();
    ranked.sort();
    aliased
        .into_iter()
        .chain(ranked.into_iter().map(|(_, p)| p.to_string()))
        .take(count)
        .collect()
}

fn json_entries(catalogue: &Catalogue, hits: &Hits) -> Vec<Value> {
    let member_entry = |path: &str| {
        catalogue
            .members
            .iter()
            .position(|m| m.path == path)
            .map(|i| catalogue.raw_members[i].clone())
    };
    match hits {
        Hits::Root => catalogue
            .members
            .iter()
            .enumerate()
            .filter(|(_, m)| is_root_entry(m))
            .map(|(i, _)| catalogue.raw_members[i].clone())
            .collect(),
        Hits::Member(i) => {
            let member = &catalogue.members[*i];
            let mut out = vec![catalogue.raw_members[*i].clone()];
            if member.kind == "namespace" {
                out.extend(
                    catalogue
                        .children(&member.path)
                        .iter()
                        .filter_map(|c| member_entry(&c.path)),
                );
            } else {
                out.extend(direct_types(catalogue, member).iter().filter_map(|t| {
                    catalogue
                        .types
                        .iter()
                        .position(|e| e.name == t.name)
                        .map(|i| catalogue.raw_types[i].clone())
                }));
            }
            out
        }
        Hits::Type(i) => vec![catalogue.raw_types[*i].clone()],
        Hits::List(_, paths) => paths.iter().filter_map(|p| member_entry(p)).collect(),
        Hits::Miss(_) => Vec::new(),
    }
}

fn is_root_entry(member: &Member) -> bool {
    let depth = member.path.matches('.').count();
    (member.path.starts_with("lx.") && depth == 1)
        || (!member.path.starts_with("lx.") && depth == 0)
        || member.path == "page"
}

fn header(catalogue: &Catalogue, title: &str) -> Vec<String> {
    let mut lines = Vec::new();
    if catalogue.origin.is_none() {
        lines.push(format!(
            "note: no project install found; showing @lingxia/types {}",
            catalogue.version
        ));
    }
    let tag = format!("@lingxia/types {}", catalogue.version);
    let width = 64usize.saturating_sub(title.len()).max(2);
    lines.push(format!("{title}{}{tag}", " ".repeat(width)));
    lines
}

fn head(member: &Member) -> String {
    let name = member.path.rsplit('.').next().unwrap_or(&member.path);
    // The last overload is the general one.
    match member.signatures.last() {
        Some(signature) => {
            let params: Vec<String> = signature
                .parameters
                .iter()
                .map(|p| format!("{}{}", p.name, if p.required { "" } else { "?" }))
                .collect();
            format!("{name}({})", params.join(", "))
        }
        None => member.kind.clone(),
    }
}

fn aligned(rows: &[(String, String, String)]) -> Vec<String> {
    let first = rows.iter().map(|r| r.0.len()).max().unwrap_or(0);
    let second = rows.iter().map(|r| r.1.len()).max().unwrap_or(0).min(36);
    rows.iter()
        .map(|(a, b, c)| {
            format!("  {a:first$}  {b:second$}  {c}")
                .trim_end()
                .to_string()
        })
        .collect()
}

fn render_root(catalogue: &Catalogue) -> String {
    let mut lines = header(catalogue, "Logic API");
    lines.push(
        "Query a member (lx.nav.go), a namespace (lx.network), a type, or words.".to_string(),
    );
    lines.push(String::new());
    let rows: Vec<_> = catalogue
        .members
        .iter()
        .filter(|m| is_root_entry(m))
        .map(|m| (m.path.clone(), m.kind.clone(), m.summary.clone()))
        .collect();
    lines.extend(aligned(&rows));
    finish(lines)
}

fn render_namespace(catalogue: &Catalogue, member: &Member) -> String {
    let mut lines = header(catalogue, &member.path);
    if !member.summary.is_empty() {
        lines.push(member.summary.clone());
    }
    lines.push(String::new());
    let rows: Vec<_> = catalogue
        .children(&member.path)
        .into_iter()
        .map(|child| {
            let shape = if child.kind == "namespace" {
                format!("namespace ({})", catalogue.children(&child.path).len())
            } else {
                head(child)
            };
            (child.path.clone(), shape, child.summary.clone())
        })
        .collect();
    lines.extend(aligned(&rows));
    finish(lines)
}

fn render_list(catalogue: &Catalogue, title: &str, paths: &[String]) -> String {
    let mut lines = header(catalogue, title);
    lines.push(String::new());
    let rows: Vec<_> = paths
        .iter()
        .filter_map(|p| catalogue.member(p))
        .map(|m| (m.path.clone(), head(m), m.summary.clone()))
        .collect();
    lines.extend(aligned(&rows));
    finish(lines)
}

fn shorten(text: &str, max: usize) -> String {
    if text.chars().count() <= max {
        return text.to_string();
    }
    let cut: String = text.chars().take(max - 1).collect();
    format!("{cut}…")
}

/// The named types a member's own signatures mention, and its inline option and result objects.
/// Ordered as they appear, the general (last) overload first.
fn direct_types<'a>(catalogue: &'a Catalogue, member: &Member) -> Vec<&'a TypeEntry> {
    let text: String = member
        .signatures
        .iter()
        .rev()
        .map(|s| s.text.as_str())
        .chain(member.value_type.as_deref())
        .collect::<Vec<_>>()
        .join(" ");
    let inline = format!("{}(", member.path);
    let mut names: Vec<&str> = Vec::new();
    for word in text.split(|c: char| !(c.is_alphanumeric() || c == '_')) {
        if !names.contains(&word) && member.types.iter().any(|t| t == word) {
            names.push(word);
        }
    }
    names.extend(
        member
            .types
            .iter()
            .filter(|t| t.starts_with(&inline))
            .map(String::as_str),
    );
    names
        .into_iter()
        .filter_map(|name| catalogue.type_entry(name))
        .filter(|t| !t.fields.is_empty() || t.text.is_some())
        .collect()
}

/// What a type is to the member: `Options  DownloadOptions`, `Returns  DownloadTask`.
fn role(member: &Member, entry: &TypeEntry) -> String {
    if let Some(rest) = entry.name.strip_prefix(&member.path) {
        return match rest.strip_prefix("()") {
            Some(suffix) => format!("Returns{suffix}"),
            None => {
                let (param, suffix) = rest.split_once(')').unwrap_or((rest, ""));
                format!("Options {param}){suffix}")
            }
        };
    }
    let in_parameters = member.signatures.iter().any(|s| {
        s.parameters.iter().any(|p| {
            p.param_type
                .split(|c: char| !(c.is_alphanumeric() || c == '_'))
                .any(|w| w == entry.name)
        })
    });
    let kind = if member.signatures.is_empty() {
        "Type"
    } else if in_parameters {
        "Options"
    } else {
        "Returns"
    };
    format!("{kind}  {}", entry.name)
}

fn field_rows(fields: &[Field]) -> Vec<String> {
    let rows: Vec<_> = fields
        .iter()
        .take(MAX_FIELDS)
        .map(|f| {
            let mut note = f.summary.clone();
            if let Some(default) = &f.default {
                note.push_str(&format!(" Default: {default}."));
            }
            for platform in &f.tags.platform {
                note.push_str(&format!(" Platform: {platform}"));
            }
            (
                format!("{}{}", f.name, if f.required { "" } else { "?" }),
                shorten(
                    if f.required {
                        &f.field_type
                    } else {
                        f.field_type
                            .strip_suffix(" | undefined")
                            .unwrap_or(&f.field_type)
                    },
                    36,
                ),
                note,
            )
        })
        .collect();
    let mut lines = aligned(&rows);
    if fields.len() > MAX_FIELDS {
        lines.push(format!("  … {} more fields", fields.len() - MAX_FIELDS));
    }
    lines
}

fn description_body(summary: &str, description: &str) -> Vec<String> {
    let rest = description
        .strip_prefix(summary)
        .unwrap_or(description)
        .trim();
    if rest.is_empty() {
        return Vec::new();
    }
    // Rejoin wrapped prose; keep lists, code and blank lines as they are.
    let mut lines: Vec<String> = Vec::new();
    let mut fenced = false;
    for line in rest.lines() {
        let starts_block =
            line.is_empty() || line.starts_with(['-', '*', ' ', '|']) || line.starts_with("```");
        let joins = !fenced
            && !starts_block
            && lines
                .last()
                .is_some_and(|l| !l.is_empty() && !l.starts_with("```"));
        if line.starts_with("```") {
            fenced = !fenced;
        }
        match lines.last_mut() {
            Some(last) if joins => {
                last.push(' ');
                last.push_str(line);
            }
            _ => lines.push(line.to_string()),
        }
    }
    lines.truncate(12);
    lines.push(String::new());
    lines
}

fn render_member(catalogue: &Catalogue, member: &Member) -> String {
    let mut lines = header(catalogue, &member.path);
    lines.push(member.summary.clone());
    lines.push(String::new());
    let shown = member.signatures.len().min(4);
    for signature in &member.signatures[..shown] {
        lines.push(format!("  {}", signature.text));
    }
    if member.signatures.len() > shown {
        lines.push(format!(
            "  … {} more overloads",
            member.signatures.len() - shown
        ));
    }
    if let Some(value_type) = &member.value_type {
        lines.push(format!(
            "  {}: {}",
            member.path.rsplit('.').next().unwrap_or_default(),
            value_type
        ));
    }
    lines.push(String::new());
    lines.extend(description_body(&member.summary, &member.description));

    let types = direct_types(catalogue, member);
    for entry in types.iter().take(MAX_TYPES) {
        let label = role(member, entry);
        match &entry.text {
            Some(text) if entry.fields.is_empty() => {
                lines.push(format!("{label} = {}", shorten(text, 160)));
            }
            _ => {
                let summary = if entry.summary.is_empty() {
                    String::new()
                } else {
                    format!("  {}", entry.summary)
                };
                lines.push(format!("{label}{summary}"));
                lines.extend(field_rows(&entry.fields));
            }
        }
    }
    let shown_names: Vec<&str> = types
        .iter()
        .take(MAX_TYPES)
        .map(|t| t.name.as_str())
        .collect();
    let more: Vec<&str> = member
        .types
        .iter()
        .map(String::as_str)
        .filter(|name| !shown_names.contains(name) && !name.contains('('))
        .collect();
    if !more.is_empty() {
        lines.push(format!("Types       {}", more.join(", ")));
    }
    if !types.is_empty() || !more.is_empty() {
        lines.push(String::new());
    }

    let tags = &member.tags;
    lines.push(format!(
        "Platforms   {}",
        if tags.platform.is_empty() {
            "no platform notes".to_string()
        } else {
            tags.platform.join(" · ")
        }
    ));
    if !tags.permission.is_empty() {
        lines.push(format!("Permission  {}", tags.permission.join(" · ")));
    }
    if !tags.supports.is_empty() {
        lines.push(format!(
            "Supports    lx.supports('{}')",
            tags.supports.join("') · lx.supports('")
        ));
    }
    if let Some(default) = &tags.default {
        lines.push(format!("Default     {default}"));
    }
    if !tags.throws.is_empty() {
        let rejects: Vec<String> = tags
            .throws
            .iter()
            .map(|t| {
                format!("{} {}", t.code.as_deref().unwrap_or(""), t.when)
                    .trim()
                    .to_string()
            })
            .collect();
        lines.push(format!("Rejects     {}", rejects.join(" · ")));
    }
    if let Some(example) = member.examples.first() {
        lines.push("Example".to_string());
        lines.extend(example.lines().take(12).map(|l| format!("  {l}")));
    }
    if member.entry != "global" && !member.path.contains('.') {
        lines.push(format!(
            "Import      import {{ {} }} from '{}'",
            member.path, member.entry
        ));
    }
    let children = catalogue.children(&member.path);
    if !children.is_empty() {
        let names: Vec<&str> = children.iter().map(|c| c.path.as_str()).collect();
        lines.push(format!("Members     {}", names.join(", ")));
    }
    if let Some(source) = &member.source {
        lines.push(format!("Source      {}", source_path(catalogue, source)));
    }
    finish(lines)
}

fn render_type(catalogue: &Catalogue, entry: &TypeEntry) -> String {
    let mut lines = header(catalogue, &entry.name);
    if !entry.summary.is_empty() {
        lines.push(entry.summary.clone());
    }
    lines.push(String::new());
    lines.extend(description_body(&entry.summary, &entry.description));
    if let Some(text) = &entry.text {
        lines.push(format!("  {}", shorten(text, 600)));
        lines.push(String::new());
    }
    if !entry.fields.is_empty() {
        lines.extend(field_rows(&entry.fields));
        lines.push(String::new());
    }
    let users: Vec<&str> = catalogue
        .members
        .iter()
        .filter(|m| m.types.contains(&entry.name))
        .map(|m| m.path.as_str())
        .take(8)
        .collect();
    if !users.is_empty() {
        lines.push(format!("Used by     {}", users.join(", ")));
    }
    if let Some(source) = &entry.source {
        lines.push(format!("Source      {}", source_path(catalogue, source)));
    }
    finish(lines)
}

/// A source line inside the package the catalogue came from.
fn source_path(catalogue: &Catalogue, source: &str) -> String {
    match catalogue.origin.as_deref().and_then(package_dir) {
        Some(dir) => dir.join(source).display().to_string(),
        None => format!("@lingxia/types/{source}"),
    }
}

fn package_dir(file: &Path) -> Option<&Path> {
    let parent = file.parent()?;
    Some(if parent.ends_with("dist") {
        parent.parent()?
    } else {
        parent
    })
}

fn finish(mut lines: Vec<String>) -> String {
    while lines.last().is_some_and(|l| l.is_empty()) {
        lines.pop();
    }
    if lines.len() > MAX_LINES {
        let cut = lines.len() - (MAX_LINES - 1);
        lines.truncate(MAX_LINES - 1);
        lines.push(format!("… {cut} more lines (--json for everything)"));
    }
    lines.join("\n") + "\n"
}

/// Load the catalogue `lingxia api` answers from: the project's install, else this CLI's copy.
pub fn load(start: &Path) -> Result<Catalogue> {
    let embedded = Catalogue::embedded();
    let Some(file) = project_catalogue(start) else {
        return Ok(embedded);
    };
    let text =
        std::fs::read_to_string(&file).with_context(|| format!("read {}", file.display()))?;
    match Catalogue::parse(&text, Some(file.clone())) {
        Ok(project) => {
            if project.version != embedded.version {
                eprintln!(
                    "note: the project has @lingxia/types {}; this CLI carries {}. Showing the project's.",
                    project.version, embedded.version
                );
            }
            Ok(project)
        }
        Err(err) => {
            eprintln!(
                "note: {} unreadable ({err:#}); showing this CLI's copy",
                file.display()
            );
            Ok(embedded)
        }
    }
}

pub fn execute(query: &[String], json: bool) -> Result<()> {
    let catalogue = load(&std::env::current_dir()?)?;
    match answer(&catalogue, &query.join(" "), json) {
        Outcome::Found(text) => {
            print!("{text}");
            if json {
                println!();
            }
            Ok(())
        }
        Outcome::Miss(text) => {
            if json {
                println!("{text}");
            } else {
                eprintln!("{text}");
            }
            std::process::exit(1);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn found(query: &str) -> String {
        match answer(&Catalogue::embedded(), query, false) {
            Outcome::Found(text) => text,
            Outcome::Miss(text) => panic!("{query}: {text}"),
        }
    }

    fn missed(query: &str) -> String {
        match answer(&Catalogue::embedded(), query, false) {
            Outcome::Miss(text) => text,
            Outcome::Found(text) => panic!("{query} found:\n{text}"),
        }
    }

    #[test]
    fn a_member_path_prints_its_signature_and_option_type() {
        let text = found("lx.network.download");
        assert!(text.contains("download(url: string"), "{text}");
        assert!(text.contains("Options  DownloadOptions"), "{text}");
        assert!(text.contains("Returns  DownloadTask"), "{text}");
        assert!(
            found("lx.pick.directory").contains("Returns"),
            "inline result object labelled"
        );
        assert!(text.contains("Platforms"), "{text}");
        let lines = text.lines().count();
        assert!((10..=60).contains(&lines), "{lines} lines:\n{text}");
    }

    #[test]
    fn a_namespace_lists_its_members_one_line_each() {
        let text = found("lx.network");
        assert!(text.contains("lx.network.download"), "{text}");
        assert!(text.contains("lx.network.upload"), "{text}");
        assert_eq!(text, found("network"));
    }

    #[test]
    fn a_bare_name_resolves_to_its_member() {
        let catalogue = Catalogue::embedded();
        let entry = &catalogue.member("definePage").unwrap().entry;
        let import = format!("Import      import {{ definePage }} from '{entry}'");
        assert!(found("definePage").contains(&import));
        assert!(found("download").starts_with(&found("lx.network.download")));
    }

    #[test]
    fn a_type_name_prints_its_fields_and_users() {
        let text = found("DownloadsDownloadOptions");
        assert!(text.contains("Used by     lx.network.download"), "{text}");
    }

    #[test]
    fn words_rank_members_by_their_docs() {
        let text = found("pick a file");
        assert!(text.contains("lx.pick.file"), "{text}");
        assert!(text.lines().count() <= 8, "{text}");
    }

    #[test]
    fn a_miss_names_the_nearest_members_and_known_aliases() {
        assert!(missed("showModal").contains("Nearest: lx.ui.confirm, lx.ui.alert"));
        assert!(missed("lx.ui.showModal").contains("lx.ui.confirm"));
        assert!(missed("lx.device.getDeviceInfo").contains("lx.device.info"));
        assert!(missed("qqqqzzzz").starts_with("no member \"qqqqzzzz\". Nearest: "));
    }

    #[test]
    fn no_query_lists_the_namespaces() {
        let text = found("");
        assert!(text.contains("lx.network "), "{text}");
        assert!(text.contains("definePage "), "{text}");
        assert!(!text.contains("lx.network.download"), "{text}");
    }

    #[test]
    fn json_returns_catalogue_entries_and_an_empty_array_on_a_miss() {
        let Outcome::Found(text) = answer(&Catalogue::embedded(), "lx.network.download", true)
        else {
            panic!("expected a hit");
        };
        let entries: Vec<Value> = serde_json::from_str(&text).unwrap();
        assert_eq!(entries[0]["path"], "lx.network.download");
        assert!(
            entries
                .iter()
                .any(|e| e["name"] == "DownloadsDownloadOptions")
        );
        assert!(
            matches!(answer(&Catalogue::embedded(), "showModal", true), Outcome::Miss(t) if t == "[]")
        );
    }

    #[test]
    fn the_project_install_wins_over_the_embedded_copy() {
        let dir = tempfile::tempdir().unwrap();
        let package = dir.path().join("lxapp/node_modules/@lingxia/types/dist");
        std::fs::create_dir_all(&package).unwrap();
        let mut value: Value = serde_json::from_str(EMBEDDED).unwrap();
        value["version"] = Value::from("9.9.9");
        std::fs::write(package.join("api.json"), value.to_string()).unwrap();
        let nested = dir.path().join("lxapp/pages/home");
        std::fs::create_dir_all(&nested).unwrap();
        let catalogue = load(&nested).unwrap();
        assert_eq!(catalogue.version, "9.9.9");
        assert!(found_in(&catalogue, "lx.nav.go").contains("@lingxia/types 9.9.9"));
        assert!(!found_in(&catalogue, "lx.nav.go").contains("note: no project install"));
    }

    fn found_in(catalogue: &Catalogue, query: &str) -> String {
        match answer(catalogue, query, false) {
            Outcome::Found(text) => text,
            Outcome::Miss(text) => panic!("{text}"),
        }
    }
}
