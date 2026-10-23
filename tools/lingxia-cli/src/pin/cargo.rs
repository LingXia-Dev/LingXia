//! The pin's half of the root `Cargo.toml`: `[patch.crates-io]` entries that
//! point LingXia crates at the pinned commit's checkout in `.lingxia/`, in
//! one block between `# lingxia.lock` comments, so a later pin replaces
//! exactly these lines and nothing the project wrote itself. The checkout is already there for the
//! SDKs, so cargo never fetches LingXia a second time.

use anyhow::{Context, Result, bail};
use std::collections::BTreeSet;
use std::path::Path;
use std::process::Command;

pub const MARKER: &str = "# lingxia.lock";
/// The patch block opens and closes with these, so its entries stay plain.
const BLOCK_BEGIN: &str = "# lingxia.lock: managed by `lingxia pin`, which rewrites this block";
const BLOCK_END: &str = "# end lingxia.lock";

/// Each line with whether the pin manages it: one ending in the marker, or
/// any line of the patch block. A block whose end line was lost stops at the
/// next table, so the rest of the manifest is never taken for it.
fn classify(content: &str) -> Vec<(bool, &str)> {
    let mut in_block = false;
    content
        .split_inclusive('\n')
        .map(|line| {
            let trimmed = line.trim();
            if trimmed.starts_with("# lingxia.lock:") {
                in_block = true;
                return (true, line);
            }
            if in_block && is_header(line) {
                in_block = false;
            }
            if in_block {
                if trimmed == BLOCK_END {
                    in_block = false;
                }
                return (true, line);
            }
            (line.trim_end().ends_with(MARKER), line)
        })
        .collect()
}
const PATCH_HEADER: &str = "[patch.crates-io]";
const WORKSPACE_HEADER: &str = "[workspace]";

/// A table header as written, without a trailing comment or key quotes.
fn header_is(line: &str, header: &str) -> bool {
    let bare = line.split('#').next().unwrap_or("").trim().replace('"', "");
    bare == header
}
/// Cargo makes a path dependency inside the workspace's directory a member,
/// so LingXia's crates in `.lingxia/` would inherit the project's
/// `[workspace.package]` instead of their own; excluded, they find theirs.
const EXCLUDE_LINE: &str = "exclude = [\".lingxia\"]";

/// One managed patch: crate name and its inline TOML value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Patch {
    pub name: String,
    pub value: String,
}

/// The pinned checkout as the project's `Cargo.toml` names it: relative, so
/// the manifest builds on any machine once the CLI has prepared it.
pub const SOURCE_PATH: &str = ".lingxia/lingxia-src";

/// `name` from `dir` (relative to `base`, forward slashes).
pub fn path_patch(name: &str, base: &str, dir: &str) -> Patch {
    Patch {
        name: name.to_string(),
        value: format!("{{ path = {} }}", toml_string(&format!("{base}/{dir}"))),
    }
}

/// `path` under `root` with forward slashes, as Cargo.toml spells paths.
pub fn relative_dir(root: &Path, path: &Path) -> Option<String> {
    let rel = path.strip_prefix(root).ok()?;
    Some(
        rel.components()
            .map(|c| c.as_os_str().to_string_lossy().into_owned())
            .collect::<Vec<_>>()
            .join("/"),
    )
}

fn toml_string(value: &str) -> String {
    toml::Value::String(value.to_string()).to_string()
}

fn is_header(line: &str) -> bool {
    line.trim_start().starts_with('[')
}

fn entry_name(line: &str) -> Option<String> {
    let trimmed = line.trim();
    if trimmed.is_empty() || trimmed.starts_with('#') || trimmed.starts_with('[') {
        return None;
    }
    let (key, _) = trimmed.split_once('=')?;
    Some(key.trim().trim_matches('"').to_string())
}

/// Names the project patches itself (not managed by the pin).
pub fn project_patches(content: &str) -> BTreeSet<String> {
    let mut names = BTreeSet::new();
    let mut in_table = false;
    for (managed, line) in classify(content) {
        if managed {
            continue;
        }
        if is_header(line) {
            let header = line.trim();
            in_table = header_is(header, PATCH_HEADER);
            if let Some(name) = header
                .strip_prefix("[patch.crates-io.")
                .and_then(|rest| rest.strip_suffix(']'))
            {
                names.insert(name.trim_matches('"').to_string());
            }
            continue;
        }
        if in_table && let Some(name) = entry_name(line) {
            names.insert(name);
        }
    }
    names
}

/// The manifest with exactly `patches` as its managed entries. Entries the
/// project patches itself are kept and win; their names come back as skipped.
pub fn rewrite(content: &str, patches: &[Patch]) -> (String, Vec<String>) {
    let own = project_patches(content);
    let mut skipped = Vec::new();
    let lines: Vec<String> = patches
        .iter()
        .filter(|patch| {
            let keep = !own.contains(&patch.name);
            if !keep {
                skipped.push(patch.name.clone());
            }
            keep
        })
        .map(|patch| format!("{} = {}\n", patch.name, patch.value))
        .collect();
    let block = if lines.is_empty() {
        String::new()
    } else {
        format!("{BLOCK_BEGIN}\n{}{BLOCK_END}\n", lines.concat())
    };

    let mut exclude =
        lines.iter().any(|line| line.contains(SOURCE_PATH)) && workspace_exclude(content).is_none();
    let mut out = String::with_capacity(content.len());
    let mut inserted = lines.is_empty();
    for (managed, line) in classify(content) {
        if managed {
            continue;
        }
        out.push_str(line);
        if exclude && header_is(line, WORKSPACE_HEADER) {
            if !line.ends_with('\n') {
                out.push('\n');
            }
            out.push_str(&format!("{EXCLUDE_LINE} {MARKER}\n"));
            exclude = false;
        }
        if !inserted && header_is(line, PATCH_HEADER) {
            if !line.ends_with('\n') {
                out.push('\n');
            }
            out.push_str(&block);
            inserted = true;
        }
    }
    if !inserted {
        if !out.is_empty() && !out.ends_with('\n') {
            out.push('\n');
        }
        out.push('\n');
        out.push_str(PATCH_HEADER);
        out.push('\n');
        out.push_str(&block);
    }
    if lines.is_empty() {
        out = drop_empty_patch_table(&out);
    }
    (out, skipped)
}

/// The manifest without a `[patch.crates-io]` table that has no entries left
/// (the pin added it), nor the blank line before it.
fn drop_empty_patch_table(content: &str) -> String {
    let lines: Vec<&str> = content.split_inclusive('\n').collect();
    let Some(start) = lines.iter().position(|line| header_is(line, PATCH_HEADER)) else {
        return content.to_string();
    };
    let end = lines[start + 1..]
        .iter()
        .position(|line| is_header(line))
        .map_or(lines.len(), |offset| start + 1 + offset);
    if lines[start + 1..end]
        .iter()
        .any(|line| entry_name(line).is_some())
    {
        return content.to_string();
    }
    let from = if start > 0 && lines[start - 1].trim().is_empty() {
        start - 1
    } else {
        start
    };
    let mut out: String = lines[..from].concat();
    out.push_str(&lines[end..].concat());
    out
}

/// The project's own `[workspace] exclude` value, when it writes one (the
/// pin then leaves it to the project).
fn workspace_exclude(content: &str) -> Option<String> {
    let mut in_workspace = false;
    let mut value: Option<String> = None;
    for line in content.lines() {
        if is_header(line) {
            if value.is_some() {
                break;
            }
            in_workspace = header_is(line, WORKSPACE_HEADER);
            continue;
        }
        if let Some(value) = value.as_mut() {
            value.push_str(line);
            if line.contains(']') {
                break;
            }
            continue;
        }
        if in_workspace
            && !line.trim_end().ends_with(MARKER)
            && entry_name(line).as_deref() == Some("exclude")
        {
            value = Some(line.to_string());
            if line.contains(']') {
                break;
            }
        }
    }
    value
}

/// Whether the project's own `exclude` leaves `.lingxia/` a workspace member.
pub fn exclude_needs_lingxia(content: &str) -> bool {
    workspace_exclude(content).is_some_and(|value| !value.contains(".lingxia"))
}

/// The managed entries a manifest currently has.
pub fn managed(content: &str) -> Vec<Patch> {
    classify(content)
        .into_iter()
        .filter(|(managed, _)| *managed)
        .map(|(_, line)| line.trim())
        .filter(|line| !line.starts_with('#') && entry_name(line).as_deref() != Some("exclude"))
        .filter_map(|line| {
            let body = line.strip_suffix(MARKER).unwrap_or(line).trim_end();
            let (name, value) = body.split_once('=')?;
            Some(Patch {
                name: name.trim().to_string(),
                value: value.trim().to_string(),
            })
        })
        .collect()
}

/// Packages under `crates/` with their directories.
pub fn workspace_crate_dirs(checkout: &Path) -> Result<Vec<(String, std::path::PathBuf)>> {
    let output = Command::new("cargo")
        .args([
            "metadata",
            "--no-deps",
            "--format-version",
            "1",
            "--manifest-path",
        ])
        .arg(checkout.join("Cargo.toml"))
        .output()
        .context("run cargo metadata on the pinned source")?;
    if !output.status.success() {
        bail!(
            "cargo metadata on the pinned source failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    let metadata: serde_json::Value = serde_json::from_slice(&output.stdout)?;
    let crates_dir = checkout.join("crates");
    Ok(metadata["packages"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|package| {
            let manifest = Path::new(package["manifest_path"].as_str()?);
            if !manifest.starts_with(&crates_dir) {
                return None;
            }
            Some((
                package["name"].as_str()?.to_string(),
                manifest.parent()?.to_path_buf(),
            ))
        })
        .collect())
}

/// Every crate name a project's `Cargo.lock` holds, whatever its source.
pub fn locked_crates(lock_text: &str) -> BTreeSet<String> {
    let Ok(lock) = lock_text.parse::<toml::Table>() else {
        return BTreeSet::new();
    };
    lock.get("package")
        .and_then(toml::Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|package| package.get("name").and_then(toml::Value::as_str))
        .map(str::to_string)
        .collect()
}

/// Patches cargo resolved without using, from `Cargo.lock`'s
/// `[[patch.unused]]`.
pub fn unused_patches(lock_text: &str) -> BTreeSet<String> {
    let Ok(lock) = lock_text.parse::<toml::Table>() else {
        return BTreeSet::new();
    };
    lock.get("patch")
        .and_then(|patch| patch.get("unused"))
        .and_then(toml::Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|package| package.get("name").and_then(toml::Value::as_str))
        .map(str::to_string)
        .collect()
}

impl Patch {
    /// The crate this entry replaces: its `package` when it renames one.
    pub fn crate_name(&self) -> &str {
        self.value
            .split_once("package = \"")
            .and_then(|(_, rest)| rest.split_once('"'))
            .map_or(self.name.as_str(), |(package, _)| package)
    }
}

/// The pinned commit's own `[patch.crates-io]`: cargo applies patches only
/// from the root workspace, so a dependency's patches are lost unless the
/// project carries them. Its path patches are rebased onto `base`, the
/// checkout as the caller names it.
pub fn transcribed_patches(checkout: &Path, base: &str) -> Result<Vec<Patch>> {
    let manifest = checkout.join("Cargo.toml");
    let text = std::fs::read_to_string(&manifest)
        .with_context(|| format!("read {}", manifest.display()))?;
    let table: toml::Table = text
        .parse()
        .with_context(|| format!("parse {}", manifest.display()))?;
    let mut patches = Vec::new();
    let entries = table
        .get("patch")
        .and_then(|p| p.get("crates-io"))
        .and_then(toml::Value::as_table);
    for (name, value) in entries.into_iter().flatten() {
        let Some(spec) = value.as_table() else {
            continue;
        };
        let fields: Vec<String> = spec
            .iter()
            .map(|(key, value)| match (key.as_str(), value.as_str()) {
                ("path", Some(path)) => format!(
                    "path = {}",
                    toml_string(&format!("{base}/{}", path.trim_start_matches("./")))
                ),
                _ => format!("{key} = {value}"),
            })
            .collect();
        patches.push(Patch {
            name: name.clone(),
            value: format!("{{ {} }}", fields.join(", ")),
        });
    }
    Ok(patches)
}

#[cfg(test)]
mod tests {
    use super::*;

    const MANIFEST: &str = "\
[workspace]
members = [\"native\"]

[patch.crates-io]
portable-pty = { git = \"https://example.com/pty.git\", rev = \"abc\" }

[profile.release]
lto = true
";

    #[test]
    fn rewrite_inserts_replaces_and_removes_managed_entries() {
        let patches = vec![
            path_patch("lingxia", "s1", "crates/lingxia"),
            path_patch("portable-pty", "s1", "vendor/pty"),
        ];
        let (first, skipped) = rewrite(MANIFEST, &patches);
        assert_eq!(skipped, vec!["portable-pty"]);
        assert!(first.contains(&format!(
            "[patch.crates-io]\n{BLOCK_BEGIN}\nlingxia = {{ path = \"s1/crates/lingxia\" }}\n{BLOCK_END}\nportable-pty = {{ git = \"https://example.com/pty.git\""
        )));
        assert_eq!(
            managed(&first),
            vec![path_patch("lingxia", "s1", "crates/lingxia")]
        );

        let (second, _) = rewrite(&first, &[path_patch("lingxia", "s2", "crates/lingxia")]);
        assert!(second.contains("\"s2/crates/lingxia\""));
        assert!(!second.contains("s1/"));
        assert_eq!(second.matches(PATCH_HEADER).count(), 1);

        let (cleared, _) = rewrite(&second, &[]);
        assert_eq!(cleared, MANIFEST);
    }

    #[test]
    fn rewrite_adds_the_table_when_missing() {
        let (out, _) = rewrite(
            "[workspace]\nmembers = []\n",
            &[path_patch("lingxia", SOURCE_PATH, "crates/lingxia")],
        );
        assert!(out.ends_with(&format!(
            "\n[patch.crates-io]\n{BLOCK_BEGIN}\nlingxia = {{ path = \".lingxia/lingxia-src/crates/lingxia\" }}\n{BLOCK_END}\n"
        )));
    }

    #[test]
    fn path_patches_exclude_the_checkout_from_the_workspace() {
        let patches = [path_patch("lingxia", SOURCE_PATH, "crates/lingxia")];
        let (out, _) = rewrite(MANIFEST, &patches);
        assert!(out.starts_with("[workspace]\nexclude = [\".lingxia\"] # lingxia.lock\nmembers"));
        assert_eq!(managed(&out), patches.to_vec());
        let (again, _) = rewrite(&out, &patches);
        assert_eq!(again, out);
        assert_eq!(rewrite(&out, &[]).0, MANIFEST);

        let own = "[workspace]\nmembers = []\nexclude = [\n  \"tools\",\n]\n";
        let (kept, _) = rewrite(own, &patches);
        assert_eq!(kept.matches("exclude").count(), 1);
        assert!(exclude_needs_lingxia(own));
        assert!(!exclude_needs_lingxia(
            "[workspace]\nexclude = [\".lingxia\"]\n"
        ));
    }

    #[test]
    fn per_line_markers_give_way_to_the_block() {
        let old = "[patch.crates-io]\nlingxia = { git = \"u\", rev = \"c\" } # lingxia.lock\nmine = { path = \"x\" }\n";
        let (out, _) = rewrite(old, &[path_patch("lingxia", SOURCE_PATH, "crates/lingxia")]);
        assert!(!out.contains("rev = \"c\""));
        assert_eq!(out.matches("lingxia = ").count(), 1);
        assert!(out.contains("mine = { path = \"x\" }"));
        assert_eq!(
            project_patches(&out).into_iter().collect::<Vec<_>>(),
            vec!["mine"]
        );
    }

    #[test]
    fn clearing_removes_a_patch_table_the_pin_added_and_survives_a_lost_end() {
        let plain = "[workspace]\nmembers = []\n\n[profile.release]\nlto = true\n";
        let patches = [path_patch("lingxia", SOURCE_PATH, "crates/lingxia")];
        let (pinned, _) = rewrite(plain, &patches);
        assert_eq!(rewrite(&pinned, &[]).0, plain);

        let lost_end = pinned.replace(&format!("{BLOCK_END}\n"), "");
        let lost_end = lost_end.replace(
            "[profile.release]",
            "[patch.other]\nx = 1\n\n[profile.release]",
        );
        let (cleared, _) = rewrite(&lost_end, &[]);
        assert!(
            cleared.contains("[profile.release]\nlto = true"),
            "{cleared}"
        );
        assert!(cleared.contains("[patch.other]"));

        let commented =
            "[workspace]\nmembers = []\n\n[patch.\"crates-io\"] # ours\nmine = { path = \"x\" }\n";
        let (out, _) = rewrite(commented, &patches);
        assert_eq!(out.matches("crates-io").count(), 1, "{out}");
        assert!(out.contains("mine = { path = \"x\" }"));
    }

    #[test]
    fn project_patches_include_expanded_tables() {
        let names = project_patches(
            "[patch.crates-io]\na = { path = \"x\" }\n[patch.crates-io.b]\npath = \"y\"\n",
        );
        assert_eq!(names.into_iter().collect::<Vec<_>>(), vec!["a", "b"]);
    }

    #[test]
    fn unused_patches_come_from_the_lock() {
        let lock = "version = 4\n\n[[package]]\nname = \"lingxia\"\nversion = \"0.27.0\"\n\n[[patch.unused]]\nname = \"portable-pty\"\nversion = \"0.9.0\"\n\n[[patch.unused]]\nname = \"lingxia-terminal\"\nversion = \"0.27.0\"\n";
        assert_eq!(
            unused_patches(lock).into_iter().collect::<Vec<_>>(),
            vec!["lingxia-terminal", "portable-pty"]
        );
        let renamed = Patch {
            name: "pty".into(),
            value: "{ git = \"u\", package = \"portable-pty\" }".into(),
        };
        assert_eq!(renamed.crate_name(), "portable-pty");
        assert_eq!(path_patch("lingxia", "s", "c").crate_name(), "lingxia");
    }

    #[test]
    fn transcribed_patches_keep_git_and_rebase_path() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("Cargo.toml"),
            "[patch.crates-io]\npty = { git = \"https://g/pty.git\", rev = \"59d0\", package = \"portable-pty\" }\nlocal = { path = \"./vendor/x\" }\n",
        )
        .unwrap();
        let patches = transcribed_patches(dir.path(), SOURCE_PATH).unwrap();
        assert_eq!(patches.len(), 2);
        let local = patches.iter().find(|p| p.name == "local").unwrap();
        assert_eq!(local.value, "{ path = \".lingxia/lingxia-src/vendor/x\" }");
        let pty = patches.iter().find(|p| p.name == "pty").unwrap();
        assert!(pty.value.contains("git = \"https://g/pty.git\""));
        assert!(pty.value.contains("package = \"portable-pty\""));
    }
}
