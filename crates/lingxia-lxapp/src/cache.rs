use std::path::Path;

use filetime::FileTime;

pub fn touch_access_time(path: &Path) {
    let now = FileTime::now();
    let _ = filetime::set_file_atime(path, now);
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
    use crate::lxapp::{LINGXIA_DIR, LXAPPS_DIR, TEMP_DIR, USER_CACHE_DIR, runtime_registry};

    /// Shared runtime artwork cache (`{cache}/lingxia/lxapps/icons`). Harmless
    /// if the host never writes there; when a registry provider does, a clear
    /// drops the files and they re-fetch on demand.
    const ICONS_DIR: &str = "icons";
    use lingxia_platform::traits::app_runtime::AppRuntime;
    use std::collections::HashSet;
    use std::fs;
    use std::path::PathBuf;

    struct Roots {
        /// Every lxapp's `lx://usercache`, one directory per app.
        usercache: PathBuf,
        /// Every lxapp's temp, one directory per app and then per session.
        temp: PathBuf,
        /// Registry artwork, content-addressed and shared across apps.
        icons: PathBuf,
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
        })
    }

    /// Temp directories belonging to sessions that are still running. Their
    /// files back in-flight work — an upload body, a preview being written — so
    /// they are the one part of temp a clear must leave alone.
    fn live_session_dirs() -> HashSet<PathBuf> {
        runtime_registry::live_temp_dirs().into_iter().collect()
    }

    /// Total bytes the caches below currently occupy.
    ///
    /// Covers LingXia-managed files only. The WebView's own HTTP cache is not
    /// included: the platform stores expose a site count, not a byte total, so
    /// any figure here would be invented. [`clear`] does drop it.
    pub fn usage_bytes() -> u64 {
        let Some(roots) = roots() else {
            return 0;
        };
        let live = live_session_dirs();
        let mut total = lingxia_service::storage::dir_size(&roots.usercache);
        total += lingxia_service::storage::dir_size(&roots.icons);
        for session in session_dirs(&roots.temp) {
            if !live.contains(&session) {
                total += lingxia_service::storage::dir_size(&session);
            }
        }
        total
    }

    /// Drop every cache [`usage_bytes`] counts, plus the WebView's regenerable
    /// cache where the platform supports it.
    ///
    /// Returns the bytes freed from LingXia-managed storage. A platform with no
    /// WebView cache API contributes nothing rather than failing the call: a
    /// settings button that errors because one of five things is unavailable is
    /// worse than one that clears the other four.
    pub async fn clear() -> u64 {
        let before = usage_bytes();
        if let Some(roots) = roots() {
            let live = live_session_dirs();
            // The per-app directory stays; a running lxapp holds its path and
            // writes into it without re-creating it.
            for app_dir in child_dirs(&roots.usercache) {
                empty_dir(&app_dir);
            }
            for session in session_dirs(&roots.temp) {
                if !live.contains(&session) {
                    let _ = fs::remove_dir_all(&session);
                }
            }
            empty_dir(&roots.icons);
        }
        if let Err(err) = lingxia_webview::data_store::clear_cache(None).await {
            crate::info!("WebView cache not cleared: {}", err);
        }
        before.saturating_sub(usage_bytes())
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

    fn empty_dir(dir: &std::path::Path) {
        let Ok(entries) = fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if entry.file_type().is_ok_and(|kind| kind.is_dir()) {
                let _ = fs::remove_dir_all(&path);
            } else {
                let _ = fs::remove_file(&path);
            }
        }
    }
}
