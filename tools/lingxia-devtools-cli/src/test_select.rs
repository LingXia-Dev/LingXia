//! What `lxdev test` runs: its `PATH[:LINE]` arguments, the default entry,
//! and the spec calls a `FILE:LINE` names, found by parsing the file.

use crate::test_bundle::{collect_test_files, find_project_root, source_name};
use anyhow::{Context, Result, anyhow};
use oxc_allocator::Allocator;
use oxc_ast::AstKind;
use oxc_ast::ast::{Argument, Expression, ImportDeclarationSpecifier};
use oxc_ast_visit::Visit;
use oxc_parser::Parser;
use oxc_span::{GetSpan, SourceType};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// One positional argument: a file or directory, and for a file an optional
/// line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TestPath {
    pub path: PathBuf,
    pub line: Option<u32>,
    /// As given, for the rerun command.
    pub raw: String,
}

/// `PATH` or `PATH:LINE`. Only the last colon counts, and only before
/// digits, so `C:\x\a.test.ts:42` is `C:\x\a.test.ts` line 42.
pub fn parse_test_path(raw: &str) -> Result<TestPath, String> {
    if raw.is_empty() {
        return Err("a test path cannot be empty".into());
    }
    let (path, line) = match raw.rsplit_once(':') {
        Some((path, digits))
            if !path.is_empty()
                && !digits.is_empty()
                && digits.bytes().all(|b| b.is_ascii_digit())
                // `C:` alone is a drive, not a path with a line.
                && !(path.len() == 1 && path.as_bytes()[0].is_ascii_alphabetic()) =>
        {
            let line: u32 = digits
                .parse()
                .map_err(|_| format!("line {digits} is out of range"))?;
            if line == 0 {
                return Err(format!("{raw}: lines start at 1"));
            }
            (path, Some(line))
        }
        _ => (raw, None),
    };
    Ok(TestPath {
        path: PathBuf::from(path),
        line,
        raw: raw.to_string(),
    })
}

/// A usage error: exit code 2, like a bad flag.
pub fn usage(message: impl std::fmt::Display) -> anyhow::Error {
    clap::Error::raw(
        clap::error::ErrorKind::ValueValidation,
        format!("{message}\n"),
    )
    .into()
}

/// Source name → the line ranges selected in it, `None` for all.
pub type Locations = BTreeMap<String, Option<Vec<(u32, u32)>>>;

/// The files a run bundles, and the lines `FILE:LINE` arguments select.
#[derive(Debug)]
pub struct Selection {
    /// Canonical test files, in argument order, each once.
    pub files: Vec<PathBuf>,
    /// The one path given, or the project root for several; names the
    /// bundle and anchors its source names.
    pub identity: PathBuf,
    /// The project the files belong to.
    pub root: PathBuf,
    /// The `locations` run control: every file, by source name, to the line
    /// ranges of the spec calls it selects (`None`: all of them). Only when
    /// a `FILE:LINE` was given.
    pub locations: Option<Locations>,
    /// The arguments as given (or the default entry), for rerun commands.
    pub shown: Vec<String>,
}

pub const DEFAULT_ENTRY_DIR: &str = "tests";

/// Resolve the positional arguments, relative to `cwd`. Without any (and
/// without `test.entry` in lxdev.json, which the preset expansion has
/// already put in) the project's `tests/` directory.
pub fn select(paths: &[TestPath], cwd: &Path) -> Result<Selection> {
    let defaulted;
    let paths = if paths.is_empty() {
        let root = find_project_root(cwd);
        let dir = root.join(DEFAULT_ENTRY_DIR);
        if !dir.is_dir() {
            return Err(usage(format!(
                "no tests to run: {} does not exist. Pass a file or directory \
                 (`lxdev test path/to/specs`), or set `test.entry` in lxdev.json",
                dir.display()
            )));
        }
        let raw = display_path(&dir, cwd);
        defaulted = vec![TestPath {
            path: dir,
            line: None,
            raw,
        }];
        &defaulted[..]
    } else {
        paths
    };

    let mut resolved = Vec::with_capacity(paths.len());
    for arg in paths {
        let absolute = if arg.path.is_absolute() {
            arg.path.clone()
        } else {
            cwd.join(&arg.path)
        };
        let canonical = std::fs::canonicalize(&absolute)
            .map_err(|_| usage(format!("{}: no such file or directory", arg.path.display())))?;
        if arg.line.is_some() && canonical.is_dir() {
            return Err(usage(format!(
                "{}: a line needs a file, and this is a directory",
                arg.raw
            )));
        }
        resolved.push((arg, canonical));
    }

    let identity = if resolved.len() == 1 {
        resolved[0].1.clone()
    } else {
        find_project_root(&resolved[0].1)
    };
    let root = find_project_root(&identity);

    let mut files: Vec<PathBuf> = Vec::new();
    let mut whole: Vec<PathBuf> = Vec::new();
    let mut lines: BTreeMap<PathBuf, Vec<(u32, u32)>> = BTreeMap::new();
    for (arg, path) in &resolved {
        let found = if path.is_dir() {
            collect_test_files(path)?
        } else {
            vec![path.clone()]
        };
        for file in found {
            if !files.contains(&file) {
                files.push(file.clone());
            }
            match arg.line {
                None => whole.push(file),
                Some(line) => {
                    let ranges = spec_ranges_at(&file, line, &display_path(&file, cwd))?;
                    lines.entry(file).or_default().extend(ranges);
                }
            }
        }
    }
    let locations = (!lines.is_empty()).then(|| {
        files
            .iter()
            .map(|file| {
                let ranges = if whole.contains(file) {
                    None
                } else {
                    lines.get(file).cloned()
                };
                (source_name(file, &root), ranges)
            })
            .collect()
    });
    Ok(Selection {
        files,
        identity,
        root,
        locations,
        shown: paths.iter().map(|arg| arg.raw.clone()).collect(),
    })
}

/// `path` relative to `cwd` when it is inside it, else as is.
pub fn display_path(path: &Path, cwd: &Path) -> String {
    let cwd = std::fs::canonicalize(cwd).unwrap_or_else(|_| cwd.to_path_buf());
    let path = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    match path.strip_prefix(&cwd) {
        Ok(relative) if relative.as_os_str().is_empty() => ".".into(),
        Ok(relative) => relative.to_string_lossy().replace('\\', "/"),
        Err(_) => path.to_string_lossy().into_owned(),
    }
}

/// `--list` text: one `file:line  id  title  [tags]` line per spec, the
/// first two columns aligned.
pub fn format_listing(specs: &[serde_json::Value]) -> String {
    let rows: Vec<(String, &str, &str, String)> = specs
        .iter()
        .map(|spec| {
            let tags = spec["tags"]
                .as_array()
                .map(|tags| {
                    tags.iter()
                        .filter_map(|tag| tag.as_str())
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            (
                format!(
                    "{}:{}",
                    spec["file"].as_str().unwrap_or("?"),
                    spec["line"].as_u64().unwrap_or(0)
                ),
                spec["id"].as_str().unwrap_or(""),
                spec["title"].as_str().unwrap_or(""),
                if tags.is_empty() {
                    String::new()
                } else {
                    format!("  [{}]", tags.join(", "))
                },
            )
        })
        .collect();
    let at = rows.iter().map(|row| row.0.len()).max().unwrap_or(0);
    let id = rows.iter().map(|row| row.1.len()).max().unwrap_or(0);
    rows.iter()
        .map(|(location, spec_id, title, tags)| {
            format!("{location:<at$}  {spec_id:<id$}  {title}{tags}\n")
        })
        .collect()
}

/// A spec registration in a source file: `spec(…)`, `spec.only(…)`, ….
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpecCall {
    pub from: u32,
    pub to: u32,
    pub title: Option<String>,
}

/// Every spec call of a file, and the line span of every other syntax node
/// that contains one (a loop, a helper function, a grouping call).
#[derive(Debug, Default)]
pub struct SpecMap {
    pub calls: Vec<SpecCall>,
    groups: Vec<(u32, u32)>,
}

const SPEC_MODIFIERS: [&str; 4] = ["skip", "only", "fixme", "fail"];

impl SpecMap {
    pub fn parse(source: &str, path: &Path) -> Result<Self> {
        let source_type = SourceType::from_path(path)
            .map_err(|_| anyhow!("{}: not a JavaScript or TypeScript file", path.display()))?;
        let allocator = Allocator::default();
        let parsed = Parser::new(&allocator, source, source_type).parse();
        if !parsed.diagnostics.is_empty() {
            return Err(anyhow!(
                "{}: cannot parse it to find its specs",
                path.display()
            ));
        }
        let mut names = vec!["spec".to_string()];
        for statement in &parsed.program.body {
            if let oxc_ast::ast::Statement::ImportDeclaration(import) = statement {
                for specifier in import.specifiers.iter().flatten() {
                    if let ImportDeclarationSpecifier::ImportSpecifier(specifier) = specifier
                        && crate::test_bundle::module_export_name(&specifier.imported).as_deref()
                            == Some("spec")
                    {
                        names.push(specifier.local.name.as_str().to_string());
                    }
                }
            }
        }
        let mut finder = SpecFinder {
            names,
            lines: LineIndex::new(source),
            calls: Vec::new(),
            nodes: Vec::new(),
        };
        finder.visit_program(&parsed.program);
        let groups = finder
            .nodes
            .iter()
            .copied()
            .filter(|&(from, to)| {
                finder
                    .calls
                    .iter()
                    .any(|call| from <= call.from && call.to <= to)
            })
            .collect();
        Ok(Self {
            calls: finder.calls,
            groups,
        })
    }

    /// The spec calls `line` selects: the one it is in, else every one in
    /// the smallest node around it that has any. `Err` lists the nearest.
    pub fn at(&self, line: u32) -> Result<Vec<&SpecCall>, Vec<&SpecCall>> {
        let enclosing = self
            .calls
            .iter()
            .map(|call| (call.from, call.to))
            .chain(self.groups.iter().copied())
            .filter(|&(from, to)| from <= line && line <= to)
            .min_by_key(|&(from, to)| to - from);
        if let Some((from, to)) = enclosing {
            return Ok(self
                .calls
                .iter()
                .filter(|call| from <= call.from && call.to <= to)
                .collect());
        }
        let mut nearest: Vec<&SpecCall> = self.calls.iter().collect();
        nearest.sort_by_key(|call| {
            if line < call.from {
                call.from - line
            } else {
                line - call.to
            }
        });
        nearest.truncate(3);
        nearest.sort_by_key(|call| call.from);
        Err(nearest)
    }
}

/// The line ranges of the spec calls `file:line` selects.
fn spec_ranges_at(file: &Path, line: u32, shown: &str) -> Result<Vec<(u32, u32)>> {
    let source =
        std::fs::read_to_string(file).with_context(|| format!("cannot read {}", file.display()))?;
    let map = SpecMap::parse(&source, file)?;
    match map.at(line) {
        Ok(calls) => Ok(calls.iter().map(|call| (call.from, call.to)).collect()),
        Err(nearest) if nearest.is_empty() => {
            Err(usage(format!("{shown}:{line}: {shown} has no spec calls")))
        }
        Err(nearest) => {
            let listed = nearest
                .iter()
                .map(|call| {
                    format!(
                        "  {shown}:{}  {}",
                        call.from,
                        call.title.as_deref().unwrap_or("(computed title)")
                    )
                })
                .collect::<Vec<_>>()
                .join("\n");
            Err(usage(format!(
                "{shown}:{line} is not inside a spec. Nearest:\n{listed}"
            )))
        }
    }
}

struct LineIndex {
    starts: Vec<u32>,
}

impl LineIndex {
    fn new(source: &str) -> Self {
        let mut starts = vec![0];
        starts.extend(
            source
                .bytes()
                .enumerate()
                .filter(|(_, byte)| *byte == b'\n')
                .map(|(index, _)| index as u32 + 1),
        );
        Self { starts }
    }

    /// 1-based line of a byte offset.
    fn line(&self, offset: u32) -> u32 {
        self.starts.partition_point(|&start| start <= offset) as u32
    }
}

struct SpecFinder {
    names: Vec<String>,
    lines: LineIndex,
    calls: Vec<SpecCall>,
    nodes: Vec<(u32, u32)>,
}

impl SpecFinder {
    fn is_spec_callee(&self, callee: &Expression<'_>) -> bool {
        let named = |expression: &Expression<'_>| matches!(expression, Expression::Identifier(id) if self.names.iter().any(|name| name == id.name.as_str()));
        match callee {
            Expression::StaticMemberExpression(member) => {
                named(&member.object) && SPEC_MODIFIERS.contains(&member.property.name.as_str())
            }
            other => named(other),
        }
    }
}

impl<'a> Visit<'a> for SpecFinder {
    fn enter_node(&mut self, kind: AstKind<'a>) {
        if matches!(kind, AstKind::Program(_)) {
            return;
        }
        let span = kind.span();
        let range = (self.lines.line(span.start), self.lines.line(span.end));
        if let AstKind::CallExpression(call) = kind
            && self.is_spec_callee(&call.callee)
        {
            let title = match call.arguments.first() {
                Some(Argument::StringLiteral(literal)) => Some(literal.value.as_str().to_string()),
                Some(Argument::TemplateLiteral(template)) => template
                    .single_quasi()
                    .map(|text| text.as_str().to_string()),
                _ => None,
            };
            self.calls.push(SpecCall {
                from: range.0,
                to: range.1,
                title,
            });
            return;
        }
        self.nodes.push(range);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_listing_is_one_aligned_line_per_spec() {
        let specs = [
            serde_json::json!({"file": "tests/cart.test.ts", "line": 3, "id": "first", "title": "first", "tags": ["unit", "smoke"]}),
            serde_json::json!({"file": "tests/a.test.ts", "line": 12, "id": "S-2", "title": "second", "tags": []}),
        ];
        assert_eq!(
            format_listing(&specs),
            "tests/cart.test.ts:3  first  first  [unit, smoke]\n\
             tests/a.test.ts:12    S-2    second\n"
        );
        assert_eq!(format_listing(&[]), "");
    }

    #[test]
    fn a_path_takes_a_line_after_its_last_colon() {
        let at = |raw: &str| {
            let parsed = parse_test_path(raw).unwrap();
            (parsed.path.to_string_lossy().into_owned(), parsed.line)
        };
        assert_eq!(at("tests/"), ("tests/".into(), None));
        assert_eq!(
            at("tests/cart.test.ts"),
            ("tests/cart.test.ts".into(), None)
        );
        assert_eq!(
            at("tests/cart.test.ts:42"),
            ("tests/cart.test.ts".into(), Some(42))
        );
        assert_eq!(
            at(r"C:\x\a.test.ts:42"),
            (r"C:\x\a.test.ts".into(), Some(42))
        );
        assert_eq!(at(r"C:\x\a.test.ts"), (r"C:\x\a.test.ts".into(), None));
        assert_eq!(at("C:/x/a.test.ts:7"), ("C:/x/a.test.ts".into(), Some(7)));
        // Not digits after the colon: part of the name.
        assert_eq!(at("odd:name.test.ts"), ("odd:name.test.ts".into(), None));
        assert_eq!(at("a.test.ts:"), ("a.test.ts:".into(), None));
        assert_eq!(at("a.test.ts:4x"), ("a.test.ts:4x".into(), None));
        assert!(parse_test_path("a.test.ts:0").is_err());
        assert!(parse_test_path("").is_err());
    }

    const SOURCE: &str = r#"import { spec as it } from "@lingxia/test";

it("first", async () => {
  await 1;
});

it.skip(`second`, { id: "S-2" }, async () => {});

for (const name of ["a", "b"]) {
  it(name, async () => {});
}

spec.beforeEach(async () => {});
"#;

    fn map() -> SpecMap {
        SpecMap::parse(SOURCE, Path::new("cart.test.ts")).unwrap()
    }

    #[test]
    fn spec_calls_are_found_with_their_lines_and_titles() {
        let calls = map().calls;
        assert_eq!(
            calls
                .iter()
                .map(|c| (c.from, c.to, c.title.as_deref()))
                .collect::<Vec<_>>(),
            [
                (3, 5, Some("first")),
                (7, 7, Some("second")),
                (10, 10, None)
            ]
        );
    }

    #[test]
    fn a_line_selects_its_spec_or_every_spec_of_its_group() {
        let map = map();
        let lines = |line| {
            map.at(line)
                .unwrap()
                .iter()
                .map(|c| c.from)
                .collect::<Vec<_>>()
        };
        assert_eq!(lines(3), [3]);
        assert_eq!(lines(4), [3]);
        assert_eq!(lines(5), [3]);
        assert_eq!(lines(7), [7]);
        // The loop around a spec call is its group.
        assert_eq!(lines(9), [10]);
        assert_eq!(lines(11), [10]);
    }

    #[test]
    fn a_line_outside_every_spec_names_the_nearest() {
        let map = map();
        let nearest = |line| {
            map.at(line)
                .unwrap_err()
                .iter()
                .map(|c| c.from)
                .collect::<Vec<_>>()
        };
        assert_eq!(nearest(1), [3, 7, 10]);
        assert_eq!(nearest(6), [3, 7, 10]);
        // A hook is not a spec.
        assert_eq!(nearest(13), [3, 7, 10]);
    }

    fn project() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("package.json"), "{}").unwrap();
        std::fs::create_dir_all(dir.path().join("tests/nested")).unwrap();
        std::fs::write(dir.path().join("tests/cart.test.ts"), SOURCE).unwrap();
        std::fs::write(dir.path().join("tests/nested/home.test.ts"), SOURCE).unwrap();
        std::fs::write(dir.path().join("tests/helper.ts"), "").unwrap();
        dir
    }

    fn paths(raw: &[&str]) -> Vec<TestPath> {
        raw.iter()
            .map(|raw| parse_test_path(raw).unwrap())
            .collect()
    }

    fn names(selection: &Selection) -> Vec<String> {
        selection
            .files
            .iter()
            .map(|file| source_name(file, &selection.root))
            .collect()
    }

    #[test]
    fn without_paths_the_project_tests_directory_runs() {
        let dir = project();
        let selection = select(&[], &dir.path().join("tests/nested")).unwrap();
        assert_eq!(
            names(&selection),
            ["tests/cart.test.ts", "tests/nested/home.test.ts"]
        );
        assert!(selection.locations.is_none());

        let empty = tempfile::tempdir().unwrap();
        std::fs::write(empty.path().join("package.json"), "{}").unwrap();
        let error = select(&[], empty.path()).unwrap_err();
        assert!(error.downcast_ref::<clap::Error>().is_some());
        assert!(format!("{error}").contains("test.entry"), "{error}");
    }

    #[test]
    fn several_paths_mix_files_and_directories_once_each() {
        let dir = project();
        let selection = select(
            &paths(&["tests/nested", "tests/cart.test.ts", "tests/"]),
            dir.path(),
        )
        .unwrap();
        assert_eq!(
            names(&selection),
            ["tests/nested/home.test.ts", "tests/cart.test.ts"]
        );
        assert_eq!(
            selection.shown,
            ["tests/nested", "tests/cart.test.ts", "tests/"]
        );
    }

    #[test]
    fn a_missing_path_is_a_usage_error_that_names_it() {
        let dir = project();
        let error = select(&paths(&["tests/", "tests/nope.test.ts"]), dir.path()).unwrap_err();
        assert!(error.downcast_ref::<clap::Error>().is_some());
        assert!(format!("{error}").contains("tests/nope.test.ts"), "{error}");
        let error = select(&paths(&["tests:3"]), dir.path()).unwrap_err();
        assert!(format!("{error}").contains("directory"), "{error}");
    }

    #[test]
    fn a_line_becomes_the_ranges_of_its_specs() {
        let dir = project();
        let selection = select(
            &paths(&["tests/cart.test.ts:4", "tests/nested/home.test.ts"]),
            dir.path(),
        )
        .unwrap();
        let locations = selection.locations.unwrap();
        assert_eq!(locations["tests/cart.test.ts"], Some(vec![(3, 5)]));
        assert_eq!(locations["tests/nested/home.test.ts"], None);

        // The same file whole and at a line: whole.
        let selection = select(
            &paths(&["tests/cart.test.ts:4", "tests/cart.test.ts"]),
            dir.path(),
        )
        .unwrap();
        assert_eq!(selection.locations.unwrap()["tests/cart.test.ts"], None);

        let error = select(&paths(&["tests/cart.test.ts:13"]), dir.path()).unwrap_err();
        let message = format!("{error}");
        assert!(
            message.contains("tests/cart.test.ts:13 is not inside a spec"),
            "{message}"
        );
        assert!(message.contains("tests/cart.test.ts:3  first"), "{message}");
        assert!(
            message.contains("tests/cart.test.ts:10  (computed title)"),
            "{message}"
        );
    }
}
