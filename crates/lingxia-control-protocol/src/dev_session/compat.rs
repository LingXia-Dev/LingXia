//! One version check for the pieces a dev session combines: the CLI, the
//! host (app or Runner) it drives, and the project's installed `@lingxia/*`
//! packages. They must share a major.minor line; a piece that records the
//! commit it was built from — a locally built package (`0.19.0+abc1234`, or
//! `"lingxia": { "commit": … }` in its package.json), or the session's
//! `lingxia` — must also match the CLI's commit. Offline and cheap: it reads a few package.json files.
//!
//! `LINGXIA_ALLOW_SKEW=1` turns the failure into a warning.

use std::path::Path;

pub const ALLOW_SKEW_ENV: &str = "LINGXIA_ALLOW_SKEW";

/// One versioned piece of the session.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Component {
    /// `CLI`, `Runner`, `host`, or a package name.
    pub name: String,
    pub version: String,
    /// Commit the piece was built from, when it says.
    pub commit: Option<String>,
}

impl Component {
    pub fn new(name: impl Into<String>, version: impl Into<String>) -> Self {
        let version = version.into();
        let (version, commit) = split_build(&version);
        Self {
            name: name.into(),
            version,
            commit,
        }
    }

    /// A peer that answered without its version: a build from before peers
    /// reported one, so older than any line that checks.
    pub fn unreported(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            version: String::new(),
            commit: None,
        }
    }

    fn is_unreported(&self) -> bool {
        self.version.is_empty()
    }

    pub fn with_commit(mut self, commit: Option<&str>) -> Self {
        if let Some(commit) = commit.map(str::trim).filter(|commit| is_commit(commit)) {
            self.commit = Some(commit.to_string());
        }
        self
    }

    fn label(&self) -> String {
        if self.is_unreported() {
            return format!("{} (too old to report its version)", self.name);
        }
        match &self.commit {
            Some(commit) => format!("{} {} ({})", self.name, self.version, short(commit)),
            None => format!("{} {}", self.name, self.version),
        }
    }
}

/// `0.19.0+abc1234` → (`0.19.0`, `abc1234`); a build stamp such as
/// `0.19.0 (abc1234 2026-09-25)` → (`0.19.0`, `abc1234`).
fn split_build(raw: &str) -> (String, Option<String>) {
    let raw = raw.trim();
    if let Some((version, rest)) = raw.split_once(" (") {
        let commit = rest
            .trim_end_matches(')')
            .split_whitespace()
            .next()
            .map(|commit| commit.trim_end_matches("-dirty"))
            .filter(|commit| is_commit(commit))
            .map(str::to_string);
        return (version.to_string(), commit);
    }
    if let Some((version, build)) = raw.split_once('+') {
        let commit = build
            .split('.')
            .find(|part| is_commit(part))
            .map(str::to_string);
        return (version.to_string(), commit);
    }
    (raw.to_string(), None)
}

fn is_commit(value: &str) -> bool {
    (7..=40).contains(&value.len()) && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn short(commit: &str) -> &str {
    &commit[..commit.len().min(9)]
}

/// `major.minor` of a version, if it parses.
pub fn line(version: &str) -> Option<(u64, u64)> {
    let mut parts = version
        .trim()
        .trim_start_matches('v')
        .split(['.', '-', '+']);
    let major = parts.next()?.parse().ok()?;
    let minor = parts.next()?.parse().ok()?;
    Some((major, minor))
}

/// The `@lingxia/*` packages installed in `project_root/node_modules`.
pub fn installed_packages(project_root: &Path) -> Vec<Component> {
    let scope = project_root.join("node_modules").join("@lingxia");
    let Ok(entries) = std::fs::read_dir(&scope) else {
        return Vec::new();
    };
    let mut packages: Vec<Component> = entries
        .flatten()
        .filter_map(|entry| {
            let text = std::fs::read_to_string(entry.path().join("package.json")).ok()?;
            let manifest: serde_json::Value = serde_json::from_str(&text).ok()?;
            let name = manifest.get("name")?.as_str()?.to_string();
            let version = manifest.get("version")?.as_str()?;
            let commit = manifest
                .get("lingxia")
                .and_then(|lingxia| lingxia.get("commit"))
                .and_then(serde_json::Value::as_str);
            Some(Component::new(name, version).with_commit(commit))
        })
        .collect();
    packages.sort_by(|a, b| a.name.cmp(&b.name));
    packages
}

/// What the check found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Skew {
    /// One line: what disagrees, and the fix.
    pub message: String,
}

impl std::fmt::Display for Skew {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

/// Compare `cli` with the binaries it drives (`hosts`: the Runner, the app
/// host, the session's `lingxia`) and the project's `packages`.
pub fn check(cli: &Component, hosts: &[Component], packages: &[Component]) -> Result<(), Skew> {
    let Some(cli_line) = line(&cli.version) else {
        return Ok(());
    };
    let off_line = |component: &Component| line(&component.version).is_some_and(|l| l != cli_line);
    let off_commit = |component: &Component| match (&component.commit, &cli.commit) {
        (Some(theirs), Some(ours)) => {
            let (a, b) = (theirs.to_ascii_lowercase(), ours.to_ascii_lowercase());
            !(a.starts_with(&b) || b.starts_with(&a))
        }
        _ => false,
    };
    let old_packages: Vec<&Component> = packages
        .iter()
        .filter(|package| line(&package.version).is_some_and(|l| l < cli_line))
        .collect();
    let new_packages: Vec<&Component> = packages
        .iter()
        .filter(|package| line(&package.version).is_some_and(|l| l > cli_line))
        .collect();
    let commit_packages: Vec<&Component> = packages
        .iter()
        .filter(|package| !off_line(package) && off_commit(package))
        .collect();
    let older_host = |host: &Component| {
        host.is_unreported() || line(&host.version).is_some_and(|l| l < cli_line)
    };
    let off_hosts: Vec<&Component> = hosts
        .iter()
        .filter(|host| host.is_unreported() || off_line(host) || off_commit(host))
        .collect();
    if old_packages.is_empty()
        && new_packages.is_empty()
        && commit_packages.is_empty()
        && off_hosts.is_empty()
    {
        return Ok(());
    }

    let skewed: Vec<String> = old_packages
        .iter()
        .chain(&new_packages)
        .chain(&commit_packages)
        .map(|package| package.label())
        .collect();
    let mut reference = vec![cli.label()];
    reference.extend(hosts.iter().map(Component::label));

    let mut fixes = Vec::new();
    let (major, minor) = cli_line;
    let reinstall: Vec<String> = old_packages
        .iter()
        .chain(&commit_packages)
        .map(|package| format!("{}@~{major}.{minor}.0", package.name))
        .collect();
    if !reinstall.is_empty() {
        fixes.push(format!("npm install {}", reinstall.join(" ")));
    }
    if !new_packages.is_empty()
        || off_hosts
            .iter()
            .any(|host| line(&host.version).is_some_and(|l| l > cli_line))
    {
        fixes.push("lingxia upgrade".to_string());
    }
    if off_hosts.iter().any(|host| older_host(host)) {
        fixes.push("restart the session with this CLI (`lingxia dev`)".to_string());
    }
    if off_hosts
        .iter()
        .any(|host| !host.is_unreported() && !off_line(host) && off_commit(host))
    {
        fixes.push(format!(
            "use {} and `lingxia` from one build (`lingxia upgrade`, or build both from one \
             checkout), then restart `lingxia dev`",
            cli.name
        ));
    }

    let what = if skewed.is_empty() {
        off_hosts
            .iter()
            .map(|host| host.label())
            .collect::<Vec<_>>()
            .join(", ")
    } else {
        let mut all = skewed;
        all.extend(off_hosts.iter().map(|host| host.label()));
        all.join(", ")
    };
    let reference: Vec<String> = reference
        .into_iter()
        .filter(|label| !what.contains(label.as_str()))
        .collect();
    Err(Skew {
        message: format!(
            "version skew: {what} vs {} — fix: {} (or {ALLOW_SKEW_ENV}=1)",
            reference.join(", "),
            fixes.join("; ")
        ),
    })
}

/// Whether `LINGXIA_ALLOW_SKEW` asks to warn instead of fail.
pub fn skew_allowed() -> bool {
    std::env::var(ALLOW_SKEW_ENV).is_ok_and(|value| {
        !value.is_empty() && value != "0" && !value.eq_ignore_ascii_case("false")
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cli() -> Component {
        Component::new("CLI", "0.18.0 (ae2f993ab 2026-09-20)")
    }

    #[test]
    fn versions_and_build_stamps_parse() {
        assert_eq!(line("0.18.3"), Some((0, 18)));
        assert_eq!(line("v1.2.0-beta.1"), Some((1, 2)));
        assert_eq!(line("x"), None);
        let stamped = cli();
        assert_eq!(stamped.version, "0.18.0");
        assert_eq!(stamped.commit.as_deref(), Some("ae2f993ab"));
        let dirty = Component::new("CLI", "0.18.0 (ae2f993ab-dirty 2026-09-20)");
        assert_eq!(dirty.commit.as_deref(), Some("ae2f993ab"));
        let packed = Component::new("@lingxia/test", "0.18.0+ae2f993");
        assert_eq!(packed.version, "0.18.0");
        assert_eq!(packed.commit.as_deref(), Some("ae2f993"));
        assert_eq!(Component::new("x", "0.18.0+build.1").commit, None);
    }

    #[test]
    fn one_line_passes_whatever_the_patch() {
        let packages = [
            Component::new("@lingxia/react", "0.18.4"),
            Component::new("@lingxia/test", "0.18.0"),
        ];
        let hosts = [Component::new("Runner", "0.18.1")];
        assert_eq!(check(&cli(), &hosts, &packages), Ok(()));
        assert_eq!(check(&cli(), &[], &[]), Ok(()));
    }

    #[test]
    fn an_older_package_names_the_install_that_fixes_it() {
        let packages = [
            Component::new("@lingxia/react", "0.17.2"),
            Component::new("@lingxia/test", "0.18.0"),
        ];
        let hosts = [Component::new("Runner", "0.18.0")];
        let skew = check(&cli(), &hosts, &packages).unwrap_err().message;
        assert_eq!(
            skew,
            "version skew: @lingxia/react 0.17.2 vs CLI 0.18.0 (ae2f993ab), Runner 0.18.0 — \
             fix: npm install @lingxia/react@~0.18.0 (or LINGXIA_ALLOW_SKEW=1)"
        );
        assert!(!skew.contains('\n'));
    }

    #[test]
    fn a_newer_package_or_host_asks_for_a_newer_cli() {
        let skew = check(&cli(), &[], &[Component::new("@lingxia/test", "0.19.0")])
            .unwrap_err()
            .message;
        assert!(skew.contains("fix: lingxia upgrade"), "{skew}");
        let skew = check(&cli(), &[Component::new("host", "0.17.0")], &[])
            .unwrap_err()
            .message;
        assert!(
            skew.starts_with("version skew: host 0.17.0 vs CLI"),
            "{skew}"
        );
        assert!(skew.contains("restart the session"), "{skew}");
    }

    #[test]
    fn a_host_that_reports_no_version_is_too_old() {
        let skew = check(&cli(), &[Component::unreported("Runner")], &[])
            .unwrap_err()
            .message;
        assert!(
            skew.starts_with("version skew: Runner (too old to report its version) vs CLI"),
            "{skew}"
        );
        assert!(skew.contains("restart the session with this CLI"), "{skew}");
    }

    #[test]
    fn a_session_of_another_commit_is_skew() {
        let lxdev = Component::new("lxdev", "0.18.0 (ae2f993ab 2026-09-20)");
        let same = Component::new("lingxia", "0.18.0 (ae2f993ab-dirty 2026-09-21)");
        assert_eq!(check(&lxdev, &[same], &[]), Ok(()));
        let other = Component::new("lingxia", "0.18.0 (1234567ab 2026-09-19)");
        let skew = check(&lxdev, &[other], &[]).unwrap_err().message;
        assert!(
            skew.starts_with(
                "version skew: lingxia 0.18.0 (1234567ab) vs lxdev 0.18.0 (ae2f993ab)"
            ),
            "{skew}"
        );
        assert!(
            skew.contains("use lxdev and `lingxia` from one build"),
            "{skew}"
        );
        // A runtime reports no commit: only its line counts.
        assert_eq!(
            check(&lxdev, &[Component::new("host", "0.18.2")], &[]),
            Ok(())
        );
    }

    #[test]
    fn a_locally_built_package_must_match_the_cli_commit() {
        let same = Component::new("@lingxia/test", "0.18.0+ae2f993");
        assert_eq!(check(&cli(), &[], &[same]), Ok(()));
        let other = Component::new("@lingxia/test", "0.18.0").with_commit(Some("1234567"));
        let skew = check(&cli(), &[], &[other]).unwrap_err().message;
        assert!(skew.contains("@lingxia/test 0.18.0 (1234567)"), "{skew}");
        // A CLI without a commit cannot tell; the line decides.
        let bare = Component::new("CLI", "0.18.0");
        let other = Component::new("@lingxia/test", "0.18.0+1234567");
        assert_eq!(check(&bare, &[], &[other]), Ok(()));
    }

    #[test]
    fn installed_packages_are_read_from_node_modules() {
        let dir = std::env::temp_dir().join(format!("lx-compat-{}", std::process::id()));
        let scope = dir.join("node_modules/@lingxia");
        for (name, manifest) in [
            ("test", r#"{"name":"@lingxia/test","version":"0.18.0"}"#),
            (
                "react",
                r#"{"name":"@lingxia/react","version":"0.17.2","lingxia":{"commit":"abcdef1"}}"#,
            ),
            ("broken", "not json"),
        ] {
            std::fs::create_dir_all(scope.join(name)).unwrap();
            std::fs::write(scope.join(name).join("package.json"), manifest).unwrap();
        }
        let packages = installed_packages(&dir);
        let _ = std::fs::remove_dir_all(&dir);
        assert_eq!(packages.len(), 2);
        assert_eq!(packages[0].name, "@lingxia/react");
        assert_eq!(packages[0].commit.as_deref(), Some("abcdef1"));
        assert!(installed_packages(Path::new("/nonexistent")).is_empty());
    }
}
