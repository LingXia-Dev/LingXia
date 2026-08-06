//! Host-supplied launch-cover selection.
//!
//! A host app registers one Rust function (through [`crate::HostAddon`]) that
//! picks which cover this cold start shows. Writing it once in Rust replaces
//! writing the same policy in Swift, Kotlin and ArkTS, and the platforms stay
//! a thin renderer: this module hands them a resolved file path, never a
//! view.
//!
//! Two responsibilities that look like one, and must not be mixed:
//!
//! - **Selection** is synchronous and affects *this* launch. It may only
//!   choose among assets already on disk — never download, never block.
//! - **Acquisition** is handed to [`crate::spawn`] and affects the *next*
//!   launch. That is the only honest place for network work: the OS
//!   placeholder is already on screen by the time it could finish.
//!
//! The background color is deliberately not selectable here. It is baked into
//! the OS launch frame at build time, so a runtime override could only ever
//! disagree with the frame the user already saw.

use std::path::PathBuf;
use std::time::Duration;

/// How long selection may take before the framework stops waiting and uses the
/// configured cover. Selection runs on the cold-start path; a host that stalls
/// here would be delaying the very screen it is trying to decorate.
const SELECTION_BUDGET: Duration = Duration::from_millis(50);

/// Directory holding covers acquired at runtime, under the app data dir.
const CACHE_SUBDIR: &str = "lingxia/splash";

/// A substitute cover for one cold start.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SplashCover {
    /// A cover in the managed cache, addressed by the key it was stored under.
    /// Falls back to the bundled cover when the file is gone (evicted, never
    /// downloaded, cleared).
    Cached(String),
    /// An app-owned absolute path, for hosts that manage their own storage.
    Path(PathBuf),
}

/// A host's answer for one cold start.
///
/// The default answer is the cover configured in `lingxia.yaml`; a hook only
/// exists to substitute a different file for this launch. Either way the
/// cover is the app's first frame, revealed when the OS placeholder exits —
/// it can never appear before the placeholder does.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SplashChoice {
    pub cover: Option<SplashCover>,
    /// Override the configured minimum hold, in milliseconds. Still bounded by
    /// the framework's upper cap.
    pub min_duration_ms: Option<u32>,
}

impl SplashChoice {
    /// Use the cover configured in `lingxia.yaml`. Also what [`Default`]
    /// yields — the explicit name reads better at a hook's return sites.
    pub fn bundled() -> Self {
        Self::default()
    }

    /// Use a cover from the managed cache.
    pub fn cached(key: impl Into<String>) -> Self {
        Self {
            cover: Some(SplashCover::Cached(key.into())),
            min_duration_ms: None,
        }
    }

    /// Use an app-owned file.
    pub fn path(path: impl Into<PathBuf>) -> Self {
        Self {
            cover: Some(SplashCover::Path(path.into())),
            min_duration_ms: None,
        }
    }

    pub fn min_duration_ms(mut self, ms: u32) -> Self {
        self.min_duration_ms = Some(ms);
        self
    }
}

/// The cold start being decided: what the hook can see and touch.
pub struct Launch {
    dark: bool,
    data_dir: PathBuf,
}

impl Launch {
    /// The appearance the launch frame is already showing. Prefer this over
    /// reading a system setting: it is the value the user is looking at.
    pub fn is_dark(&self) -> bool {
        self.dark
    }

    /// Where runtime-acquired covers live. Acquisition work (spawned with
    /// [`crate::spawn`]) writes here; the file becomes selectable on the
    /// next launch.
    pub fn cache_dir(&self) -> PathBuf {
        self.data_dir.join(CACHE_SUBDIR)
    }

    /// Resolve a cache key to a file, if it is actually there.
    pub fn cached(&self, key: &str) -> Option<PathBuf> {
        let path = self.cache_dir().join(format!("{key}.png"));
        path.is_file().then_some(path)
    }
}

/// Ask the registered host addons which cover to show.
///
/// Runs selection off the calling thread so a slow or wedged host cannot hold
/// up the launch: past [`SELECTION_BUDGET`] the configured cover wins and the
/// host's answer is ignored for this launch.
fn select(dark: bool, data_dir: PathBuf) -> SplashChoice {
    let (tx, rx) = std::sync::mpsc::sync_channel(1);
    std::thread::spawn(move || {
        let launch = Launch { dark, data_dir };
        let choice = crate::host_addon::run_select_splash(&launch);
        // A full channel means the budget already expired; nothing to do.
        let _ = tx.try_send(choice);
    });

    match rx.recv_timeout(SELECTION_BUDGET) {
        Ok(choice) => choice,
        Err(_) => {
            log::warn!("splash selection exceeded its budget; using the configured cover");
            SplashChoice::default()
        }
    }
}

/// Resolve this launch's cover before the platform builds its first frame.
///
/// Called by the platform on the attach path, so the first frame the OS
/// placeholder reveals is already the host's choice — the bundled cover
/// never flashes first. Blocks for at most the selection budget; `None` —
/// also every fallback — means the bundled cover, which the platform holds
/// as a build-time resource. The hook's `min_duration_ms` lands in the app
/// context, where the core's dismissal hold reads it.
pub fn select_cover(data_dir: std::path::PathBuf, dark: bool) -> Option<std::path::PathBuf> {
    if !crate::host_addon::any_registered() {
        return None;
    }
    let choice = select(dark, data_dir.clone());

    if let Some(ms) = choice.min_duration_ms {
        lingxia_app_context::set_splash_min_duration_override(ms);
    }

    let cover = match &choice.cover {
        None => None,
        Some(SplashCover::Cached(key)) => {
            let path = data_dir.join(CACHE_SUBDIR).join(format!("{key}.png"));
            path.is_file().then_some(path)
        }
        Some(SplashCover::Path(path)) => path.is_file().then(|| path.clone()),
    };
    if let Some(path) = &cover {
        log::info!("splash cover selected: {}", path.display());
    }
    cover
}
