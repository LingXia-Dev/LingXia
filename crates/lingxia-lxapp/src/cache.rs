use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};

use filetime::FileTime;

pub fn touch_access_time(path: &Path) {
    let now = FileTime::now();
    let _ = filetime::set_file_atime(path, now);
}

pub(crate) fn touch_modified_time(path: &Path) {
    let now = FileTime::now();
    let _ = filetime::set_file_mtime(path, now);
}

static CLEANUP_PROTECTIONS: OnceLock<Mutex<HashMap<PathBuf, usize>>> = OnceLock::new();

/// Keeps paths that back active runtime work out of product cache cleanup.
pub(crate) struct CleanupProtection {
    paths: Vec<PathBuf>,
}

pub(crate) fn protect_from_cleanup(paths: impl IntoIterator<Item = PathBuf>) -> CleanupProtection {
    let paths: Vec<_> = paths.into_iter().collect();
    let mut protections = CLEANUP_PROTECTIONS
        .get_or_init(|| Mutex::new(HashMap::new()))
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    for path in &paths {
        *protections.entry(path.clone()).or_default() += 1;
    }
    CleanupProtection { paths }
}

impl Drop for CleanupProtection {
    fn drop(&mut self) {
        let mut protections = CLEANUP_PROTECTIONS
            .get_or_init(|| Mutex::new(HashMap::new()))
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        for path in &self.paths {
            if let Some(count) = protections.get_mut(path) {
                *count -= 1;
                if *count == 0 {
                    protections.remove(path);
                }
            }
        }
    }
}

pub(crate) fn is_protected_from_cleanup(path: &Path) -> bool {
    CLEANUP_PROTECTIONS
        .get_or_init(|| Mutex::new(HashMap::new()))
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .contains_key(path)
}

/// Atomically checks cleanup protection and removes the path while new work is
/// prevented from claiming it. If removal wins the race, the new work starts
/// against a clean path; if protection wins, cleanup leaves the path alone.
fn remove_if_unprotected(path: &Path) -> std::io::Result<Option<u64>> {
    let protections = CLEANUP_PROTECTIONS
        .get_or_init(|| Mutex::new(HashMap::new()))
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    if protections.contains_key(path) {
        return Ok(None);
    }
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(Some(0)),
        Err(err) => return Err(err),
    };
    let bytes = lingxia_service::storage::path_size(path);
    if metadata.file_type().is_dir() {
        fs::remove_dir_all(path)?;
    } else {
        fs::remove_file(path)?;
    }
    Ok(Some(bytes))
}

/// Product-wide cache maintenance, backing the host app's "clear cache"
/// control. Deliberately app-scoped rather than lxapp-scoped: the number a
/// settings screen shows is the whole product's, so it has to span every lxapp
/// the host has ever run, not just the one asking.
///
/// What it covers, and what it must never touch, follows the storage contract:
/// temp is disposable by definition and usercache is regenerable by contract,
/// so both are fair game. Userdata, the `lx.getStorage` key-value store, the
/// user's downloads, and installed lxapp packages are not caches — deleting any
/// of them behind a "clear cache" button is data loss, not maintenance.
pub mod product {
    use crate::LxAppError;
    use crate::lxapp::{
        LINGXIA_DIR, LXAPPS_DIR, TEMP_DIR, USER_CACHE_DIR, metadata, runtime_registry,
    };

    /// Shared runtime artwork cache (`{cache}/lingxia/lxapps/icons`). Harmless
    /// if the host never writes there; when a registry provider does, a clear
    /// drops the files and they re-fetch on demand.
    const ICONS_DIR: &str = "icons";
    use lingxia_platform::traits::app_runtime::AppRuntime;
    use std::fs;
    use std::path::PathBuf;
    use std::time::{Duration, SystemTime};

    /// Nothing touched this recently is treated as garbage, however unreferenced
    /// it looks. An install is extracted before its record is written and a
    /// download lands before its record is written, so both have a window where
    /// live work is referenced by nothing — and deleting into that window would
    /// destroy exactly the update the user is waiting for.
    const RECENT_GRACE: Duration = Duration::from_secs(60 * 60);

    struct Roots {
        /// Every lxapp's `lx://usercache`, one directory per app.
        usercache: PathBuf,
        /// Every lxapp's temp, one directory per app and then per session.
        temp: PathBuf,
        /// Shared runtime artwork (`icons/`), empty until a host writes it.
        icons: PathBuf,
        /// Unpacked lxapp installs, one versioned directory per install.
        installs: PathBuf,
        /// Staged update archives waiting to be applied.
        downloads: PathBuf,
    }

    fn roots() -> Option<Roots> {
        let runtime = runtime_registry::get_platform()?;
        let cache_dir = runtime.app_cache_dir().join(LINGXIA_DIR).join(LXAPPS_DIR);
        Some(Roots {
            usercache: runtime
                .app_data_dir()
                .join(LINGXIA_DIR)
                .join(USER_CACHE_DIR),
            temp: cache_dir.join(TEMP_DIR),
            icons: cache_dir.join(ICONS_DIR),
            installs: runtime.app_data_dir().join(LINGXIA_DIR).join(LXAPPS_DIR),
            downloads: cache_dir.join("download"),
        })
    }

    /// Estimated reclaimable bytes, excluding protected runtime storage.
    ///
    /// Covers LingXia-managed files only. The WebView's own HTTP cache is not
    /// included: the platform stores expose a site count, not a byte total, so
    /// any figure here would be invented. [`clear`] does drop it.
    pub fn usage_bytes() -> u64 {
        let Some(roots) = roots() else {
            return 0;
        };
        let mut total = 0;
        for app_dir in child_dirs(&roots.usercache) {
            if !super::is_protected_from_cleanup(&app_dir) {
                total += lingxia_service::storage::dir_size(&app_dir);
            }
        }
        total += lingxia_service::storage::dir_size(&roots.icons);
        for session in session_dirs(&roots.temp) {
            if !super::is_protected_from_cleanup(&session) {
                total += lingxia_service::storage::dir_size(&session);
            }
        }
        for orphan in orphaned_packages(&roots) {
            total += lingxia_service::storage::path_size(&orphan);
        }
        total
    }

    /// Install directories and staged archives no record points at any more.
    ///
    /// These leak by design gaps the runtime already admits: replacing an
    /// install removes the previous directory best-effort and warns "manual
    /// cleanup may be needed" when it cannot, and dropping a pending update
    /// deletes its archive best-effort and warns "disk space may be wasted".
    /// Neither path ever retries, so a failed removal is permanent until
    /// something sweeps — which is what this is.
    fn orphaned_packages(roots: &Roots) -> Vec<PathBuf> {
        let mut orphans = Vec::new();
        if let Ok(referenced) = metadata::installed_paths() {
            for dir in child_dirs(&roots.installs) {
                if is_orphan(&dir, &referenced) {
                    orphans.push(dir);
                }
            }
        }
        if let Ok(referenced) = metadata::downloaded_archives()
            && let Ok(entries) = fs::read_dir(&roots.downloads)
        {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_file() && is_orphan(&path, &referenced) {
                    orphans.push(path);
                }
            }
        }
        orphans
    }

    fn is_orphan(path: &std::path::Path, referenced: &std::collections::BTreeSet<String>) -> bool {
        if referenced.contains(&path.to_string_lossy().into_owned())
            || super::is_protected_from_cleanup(path)
        {
            return false;
        }
        // Too new or impossible to date means its record may not be written yet.
        !recent_or_unknown(path, RECENT_GRACE)
    }

    fn recent_or_unknown(path: &std::path::Path, window: Duration) -> bool {
        let Ok(modified) = fs::metadata(path).and_then(|metadata| metadata.modified()) else {
            return true;
        };
        match SystemTime::now().duration_since(modified) {
            Ok(age) => age < window,
            // A future timestamp usually means the wall clock moved backwards.
            // It is not evidence that an unreferenced package is safe to delete.
            Err(_) => true,
        }
    }

    /// A best-effort maintenance report. Byte counts exclude WebView storage.
    #[derive(Debug, Default)]
    pub struct ClearReport {
        pub freed_bytes: u64,
        pub skipped_active_paths: usize,
        pub failures: Vec<String>,
        pub webview: &'static str,
    }

    /// Clear currently reclaimable caches, preserving live session storage.
    pub async fn clear() -> Result<ClearReport, LxAppError> {
        let mut report = tokio::task::spawn_blocking(clear_managed_files)
            .await
            .map_err(|err| LxAppError::Runtime(format!("cache cleanup task failed: {err}")))??;
        record_webview_result(
            &mut report,
            lingxia_webview::data_store::clear_cache(None).await,
        );
        Ok(report)
    }

    fn record_webview_result(
        report: &mut ClearReport,
        result: Result<(), lingxia_webview::WebViewError>,
    ) {
        report.webview = match result {
            Ok(()) => "cleared",
            Err(lingxia_webview::WebViewError::Unsupported(_)) => "unsupported",
            Err(err) => {
                report.failures.push(format!("WebView cache: {err}"));
                "failed"
            }
        };
    }

    fn clear_managed_files() -> Result<ClearReport, LxAppError> {
        let roots = roots()
            .ok_or_else(|| LxAppError::Runtime("app runtime is not initialized".to_string()))?;
        Ok(clear_managed_roots(&roots))
    }

    fn clear_managed_roots(roots: &Roots) -> ClearReport {
        let mut report = ClearReport::default();
        clear_private_storage(roots, &mut report);
        empty_dir(&roots.icons, &mut report);
        sweep_orphaned_packages(roots, &mut report);
        report
    }

    fn clear_private_storage(roots: &Roots, report: &mut ClearReport) {
        for app_dir in child_dirs_for_cleanup(&roots.usercache, &mut report.failures) {
            remove_path(&app_dir, report);
        }
        for session in session_dirs_for_cleanup(&roots.temp, &mut report.failures) {
            remove_path(&session, report);
        }
    }

    // Package garbage collection remains an internal maintenance operation.
    fn sweep_orphaned_packages(roots: &Roots, report: &mut ClearReport) {
        for orphan in orphaned_packages_for_cleanup(roots, &mut report.failures) {
            remove_path(&orphan, report);
        }
    }

    fn remove_path(path: &std::path::Path, report: &mut ClearReport) {
        match super::remove_if_unprotected(path) {
            Ok(Some(bytes)) => report.freed_bytes += bytes,
            Ok(None) => report.skipped_active_paths += 1,
            Err(err) => report.failures.push(format!("{}: {err}", path.display())),
        }
    }

    fn child_dirs(root: &std::path::Path) -> Vec<PathBuf> {
        let Ok(entries) = fs::read_dir(root) else {
            return Vec::new();
        };
        entries
            .flatten()
            .filter(|entry| entry.file_type().is_ok_and(|kind| kind.is_dir()))
            .map(|entry| entry.path())
            .collect()
    }

    /// `temp/<app>/<session>` — two levels, because a clear keeps live sessions
    /// while dropping the rest of the same app's.
    fn session_dirs(temp_root: &std::path::Path) -> Vec<PathBuf> {
        child_dirs(temp_root)
            .iter()
            .flat_map(|app_dir| child_dirs(app_dir))
            .collect()
    }

    fn child_dirs_for_cleanup(root: &std::path::Path, failures: &mut Vec<String>) -> Vec<PathBuf> {
        let entries = match fs::read_dir(root) {
            Ok(entries) => entries,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Vec::new(),
            Err(err) => {
                failures.push(format!("{}: {err}", root.display()));
                return Vec::new();
            }
        };
        let mut dirs = Vec::new();
        for entry in entries {
            let entry = match entry {
                Ok(entry) => entry,
                Err(err) => {
                    failures.push(format!("{}: {err}", root.display()));
                    continue;
                }
            };
            match entry.file_type() {
                Ok(kind) if kind.is_dir() => dirs.push(entry.path()),
                Ok(_) => {}
                Err(err) => failures.push(format!("{}: {err}", entry.path().display())),
            }
        }
        dirs
    }

    fn session_dirs_for_cleanup(
        temp_root: &std::path::Path,
        failures: &mut Vec<String>,
    ) -> Vec<PathBuf> {
        child_dirs_for_cleanup(temp_root, failures)
            .iter()
            .flat_map(|app_dir| child_dirs_for_cleanup(app_dir, failures))
            .collect()
    }

    fn orphaned_packages_for_cleanup(roots: &Roots, failures: &mut Vec<String>) -> Vec<PathBuf> {
        let mut orphans = Vec::new();
        match metadata::installed_paths() {
            Ok(referenced) => {
                for dir in child_dirs_for_cleanup(&roots.installs, failures) {
                    if is_orphan(&dir, &referenced) {
                        orphans.push(dir);
                    }
                }
            }
            Err(err) => failures.push(format!("installed package metadata: {err}")),
        }
        match metadata::downloaded_archives() {
            Ok(referenced) => {
                let entries = match fs::read_dir(&roots.downloads) {
                    Ok(entries) => Some(entries),
                    Err(err) if err.kind() == std::io::ErrorKind::NotFound => None,
                    Err(err) => {
                        failures.push(format!("{}: {err}", roots.downloads.display()));
                        None
                    }
                };
                for entry in entries.into_iter().flatten() {
                    let entry = match entry {
                        Ok(entry) => entry,
                        Err(err) => {
                            failures.push(format!("{}: {err}", roots.downloads.display()));
                            continue;
                        }
                    };
                    let path = entry.path();
                    match entry.file_type() {
                        Ok(kind) if kind.is_file() && is_orphan(&path, &referenced) => {
                            orphans.push(path);
                        }
                        Ok(_) => {}
                        Err(err) => failures.push(format!("{}: {err}", path.display())),
                    }
                }
            }
            Err(err) => failures.push(format!("staged update metadata: {err}")),
        }
        orphans
    }

    fn empty_dir(dir: &std::path::Path, report: &mut ClearReport) {
        let entries = match fs::read_dir(dir) {
            Ok(entries) => entries,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => return,
            Err(err) => {
                report.failures.push(format!("{}: {err}", dir.display()));
                return;
            }
        };
        for entry in entries {
            match entry {
                Ok(entry) => remove_path(&entry.path(), report),
                Err(err) => report.failures.push(format!("{}: {err}", dir.display())),
            }
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use filetime::FileTime;
        use std::collections::BTreeSet;

        #[test]
        fn orphan_classifier_preserves_referenced_recent_and_uncertain_paths() {
            let root = tempfile::tempdir().unwrap();
            let old = root.path().join("old");
            let old_part = root.path().join("update.part");
            let recent = root.path().join("recent");
            let future = root.path().join("future");
            fs::create_dir(&old).unwrap();
            fs::write(&old_part, b"partial").unwrap();
            fs::create_dir(&recent).unwrap();
            fs::create_dir(&future).unwrap();
            filetime::set_file_mtime(
                &old,
                FileTime::from_system_time(SystemTime::now() - Duration::from_secs(7200)),
            )
            .unwrap();
            filetime::set_file_mtime(
                &old_part,
                FileTime::from_system_time(SystemTime::now() - Duration::from_secs(7200)),
            )
            .unwrap();
            filetime::set_file_mtime(
                &future,
                FileTime::from_system_time(SystemTime::now() + Duration::from_secs(7200)),
            )
            .unwrap();

            let mut referenced = BTreeSet::new();
            referenced.insert(old.to_string_lossy().into_owned());
            assert!(!is_orphan(&old, &referenced));

            referenced.clear();
            let active = super::super::protect_from_cleanup([old.clone()]);
            assert!(!is_orphan(&old, &referenced));
            drop(active);
            assert!(is_orphan(&old, &referenced));
            assert!(is_orphan(&old_part, &referenced));
            assert!(!is_orphan(&recent, &referenced));
            assert!(!is_orphan(&future, &referenced));
            assert!(!is_orphan(&root.path().join("missing"), &referenced));
        }

        #[test]
        fn cleanup_protection_is_reference_counted_and_blocks_removal() {
            let root = tempfile::tempdir().unwrap();
            let path = root.path().join("active.part");
            fs::write(&path, b"partial").unwrap();
            let first = super::super::protect_from_cleanup([path.clone()]);
            let second = super::super::protect_from_cleanup([path.clone()]);

            assert_eq!(super::super::remove_if_unprotected(&path).unwrap(), None);
            drop(first);
            assert_eq!(super::super::remove_if_unprotected(&path).unwrap(), None);
            drop(second);
            assert_eq!(super::super::remove_if_unprotected(&path).unwrap(), Some(7));
            assert!(!path.exists());
        }

        #[test]
        fn private_cleanup_preserves_live_cache_temp_and_durable_data() {
            let root = tempfile::tempdir().unwrap();
            let roots = Roots {
                usercache: root.path().join("usercache"),
                temp: root.path().join("temp"),
                icons: root.path().join("icons"),
                installs: root.path().join("installs"),
                downloads: root.path().join("downloads"),
            };
            let live_cache = roots.usercache.join("live");
            let idle_cache = roots.usercache.join("idle");
            let live_temp = roots.temp.join("live/session");
            let stale_temp = roots.temp.join("live/stale");
            let userdata = root.path().join("userdata/live");
            let kv = root.path().join("storage/live.redb");
            let downloads = root.path().join("user-downloads");
            let host_state = root.path().join("app_state/terminal/conpty");
            for dir in [
                &live_cache,
                &idle_cache,
                &live_temp,
                &stale_temp,
                &userdata,
                &downloads,
                &host_state,
            ] {
                fs::create_dir_all(dir).unwrap();
                fs::write(dir.join("probe"), b"keep").unwrap();
            }
            fs::create_dir_all(kv.parent().unwrap()).unwrap();
            fs::write(&kv, b"kv").unwrap();
            let _active =
                super::super::protect_from_cleanup([live_cache.clone(), live_temp.clone()]);
            let mut report = ClearReport::default();
            clear_private_storage(&roots, &mut report);
            assert!(report.failures.is_empty());
            assert_eq!(report.skipped_active_paths, 2);
            assert_eq!(report.freed_bytes, 8);
            assert!(!idle_cache.exists());
            assert!(!stale_temp.exists());
            for dir in [&live_cache, &live_temp, &userdata, &downloads, &host_state] {
                assert_eq!(fs::read(dir.join("probe")).unwrap(), b"keep");
            }
            assert_eq!(fs::read(kv).unwrap(), b"kv");
        }

        #[test]
        fn startup_claim_and_cleanup_cannot_delete_new_session_writes() {
            use std::sync::{Arc, Barrier, mpsc};
            let root = tempfile::tempdir().unwrap();
            let path = root.path().join("cache");
            fs::create_dir(&path).unwrap();
            fs::write(path.join("old"), b"old").unwrap();
            let barrier = Arc::new(Barrier::new(2));
            let startup_barrier = barrier.clone();
            let startup_path = path.clone();
            let (done_tx, done_rx) = mpsc::channel();
            let startup = std::thread::spawn(move || {
                startup_barrier.wait();
                let _active = super::super::protect_from_cleanup([startup_path.clone()]);
                fs::create_dir_all(&startup_path).unwrap();
                fs::write(startup_path.join("new"), b"new").unwrap();
                done_rx.recv().unwrap();
            });
            barrier.wait();
            let result = super::super::remove_if_unprotected(&path).unwrap();
            done_tx.send(()).unwrap();
            startup.join().unwrap();
            assert_eq!(fs::read(path.join("new")).unwrap(), b"new");
            assert_eq!(path.join("old").exists(), result.is_none());
        }

        #[test]
        fn webview_unsupported_and_failure_are_distinct() {
            let mut report = ClearReport::default();
            record_webview_result(
                &mut report,
                Err(lingxia_webview::WebViewError::Unsupported("cache".into())),
            );
            assert_eq!(report.webview, "unsupported");
            assert!(report.failures.is_empty());
            record_webview_result(
                &mut report,
                Err(lingxia_webview::WebViewError::WebView(
                    "profile unavailable".into(),
                )),
            );
            assert_eq!(report.webview, "failed");
            assert_eq!(report.failures.len(), 1);
            assert!(report.failures[0].contains("profile unavailable"));
            record_webview_result(&mut report, Ok(()));
            assert_eq!(report.webview, "cleared");
        }

        #[test]
        fn private_cleanup_continues_after_one_category_fails() {
            let root = tempfile::tempdir().unwrap();
            let roots = Roots {
                usercache: root.path().join("not-a-directory"),
                temp: root.path().join("temp"),
                icons: root.path().join("icons"),
                installs: root.path().join("installs"),
                downloads: root.path().join("downloads"),
            };
            fs::write(&roots.usercache, b"invalid").unwrap();
            let stale = roots.temp.join("app/stale");
            fs::create_dir_all(&stale).unwrap();
            fs::write(stale.join("probe"), b"temp").unwrap();
            let mut report = ClearReport::default();
            clear_private_storage(&roots, &mut report);
            assert_eq!(report.failures.len(), 1);
            assert!(!stale.exists());
            assert_eq!(report.freed_bytes, 4);
        }

        #[test]
        fn cleanup_directory_errors_are_reported() {
            let root = tempfile::tempdir().unwrap();
            let file = root.path().join("not-a-directory");
            fs::write(&file, b"cache").unwrap();
            let mut report = ClearReport::default();

            empty_dir(&file, &mut report);

            assert_eq!(report.failures.len(), 1);
            assert!(report.failures[0].contains("not-a-directory"));
        }
    }
}
