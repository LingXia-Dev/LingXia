//! The pin's half of the root `Cargo.toml`: `[patch.crates-io]` entries that
//! point LingXia crates at the pinned commit, each marked `# lingxia.lock`
//! so a later pin replaces exactly these lines and nothing the project
//! wrote itself.

use anyhow::{Context, Result, bail};
use std::collections::BTreeSet;
use std::path::Path;
use std::process::Command;

pub const MARKER: &str = "# lingxia.lock";
const PATCH_HEADER: &str = "[patch.crates-io]";

/// One managed patch: crate name and its inline TOML value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Patch {
    pub name: String,
    pub value: String,
}

pub fn git_patch(name: &str, git_url: &str, commit: &str) -> Patch {
    Patch {
        name: name.to_string(),
        value: format!(
            "{{ git = {}, rev = {} }}",
            toml_string(git_url),
            toml_string(commit)
        ),
    }
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
    for line in content.lines() {
        if is_header(line) {
            let header = line.trim();
            in_table = header == PATCH_HEADER;
            if let Some(name) = header
                .strip_prefix("[patch.crates-io.")
                .and_then(|rest| rest.strip_suffix(']'))
            {
                names.insert(name.trim_matches('"').to_string());
            }
            continue;
        }
        if in_table
            && !line.trim_end().ends_with(MARKER)
            && let Some(name) = entry_name(line)
        {
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
        .map(|patch| format!("{} = {} {MARKER}\n", patch.name, patch.value))
        .collect();

    let mut out = String::with_capacity(content.len());
    let mut inserted = lines.is_empty();
    for line in content.split_inclusive('\n') {
        if line.trim_end().ends_with(MARKER) {
            continue;
        }
        out.push_str(line);
        if !inserted && line.trim() == PATCH_HEADER {
            if !line.ends_with('\n') {
                out.push('\n');
            }
            for managed in &lines {
                out.push_str(managed);
            }
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
        for managed in &lines {
            out.push_str(managed);
        }
    }
    (out, skipped)
}

/// The managed entries a manifest currently has.
pub fn managed(content: &str) -> Vec<Patch> {
    content
        .lines()
        .filter(|line| line.trim_end().ends_with(MARKER))
        .filter_map(|line| {
            let body = line.trim_end().strip_suffix(MARKER)?.trim_end();
            let (name, value) = body.split_once('=')?;
            Some(Patch {
                name: name.trim().to_string(),
                value: value.trim().to_string(),
            })
        })
        .collect()
}

/// Packages under `crates/` in the pinned checkout's workspace.
pub fn workspace_crates(checkout: &Path) -> Result<BTreeSet<String>> {
    Ok(workspace_crate_dirs(checkout)?
        .into_iter()
        .map(|(name, _)| name)
        .collect())
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

/// Which of the pinned workspace's crates get a patch: those the project's
/// `Cargo.lock` holds from any source (a previous pin left them as git
/// sources) and those the manifest already manages, else the SDK crates.
pub fn patch_names(available: &BTreeSet<String>, lock_text: &str, manifest: &str) -> Vec<String> {
    let mut used = locked_crates(lock_text);
    used.extend(managed(manifest).into_iter().map(|patch| patch.name));
    let names: BTreeSet<String> = if used.is_empty() {
        lingxia_control_protocol::dev_session::compat::SDK_CRATES
            .iter()
            .map(|name| name.to_string())
            .collect()
    } else {
        used
    };
    available.intersection(&names).cloned().collect()
}

/// The pinned commit's own `[patch.crates-io]`: cargo applies patches only
/// from the root workspace, so a git dependency's patches are lost unless
/// the project carries them. Path patches cannot travel and come back as
/// skipped.
pub fn transcribed_patches(checkout: &Path) -> Result<(Vec<Patch>, Vec<String>)> {
    transcribed(checkout, false)
}

/// The same, with path patches made absolute: a local override can use them.
pub fn transcribed_patches_with_paths(checkout: &Path) -> Result<(Vec<Patch>, Vec<String>)> {
    transcribed(checkout, true)
}

fn transcribed(checkout: &Path, absolute_paths: bool) -> Result<(Vec<Patch>, Vec<String>)> {
    let manifest = checkout.join("Cargo.toml");
    let text = std::fs::read_to_string(&manifest)
        .with_context(|| format!("read {}", manifest.display()))?;
    let table: toml::Table = text
        .parse()
        .with_context(|| format!("parse {}", manifest.display()))?;
    let mut patches = Vec::new();
    let mut skipped = Vec::new();
    let entries = table
        .get("patch")
        .and_then(|p| p.get("crates-io"))
        .and_then(toml::Value::as_table);
    for (name, value) in entries.into_iter().flatten() {
        let Some(spec) = value.as_table() else {
            continue;
        };
        if spec.contains_key("path") && !absolute_paths {
            skipped.push(name.clone());
            continue;
        }
        let fields: Vec<String> = spec
            .iter()
            .map(|(key, value)| match (key.as_str(), value.as_str()) {
                ("path", Some(path)) => format!(
                    "path = {}",
                    toml::Value::String(checkout.join(path).to_string_lossy().into_owned())
                ),
                _ => format!("{key} = {value}"),
            })
            .collect();
        patches.push(Patch {
            name: name.clone(),
            value: format!("{{ {} }}", fields.join(", ")),
        });
    }
    Ok((patches, skipped))
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
            git_patch("lingxia", "https://g/LingXia.git", "1111"),
            git_patch("portable-pty", "https://g/pty.git", "2222"),
        ];
        let (first, skipped) = rewrite(MANIFEST, &patches);
        assert_eq!(skipped, vec!["portable-pty"]);
        assert!(first.contains(
            "[patch.crates-io]\nlingxia = { git = \"https://g/LingXia.git\", rev = \"1111\" } # lingxia.lock\nportable-pty = { git = \"https://example.com/pty.git\""
        ));
        assert_eq!(
            managed(&first),
            vec![git_patch("lingxia", "https://g/LingXia.git", "1111")]
        );

        let (second, _) = rewrite(
            &first,
            &[git_patch("lingxia", "https://g/LingXia.git", "3333")],
        );
        assert!(second.contains("rev = \"3333\""));
        assert!(!second.contains("1111"));
        assert_eq!(second.matches(PATCH_HEADER).count(), 1);

        let (cleared, _) = rewrite(&second, &[]);
        assert_eq!(cleared, MANIFEST);
    }

    #[test]
    fn rewrite_adds_the_table_when_missing() {
        let (out, _) = rewrite(
            "[workspace]\nmembers = []\n",
            &[git_patch("lingxia", "u", "c")],
        );
        assert!(out.ends_with(
            "\n[patch.crates-io]\nlingxia = { git = \"u\", rev = \"c\" } # lingxia.lock\n"
        ));
    }

    #[test]
    fn project_patches_include_expanded_tables() {
        let names = project_patches(
            "[patch.crates-io]\na = { path = \"x\" }\n[patch.crates-io.b]\npath = \"y\"\n",
        );
        assert_eq!(names.into_iter().collect::<Vec<_>>(), vec!["a", "b"]);
    }

    #[test]
    fn a_second_pin_keeps_patching_crates_the_first_moved_to_git() {
        let available: BTreeSet<String> = ["lingxia", "lingxia-shell", "lingxia-unused"]
            .into_iter()
            .map(String::from)
            .collect();
        let lock = "\
[[package]]
name = \"lingxia\"
version = \"0.27.0\"
source = \"git+https://g/LingXia.git?rev=aaa#aaa\"

[[package]]
name = \"serde\"
version = \"1.0.0\"
source = \"registry+https://github.com/rust-lang/crates.io-index\"
";
        let manifest =
            "[patch.crates-io]\nlingxia-shell = { git = \"u\", rev = \"aaa\" } # lingxia.lock\n";
        assert_eq!(
            patch_names(&available, lock, manifest),
            vec!["lingxia".to_string(), "lingxia-shell".to_string()]
        );
    }

    #[test]
    fn transcribed_patches_keep_git_and_skip_path() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("Cargo.toml"),
            "[patch.crates-io]\npty = { git = \"https://g/pty.git\", rev = \"59d0\", package = \"portable-pty\" }\nlocal = { path = \"../x\" }\n",
        )
        .unwrap();
        let (patches, skipped) = transcribed_patches(dir.path()).unwrap();
        assert_eq!(skipped, vec!["local"]);
        assert_eq!(patches.len(), 1);
        assert!(patches[0].value.contains("git = \"https://g/pty.git\""));
        assert!(patches[0].value.contains("package = \"portable-pty\""));
    }
}
