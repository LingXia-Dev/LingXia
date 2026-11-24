//! Page actions from the type checker, for actions a page's source does not spell out.
//!
//! One program answers every page asked in a build. Outside a release build each answer is
//! kept in `.lingxia/action-types.json` with a content hash of the module graph it was read
//! from, so a dev save re-asks only the pages whose graph changed.

use super::super::{page_types, typecheck};
use crate::lxapp::framework::{PageAction, PageActionMode, page_base};
use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use sha2::Digest;
use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};

use super::super::ts_worker;

const CACHE: &str = "action-types.json";

#[derive(Clone, Serialize, Deserialize)]
struct Reply {
    #[serde(default)]
    actions: Vec<Found>,
    #[serde(default)]
    error: Option<String>,
    #[serde(default)]
    name: Option<String>,
    files: Vec<PathBuf>,
}

#[derive(Clone, Serialize, Deserialize)]
struct Found {
    name: String,
    stream: bool,
}

#[derive(Serialize, Deserialize)]
struct Cached {
    /// Each file of the graph and the SHA-256 of its content.
    hashes: BTreeMap<PathBuf, String>,
    reply: Reply,
}

#[derive(Default, Serialize, Deserialize)]
struct Cache {
    /// The CLI and TypeScript versions the answers came from; any other reads as empty.
    version: String,
    pages: BTreeMap<PathBuf, Cached>,
}

/// A page whose actions the type checker must read.
pub(crate) struct Composed<'a> {
    pub page: &'a str,
    pub logic: PathBuf,
}

fn hash(file: &Path) -> Option<String> {
    let bytes = std::fs::read(file).ok()?;
    let digest = sha2::Sha256::digest(&bytes);
    Some(digest.iter().map(|byte| format!("{byte:02x}")).collect())
}

/// The tsconfigs that decide how the graph resolves.
fn configs(root: &Path) -> Vec<PathBuf> {
    [root.to_path_buf(), root.join(".lingxia")]
        .iter()
        .filter_map(|dir| std::fs::read_dir(dir).ok())
        .flatten()
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with("tsconfig") && name.ends_with(".json"))
        })
        .collect()
}

fn fresh(cached: &Cached) -> bool {
    !cached.hashes.is_empty()
        && cached
            .hashes
            .iter()
            .all(|(file, digest)| hash(file).as_deref() == Some(digest.as_str()))
}

/// The actions of each page, by page path, asking TypeScript once for every page not answered.
/// `cached` lets an answer whose module graph is unchanged stand; a release build passes false.
pub(crate) fn actions(
    root: &Path,
    composed: &[Composed<'_>],
    cached: bool,
) -> Result<HashMap<String, Vec<PageAction>>> {
    if composed.is_empty() {
        return Ok(HashMap::new());
    }
    let first_page = page_base(composed[0].page);
    let tsc = typecheck::resolve_tsc(root).with_context(|| {
        format!("{first_page}: its actions are composed, so the build reads them with TypeScript")
    })?;
    let typescript = typecheck::supported_typescript(&tsc)?;
    let ts_version = std::fs::read(typescript.join("package.json"))
        .ok()
        .and_then(|bytes| serde_json::from_slice::<serde_json::Value>(&bytes).ok())
        .and_then(|package| package["version"].as_str().map(str::to_owned))
        .unwrap_or_default();
    let version = format!("{}+typescript-{ts_version}", env!("CARGO_PKG_VERSION"));
    let cache_path = root.join(".lingxia").join(CACHE);
    let mut cache: Cache = std::fs::read(&cache_path)
        .ok()
        .and_then(|bytes| serde_json::from_slice::<Cache>(&bytes).ok())
        .filter(|cache| cached && cache.version == version)
        .unwrap_or_default();
    cache.version = version;
    let stale: Vec<&Composed<'_>> = composed
        .iter()
        .filter(|page| !cache.pages.get(&page.logic).is_some_and(fresh))
        .collect();
    let mut answers: HashMap<PathBuf, Reply> = HashMap::new();
    if !stale.is_empty() {
        let logics: Vec<&Path> = stale.iter().map(|page| page.logic.as_path()).collect();
        let raw = ts_worker::action_types(root, typescript, &logics).map_err(|err| {
            anyhow::anyhow!("{first_page}: cannot read page actions with TypeScript:\n{err}")
        })?;
        answers = serde_json::from_value(raw).context("unreadable page action types")?;
        let configs = configs(root);
        for page in &stale {
            let Some(reply) = answers.get(&page.logic) else {
                continue;
            };
            let hashes = reply
                .files
                .iter()
                .chain(&configs)
                .filter_map(|file| Some((file.clone(), hash(file)?)))
                .collect();
            cache.pages.insert(
                page.logic.clone(),
                Cached {
                    hashes,
                    reply: reply.clone(),
                },
            );
        }
        if let Some(dir) = cache_path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let _ = std::fs::write(&cache_path, serde_json::to_vec(&cache)?);
    }
    let mut found = HashMap::new();
    let (mut unprojected, mut problems) = (Vec::new(), Vec::new());
    for page in composed {
        let reply = answers.remove(&page.logic).or_else(|| {
            cache
                .pages
                .get(&page.logic)
                .map(|cached| cached.reply.clone())
        });
        let name = page_base(page.page);
        match reply {
            None => problems.push(format!("{name}: TypeScript gave no answer for its actions")),
            Some(reply) if reply.error.as_deref() == Some("unprojected") => unprojected.push(page),
            Some(reply) => match answer(&name, reply) {
                Ok(actions) => {
                    found.insert(page.page.to_string(), actions);
                }
                Err(problem) => problems.push(problem),
            },
        }
    }
    if !unprojected.is_empty() {
        problems.push(unprojected_problem(root, &unprojected));
    }
    if !problems.is_empty() {
        bail!("{}", problems.join("\n"));
    }
    Ok(found)
}

/// Pages listed in one problem before the rest are counted.
const LISTED: usize = 5;

/// One problem for every page no tsconfig project reaches, with the fix for this project's wiring.
fn unprojected_problem(root: &Path, pages: &[&Composed<'_>]) -> String {
    let mut lines = vec![if pages.len() == 1 {
        "A page composes its actions, which the build reads with TypeScript, \
         but no tsconfig project includes its Logic module:"
            .to_string()
    } else {
        format!(
            "{} pages compose their actions, which the build reads with TypeScript, \
             but no tsconfig project includes their Logic modules:",
            pages.len()
        )
    }];
    for page in pages.iter().take(LISTED) {
        let file = page.logic.strip_prefix(root).unwrap_or(&page.logic);
        lines.push(format!("  {} ({})", page_base(page.page), file.display()));
    }
    if pages.len() > LISTED {
        lines.push(format!("  and {} more", pages.len() - LISTED));
    }
    lines.push(if page_types::has_logic_tsconfig(root) {
        format!(
            "Add {} to the Logic tsconfig's `include`.",
            if pages.len() == 1 { "it" } else { "them" }
        )
    } else {
        format!(
            "This project has no Logic tsconfig: create `tsconfig.logic.json` with \
             `{{ \"extends\": \"./{}\" }}` and add `{{ \"path\": \"./tsconfig.logic.json\" }}` \
             to the `references` of `tsconfig.json`.",
            page_types::LOGIC_BASE
        )
    });
    lines.join("\n")
}

fn answer(name: &str, reply: Reply) -> std::result::Result<Vec<PageAction>, String> {
    match reply.error.as_deref() {
        None => Ok(reply
            .actions
            .into_iter()
            .map(|found| PageAction {
                name: found.name,
                mode: if found.stream {
                    PageActionMode::Stream
                } else {
                    PageActionMode::Call
                },
            })
            .collect()),
        Some("unknown") => Err(format!(
            "{name}: actions must be statically known: write them in the object, or type the helper's return ({})",
            match reply.name {
                Some(action) => format!("`{action}` is not a function"),
                None => "the actions have no fixed names".to_string(),
            }
        )),
        Some(_) => Err(format!(
            "{name}: the Logic module must `export default definePage({{ … }})` from '@lingxia/logic'"
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unreached_pages_are_one_problem_with_the_wiring_fix() {
        let dir = tempfile::tempdir().unwrap();
        let names: Vec<String> = (0..7).map(|n| format!("pages/p{n}/index")).collect();
        let pages: Vec<Composed<'_>> = names
            .iter()
            .map(|page| Composed {
                page,
                logic: dir.path().join(format!("{page}.ts")),
            })
            .collect();
        let refs: Vec<&Composed<'_>> = pages.iter().collect();
        let problem = unprojected_problem(dir.path(), &refs);
        let lines: Vec<&str> = problem.lines().collect();
        assert!(
            lines[0].starts_with("7 pages compose their actions"),
            "{problem}"
        );
        assert_eq!(
            lines[1],
            format!(
                "  pages/p0/index ({})",
                Path::new("pages/p0/index.ts").display()
            )
        );
        assert_eq!(lines[6], "  and 2 more");
        assert!(
            lines[7].starts_with("This project has no Logic tsconfig"),
            "{problem}"
        );

        std::fs::write(
            dir.path().join("tsconfig.logic.json"),
            r#"{ "extends": "./.lingxia/tsconfig.logic.json" }"#,
        )
        .unwrap();
        std::fs::create_dir_all(dir.path().join(".lingxia")).unwrap();
        std::fs::write(dir.path().join(page_types::LOGIC_BASE), "{}").unwrap();
        let problem = unprojected_problem(dir.path(), &refs[..1]);
        assert!(
            problem.starts_with("A page composes its actions"),
            "{problem}"
        );
        assert!(
            problem.ends_with("Add it to the Logic tsconfig's `include`."),
            "{problem}"
        );
    }

    #[test]
    fn an_answer_stands_only_while_its_graph_has_the_same_content() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("shared.ts");
        std::fs::write(&file, "export const a = 1;").unwrap();
        let when = std::fs::metadata(&file).unwrap().modified().unwrap();
        let cached = Cached {
            hashes: BTreeMap::from([(file.clone(), hash(&file).unwrap())]),
            reply: Reply {
                actions: Vec::new(),
                error: None,
                name: None,
                files: vec![file.clone()],
            },
        };
        assert!(fresh(&cached));
        // Same length, same modification time, different content.
        std::fs::write(&file, "export const b = 1;").unwrap();
        std::fs::File::options()
            .write(true)
            .open(&file)
            .unwrap()
            .set_modified(when)
            .unwrap();
        assert!(!fresh(&cached));
        std::fs::remove_file(&file).unwrap();
        assert!(!fresh(&cached));
    }
}
