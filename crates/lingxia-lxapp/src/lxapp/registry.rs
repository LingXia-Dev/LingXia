//! Cache and resolution for lxapp registry records — the app's name, icon,
//! description, and status as the server owns them.
//!
//! Separate from the update path on purpose. A name or icon changes without any
//! package changing, and the sidebar has to draw apps that were never
//! installed, neither of which the update check can express: it is scoped to an
//! OTA-managed target and answers `None` for "already up to date".
//!
//! Icons are content-addressed, so an unchanged icon costs nothing after the
//! first fetch and the same artwork resolved for two locales is one file.
//! Names are not: a string that short is cheaper to re-fetch than to reconcile.

use super::metadata::{self, RegistryRecord};
use super::runtime_registry;
use crate::archive;
use crate::error::LxAppError;
use crate::provider::{LxAppRegistryInfo, LxAppStatus, lxapp_registry_provider};
use lingxia_platform::traits::app_runtime::AppRuntime;
use rong_rt::download as service_executor;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

/// How long a cached name/icon is served without a background refresh. Artwork
/// is not time-critical; the cost of being a day late is nil.
const LISTING_TTL: Duration = Duration::from_secs(24 * 60 * 60);

/// How long a cached status counts as a current answer. Far shorter than the
/// listing TTL: a day-old "published" for a suspended app is an incident.
const STATUS_TTL: Duration = Duration::from_secs(15 * 60);

/// Ceiling on the pre-open status check. Opening an app must not wait on a slow
/// network — past this we fall back to the standing local permission.
const OPEN_GATE_TIMEOUT: Duration = Duration::from_secs(3);

/// Whole-request ceiling for one icon body. Without it a stalled response hangs
/// the fetch forever, and a detached refresh holds its dedupe slot for the life
/// of the process.
const ICON_REQUEST_TIMEOUT: Duration = Duration::from_secs(20);

/// Floor between refresh attempts for the same app. The sidebar asks on every
/// layout pass, and a fast-failing provider (offline, connection refused)
/// releases the in-flight guard immediately — without this, a window drag turns
/// into a request per frame.
const REFRESH_RETRY_INTERVAL: Duration = Duration::from_secs(60);

/// Age past which a leftover download staging file is swept. Cancellation drops
/// the download future without running its cleanup, so some always leak.
const STAGING_MAX_AGE: Duration = Duration::from_secs(60 * 60);

const ICONS_DIR: &str = "icons";
const STAGING_SUFFIX: &str = ".part";

fn now_secs() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| elapsed.as_secs() as i64)
        .unwrap_or(0)
}

/// Path to the icon cache without touching the filesystem. Resolution runs on
/// the layout path, where a `create_dir_all` per sidebar row would be pure
/// overhead; only the download path needs the directory to exist.
fn icons_dir() -> Option<PathBuf> {
    let runtime = runtime_registry::get_platform()?;
    Some(
        runtime
            .app_cache_dir()
            .join(super::LINGXIA_DIR)
            .join(super::LXAPPS_DIR)
            .join(ICONS_DIR),
    )
}

fn ensure_icons_dir() -> Option<PathBuf> {
    let dir = icons_dir()?;
    if let Err(err) = fs::create_dir_all(&dir) {
        crate::warn!("Failed to create lxapp icon cache dir: {}", err);
        return None;
    }
    Some(dir)
}

fn current_locale() -> String {
    runtime_registry::get_display_language()
}

/// Keep the extension the server used where it is a plausible image, so the
/// cached file stays loadable by path alone.
fn icon_extension(url: &str) -> &'static str {
    let path = url.split(&['?', '#'][..]).next().unwrap_or(url);
    let ext = path
        .rsplit('/')
        .next()
        .and_then(|segment| segment.rsplit_once('.'))
        .map(|(_, ext)| ext.to_ascii_lowercase());
    match ext.as_deref() {
        Some("svg") => "svg",
        Some("jpg") | Some("jpeg") => "jpg",
        Some("webp") => "webp",
        Some("ico") => "ico",
        _ => "png",
    }
}

fn active_refreshes() -> &'static Mutex<HashSet<String>> {
    static ACTIVE: OnceLock<Mutex<HashSet<String>>> = OnceLock::new();
    ACTIVE.get_or_init(|| Mutex::new(HashSet::new()))
}

/// Marks a background refresh in flight and releases it on drop, so a sidebar
/// that repaints ten times does not issue ten identical fetches.
///
/// Only the background path takes it. The pre-open gate deliberately does not:
/// losing this race there would resolve to "no answer", and "no answer" means
/// the open is allowed — two simultaneous opens of a suspended app would let
/// one through.
struct RefreshGuard(String);

impl RefreshGuard {
    fn acquire(key: String) -> Option<Self> {
        let mut active = active_refreshes()
            .lock()
            .unwrap_or_else(|err| err.into_inner());
        active.insert(key.clone()).then(|| Self(key))
    }
}

impl Drop for RefreshGuard {
    fn drop(&mut self) {
        if let Ok(mut active) = active_refreshes().lock() {
            active.remove(&self.0);
        }
    }
}

/// Last refresh attempt per `(appid, locale)`, successful or not. Separate from
/// the record's `fetched_at`, which only advances on an answer.
fn last_attempts() -> &'static Mutex<HashMap<String, Instant>> {
    static ATTEMPTS: OnceLock<Mutex<HashMap<String, Instant>>> = OnceLock::new();
    ATTEMPTS.get_or_init(|| Mutex::new(HashMap::new()))
}

fn attempted_recently(appid: &str, locale: &str) -> bool {
    let attempts = last_attempts()
        .lock()
        .unwrap_or_else(|err| err.into_inner());
    attempts
        .get(&attempt_key(appid, locale))
        .is_some_and(|at| at.elapsed() < REFRESH_RETRY_INTERVAL)
}

fn mark_attempted(appids: &[String], locale: &str) {
    let mut attempts = last_attempts()
        .lock()
        .unwrap_or_else(|err| err.into_inner());
    let now = Instant::now();
    for appid in appids {
        attempts.insert(attempt_key(appid, locale), now);
    }
}

fn attempt_key(appid: &str, locale: &str) -> String {
    format!("{}::{}", appid, locale)
}

type RegistryChangeListener = Box<dyn Fn(&[String]) + Send + Sync>;

fn change_listener() -> &'static Mutex<Option<RegistryChangeListener>> {
    static LISTENER: OnceLock<Mutex<Option<RegistryChangeListener>>> = OnceLock::new();
    LISTENER.get_or_init(|| Mutex::new(None))
}

/// Install the hook a host uses to repaint after a refresh lands.
///
/// Refreshes are asynchronous and nothing else observes them: a sidebar that
/// asked for a refresh while painting has already finished painting by the time
/// the answer arrives, so without this the new name or icon waits for whatever
/// unrelated event next triggers a relayout.
pub fn set_registry_change_listener(listener: RegistryChangeListener) {
    *change_listener().lock().unwrap_or_else(|e| e.into_inner()) = Some(listener);
}

fn notify_changed(appids: &[String]) {
    if appids.is_empty() {
        return;
    }
    let guard = change_listener().lock().unwrap_or_else(|e| e.into_inner());
    if let Some(listener) = guard.as_ref() {
        listener(appids);
    }
}

fn record(appid: &str, locale: &str) -> Option<RegistryRecord> {
    metadata::registry_get(appid, locale).ok().flatten()
}

/// A record written with a clock that was ahead reads as "aged negative
/// seconds". Treat it as expired rather than as eternally fresh — otherwise one
/// bad clock pins a stale `Published` past every later suspension.
fn is_expired(record: &RegistryRecord, ttl: Duration) -> bool {
    let now = now_secs();
    now < record.fetched_at || now - record.fetched_at > ttl.as_secs() as i64
}

/// A record naming artwork that is no longer on disk. Records live in the data
/// directory and icons in the cache directory, so the OS can drop the artwork
/// and leave the record behind — and that record still looks fresh, which would
/// otherwise mean up to a full listing TTL with no icon and no attempt to get
/// one back.
fn cached_icon_is_gone(record: &RegistryRecord, icons_dir: &Path) -> bool {
    record
        .icon_file
        .as_ref()
        .is_some_and(|file| !icons_dir.join(file).exists())
}

/// The registry's name for this app in the current locale, else the freshest
/// name it has in any locale.
///
/// The cross-locale fallback matters offline: switching display language with
/// no network would otherwise blank every row, and a name in the previous
/// language is strictly better than no name. Status gets no such fallback —
/// see [`ensure_open_allowed`].
pub(crate) fn name(appid: &str) -> Option<String> {
    lxapp_registry_provider()?;
    let locale = current_locale();
    record(appid, &locale)
        .and_then(|record| record.name)
        .or_else(|| any_locale(appid).and_then(|record| record.name))
        .filter(|name| !name.trim().is_empty())
}

/// Absolute path to the cached icon, or `None` when nothing is cached yet.
/// Callers fall back to the packaged icon from here.
pub(crate) fn icon_path(appid: &str) -> Option<String> {
    lxapp_registry_provider()?;
    let locale = current_locale();
    let file = record(appid, &locale)
        .and_then(|record| record.icon_file)
        .or_else(|| any_locale(appid).and_then(|record| record.icon_file))?;
    let path = icons_dir()?.join(file);
    path.exists().then(|| path.to_string_lossy().into_owned())
}

fn any_locale(appid: &str) -> Option<RegistryRecord> {
    metadata::registry_any_locale(appid).ok().flatten()
}

pub(crate) fn status(appid: &str) -> LxAppStatus {
    if lxapp_registry_provider().is_none() {
        return LxAppStatus::Unknown;
    }
    let locale = current_locale();
    record(appid, &locale)
        .map(|record| LxAppStatus::from_str_lossy(&record.status))
        .unwrap_or_default()
}

/// Refresh in the background when the cache is missing or past its TTL.
/// The sidebar calls this as it populates; nothing waits on the result.
pub(crate) fn ensure_fresh(appids: &[String]) {
    if lxapp_registry_provider().is_none() {
        return;
    }
    let locale = current_locale();
    let icons_dir = icons_dir();
    let stale: Vec<String> = appids
        .iter()
        .filter(|appid| {
            let cached = record(appid, &locale);
            let needs_fetch = match (&cached, &icons_dir) {
                (None, _) => true,
                (Some(cached), Some(icons_dir)) => {
                    is_expired(cached, LISTING_TTL) || cached_icon_is_gone(cached, icons_dir)
                }
                (Some(cached), None) => is_expired(cached, LISTING_TTL),
            };
            needs_fetch && !attempted_recently(appid, &locale)
        })
        .cloned()
        .collect();
    if stale.is_empty() {
        return;
    }
    mark_attempted(&stale, &locale);

    let mut key = stale.clone();
    key.sort();
    let key = format!("{}::{}", key.join(","), locale);
    std::mem::drop(crate::executor::spawn(Box::pin(async move {
        let Some(_guard) = RefreshGuard::acquire(key) else {
            return;
        };
        match fetch_records(&stale, &locale).await {
            Ok(infos) => fetch_icons(&infos, &locale).await,
            Err(err) => {
                crate::warn!("lxapp registry refresh failed: {}", err);
            }
        }
    })));
}

/// Gate on a *fresh negative* answer only.
///
/// A stale `Published` is not evidence that an app is still permitted, so we
/// re-check when the cached status has aged out. But a check that cannot reach
/// the registry is not evidence of anything either, and an installed app must
/// keep opening offline — so only a status we just confirmed can block.
pub(crate) async fn ensure_open_allowed(appid: &str) -> Result<(), LxAppError> {
    if lxapp_registry_provider().is_none() {
        return Ok(());
    }
    let locale = current_locale();
    let fresh = record(appid, &locale)
        .filter(|record| !is_expired(record, STATUS_TTL))
        .map(|record| LxAppStatus::from_str_lossy(&record.status));

    let status = match fresh {
        Some(status) => status,
        None => {
            let appids = [appid.to_string()];
            match tokio::time::timeout(OPEN_GATE_TIMEOUT, fetch_records(&appids, &locale)).await {
                Ok(Ok(infos)) => {
                    // Artwork is fetched outside the deadline: it is not what
                    // the gate is waiting for, and awaiting it here would let a
                    // slow image expire a check that already had its answer.
                    let detached = infos.clone();
                    let detached_locale = locale.clone();
                    std::mem::drop(crate::executor::spawn(Box::pin(async move {
                        fetch_icons(&detached, &detached_locale).await;
                    })));
                    infos
                        .into_iter()
                        .find(|info| info.appid == appid)
                        .map(|info| info.status)
                        .unwrap_or_default()
                }
                // Unreachable registry keeps the standing local permission.
                Ok(Err(err)) => {
                    crate::warn!("Registry status check failed for {}: {}", appid, err)
                        .with_appid(appid);
                    return Ok(());
                }
                Err(_) => {
                    crate::warn!("Registry status check timed out for {}", appid).with_appid(appid);
                    return Ok(());
                }
            }
        }
    };

    if status.blocks_open() {
        return Err(LxAppError::Runtime(format!(
            "lxapp {} is {} and cannot be opened",
            appid, status
        )));
    }
    Ok(())
}

/// Fetch the registry's answer and store it. Artwork is *not* fetched here.
///
/// The status is the gating fact, and [`ensure_open_allowed`] bounds this call
/// with a deadline it fails open on. If an icon body were awaited inside that
/// deadline, a slow image would expire a check that had already been told the
/// app is suspended — and the drop would cancel the download too, so the next
/// attempt would be no faster.
pub(crate) async fn fetch_records(
    appids: &[String],
    locale: &str,
) -> Result<Vec<LxAppRegistryInfo>, LxAppError> {
    let Some(provider) = lxapp_registry_provider() else {
        return Ok(Vec::new());
    };
    if appids.is_empty() {
        return Ok(Vec::new());
    }

    let infos = provider
        .fetch_registry_info(appids, locale)
        .await
        .map_err(|err| crate::provider::provider_error_to_lxapp_error(&err))?;
    mark_attempted(appids, locale);

    for appid in appids {
        let previous = record(appid, locale);
        let info = infos.iter().find(|info| info.appid == *appid);
        // An app the registry omitted is cached as `Unknown` rather than left
        // absent. Without this every open of a dev project, an unpublished app,
        // or a bundled builtin pays a fresh round trip — and a record that once
        // said `suspended` would keep saying so after the registry stopped
        // listing the app.
        let record = RegistryRecord {
            appid: appid.clone(),
            locale: locale.to_string(),
            name: info.and_then(|info| info.name.clone()),
            description: info.and_then(|info| info.description.clone()),
            icon_url: info.and_then(|info| info.icon_url.clone()),
            // Artwork is replaced by `fetch_icons`. Carrying the old file over
            // meanwhile keeps a row showing a slightly stale icon rather than
            // blanking it for the duration of a download — but an app that no
            // longer advertises an icon drops it, so an icon can be withdrawn
            // and not merely replaced.
            icon_file: match info {
                Some(info) if info.icon_url.is_none() => None,
                _ => previous.and_then(|previous| previous.icon_file),
            },
            status: info
                .map(|info| info.status)
                .unwrap_or_default()
                .as_str()
                .to_string(),
            fetched_at: now_secs(),
        };
        if let Err(err) = metadata::registry_upsert(&record) {
            crate::warn!("Failed to cache registry record for {}: {}", appid, err);
        }
    }
    notify_changed(appids);
    Ok(infos)
}

/// Bring cached artwork in line with records already stored by [`fetch_records`].
async fn fetch_icons(infos: &[LxAppRegistryInfo], locale: &str) {
    sweep_staging_files();
    let mut changed = Vec::new();
    for info in infos {
        let cached = record(&info.appid, locale);
        let Some(icon_file) = resolve_icon_file(info, cached.as_ref()).await else {
            continue;
        };
        let Some(mut record) = cached else {
            continue;
        };
        if record.icon_file.as_deref() == Some(icon_file.as_str()) {
            continue;
        }
        record.icon_file = Some(icon_file);
        if let Err(err) = metadata::registry_upsert(&record) {
            crate::warn!("Failed to cache registry icon for {}: {}", info.appid, err);
            continue;
        }
        changed.push(info.appid.clone());
    }
    if !changed.is_empty() {
        notify_changed(&changed);
    }
}

/// Returns the cached file name for this info's icon, downloading only when the
/// URL it came from has changed.
///
/// The URL is the cache key, so a registry that edits artwork behind a stable
/// URL is never picked up — the contract requires the URL to change with the
/// image. The file is still named by the bytes' own hash, so the same artwork
/// reached through two URLs, or by two lxapps, is one file on disk.
async fn resolve_icon_file(
    info: &LxAppRegistryInfo,
    cached: Option<&RegistryRecord>,
) -> Option<String> {
    let url = info.icon_url.as_deref().filter(|url| !url.is_empty())?;
    let dir = ensure_icons_dir()?;
    let extension = icon_extension(url);

    if let Some(cached) = cached
        && cached.icon_url.as_deref() == Some(url)
        && let Some(file) = cached.icon_file.as_deref()
        && dir.join(file).exists()
    {
        return Some(file.to_string());
    }

    let staging = dir.join(format!(
        "download-{}{}",
        uuid::Uuid::new_v4(),
        STAGING_SUFFIX
    ));
    let options = service_executor::DownloadOptions::new(url.to_string(), staging.clone())
        .with_connect_timeout(Duration::from_secs(10))
        .with_request_timeout(ICON_REQUEST_TIMEOUT);
    let receiver = service_executor::spawn_download(options, None)
        .map_err(|err| crate::warn!("Failed to start icon download: {}", err))
        .ok()?;
    match receiver.await {
        Ok(Ok(())) => {}
        Ok(Err(err)) => {
            crate::warn!("Icon download failed for {}: {}", info.appid, err);
            let _ = fs::remove_file(&staging);
            return None;
        }
        Err(_) => {
            let _ = fs::remove_file(&staging);
            return None;
        }
    }

    let digest = archive::sha256_hex(&staging).ok()?;
    let file = format!("{}.{}", digest, extension);
    let destination = dir.join(&file);
    // Content-addressed, so a destination that already exists holds exactly
    // these bytes: a concurrent refresh for another locale resolving to the
    // same artwork won the race. POSIX rename would overwrite silently; on
    // Windows it errors, and treating that as failure would drop the icon.
    if destination.exists() {
        let _ = fs::remove_file(&staging);
        return Some(file);
    }
    if let Err(err) = fs::rename(&staging, &destination) {
        let _ = fs::remove_file(&staging);
        if !destination.exists() {
            crate::warn!("Failed to store cached icon for {}: {}", info.appid, err);
            return None;
        }
    }
    Some(file)
}

/// Delete staging files old enough that no live download owns them. Downloads
/// cancelled by the pre-open timeout are dropped without running their cleanup,
/// so nothing else would ever remove these.
fn sweep_staging_files() {
    let Some(dir) = icons_dir() else {
        return;
    };
    let Ok(entries) = fs::read_dir(&dir) else {
        return;
    };
    for entry in entries.flatten() {
        if !entry
            .file_name()
            .to_string_lossy()
            .ends_with(STAGING_SUFFIX)
        {
            continue;
        }
        let stale = entry
            .metadata()
            .and_then(|metadata| metadata.modified())
            .map(|modified| {
                modified
                    .elapsed()
                    .is_ok_and(|elapsed| elapsed > STAGING_MAX_AGE)
            })
            .unwrap_or(false);
        if stale {
            let _ = fs::remove_file(entry.path());
        }
    }
}

/// Drop an app's registry cache on uninstall, and the artwork no remaining app
/// references. Icons are content-addressed and therefore shared, so deletion is
/// by survivor set rather than by the removed app's file list.
pub(crate) fn clear(appid: &str) {
    // Records go first and unconditionally: an unavailable icon directory must
    // not leave an uninstalled app's name and status answering lookups.
    let orphan_candidates = match metadata::registry_remove_all(appid) {
        Ok(files) => files,
        Err(err) => {
            crate::warn!("Failed to clear registry cache for {}: {}", appid, err);
            return;
        }
    };
    if orphan_candidates.is_empty() {
        return;
    }
    let Some(dir) = icons_dir() else {
        return;
    };
    sweep_orphan_icons(&orphan_candidates, &dir);
}

fn sweep_orphan_icons(candidates: &[String], icons_dir: &Path) {
    let Ok(still_referenced) = metadata::registry_referenced_icon_files() else {
        return;
    };
    for file in candidates {
        if !still_referenced.contains(file) {
            let _ = fs::remove_file(icons_dir.join(file));
        }
    }
}

/// The name to show for an lxapp anywhere it is listed: the registry's answer,
/// else the name the installed package declares.
///
/// One entry point on purpose — a sidebar row and the window title reading
/// different sources is how the same app ends up with two names on screen.
pub fn display_name(appid: &str) -> Option<String> {
    name(appid)
        .or_else(|| runtime_registry::try_get(appid).map(|app| app.get_lxapp_info().app_name))
        .filter(|name| !name.trim().is_empty())
}

/// The icon to show for an lxapp: the cached registry artwork, or nothing.
///
/// The registry is the only source — a package declares no icon. Before the
/// first fetch lands, and for a local project the registry has never heard of,
/// callers get `None` and draw their own default mark.
pub fn display_icon_path(appid: &str) -> Option<String> {
    icon_path(appid).filter(|path| !path.trim().is_empty())
}

/// Registry state for an lxapp, for callers deciding whether to still offer it.
pub fn display_status(appid: &str) -> LxAppStatus {
    status(appid)
}

/// Ask for a background refresh of these apps' registry records. Call it where
/// a list of apps is built; it returns immediately and never fails the caller.
pub fn refresh_registry(appids: &[String]) {
    ensure_fresh(appids);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn icon_extension_keeps_known_image_types_and_defaults_to_png() {
        assert_eq!(icon_extension("https://cdn.example.com/a/logo.svg"), "svg");
        assert_eq!(icon_extension("https://cdn.example.com/a/logo.JPEG"), "jpg");
        assert_eq!(
            icon_extension("https://cdn.example.com/a/logo.webp?v=2"),
            "webp"
        );
        // No extension, and an unknown one, both land on the safe default.
        assert_eq!(icon_extension("https://cdn.example.com/icon/42"), "png");
        assert_eq!(icon_extension("https://cdn.example.com/a/logo.bin"), "png");
    }

    fn record_for(appid: &str, locale: &str, icon_file: Option<&str>) -> RegistryRecord {
        RegistryRecord {
            appid: appid.to_string(),
            locale: locale.to_string(),
            name: Some(format!("{appid}-{locale}")),
            description: None,
            icon_url: None,
            icon_file: icon_file.map(str::to_string),
            status: LxAppStatus::Published.as_str().to_string(),
            fetched_at: now_secs(),
        }
    }

    #[test]
    fn expiry_uses_the_ttl_it_is_given() {
        let mut record = record_for("demo", "en-US", None);
        assert!(!is_expired(&record, STATUS_TTL));

        // Aged past the status window but still inside the listing window: the
        // icon stays usable while the status must be re-confirmed.
        record.fetched_at = now_secs() - (STATUS_TTL.as_secs() as i64) - 1;
        assert!(is_expired(&record, STATUS_TTL));
        assert!(!is_expired(&record, LISTING_TTL));
    }

    #[test]
    fn a_record_stamped_in_the_future_is_expired_not_eternally_fresh() {
        let mut record = record_for("demo", "en-US", None);
        // A clock that ran ahead before NTP corrected it. Saturating the
        // subtraction would make this record outlive every later suspension.
        record.fetched_at = now_secs() + 60 * 60 * 24 * 365;
        assert!(is_expired(&record, STATUS_TTL));
        assert!(is_expired(&record, LISTING_TTL));
    }

    #[test]
    fn only_suspended_blocks_opening() {
        assert!(LxAppStatus::Suspended.blocks_open());
        // Delisted apps are no longer offered, but an installed copy keeps working.
        assert!(!LxAppStatus::Delisted.blocks_open());
        assert!(!LxAppStatus::Published.blocks_open());
        assert!(!LxAppStatus::Unknown.blocks_open());
    }

    #[test]
    fn unknown_server_states_degrade_instead_of_blocking() {
        let status = LxAppStatus::from_str_lossy("quarantined-pending-review");
        assert_eq!(status, LxAppStatus::Unknown);
        assert!(!status.blocks_open());
    }

    /// Serialized because the metadata database is a process-wide singleton.
    fn with_store<T>(body: impl FnOnce(&Path) -> T) -> T {
        static STORE: OnceLock<Mutex<PathBuf>> = OnceLock::new();
        let dir = STORE.get_or_init(|| {
            let root = std::env::temp_dir().join(format!("lx-registry-{}", uuid::Uuid::new_v4()));
            fs::create_dir_all(&root).expect("create test cache root");
            metadata::init(root.join("metadata.redb")).expect("init metadata database");
            Mutex::new(root)
        });
        let root = dir.lock().unwrap_or_else(|err| err.into_inner());
        body(&root)
    }

    #[test]
    fn a_name_in_another_locale_beats_no_name_at_all() {
        with_store(|_| {
            let appid = "com.example.crosslocale";
            metadata::registry_upsert(&record_for(appid, "zh-CN", None)).unwrap();

            // Nothing cached for this locale, but the app still has a name.
            assert!(metadata::registry_get(appid, "en-US").unwrap().is_none());
            let fallback = metadata::registry_any_locale(appid).unwrap().unwrap();
            assert_eq!(
                fallback.name.as_deref(),
                Some("com.example.crosslocale-zh-CN")
            );
        });
    }

    #[test]
    fn uninstall_keeps_artwork_another_app_still_references() {
        with_store(|icons| {
            let shared = "shared-artwork.png";
            let solo = "solo-artwork.png";
            fs::write(icons.join(shared), b"shared").unwrap();
            fs::write(icons.join(solo), b"solo").unwrap();

            // Two apps resolved to the same artwork; content addressing means
            // one file, so uninstalling either must not orphan the other's icon.
            metadata::registry_upsert(&record_for("com.example.keeper", "en-US", Some(shared)))
                .unwrap();
            metadata::registry_upsert(&record_for("com.example.leaver", "en-US", Some(shared)))
                .unwrap();
            metadata::registry_upsert(&record_for("com.example.leaver", "zh-CN", Some(solo)))
                .unwrap();

            let orphans = metadata::registry_remove_all("com.example.leaver").unwrap();
            sweep_orphan_icons(&orphans, icons);

            assert!(
                metadata::registry_get("com.example.leaver", "en-US")
                    .unwrap()
                    .is_none()
            );
            assert!(
                metadata::registry_get("com.example.leaver", "zh-CN")
                    .unwrap()
                    .is_none()
            );
            assert!(
                icons.join(shared).exists(),
                "still referenced by the keeper"
            );
            assert!(!icons.join(solo).exists(), "last reference went away");

            let orphans = metadata::registry_remove_all("com.example.keeper").unwrap();
            sweep_orphan_icons(&orphans, icons);
            assert!(!icons.join(shared).exists());
        });
    }

    #[test]
    fn artwork_the_os_purged_counts_as_stale_however_fresh_the_record_is() {
        let dir = std::env::temp_dir().join(format!("lx-icon-gone-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&dir).unwrap();
        let present = "kept.png";
        fs::write(dir.join(present), b"art").unwrap();

        let mut record = record_for("demo", "en-US", Some(present));
        assert!(!cached_icon_is_gone(&record, &dir));

        // Records live in the data dir and icons in the cache dir; a cache
        // purge leaves this record fresh but pointing at nothing.
        record.icon_file = Some("purged.png".to_string());
        assert!(!is_expired(&record, LISTING_TTL));
        assert!(cached_icon_is_gone(&record, &dir));

        // A record that never named artwork is not stale for this reason.
        record.icon_file = None;
        assert!(!cached_icon_is_gone(&record, &dir));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_recent_attempt_suppresses_the_next_refresh() {
        let appid = "com.example.backoff";
        let locale = "en-US";
        assert!(!attempted_recently(appid, locale));
        // The sidebar asks on every layout pass; a fast provider failure would
        // otherwise let each pass start another request.
        mark_attempted(&[appid.to_string()], locale);
        assert!(attempted_recently(appid, locale));
        assert!(!attempted_recently(appid, "zh-CN"));
    }
}
