//! Watch lxapp sources during `lingxia dev` and rebuild + reload in place.
//!
//! Standalone projects watch the `lxapp.json` directory. Host apps watch each
//! `resources.bundles[].path` that is a local lxapp. Build outputs (`dist`,
//! `node_modules`, `.lingxia`, …) are not watched, so a rebuild cannot loop.

use super::server::{DevServerState, RestartOutcome};
use anyhow::{Context, Result};
use colored::Colorize;
use notify::{EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, RecvTimeoutError};
use std::thread::{self, JoinHandle};
use std::time::Duration;

const DEBOUNCE: Duration = Duration::from_millis(400);
const IDLE_POLL: Duration = Duration::from_millis(200);

const IGNORED_DIR_NAMES: &[&str] = &[
    "node_modules",
    "dist",
    "target",
    ".git",
    ".lingxia",
    "test-results",
    ".vite",
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct WatchRoot {
    pub app_id: String,
    pub path: PathBuf,
}

pub(super) struct LxAppWatchOptions {
    pub framework: Option<String>,
    pub release: bool,
}

pub(super) struct LxAppWatch {
    thread: Option<JoinHandle<()>>,
}

impl LxAppWatch {
    pub(super) fn spawn(
        state: Arc<DevServerState>,
        options: LxAppWatchOptions,
    ) -> Result<Option<Self>> {
        let roots = discover_watch_roots(&state.project_root)?;
        if roots.is_empty() {
            return Ok(None);
        }

        let names = roots
            .iter()
            .map(|root| root.app_id.as_str())
            .collect::<Vec<_>>()
            .join(", ");
        println!(
            "  {} watching {} — save to rebuild and reload",
            "•".cyan(),
            names.cyan()
        );

        let stop_flag = state.stop_flag.clone();
        let thread = thread::Builder::new()
            .name("lxapp-watch".into())
            .spawn(move || {
                if let Err(err) = run_watch(state, roots, options, stop_flag) {
                    eprintln!("⚠ lxapp auto-reload stopped ({err:#})");
                }
            })
            .context("Failed to start lxapp watcher")?;
        Ok(Some(Self {
            thread: Some(thread),
        }))
    }

    pub(super) fn join(mut self) {
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

pub(super) fn discover_watch_roots(project_root: &Path) -> Result<Vec<WatchRoot>> {
    if crate::lxapp::is_built_bundle_dir(project_root) {
        return Ok(Vec::new());
    }
    if project_root.join("lxapp.json").is_file() {
        return Ok(vec![WatchRoot {
            app_id: read_app_id(project_root)?,
            path: project_root.to_path_buf(),
        }]);
    }
    if !project_root.join("lingxia.yaml").is_file() {
        return Ok(Vec::new());
    }

    let config = crate::config::LingXiaConfig::load(project_root)?;
    let Some(resources) = config.resources.as_ref() else {
        return Ok(Vec::new());
    };
    let mut roots = Vec::new();
    for bundle in &resources.bundles {
        let Some(relative) = bundle.path.as_deref() else {
            continue;
        };
        let path = project_root.join(relative);
        if !path.join("lxapp.json").is_file() || crate::lxapp::is_built_bundle_dir(&path) {
            continue;
        }
        roots.push(WatchRoot {
            app_id: bundle.app_id.clone(),
            path,
        });
    }
    Ok(roots)
}

pub(super) fn is_ignored_path(path: &Path) -> bool {
    if path.components().any(|component| {
        component
            .as_os_str()
            .to_str()
            .is_some_and(|name| IGNORED_DIR_NAMES.contains(&name))
    }) {
        return true;
    }
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(is_ignored_file_name)
}

fn is_ignored_file_name(name: &str) -> bool {
    name.ends_with('~')
        || name.ends_with(".swp")
        || name.ends_with(".swo")
        || name.ends_with(".tmp")
        || name.ends_with(".map")
        || name == ".DS_Store"
}

pub(super) fn watch_entries(root: &Path) -> Vec<(PathBuf, RecursiveMode)> {
    let mut entries = vec![(root.to_path_buf(), RecursiveMode::NonRecursive)];
    let Ok(read) = fs::read_dir(root) else {
        return entries;
    };
    for entry in read.flatten() {
        let path = entry.path();
        if is_ignored_path(&path) || !path.is_dir() {
            continue;
        }
        entries.push((path, RecursiveMode::Recursive));
    }
    entries
}

fn read_app_id(lxapp_dir: &Path) -> Result<String> {
    let manifest: serde_json::Value = serde_json::from_str(
        &fs::read_to_string(lxapp_dir.join("lxapp.json")).with_context(|| {
            format!("Failed to read {}", lxapp_dir.join("lxapp.json").display())
        })?,
    )
    .context("lxapp.json is not valid JSON")?;
    manifest
        .get("appId")
        .and_then(|value| value.as_str())
        .filter(|app_id| !app_id.is_empty())
        .map(ToOwned::to_owned)
        .ok_or_else(|| anyhow::anyhow!("lxapp.json is missing appId"))
}

fn run_watch(
    state: Arc<DevServerState>,
    roots: Vec<WatchRoot>,
    options: LxAppWatchOptions,
    stop_flag: Arc<AtomicBool>,
) -> Result<()> {
    let (tx, rx) = mpsc::channel();
    let mut watcher = RecommendedWatcher::new(tx, notify::Config::default())
        .context("Failed to create filesystem watcher")?;
    // Top-level directories watched recursively. One created later (a new
    // `mocks/`, `pages/`) is added when it appears, and re-added when it is
    // removed and made again.
    let mut subtrees: BTreeSet<PathBuf> = BTreeSet::new();
    for root in &roots {
        for (path, mode) in watch_entries(&root.path) {
            watcher
                .watch(&path, mode)
                .with_context(|| format!("Failed to watch {}", path.display()))?;
            if mode == RecursiveMode::Recursive {
                subtrees.insert(path);
            }
        }
    }

    let mut dirty: BTreeSet<String> = BTreeSet::new();
    // Saves under `mocks/` reload the mocks only: no rebuild, no restart.
    let mut mocks_dirty: BTreeSet<String> = BTreeSet::new();
    let mut deferred = false;
    while !stop_flag.load(Ordering::Acquire) {
        let timeout = if dirty.is_empty() && mocks_dirty.is_empty() {
            IDLE_POLL
        } else {
            DEBOUNCE
        };
        match rx.recv_timeout(timeout) {
            Ok(Ok(event)) => {
                if !is_reload_event(event.kind) {
                    continue;
                }
                for path in event.paths {
                    if is_ignored_path(&path) {
                        continue;
                    }
                    if let Some(change) = subtree_change(&roots, &subtrees, &path) {
                        match change {
                            SubtreeChange::Appeared => {
                                if let Err(err) = watcher.watch(&path, RecursiveMode::Recursive) {
                                    eprintln!("⚠ lxapp watch: {}: {err}", path.display());
                                } else {
                                    subtrees.insert(path.clone());
                                }
                            }
                            SubtreeChange::Removed => {
                                let _ = watcher.unwatch(&path);
                                subtrees.remove(&path);
                            }
                        }
                    }
                    if let Some(root) = root_for_path(&roots, &path) {
                        let path = strip_verbatim_prefix(&path);
                        if super::mocks::is_mocks_path(strip_verbatim_prefix(&root.path), path) {
                            mocks_dirty.insert(root.app_id.clone());
                        } else {
                            dirty.insert(root.app_id.clone());
                        }
                    }
                }
            }
            Ok(Err(err)) => {
                eprintln!("⚠ lxapp watch: {err}");
            }
            Err(RecvTimeoutError::Timeout) => {
                if (dirty.is_empty() && mocks_dirty.is_empty()) || stop_flag.load(Ordering::Acquire)
                {
                    continue;
                }
                // A test run pauses the watcher (`session.watch.pause`):
                // rebuilding empties the build output the app loads from, and
                // reloading replaces the app under a running spec. Keep the
                // changes queued; they rebuild once when the pause ends.
                if state.watch_paused() {
                    if !deferred {
                        let waiting: BTreeSet<String> =
                            dirty.union(&mocks_dirty).cloned().collect();
                        println!(
                            "  {} watcher paused by a test run — rebuild of {} waits until it ends",
                            "•".cyan(),
                            waiting.into_iter().collect::<Vec<_>>().join(", ").cyan()
                        );
                        deferred = true;
                    }
                    continue;
                }
                deferred = false;
                for app_id in std::mem::take(&mut mocks_dirty) {
                    if let Some(root) = roots.iter().find(|root| root.app_id == app_id) {
                        state.reload_mocks(&app_id, &root.path);
                    }
                }
                let app_ids: Vec<String> = dirty.iter().cloned().collect();
                dirty.clear();
                for app_id in app_ids {
                    if stop_flag.load(Ordering::Acquire) {
                        break;
                    }
                    if reload_one(&state, &app_id, &options) == Reload::Deferred {
                        // A run started while it rebuilt: restart once it ends.
                        dirty.insert(app_id);
                        deferred = true;
                    }
                }
            }
            Err(RecvTimeoutError::Disconnected) => break,
        }
    }
    drop(watcher);
    Ok(())
}

#[derive(Debug, PartialEq, Eq)]
enum Reload {
    Done,
    Deferred,
}

fn reload_one(state: &DevServerState, app_id: &str, options: &LxAppWatchOptions) -> Reload {
    println!("  {} rebuilding {}...", "↻".cyan(), app_id.cyan());
    if let Err(err) = state.rebuild_lxapp(app_id, options.framework.as_deref(), options.release) {
        eprintln!("  {} auto-reload {app_id} failed: {err:#}", "✗".red());
        return Reload::Done;
    }
    match state.restart_lxapp(app_id) {
        Ok(RestartOutcome::Restarted) => {
            println!("  {} reloaded {}", "✓".green(), app_id.cyan());
        }
        Ok(RestartOutcome::NotConnected) => {
            println!(
                "  {} rebuilt {} (runtime not connected yet)",
                "✓".green(),
                app_id.cyan()
            );
        }
        Ok(RestartOutcome::Deferred) => {
            println!(
                "  {} rebuilt {}; a test run started meanwhile — reload deferred until it ends",
                "•".cyan(),
                app_id.cyan()
            );
            return Reload::Deferred;
        }
        Err(err) => {
            eprintln!("  {} rebuilt {app_id}, restart failed: {err:#}", "✗".red());
        }
    }
    Reload::Done
}

#[derive(Debug, PartialEq, Eq)]
enum SubtreeChange {
    Appeared,
    Removed,
}

/// Whether `path`, reported by an event, is a top-level directory of a root
/// that has just appeared (not yet watched recursively) or is gone (and was).
fn subtree_change(
    roots: &[WatchRoot],
    subtrees: &BTreeSet<PathBuf>,
    path: &Path,
) -> Option<SubtreeChange> {
    let parent = path.parent()?;
    let top_level = roots
        .iter()
        .any(|root| strip_verbatim_prefix(&root.path) == strip_verbatim_prefix(parent));
    if !top_level {
        return None;
    }
    let watched = subtrees.contains(path);
    match (path.is_dir(), watched) {
        (true, false) => Some(SubtreeChange::Appeared),
        (false, true) if !path.exists() => Some(SubtreeChange::Removed),
        _ => None,
    }
}

fn is_reload_event(kind: EventKind) -> bool {
    !matches!(kind, EventKind::Access(_) | EventKind::Other)
}

fn root_for_path<'a>(roots: &'a [WatchRoot], path: &Path) -> Option<&'a WatchRoot> {
    let path = strip_verbatim_prefix(path);
    roots
        .iter()
        .filter(|root| path.starts_with(strip_verbatim_prefix(&root.path)))
        .max_by_key(|root| root.path.as_os_str().len())
}

fn strip_verbatim_prefix(path: &Path) -> &Path {
    path.to_str()
        .and_then(|value| value.strip_prefix(r"\\?\"))
        .map(Path::new)
        .unwrap_or(path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn write(path: &Path, contents: &str) {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(path, contents).unwrap();
    }

    #[test]
    fn discover_standalone_lxapp_root() {
        let temp = tempfile::tempdir().unwrap();
        write(
            &temp.path().join("lxapp.json"),
            r#"{"appId":"chat","appName":"Chat","version":"1.0.0","pages":[]}"#,
        );
        let roots = discover_watch_roots(temp.path()).unwrap();
        assert_eq!(
            roots,
            vec![WatchRoot {
                app_id: "chat".into(),
                path: temp.path().to_path_buf(),
            }]
        );
    }

    #[test]
    fn discover_host_bundle_paths_and_skip_package_only() {
        let temp = tempfile::tempdir().unwrap();
        write(
            &temp.path().join("lingxia.yaml"),
            "app:\n  projectName: demo-host\n  packageId: app.example.demohost\n  productName: Demo Host\n  productVersion: 1.0.0\n  platforms: [windows]\n  homeAppId: demo\nresources:\n  bundles:\n    - type: lxapp\n      appId: demo\n      path: home\n    - type: lxapp\n      appId: settings\n      path: settings\n    - type: lxapp\n      appId: packaged\n      package: \"@demo/packaged\"\n      version: \"1.0.0\"\n    - type: lxapp\n      appId: missing\n      path: nowhere\n",
        );
        write(
            &temp.path().join("home/lxapp.json"),
            r#"{"appId":"demo","appName":"Demo","version":"1.0.0","pages":[]}"#,
        );
        write(
            &temp.path().join("settings/lxapp.json"),
            r#"{"appId":"settings","appName":"Settings","version":"1.0.0","pages":[]}"#,
        );

        let roots = discover_watch_roots(temp.path()).unwrap();
        assert_eq!(
            roots,
            vec![
                WatchRoot {
                    app_id: "demo".into(),
                    path: temp.path().join("home"),
                },
                WatchRoot {
                    app_id: "settings".into(),
                    path: temp.path().join("settings"),
                },
            ]
        );
    }

    #[test]
    fn a_top_level_directory_made_later_is_watched_and_one_removed_is_dropped() {
        let temp = tempfile::tempdir().unwrap();
        let roots = vec![WatchRoot {
            app_id: "demo".into(),
            path: temp.path().to_path_buf(),
        }];
        let mut subtrees = BTreeSet::new();
        let mocks = temp.path().join("mocks");
        // Not there yet: nothing to watch.
        assert_eq!(subtree_change(&roots, &subtrees, &mocks), None);
        fs::create_dir_all(mocks.join("fixtures")).unwrap();
        assert_eq!(
            subtree_change(&roots, &subtrees, &mocks),
            Some(SubtreeChange::Appeared)
        );
        // Only top-level directories: nested ones are covered recursively.
        assert_eq!(
            subtree_change(&roots, &subtrees, &mocks.join("fixtures")),
            None
        );
        subtrees.insert(mocks.clone());
        assert_eq!(subtree_change(&roots, &subtrees, &mocks), None);
        fs::remove_dir_all(&mocks).unwrap();
        assert_eq!(
            subtree_change(&roots, &subtrees, &mocks),
            Some(SubtreeChange::Removed)
        );
        subtrees.remove(&mocks);
        // Made again: watched again.
        fs::create_dir_all(&mocks).unwrap();
        assert_eq!(
            subtree_change(&roots, &subtrees, &mocks),
            Some(SubtreeChange::Appeared)
        );
        // A file at the top level is not a subtree.
        write(&temp.path().join("app.ts"), "");
        assert_eq!(
            subtree_change(&roots, &subtrees, &temp.path().join("app.ts")),
            None
        );
    }

    #[test]
    fn a_new_mocks_dir_is_watched_recursively_by_the_session() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().canonicalize().unwrap();
        let roots = vec![WatchRoot {
            app_id: "demo".into(),
            path: root.clone(),
        }];
        let (tx, rx) = mpsc::channel();
        let mut watcher = RecommendedWatcher::new(tx, notify::Config::default()).unwrap();
        let mut subtrees = BTreeSet::new();
        for (path, mode) in watch_entries(&root) {
            watcher.watch(&path, mode).unwrap();
        }
        let mocks = root.join("mocks");
        fs::create_dir_all(&mocks).unwrap();
        // The session's loop: a new top-level directory gets a recursive watch.
        let deadline = std::time::Instant::now() + Duration::from_secs(10);
        while !subtrees.contains(&mocks) && std::time::Instant::now() < deadline {
            if let Ok(Ok(event)) = rx.recv_timeout(Duration::from_millis(100)) {
                for path in event.paths {
                    if subtree_change(&roots, &subtrees, &path) == Some(SubtreeChange::Appeared) {
                        watcher.watch(&path, RecursiveMode::Recursive).unwrap();
                        subtrees.insert(path);
                    }
                }
            }
        }
        assert!(subtrees.contains(&mocks), "the new mocks/ was noticed");
        // A nested edit inside it is now seen, and it is a mocks path.
        std::thread::sleep(Duration::from_millis(200));
        let nested = mocks.join("data/devices.ts");
        write(&nested, "export default {};");
        let deadline = std::time::Instant::now() + Duration::from_secs(10);
        let mut seen = false;
        while !seen && std::time::Instant::now() < deadline {
            if let Ok(Ok(event)) = rx.recv_timeout(Duration::from_millis(100)) {
                seen = event.paths.iter().any(|path| {
                    path.starts_with(&mocks)
                        && super::super::mocks::is_mocks_path(&root, path)
                        && path.ends_with("devices.ts")
                });
            }
        }
        assert!(seen, "an edit inside the new mocks/ reached the watcher");
    }

    #[test]
    fn discover_empty_without_lxapp_or_host() {
        let temp = tempfile::tempdir().unwrap();
        assert!(discover_watch_roots(temp.path()).unwrap().is_empty());
    }

    #[test]
    fn ignore_build_outputs_and_editor_junk() {
        assert!(is_ignored_path(Path::new("pages/dist/index.js")));
        assert!(is_ignored_path(Path::new("node_modules/foo/index.js")));
        assert!(is_ignored_path(Path::new(
            ".lingxia/dev/lxapp/demo/manifest.json"
        )));
        assert!(is_ignored_path(Path::new("target/debug/lingxia.exe")));
        assert!(is_ignored_path(Path::new("pages/home/index.tsx~")));
        assert!(is_ignored_path(Path::new("pages/home/.index.tsx.swp")));
        assert!(!is_ignored_path(Path::new("pages/home/index.tsx")));
        assert!(!is_ignored_path(Path::new("lxapp.json")));
        assert!(!is_ignored_path(Path::new("pages/home/index.json")));
    }

    #[test]
    fn watch_entries_skip_ignored_top_level_dirs() {
        let temp = tempfile::tempdir().unwrap();
        write(&temp.path().join("lxapp.json"), "{}");
        write(&temp.path().join("pages/home/index.tsx"), "export {}");
        write(
            &temp.path().join("node_modules/pkg/index.js"),
            "module.exports = {}",
        );
        write(&temp.path().join("dist/index.js"), "void 0");
        fs::create_dir_all(temp.path().join("public")).unwrap();

        let entries = watch_entries(temp.path());
        let paths: Vec<PathBuf> = entries.iter().map(|(path, _)| path.clone()).collect();
        assert!(paths.contains(&temp.path().to_path_buf()));
        assert!(paths.contains(&temp.path().join("pages")));
        assert!(paths.contains(&temp.path().join("public")));
        assert!(!paths.contains(&temp.path().join("node_modules")));
        assert!(!paths.contains(&temp.path().join("dist")));
        assert!(
            entries
                .iter()
                .any(|(path, mode)| path == temp.path() && *mode == RecursiveMode::NonRecursive)
        );
    }

    #[test]
    fn root_for_path_picks_the_longest_matching_bundle() {
        let roots = vec![
            WatchRoot {
                app_id: "home".into(),
                path: PathBuf::from("/app/lxapp"),
            },
            WatchRoot {
                app_id: "settings".into(),
                path: PathBuf::from("/app/settings"),
            },
        ];
        assert_eq!(
            root_for_path(&roots, Path::new("/app/lxapp/pages/home/index.tsx"))
                .map(|root| root.app_id.as_str()),
            Some("home")
        );
        assert_eq!(
            root_for_path(&roots, Path::new("/app/settings/lxapp.json"))
                .map(|root| root.app_id.as_str()),
            Some("settings")
        );
        assert!(root_for_path(&roots, Path::new("/app/other/x")).is_none());
    }
}
