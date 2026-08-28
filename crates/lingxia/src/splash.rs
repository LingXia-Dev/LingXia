//! The launch face, and the campaign that may follow it.
//!
//! Two stages that look like one screen and must never be confused:
//!
//! - **The launch face** is fixed at build time — the `splash:` art in
//!   `lingxia.yaml`, shipped inside the app. It is what the OS launch frame
//!   hands over to, so it is the one image that can be pixel-identical to a
//!   frame composed before any code ran. Nothing chooses it at runtime;
//!   anything that did could only ever disagree with what the user already
//!   saw, which is what a "white flash" or a mid-launch swap actually is.
//!
//! - **The campaign** is the host's own screen, shown *after* the launch face
//!   has done its job, with a countdown the user can skip. Because it arrives
//!   as content rather than as the launch, it may be downloaded, differently
//!   proportioned, and different every day.
//!
//! Acquisition — the downloading — belongs to [`crate::spawn`] and lands in
//! time for a *later* launch. Selection only ever picks among files already on
//! disk, and it is never on the critical path: a campaign that is not ready
//! when the launch face lifts is skipped, because a launch that waits on a
//! campaign is worse than a launch with no campaign.

use std::path::PathBuf;

/// Directory holding campaign art acquired at runtime — deliberately under
/// the app *data* dir, not the OS cache dir: a launch must find the art on
/// disk without any network, and OS caches can be purged at any time. Nothing
/// evicts it automatically; files are key-addressed and overwritten in place,
/// so the footprint is the set of keys the host uses.
const CACHE_SUBDIR: &str = "lingxia/splash";

/// How long a campaign holds the screen when the host names no duration.
const DEFAULT_CAMPAIGN_MS: u32 = 3000;

/// The longest a campaign may hold the screen. The host owns the duration;
/// the framework owns the ceiling, so a bad number cannot strand the user in
/// front of an ad.
const MAX_CAMPAIGN_MS: u32 = 8000;

/// Cache keys may come from a campaign service. Keep them as identifiers,
/// never paths, so the managed store cannot escape its own directory.
fn cache_file_name(key: &str) -> Option<String> {
    let valid = !key.is_empty()
        && key.len() <= 128
        && key
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'));
    valid.then(|| format!("{key}.png"))
}

/// Where a campaign's art comes from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CampaignArt {
    /// Art in the managed store, addressed by the key it was stored under.
    /// A key with no file is the same as no campaign — never downloaded yet,
    /// or removed by the host.
    Cached(String),
    /// An app-owned absolute path, for hosts that manage their own storage.
    Path(PathBuf),
}

/// A host's answer for one cold start: the campaign to show once the launch
/// face has finished, if any.
///
/// [`Default`] — and [`CampaignChoice::none`] — mean "no campaign", which is
/// the right answer far more often than not: the launch face alone is the
/// fastest path to the app.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CampaignChoice {
    pub art: Option<CampaignArt>,
    /// How long the campaign holds, in milliseconds, before it lifts on its
    /// own. Capped by the framework; the user can always skip sooner.
    pub duration_ms: Option<u32>,
}

impl CampaignChoice {
    /// Show no campaign: the launch face lifts straight into the app.
    pub fn none() -> Self {
        Self::default()
    }

    /// Show art from the managed store.
    pub fn cached(key: impl Into<String>) -> Self {
        Self {
            art: Some(CampaignArt::Cached(key.into())),
            duration_ms: None,
        }
    }

    /// Show an app-owned file.
    pub fn path(path: impl Into<PathBuf>) -> Self {
        Self {
            art: Some(CampaignArt::Path(path.into())),
            duration_ms: None,
        }
    }

    pub fn duration_ms(mut self, ms: u32) -> Self {
        self.duration_ms = Some(ms);
        self
    }
}

/// The cold start being decided: what the hook can see and touch.
pub struct Launch {
    dark: bool,
    data_dir: PathBuf,
}

impl Launch {
    /// The appearance the launch face resolved to. This is the app's own
    /// appearance, which is not always the system's — a host whose user
    /// pinned dark is dark on a light phone.
    pub fn is_dark(&self) -> bool {
        self.dark
    }

    /// Where runtime-acquired campaign art lives. Acquisition work (spawned
    /// with [`crate::spawn`]) writes here; the file becomes selectable on a
    /// later launch.
    ///
    /// Backed by app data, not the OS cache: nothing — neither the OS nor
    /// the framework — evicts it. Overwrite a key to replace its art; remove
    /// files to reclaim space. Both belong to the host.
    pub fn cache_dir(&self) -> PathBuf {
        self.data_dir.join(CACHE_SUBDIR)
    }

    /// Resolve a cache key to a file, if it is actually there.
    pub fn cached(&self, key: &str) -> Option<PathBuf> {
        let path = self.cache_dir().join(cache_file_name(key)?);
        path.is_file().then_some(path)
    }
}

/// Write campaign art into the store, atomically: temp file then rename, so
/// [`Launch::cached`] only ever sees absent or complete — a launch can never
/// select a half-downloaded image. Call from acquisition work.
pub fn store(key: &str, bytes: &[u8]) -> std::io::Result<PathBuf> {
    let file_name = cache_file_name(key).ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "campaign key must be 1-128 ASCII letters, digits, '-' or '_'",
        )
    })?;
    let dir = crate::app::data_dir()
        .map_err(|e| std::io::Error::other(e.to_string()))?
        .join(CACHE_SUBDIR);
    std::fs::create_dir_all(&dir)?;
    // One temp name per key: a crashed write is overwritten by the retry.
    let tmp = dir.join(format!(".{key}.png.tmp"));
    let dest = dir.join(file_name);
    std::fs::write(&tmp, bytes)?;
    std::fs::rename(&tmp, &dest)?;
    Ok(dest)
}

/// Keep only these keys in the campaign store; every other file is deleted in
/// the background. One call bounds a store fed by rotating keys — list what
/// future launches may still select, including what acquisition is about to
/// write. Callable from any phase; only `<key>.png` files are touched.
pub fn retain<I, S>(keys: I)
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let keep: Vec<String> = keys
        .into_iter()
        .filter_map(|key| cache_file_name(key.as_ref()))
        .collect();
    crate::spawn(async move {
        let Ok(data_dir) = crate::app::data_dir() else {
            return;
        };
        let Ok(entries) = std::fs::read_dir(data_dir.join(CACHE_SUBDIR)) else {
            return;
        };
        for entry in entries.flatten() {
            let name = entry.file_name();
            let Some(name) = name.to_str() else { continue };
            if name.ends_with(".png") && !keep.iter().any(|kept| kept == name) {
                let _ = std::fs::remove_file(entry.path());
            }
        }
    });
}

/// Note that the launch face is on screen, and which appearance it is showing.
///
/// Called by the platform on its attach path, before the runtime exists. The
/// appearance is the platform's to decide, not the runtime's: the face has to
/// be identical to a frame the OS composed from the same resource bucket
/// before this process started, so whatever bucket the OS picked is the
/// answer. Where a platform persists a per-app night mode (Android), the
/// runtime pushes the user's choice into it and the OS then picks that bucket
/// for both frames on the next launch.
pub fn mark_launch_face(dark: bool) {
    // Every platform calls this on its attach path, so it is the closest
    // process-wide signal the runtime has for "the launch face is up", and the
    // hold must be measured for every launch. It is not a per-frame truth:
    // Android's bootstrap activity draws the face from resources before the
    // native library even loads, which is why its overlay re-measures the hold
    // from the face's own first draw.
    lingxia_app_context::mark_splash_visible();
    LAUNCH_DARK.store(i8::from(dark), std::sync::atomic::Ordering::Relaxed);
}

/// The appearance the launch face resolved to, or -1 where no launch face was
/// ever drawn — desktop, which has no splash at all.
static LAUNCH_DARK: std::sync::atomic::AtomicI8 = std::sync::atomic::AtomicI8::new(-1);

/// Ask the host for this launch's campaign, and hold the answer until the
/// launch face is ready to lift.
///
/// Deliberately *not* on the cold-start path: it runs once the runtime is up,
/// so a host that reads a file, checks a clock or inspects its own state
/// costs the launch nothing. If it has not answered by the time the launch
/// face lifts, this launch simply has no campaign.
pub(crate) fn resolve_campaign() {
    if !crate::host_addon::any_registered() {
        return;
    }
    // No launch face was drawn, so there is nothing for a campaign to follow.
    let dark = match LAUNCH_DARK.load(std::sync::atomic::Ordering::Relaxed) {
        0 => false,
        1 => true,
        _ => return,
    };
    let Ok(data_dir) = crate::app::data_dir() else {
        return;
    };
    // Off the boot thread: the launch face is still up, and a host that reads
    // a file here must not add its own latency to the page render that lifts
    // it. Missing the handoff costs this launch its campaign, never a delay.
    std::thread::spawn(move || resolve_campaign_now(dark, data_dir));
}

fn resolve_campaign_now(dark: bool, data_dir: PathBuf) {
    let launch = Launch {
        dark,
        data_dir: data_dir.clone(),
    };
    let choice = crate::host_addon::run_select_campaign(&launch);
    let art = match &choice.art {
        None => return,
        Some(CampaignArt::Cached(key)) => {
            let Some(file_name) = cache_file_name(key) else {
                log::warn!("invalid campaign cache key; skipping this launch");
                return;
            };
            let path = data_dir.join(CACHE_SUBDIR).join(file_name);
            path.is_file().then_some(path)
        }
        Some(CampaignArt::Path(path)) => path.is_file().then(|| path.clone()),
    };
    let Some(path) = art else {
        log::info!("campaign art is not on disk yet; skipping it this launch");
        return;
    };
    let duration = choice
        .duration_ms
        .unwrap_or(DEFAULT_CAMPAIGN_MS)
        .min(MAX_CAMPAIGN_MS);
    if lingxia_app_context::set_pending_campaign(path.to_string_lossy().into_owned(), duration) {
        log::info!("campaign ready: {} for {duration}ms", path.display());
    } else {
        log::info!("campaign resolved after home was ready; skipping this launch");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The framework owns the ceiling even though the host owns the duration,
    /// so a bad number cannot strand the user in front of an ad.
    #[test]
    fn campaign_duration_is_capped() {
        let choice = CampaignChoice::cached("promo").duration_ms(60_000);
        assert_eq!(
            choice
                .duration_ms
                .unwrap_or(DEFAULT_CAMPAIGN_MS)
                .min(MAX_CAMPAIGN_MS),
            MAX_CAMPAIGN_MS
        );
    }

    /// No campaign is the common answer, and it must be the cheap one to
    /// write at a hook's return site.
    #[test]
    fn no_campaign_is_the_default() {
        assert_eq!(CampaignChoice::none(), CampaignChoice::default());
        assert!(CampaignChoice::none().art.is_none());
    }

    #[test]
    fn campaign_cache_keys_are_identifiers_not_paths() {
        assert_eq!(
            cache_file_name("summer_2026-en"),
            Some("summer_2026-en.png".into())
        );
        assert_eq!(cache_file_name("../outside"), None);
        assert_eq!(cache_file_name("nested/promo"), None);
        assert_eq!(cache_file_name(""), None);
        assert_eq!(cache_file_name(&"x".repeat(129)), None);
    }
}
