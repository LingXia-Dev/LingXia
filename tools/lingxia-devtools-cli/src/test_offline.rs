//! `lxdev test --list` without a session: the specs as the source states
//! them.
//!
//! The exact listing needs the runtime, which runs the files: a title built
//! in a template, an id from a variable, or a spec registered in a loop is
//! only known then. Offline, each spec call is read from the source (Oxc), the
//! run's selection is applied where the values it needs are literals, and a
//! spec whose selection or identity depends on a computed value is kept and
//! marked inexact rather than guessed.

use crate::test_select::{Literal, Locations, SpecCall, SpecMap};
use anyhow::{Context, Result, anyhow, bail};
use serde_json::{Value, json};
use std::path::Path;

/// One spec call, with what the source says about it.
#[derive(Debug, Clone, PartialEq)]
pub struct StaticSpec {
    /// As `--list` shows files.
    pub file: String,
    pub line: u32,
    pub title: Option<String>,
    /// The id the runtime will give it, when that follows from the source.
    pub id: Option<String>,
    /// Its tags and its file's, when all are literals.
    pub tags: Option<Vec<String>>,
    /// Everything shown and every filter applied is known from the source.
    pub exact: bool,
}

/// The run's selection controls, as far as the source can answer them.
#[derive(Default)]
pub struct Filters {
    pub grep: Option<regex::Regex>,
    pub id: Option<String>,
    pub ids: Option<Vec<String>>,
    pub tags: Vec<Vec<(String, bool)>>,
    pub shard: Option<(u32, u32)>,
    pub locations: Option<Locations>,
    pub forbid_only: bool,
}

impl Filters {
    pub fn new(
        grep: Option<&str>,
        id: Option<&str>,
        ids: Option<Vec<String>>,
        tags: &[String],
        shard: Option<&str>,
        locations: Option<Locations>,
        forbid_only: bool,
    ) -> Result<Self> {
        let grep = grep
            .map(|pattern| {
                regex::Regex::new(pattern).map_err(|err| {
                    anyhow!(
                        "--grep {pattern:?} cannot be matched without a session ({err}); \
                         start one with `lingxia dev` to list with it"
                    )
                })
            })
            .transpose()?;
        Ok(Self {
            grep,
            id: id.map(str::to_string),
            ids,
            tags: tags
                .iter()
                .map(|raw| parse_tag_clause(raw))
                .collect::<Result<_>>()?,
            shard: shard.map(parse_shard).transpose()?,
            locations,
            forbid_only,
        })
    }
}

/// A tag, as `@lingxia/test` validates one.
fn is_tag(tag: &str) -> bool {
    let mut chars = tag.chars();
    chars.next().is_some_and(|c| c.is_ascii_alphanumeric())
        && chars.all(|c| c.is_ascii_alphanumeric() || "_.:/-".contains(c))
}

/// One `--tag` value: comma-separated alternatives, `!tag` negated.
fn parse_tag_clause(source: &str) -> Result<Vec<(String, bool)>> {
    source
        .split(',')
        .map(|raw| {
            let term = raw.trim();
            let (tag, negated) = match term.strip_prefix('!') {
                Some(tag) => (tag.trim(), true),
                None => (term, false),
            };
            if !is_tag(tag) {
                bail!("--tag {source:?}: {term:?} is not a tag or !tag");
            }
            Ok((tag.to_string(), negated))
        })
        .collect()
}

fn parse_shard(raw: &str) -> Result<(u32, u32)> {
    let parsed = raw
        .split_once('/')
        .and_then(|(index, total)| Some((index.parse().ok()?, total.parse().ok()?)))
        .filter(|&(index, total): &(u32, u32)| index >= 1 && total >= 1 && index <= total);
    parsed.ok_or_else(|| anyhow!("shard must be INDEX/TOTAL (1-based)"))
}

/// `@lingxia/test`'s `slugTitle`: the id of a spec that gives none.
fn slug_title(title: &str) -> Option<String> {
    if !title.is_ascii() {
        return None;
    }
    let mut slug = String::new();
    let mut gap = false;
    for c in title.chars() {
        if c.is_ascii_alphanumeric() {
            if gap && !slug.is_empty() {
                slug.push('-');
            }
            gap = false;
            slug.push(c.to_ascii_lowercase());
        } else {
            gap = true;
        }
    }
    (!slug.is_empty()).then_some(slug)
}

/// `@lingxia/test`'s `stableHash` (FNV-1a over UTF-16 code units), which
/// `--shard` buckets ids by.
fn stable_hash(value: &str) -> u32 {
    value.encode_utf16().fold(2_166_136_261u32, |hash, unit| {
        (hash ^ u32::from(unit)).wrapping_mul(16_777_619)
    })
}

/// Whether the spec is selected: `Some(bool)` when the source answers,
/// `None` when a computed value decides.
type Verdict = Option<bool>;

fn all(verdicts: impl IntoIterator<Item = Verdict>) -> Verdict {
    let mut known = true;
    for verdict in verdicts {
        match verdict {
            Some(false) => return Some(false),
            Some(true) => {}
            None => known = false,
        }
    }
    known.then_some(true)
}

fn by_id(id: Option<&str>, test: impl Fn(&str) -> bool) -> Verdict {
    id.map(test)
}

/// The listing of one file's spec calls. `source_name` is how the run's
/// `locations` control names the file.
pub fn file_specs(
    map: &SpecMap,
    shown: &str,
    source_name: &str,
    filters: &Filters,
    has_only: bool,
) -> Vec<StaticSpec> {
    let mut out = Vec::new();
    for call in &map.calls {
        let id = resolved_id(call);
        let tags = match (&map.file_tags, &call.tags) {
            (Literal::Known(file), Literal::Known(own)) => {
                let mut tags: Vec<String> = Vec::new();
                for tag in file.iter().chain(own) {
                    if !tags.contains(tag) {
                        tags.push(tag.clone());
                    }
                }
                Some(tags)
            }
            _ => None,
        };
        let verdict = all([
            Some(!has_only || call.modifier.as_deref() == Some("only")),
            Some(at_location(filters.locations.as_ref(), source_name, call)),
            filters.ids.as_ref().map_or(Some(true), |ids| {
                by_id(id.as_deref(), |id| ids.iter().any(|i| i == id))
            }),
            filters.shard.map_or(Some(true), |(index, total)| {
                by_id(id.as_deref(), |id| stable_hash(id) % total == index - 1)
            }),
            filters
                .id
                .as_ref()
                .map_or(Some(true), |wanted| by_id(id.as_deref(), |id| id == wanted)),
            if filters.tags.is_empty() {
                Some(true)
            } else {
                tags.as_ref().map(|tags| matches_tags(tags, &filters.tags))
            },
            filters.grep.as_ref().map_or(Some(true), |grep| {
                grep_verdict(grep, call.title.as_deref(), id.as_deref())
            }),
        ]);
        if verdict == Some(false) {
            continue;
        }
        let exact = verdict.is_some()
            && !call.repeated
            && call.title.is_some()
            && id.is_some()
            && tags.is_some();
        out.push(StaticSpec {
            file: shown.to_string(),
            line: call.from,
            title: call.title.clone(),
            id,
            tags,
            exact,
        });
    }
    out
}

fn resolved_id(call: &SpecCall) -> Option<String> {
    match &call.id {
        Literal::Known(Some(id)) if !id.is_empty() => Some(id.clone()),
        Literal::Known(_) => call.title.as_deref().and_then(slug_title),
        Literal::Computed => None,
    }
}

fn at_location(locations: Option<&Locations>, source_name: &str, call: &SpecCall) -> bool {
    let Some(locations) = locations else {
        return true;
    };
    match locations.get(source_name) {
        None => false,
        Some(None) => true,
        Some(Some(ranges)) => ranges
            .iter()
            .any(|&(from, to)| from <= call.from && call.from <= to),
    }
}

fn matches_tags(tags: &[String], clauses: &[Vec<(String, bool)>]) -> bool {
    clauses.iter().all(|clause| {
        clause
            .iter()
            .any(|(tag, negated)| tags.contains(tag) != *negated)
    })
}

fn grep_verdict(grep: &regex::Regex, title: Option<&str>, id: Option<&str>) -> Verdict {
    if title.is_some_and(|title| grep.is_match(title)) || id.is_some_and(|id| grep.is_match(id)) {
        return Some(true);
    }
    (title.is_some() && id.is_some()).then_some(false)
}

/// The offline listing of `files`, in file order.
pub fn list(
    files: &[std::path::PathBuf],
    root: &Path,
    cwd: &Path,
    filters: &Filters,
) -> Result<Vec<StaticSpec>> {
    let mut maps = Vec::with_capacity(files.len());
    for file in files {
        let source = std::fs::read_to_string(file)
            .with_context(|| format!("cannot read {}", file.display()))?;
        maps.push((file, SpecMap::parse(&source, file)?));
    }
    let has_only = maps.iter().any(|(_, map)| {
        map.calls
            .iter()
            .any(|call| call.modifier.as_deref() == Some("only"))
    });
    if has_only && filters.forbid_only {
        bail!("spec.only is registered; lxdev test --forbid-only refuses to run");
    }
    let mut out = Vec::new();
    for (file, map) in &maps {
        let shown = crate::test_select::display_path(file, cwd);
        let source_name = crate::test_bundle::source_name(file, root);
        out.extend(file_specs(map, &shown, &source_name, filters, has_only));
    }
    Ok(out)
}

/// The JSON form of one spec: unknown values are `null`.
pub fn to_json(spec: &StaticSpec) -> Value {
    json!({
        "id": spec.id,
        "title": spec.title,
        "file": spec.file,
        "line": spec.line,
        "tags": spec.tags,
        "exact": spec.exact,
    })
}

/// The text form: what `format_listing` prints, with a placeholder for what
/// only a run can tell and `~` after an inexact row.
pub fn to_listing_row(spec: &StaticSpec) -> Value {
    let title = spec.title.as_deref().unwrap_or("(computed title)");
    json!({
        "id": spec.id.as_deref().unwrap_or("?"),
        "title": if spec.exact { title.to_string() } else { format!("{title}  ~") },
        "file": spec.file,
        "line": spec.line,
        "tags": spec.tags.clone().unwrap_or_else(|| vec!["?".to_string()]),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const SOURCE: &str = r#"import { spec } from "@lingxia/test";

spec.configure({ tags: ["routed"] });

spec("adds an item", { id: "cart-add", tags: ["smoke"] }, async () => {});

spec(`removes an item`, async () => {});

spec.skip("商品", async () => {});

spec(title, { id: dynamicId }, async () => {});

for (const name of ["a", "b"]) {
  spec(`case ${name}`, async () => {});
}

spec("computed tags", { tags: TAGS }, async () => {});
"#;

    fn map() -> SpecMap {
        SpecMap::parse(SOURCE, Path::new("cart.test.ts")).unwrap()
    }

    fn listed(filters: &Filters) -> Vec<StaticSpec> {
        file_specs(
            &map(),
            "tests/cart.test.ts",
            "tests/cart.test.ts",
            filters,
            false,
        )
    }

    fn filters(
        grep: Option<&str>,
        id: Option<&str>,
        tags: &[&str],
        shard: Option<&str>,
    ) -> Filters {
        let tags: Vec<String> = tags.iter().map(|tag| tag.to_string()).collect();
        Filters::new(grep, id, None, &tags, shard, None, false).unwrap()
    }

    #[test]
    fn literals_give_the_title_id_and_tags_a_run_would() {
        let specs = listed(&Filters::default());
        let rows: Vec<_> = specs
            .iter()
            .map(|spec| {
                (
                    spec.line,
                    spec.title.as_deref(),
                    spec.id.as_deref(),
                    spec.tags.clone(),
                    spec.exact,
                )
            })
            .collect();
        let routed = |more: &[&str]| {
            Some(
                ["routed"]
                    .iter()
                    .chain(more)
                    .map(|tag| tag.to_string())
                    .collect::<Vec<_>>(),
            )
        };
        assert_eq!(
            rows,
            [
                (
                    5,
                    Some("adds an item"),
                    Some("cart-add"),
                    routed(&["smoke"]),
                    true
                ),
                (
                    7,
                    Some("removes an item"),
                    Some("removes-an-item"),
                    routed(&[]),
                    true
                ),
                // A non-ASCII title's id is numbered at run time.
                (9, Some("商品"), None, routed(&[]), false),
                (11, None, None, routed(&[]), false),
                // Registered once per loop iteration: the count is the run's.
                (14, None, None, routed(&[]), false),
                (
                    17,
                    Some("computed tags"),
                    Some("computed-tags"),
                    None,
                    false
                ),
            ]
        );
    }

    #[test]
    fn a_filter_the_source_answers_drops_and_one_it_cannot_keeps_inexact() {
        let lines = |filters: Filters| {
            listed(&filters)
                .iter()
                .map(|spec| (spec.line, spec.exact))
                .collect::<Vec<_>>()
        };
        assert_eq!(
            lines(filters(Some("^adds"), None, &[], None)),
            [(5, true), (9, false), (11, false), (14, false)]
        );
        assert_eq!(
            lines(filters(None, Some("cart-add"), &[], None)),
            [(5, true), (9, false), (11, false), (14, false)]
        );
        assert_eq!(
            lines(filters(None, None, &["smoke"], None)),
            [(5, true), (17, false)]
        );
        assert_eq!(
            lines(filters(None, None, &["!smoke,routed"], None)),
            [
                (5, true),
                (7, true),
                (9, false),
                (11, false),
                (14, false),
                (17, false)
            ]
        );
    }

    #[test]
    fn only_and_locations_narrow_as_the_runtime_does() {
        let source = "spec.only('a', async () => {});\nspec('b', async () => {});\n";
        let map = SpecMap::parse(source, Path::new("a.test.ts")).unwrap();
        let specs = file_specs(&map, "a.test.ts", "a.test.ts", &Filters::default(), true);
        assert_eq!(specs.iter().map(|s| s.line).collect::<Vec<_>>(), [1]);

        let mut locations = Locations::new();
        locations.insert("a.test.ts".into(), Some(vec![(2, 2)]));
        let filters = Filters {
            locations: Some(locations),
            ..Filters::default()
        };
        let specs = file_specs(&map, "a.test.ts", "a.test.ts", &filters, false);
        assert_eq!(specs.iter().map(|s| s.line).collect::<Vec<_>>(), [2]);
        let specs = file_specs(&map, "a.test.ts", "other.test.ts", &filters, false);
        assert!(specs.is_empty());
    }

    #[test]
    fn ids_and_shards_follow_the_runtime() {
        assert_eq!(slug_title("Adds an item!").as_deref(), Some("adds-an-item"));
        assert_eq!(slug_title("  --  ").as_deref(), None);
        assert_eq!(slug_title("é").as_deref(), None);
        // The runtime's FNV-1a: `stableHash("a")`.
        assert_eq!(stable_hash("a"), 0xe40c292c);
        let shards: Vec<Vec<u32>> = (1..=2)
            .map(|index| {
                listed(&filters(None, None, &[], Some(&format!("{index}/2"))))
                    .iter()
                    .filter(|spec| spec.exact)
                    .map(|spec| spec.line)
                    .collect()
            })
            .collect();
        let mut both: Vec<u32> = shards.concat();
        both.sort();
        assert_eq!(both, [5, 7], "each exact spec is in exactly one shard");
    }

    #[test]
    fn bad_filters_are_errors() {
        assert!(Filters::new(None, None, None, &["bad tag".into()], None, None, false).is_err());
        assert!(Filters::new(None, None, None, &[], Some("3/2"), None, false).is_err());
        assert!(Filters::new(Some("(?<=a)b"), None, None, &[], None, None, false).is_err());
    }

    #[test]
    fn forbid_only_refuses_as_a_run_would() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("a.test.ts");
        std::fs::write(&file, "spec.only('a', async () => {});\n").unwrap();
        let filters = Filters {
            forbid_only: true,
            ..Filters::default()
        };
        let err = list(&[file], dir.path(), dir.path(), &filters).unwrap_err();
        assert!(err.to_string().contains("--forbid-only"));
    }
}
