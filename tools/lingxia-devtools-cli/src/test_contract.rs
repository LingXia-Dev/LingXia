//! Selection and contract inputs of `lxdev test`: `--tag`, `--covers-manifest`
//! and `--openapi`. The files are read and checked here, before the run
//! exists, and travel to `@lingxia/test` as run controls; selection and
//! validation happen in the test runtime, next to the specs. lxdev also keeps
//! the per-tag and manifest summaries right in the reports it writes or
//! completes itself.

use anyhow::{Context, Result, anyhow, bail};
use owo_colors::OwoColorize;
use serde_json::{Value, json};
use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};

/// Controls that carry file contents; reports name the files instead.
pub const PAYLOAD_CONTROL_KEYS: [&str; 2] = ["openapi", "coversManifest"];
/// The parsed documents travel in the run's start message.
const MAX_OPENAPI_BYTES: usize = 4 * 1024 * 1024;
const UNTAGGED: &str = "(untagged)";

fn is_tag(tag: &str) -> bool {
    let mut chars = tag.chars();
    chars.next().is_some_and(|c| c.is_ascii_alphanumeric())
        && chars.all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '.' | ':' | '/' | '-'))
}

/// `--tag` value parser: comma-separated terms, each `tag` or `!tag`.
pub fn parse_tag_expr(raw: &str) -> Result<String, String> {
    for term in raw.split(',') {
        let term = term.trim();
        let tag = term.strip_prefix('!').map_or(term, str::trim);
        if !is_tag(tag) {
            return Err(format!(
                "`{term}` is not a tag or !tag (letters, digits and _ . : / -; \
                 `a,b` = either, `!a` = without a)"
            ));
        }
    }
    Ok(raw.to_string())
}

/// Whether `tags` pass every clause, as `@lingxia/test` decides it.
#[cfg(test)]
fn matches_tags(tags: &[&str], clauses: &[&str]) -> bool {
    clauses.iter().all(|clause| {
        clause.split(',').any(|term| {
            let term = term.trim();
            match term.strip_prefix('!') {
                Some(tag) => !tags.contains(&tag.trim()),
                None => tags.contains(&term),
            }
        })
    })
}

#[derive(Debug, Clone, PartialEq)]
pub struct ManifestEntry {
    pub id: String,
    pub title: Option<String>,
}

/// The files behind `--covers-manifest` and `--openapi`, read once.
#[derive(Default)]
pub struct RunInputs {
    pub manifest: Option<(PathBuf, Vec<ManifestEntry>)>,
    pub openapi: Vec<(String, Value)>,
}

impl RunInputs {
    pub fn load(manifest: Option<&Path>, openapi: &[PathBuf]) -> Result<Self> {
        let manifest = manifest
            .map(|path| load_manifest(path).map(|entries| (path.to_path_buf(), entries)))
            .transpose()?;
        let openapi = openapi
            .iter()
            .map(|path| load_openapi(path).map(|doc| (display_name(path), doc)))
            .collect::<Result<Vec<_>>>()?;
        let names: Vec<&str> = openapi.iter().map(|(name, _)| name.as_str()).collect();
        if let Some(duplicate) = names
            .iter()
            .enumerate()
            .find(|(index, name)| names[..*index].contains(name))
        {
            bail!("--openapi {} is given twice", duplicate.1);
        }
        Ok(Self { manifest, openapi })
    }

    /// Add the payload controls and the file names reports show.
    pub fn add_controls(&self, control: &mut HashMap<String, String>) -> Result<()> {
        if let Some((path, entries)) = &self.manifest {
            let list: Vec<Value> = entries
                .iter()
                .map(|entry| match &entry.title {
                    Some(title) => json!({ "id": entry.id, "title": title }),
                    None => json!({ "id": entry.id }),
                })
                .collect();
            control.insert("coversManifest".into(), serde_json::to_string(&list)?);
            control.insert("coversManifestFile".into(), display_name(path));
        }
        if !self.openapi.is_empty() {
            let docs: Vec<Value> = self
                .openapi
                .iter()
                .map(|(name, doc)| json!({ "name": name, "doc": doc }))
                .collect();
            let encoded = serde_json::to_string(&docs)?;
            if encoded.len() > MAX_OPENAPI_BYTES {
                bail!(
                    "--openapi documents are {} KiB as JSON; the limit is {} KiB. Pass only the \
                     documents this suite calls.",
                    encoded.len() / 1024,
                    MAX_OPENAPI_BYTES / 1024
                );
            }
            control.insert("openapi".into(), encoded);
            let names: Vec<&str> = self.openapi.iter().map(|(name, _)| name.as_str()).collect();
            control.insert("openapiFiles".into(), names.join(", "));
        }
        Ok(())
    }

    pub fn manifest_entries(&self) -> Option<&[ManifestEntry]> {
        self.manifest
            .as_ref()
            .map(|(_, entries)| entries.as_slice())
    }
}

/// The controls as reports show them: file names, not file contents.
pub fn reported_control(control: &HashMap<String, String>) -> HashMap<String, String> {
    let mut shown = control.clone();
    for key in PAYLOAD_CONTROL_KEYS {
        shown.remove(key);
    }
    shown
}

fn display_name(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

/// JSON or YAML by extension; an unknown extension tries JSON, then YAML.
fn read_structured(path: &Path) -> Result<Value> {
    let text =
        std::fs::read_to_string(path).with_context(|| format!("cannot read {}", path.display()))?;
    let ext = path
        .extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    let yaml = || -> Result<Value> {
        serde_yaml_ng::from_str::<Value>(&text)
            .with_context(|| format!("{} is not valid YAML", path.display()))
    };
    match ext.as_str() {
        "yaml" | "yml" => yaml(),
        "json" => serde_json::from_str(&text)
            .with_context(|| format!("{} is not valid JSON", path.display())),
        _ => serde_json::from_str(&text).or_else(|_| yaml()),
    }
}

/// A coverage manifest lists requirement ids, with optional titles:
/// a list of `"ID"` or `{ id, title }`, the same under a `covers` key, or a
/// map of `ID: title`.
pub fn load_manifest(path: &Path) -> Result<Vec<ManifestEntry>> {
    let value = read_structured(path)?;
    parse_manifest(&value).with_context(|| format!("--covers-manifest {}", path.display()))
}

fn parse_manifest(value: &Value) -> Result<Vec<ManifestEntry>> {
    let list = match value {
        Value::Array(items) => items.clone(),
        Value::Object(map) if map.get("covers").is_some_and(Value::is_array) => {
            map["covers"].as_array().cloned().unwrap_or_default()
        }
        Value::Object(map) => map
            .iter()
            .map(|(id, title)| match title {
                Value::Null => json!({ "id": id }),
                Value::String(title) => json!({ "id": id, "title": title }),
                Value::Object(fields) => {
                    json!({ "id": id, "title": fields.get("title").cloned().unwrap_or(Value::Null) })
                }
                _ => json!({ "id": id, "title": title.to_string() }),
            })
            .collect(),
        _ => bail!("expected a list of ids, a `covers:` list, or a map of id: title"),
    };
    let mut entries: Vec<ManifestEntry> = Vec::with_capacity(list.len());
    for (index, item) in list.iter().enumerate() {
        let (id, title) = match item {
            Value::String(id) => (id.clone(), None),
            Value::Object(fields) => (
                fields
                    .get("id")
                    .and_then(Value::as_str)
                    .ok_or_else(|| anyhow!("entry {index} has no string `id`"))?
                    .to_string(),
                fields
                    .get("title")
                    .and_then(Value::as_str)
                    .map(str::to_string),
            ),
            _ => bail!("entry {index} must be an id or {{ id, title }}"),
        };
        if id.trim().is_empty() {
            bail!("entry {index} has an empty id");
        }
        if entries.iter().any(|entry| entry.id == id) {
            bail!("lists `{id}` twice");
        }
        entries.push(ManifestEntry { id, title });
    }
    if entries.is_empty() {
        bail!("lists no ids");
    }
    Ok(entries)
}

/// Read an OpenAPI 3.0/3.1 document. References must be local (`#/…`):
/// bundle multi-file documents first.
pub fn load_openapi(path: &Path) -> Result<Value> {
    let doc = read_structured(path)?;
    check_openapi(&doc).with_context(|| format!("--openapi {}", path.display()))?;
    Ok(doc)
}

fn check_openapi(doc: &Value) -> Result<()> {
    let version = doc.get("openapi").and_then(Value::as_str).unwrap_or("");
    if !(version.starts_with("3.0.") || version.starts_with("3.1.")) {
        if doc.get("swagger").is_some() {
            bail!("Swagger 2.0 is not supported; convert it to OpenAPI 3");
        }
        bail!("`openapi: {version}` is not supported; use OpenAPI 3.0 or 3.1");
    }
    if !doc.get("paths").is_some_and(Value::is_object) {
        bail!("has no `paths`");
    }
    let mut external = Vec::new();
    collect_external_refs(doc, &mut external);
    if !external.is_empty() {
        external.sort();
        external.dedup();
        bail!(
            "has references outside the document ({}); bundle it into one file first \
             (for example `redocly bundle` or `swagger-cli bundle`)",
            external
                .iter()
                .take(3)
                .cloned()
                .collect::<Vec<_>>()
                .join(", ")
        );
    }
    Ok(())
}

fn collect_external_refs(value: &Value, out: &mut Vec<String>) {
    match value {
        Value::Object(map) => {
            if let Some(Value::String(target)) = map.get("$ref")
                && !target.starts_with('#')
            {
                out.push(target.clone());
            }
            for child in map.values() {
                collect_external_refs(child, out);
            }
        }
        Value::Array(items) => items
            .iter()
            .for_each(|child| collect_external_refs(child, out)),
        _ => {}
    }
}

// ----------------------------- summaries -----------------------------

const STATUSES: [&str; 6] = ["passed", "failed", "timeout", "xpass", "xfail", "skipped"];

fn is_broken(status: &str) -> bool {
    matches!(status, "failed" | "timeout" | "xpass")
}

/// `tag_summary`, the same shape `@lingxia/test` writes: one row per tag and
/// `(untagged)`, empty when no case is tagged.
pub fn tag_summary(cases: &[Value]) -> Value {
    let tags_of = |case: &Value| -> Vec<String> {
        case["tags"]
            .as_array()
            .map(|tags| {
                tags.iter()
                    .filter_map(|tag| tag.as_str().map(str::to_string))
                    .collect()
            })
            .unwrap_or_default()
    };
    if !cases.iter().any(|case| !tags_of(case).is_empty()) {
        return json!([]);
    }
    let mut rows: BTreeMap<String, serde_json::Map<String, Value>> = BTreeMap::new();
    for case in cases {
        let status = case["status"].as_str().unwrap_or("skipped");
        let mut tags = tags_of(case);
        if tags.is_empty() {
            tags.push(UNTAGGED.to_string());
        }
        for tag in tags {
            let row = rows.entry(tag.clone()).or_insert_with(|| {
                let mut row = serde_json::Map::new();
                row.insert("tag".into(), json!(tag));
                for key in ["total", "flaky"].iter().chain(STATUSES.iter()) {
                    row.insert((*key).into(), json!(0));
                }
                row.insert("ok".into(), json!(true));
                row
            });
            for key in ["total", status] {
                if let Some(count) = row.get_mut(key) {
                    *count = json!(count.as_u64().unwrap_or(0) + 1);
                }
            }
            if case["flaky"] == true
                && let Some(count) = row.get_mut("flaky")
            {
                *count = json!(count.as_u64().unwrap_or(0) + 1);
            }
            if is_broken(status) {
                row.insert("ok".into(), json!(false));
            }
        }
    }
    let untagged = rows.remove(UNTAGGED);
    let mut list: Vec<Value> = rows.into_values().map(Value::Object).collect();
    list.extend(untagged.map(Value::Object));
    Value::Array(list)
}

/// Refresh `coverage` from `cases`: a spec's status comes from its cases; a
/// spec the runtime listed as `not_run` stays so unless a case says
/// otherwise. Without an earlier summary, only the specs in `cases` count.
pub fn coverage_summary(
    manifest: &[ManifestEntry],
    cases: &[Value],
    previous: Option<&Value>,
) -> Value {
    // A spec's outcome across its executions (repeats): any break wins,
    // then a pass, else the first status.
    let mut outcome: HashMap<String, String> = HashMap::new();
    let mut covers: Vec<(String, String, Vec<String>)> = Vec::new();
    for case in cases {
        let (Some(id), Some(status)) = (case["id"].as_str(), case["status"].as_str()) else {
            continue;
        };
        let entry = outcome
            .entry(id.to_string())
            .or_insert_with(|| status.into());
        let rank = |status: &str| u8::from(status == "passed") + 2 * u8::from(is_broken(status));
        if rank(status) > rank(entry) {
            *entry = status.to_string();
        }
        if !covers.iter().any(|(known, _, _)| known == id) {
            let list = case["covers"]
                .as_array()
                .map(|list| {
                    list.iter()
                        .filter_map(|id| id.as_str().map(str::to_string))
                        .collect()
                })
                .unwrap_or_default();
            let title = case["title"].as_str().unwrap_or(id).to_string();
            covers.push((id.to_string(), title, list));
        }
    }
    // Specs the runtime knew about but this report has no case for.
    if let Some(ids) = previous.and_then(|previous| previous["ids"].as_array()) {
        for entry in ids {
            let Some(manifest_id) = entry["id"].as_str() else {
                continue;
            };
            for spec in entry["specs"].as_array().into_iter().flatten() {
                let Some(id) = spec["id"].as_str() else {
                    continue;
                };
                match covers.iter_mut().find(|(known, _, _)| known == id) {
                    Some((_, _, list)) if !list.iter().any(|c| c == manifest_id) => {
                        list.push(manifest_id.to_string())
                    }
                    Some(_) => {}
                    None => covers.push((
                        id.to_string(),
                        spec["title"].as_str().unwrap_or(id).to_string(),
                        vec![manifest_id.to_string()],
                    )),
                }
            }
        }
        for entry in previous
            .and_then(|previous| previous["unknown"].as_array())
            .into_iter()
            .flatten()
        {
            let Some(unknown_id) = entry["id"].as_str() else {
                continue;
            };
            for spec in entry["specs"].as_array().into_iter().flatten() {
                if let Some(id) = spec.as_str()
                    && let Some((_, _, list)) = covers.iter_mut().find(|(known, _, _)| known == id)
                    && !list.iter().any(|c| c == unknown_id)
                {
                    list.push(unknown_id.to_string());
                }
            }
        }
    }
    let known: Vec<&str> = manifest.iter().map(|entry| entry.id.as_str()).collect();
    let mut ids = Vec::new();
    let (mut covered, mut passing, mut failing) = (0, 0, 0);
    let mut uncovered = Vec::new();
    for entry in manifest {
        let specs: Vec<Value> = covers
            .iter()
            .filter(|(_, _, list)| list.iter().any(|id| id == &entry.id))
            .map(|(id, title, _)| {
                let status = outcome.get(id).map_or("not_run", String::as_str);
                json!({ "id": id, "title": title, "status": status })
            })
            .collect();
        let statuses: Vec<&str> = specs
            .iter()
            .filter_map(|spec| spec["status"].as_str())
            .collect();
        let status = if specs.is_empty() {
            "uncovered"
        } else if statuses.iter().any(|status| is_broken(status)) {
            "failed"
        } else if statuses.contains(&"passed") {
            "passed"
        } else if statuses.contains(&"xfail") {
            "xfail"
        } else if statuses.contains(&"skipped") {
            "skipped"
        } else {
            "not_run"
        };
        match status {
            "uncovered" => uncovered.push(match &entry.title {
                Some(title) => json!({ "id": entry.id, "title": title }),
                None => json!({ "id": entry.id }),
            }),
            "passed" => {
                covered += 1;
                passing += 1;
            }
            "failed" => {
                covered += 1;
                failing += 1;
            }
            _ => covered += 1,
        }
        let mut row = json!({ "id": entry.id, "status": status, "specs": specs });
        if let Some(title) = &entry.title {
            row["title"] = json!(title);
        }
        ids.push(row);
    }
    let mut unknown: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for (spec, _, list) in &covers {
        for id in list {
            if !known.contains(&id.as_str()) {
                let specs = unknown.entry(id.clone()).or_default();
                if !specs.contains(spec) {
                    specs.push(spec.clone());
                }
            }
        }
    }
    json!({
        "total": manifest.len(),
        "covered": covered,
        "passing": passing,
        "failing": failing,
        "uncovered": uncovered,
        "unknown": unknown
            .into_iter()
            .map(|(id, specs)| json!({ "id": id, "specs": specs }))
            .collect::<Vec<_>>(),
        "ids": ids,
    })
}

/// Recompute the summaries of a report lxdev wrote or regraded.
pub fn refresh_report(report: &mut Value, manifest: Option<&[ManifestEntry]>) {
    let cases = report["cases"].as_array().cloned().unwrap_or_default();
    let tags = tag_summary(&cases);
    match report.as_object_mut() {
        Some(object) if tags.as_array().is_some_and(|rows| !rows.is_empty()) => {
            object.insert("tag_summary".into(), tags);
        }
        Some(object) => {
            object.remove("tag_summary");
        }
        None => return,
    }
    if let Some(manifest) = manifest {
        let previous = report.get("coverage").cloned();
        report["coverage"] = coverage_summary(manifest, &cases, previous.as_ref());
    }
}

/// Refresh `report.json` on disk; a missing or unreadable report is left alone.
pub fn refresh_report_file(path: &Path, manifest: Option<&[ManifestEntry]>) -> Result<()> {
    let Ok(bytes) = std::fs::read(path) else {
        return Ok(());
    };
    let mut report: Value = serde_json::from_slice(&bytes)?;
    refresh_report(&mut report, manifest);
    std::fs::write(path, serde_json::to_vec_pretty(&report)?)?;
    Ok(())
}

/// Terminal lines for the tag, coverage and contract summaries of a report.
pub fn summary_lines(report: &Value) -> Vec<(bool, String)> {
    let mut lines = Vec::new();
    if let Some(rows) = report["tag_summary"].as_array()
        && !rows.is_empty()
    {
        let parts: Vec<String> = rows
            .iter()
            .map(|row| {
                let count = |key: &str| row[key].as_u64().unwrap_or(0);
                let broken = count("failed") + count("timeout") + count("xpass");
                let graded = count("total").saturating_sub(count("skipped"));
                if broken > 0 {
                    format!(
                        "{} {broken} failing/{graded}",
                        row["tag"].as_str().unwrap_or("?")
                    )
                } else {
                    format!(
                        "{} {}/{graded} ok",
                        row["tag"].as_str().unwrap_or("?"),
                        graded
                    )
                }
            })
            .collect();
        let ok = rows.iter().all(|row| row["ok"] == true);
        lines.push((ok, format!("by tag: {}", parts.join(" · "))));
    }
    let coverage = &report["coverage"];
    if coverage.is_object() {
        let count = |key: &str| coverage[key].as_u64().unwrap_or(0);
        let list = |key: &str| {
            coverage[key]
                .as_array()
                .map(|items| {
                    items
                        .iter()
                        .filter_map(|item| item["id"].as_str().map(str::to_string))
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default()
        };
        let uncovered = list("uncovered");
        let unknown = list("unknown");
        let mut line = format!(
            "coverage: {}/{} manifest ids covered, {} passing, {} failing",
            count("covered"),
            count("total"),
            count("passing"),
            count("failing")
        );
        if !uncovered.is_empty() {
            line.push_str(&format!("; no spec for {}", preview(&uncovered)));
        }
        if !unknown.is_empty() {
            line.push_str(&format!("; not in the manifest: {}", preview(&unknown)));
        }
        lines.push((
            count("failing") == 0 && uncovered.is_empty() && unknown.is_empty(),
            line,
        ));
    }
    let openapi = &report["openapi"];
    if openapi.is_object() {
        let num = |value: &Value| value.as_u64().unwrap_or(0);
        let unmatched: u64 = openapi["unmatched"]
            .as_array()
            .map(|items| items.iter().map(|item| num(&item["count"])).sum())
            .unwrap_or(0);
        let line = format!(
            "openapi: {}/{} responses validated; routed {} failed, server {} mismatched (warnings), {} unmatched",
            num(&openapi["validated"]),
            num(&openapi["responses"]),
            num(&openapi["routed"]["failed"]),
            num(&openapi["network"]["mismatched"]),
            unmatched,
        );
        lines.push((
            num(&openapi["routed"]["failed"]) == 0 && num(&openapi["network"]["mismatched"]) == 0,
            line,
        ));
    }
    lines
}

pub fn print_summaries(report_path: &Path) {
    let Some(report) = std::fs::read(report_path)
        .ok()
        .and_then(|bytes| serde_json::from_slice::<Value>(&bytes).ok())
    else {
        return;
    };
    for (ok, line) in summary_lines(&report) {
        if ok {
            eprintln!("{line}");
        } else {
            eprintln!("{}", line.yellow());
        }
    }
}

fn preview(ids: &[String]) -> String {
    let mut text = ids.iter().take(8).cloned().collect::<Vec<_>>().join(", ");
    if ids.len() > 8 {
        text.push_str(&format!(" (+{} more)", ids.len() - 8));
    }
    text
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tag_expressions_parse_and_select_like_the_runtime() {
        assert!(parse_tag_expr("routed").is_ok());
        assert!(parse_tag_expr("!live").is_ok());
        assert!(parse_tag_expr("unit, routed,!live").is_ok());
        assert!(parse_tag_expr("api:v2").is_ok());
        for bad in ["", "routed,", "!", "a b", "-x", "a&b"] {
            assert!(parse_tag_expr(bad).is_err(), "{bad:?} should be rejected");
        }
        assert!(matches_tags(&["routed"], &["routed"]));
        assert!(!matches_tags(&["unit"], &["routed"]));
        assert!(matches_tags(&["unit"], &["routed,unit"]));
        assert!(!matches_tags(&["routed", "live"], &["routed", "!live"]));
        // Untagged: fails an include, passes an exclude.
        assert!(!matches_tags(&[], &["routed"]));
        assert!(matches_tags(&[], &["!live"]));
    }

    #[test]
    fn manifests_accept_lists_covers_and_maps() {
        let list = parse_manifest(&json!(["A-1", { "id": "A-2", "title": "Two" }])).unwrap();
        assert_eq!(
            list,
            vec![
                ManifestEntry {
                    id: "A-1".into(),
                    title: None
                },
                ManifestEntry {
                    id: "A-2".into(),
                    title: Some("Two".into())
                },
            ]
        );
        let covers = parse_manifest(&json!({ "covers": ["A-1"] })).unwrap();
        assert_eq!(covers[0].id, "A-1");
        let map =
            parse_manifest(&json!({ "A-1": "One", "A-2": null, "A-3": { "title": "Three" } }))
                .unwrap();
        assert_eq!(map.len(), 3);
        assert_eq!(map[2].title.as_deref(), Some("Three"));
        assert!(
            parse_manifest(&json!(["A", "A"]))
                .unwrap_err()
                .to_string()
                .contains("twice")
        );
        assert!(parse_manifest(&json!([])).is_err());
        assert!(parse_manifest(&json!([{ "title": "x" }])).is_err());
        assert!(parse_manifest(&json!(7)).is_err());
    }

    #[test]
    fn yaml_manifest_and_openapi_files_load() {
        let dir = tempfile::tempdir().unwrap();
        let manifest = dir.path().join("coverage.yaml");
        std::fs::write(&manifest, "- id: DEV-1\n  title: List devices\n- DEV-2\n").unwrap();
        assert_eq!(load_manifest(&manifest).unwrap().len(), 2);

        let spec = dir.path().join("api.yaml");
        std::fs::write(
            &spec,
            "openapi: 3.1.0\ninfo: { title: T, version: '1' }\npaths:\n  /x:\n    get:\n      responses:\n        '200':\n          description: ok\n          content:\n            application/json:\n              schema: { $ref: '#/components/schemas/X' }\ncomponents:\n  schemas:\n    X: { type: [string, 'null'] }\n",
        )
        .unwrap();
        let doc = load_openapi(&spec).unwrap();
        assert_eq!(
            doc["paths"]["/x"]["get"]["responses"]["200"]["description"],
            "ok"
        );

        let swagger = dir.path().join("old.json");
        std::fs::write(&swagger, r#"{"swagger":"2.0","paths":{}}"#).unwrap();
        assert!(format!("{:#}", load_openapi(&swagger).unwrap_err()).contains("Swagger 2.0"));
        let split = dir.path().join("split.json");
        std::fs::write(
            &split,
            r##"{"openapi":"3.0.3","paths":{"/x":{"$ref":"paths/x.yaml"}}}"##,
        )
        .unwrap();
        assert!(format!("{:#}", load_openapi(&split).unwrap_err()).contains("bundle it"));

        let inputs = RunInputs::load(Some(&manifest), std::slice::from_ref(&spec)).unwrap();
        let mut control = HashMap::new();
        inputs.add_controls(&mut control).unwrap();
        assert!(control["openapi"].contains("\"name\""));
        assert!(control["openapiFiles"].ends_with("api.yaml"));
        assert!(control["coversManifest"].contains("DEV-2"));
        let shown = reported_control(&control);
        assert!(!shown.contains_key("openapi") && !shown.contains_key("coversManifest"));
        assert!(shown.contains_key("openapiFiles") && shown.contains_key("coversManifestFile"));
        assert!(RunInputs::load(None, &[spec.clone(), spec]).is_err());
    }

    #[test]
    fn summaries_follow_regraded_cases() {
        let cases = vec![
            json!({ "id": "a", "title": "A", "status": "passed", "tags": ["routed"], "covers": ["M-1"] }),
            json!({ "id": "b", "title": "B", "status": "timeout", "tags": ["live"], "covers": ["M-2", "X-9"] }),
            json!({ "id": "c", "title": "C", "status": "passed", "flaky": true }),
        ];
        let rows = tag_summary(&cases);
        let rows = rows.as_array().unwrap();
        assert_eq!(rows.len(), 3);
        assert_eq!(rows[0]["tag"], "live");
        assert_eq!(rows[0]["ok"], false);
        assert_eq!(rows[0]["timeout"], 1);
        assert_eq!(rows[2]["tag"], UNTAGGED);
        assert_eq!(rows[2]["flaky"], 1);
        assert_eq!(tag_summary(&[json!({ "status": "passed" })]), json!([]));

        let manifest = vec![
            ManifestEntry {
                id: "M-1".into(),
                title: None,
            },
            ManifestEntry {
                id: "M-2".into(),
                title: Some("two".into()),
            },
            ManifestEntry {
                id: "M-3".into(),
                title: None,
            },
            ManifestEntry {
                id: "M-4".into(),
                title: None,
            },
        ];
        let previous = json!({ "ids": [{ "id": "M-4", "specs": [{ "id": "z", "title": "Z", "status": "not_run" }] }] });
        let coverage = coverage_summary(&manifest, &cases, Some(&previous));
        assert_eq!(coverage["covered"], 3);
        assert_eq!(coverage["passing"], 1);
        assert_eq!(coverage["failing"], 1);
        assert_eq!(coverage["uncovered"], json!([{ "id": "M-3" }]));
        assert_eq!(
            coverage["unknown"],
            json!([{ "id": "X-9", "specs": ["b"] }])
        );
        assert_eq!(coverage["ids"][3]["status"], "not_run");

        let mut report = json!({ "cases": cases, "coverage": previous });
        refresh_report(&mut report, Some(&manifest));
        let lines = summary_lines(&report);
        assert_eq!(lines.len(), 2);
        assert!(lines[0].1.contains("live 1 failing/1"), "{}", lines[0].1);
        assert!(!lines[0].0);
        assert!(
            lines[1].1.contains("3/4 manifest ids covered"),
            "{}",
            lines[1].1
        );
        assert!(lines[1].1.contains("no spec for M-3"), "{}", lines[1].1);
    }
}
