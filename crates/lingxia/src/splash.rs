//! Host-supplied launch-cover selection.
//!
//! A host app registers one Rust function (through [`crate::HostAddon`]) that
//! picks which cover this cold start shows. Writing it once in Rust replaces
//! writing the same policy in Swift, Kotlin and ArkTS, and the platforms stay
//! a thin renderer: this module hands them a resolved file path, never a view.
//!
//! Two responsibilities that look like one, and must not be mixed:
//!
//! - **Selection** is synchronous and affects *this* launch. It may only
//!   choose among assets already on disk — never download, never block.
//! - **Acquisition** is spawned onto the runtime and affects the *next*
//!   launch. That is the only honest place for network work: the cover is
//!   already on screen by the time it could finish.
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

/// Which cover to show for this launch.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SplashImage {
    /// The cover generated from `lingxia.yaml` — always present, always valid.
    Bundled,
    /// A cover in the managed cache, addressed by the key it was stored under.
    /// Falls back to [`SplashImage::Bundled`] when the file is gone (evicted,
    /// never downloaded, cleared).
    Cached(String),
    /// An app-owned absolute path, for hosts that manage their own storage.
    Path(PathBuf),
}

/// A host's answer for one cold start.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SplashChoice {
    pub image: SplashImage,
    /// Override the configured minimum hold, in milliseconds. Still bounded by
    /// the framework's upper cap.
    pub min_duration_ms: Option<u32>,
}

impl Default for SplashChoice {
    fn default() -> Self {
        Self {
            image: SplashImage::Bundled,
            min_duration_ms: None,
        }
    }
}

impl SplashChoice {
    /// Use the cover configured in `lingxia.yaml`.
    pub fn bundled() -> Self {
        Self::default()
    }

    /// Use a cover from the managed cache.
    pub fn cached(key: impl Into<String>) -> Self {
        Self {
            image: SplashImage::Cached(key.into()),
            min_duration_ms: None,
        }
    }

    /// Use an app-owned file.
    pub fn path(path: impl Into<PathBuf>) -> Self {
        Self {
            image: SplashImage::Path(path.into()),
            min_duration_ms: None,
        }
    }

    pub fn min_duration_ms(mut self, ms: u32) -> Self {
        self.min_duration_ms = Some(ms);
        self
    }
}

/// What the host gets to decide from.
pub struct SplashContext {
    dark: bool,
    data_dir: PathBuf,
}

impl SplashContext {
    /// The appearance the launch frame is already showing. Prefer this over
    /// reading a system setting: it is the value the user is looking at.
    pub fn is_dark(&self) -> bool {
        self.dark
    }

    /// Where runtime-acquired covers live. Write here from spawned work; the
    /// file becomes selectable on the next launch.
    pub fn cache_dir(&self) -> PathBuf {
        self.data_dir.join(CACHE_SUBDIR)
    }

    /// Resolve a cache key to a file, if it is actually there.
    pub fn cached(&self, key: &str) -> Option<PathBuf> {
        let path = self.cache_dir().join(format!("{key}.png"));
        path.is_file().then_some(path)
    }

    /// Run acquisition work for future launches. Never blocks selection.
    pub fn spawn<F>(&self, future: F)
    where
        F: std::future::Future<Output = ()> + Send + 'static,
    {
        std::mem::drop(crate::task::spawn(future));
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
        let ctx = SplashContext { dark, data_dir };
        let choice = crate::host_addon::run_select_splash(&ctx);
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

/// Resolve the cover for this launch into something a platform can render.
///
/// Returns JSON: `{"image": <absolute path or null>, "minDuration": <ms>}`.
/// A null image means "use the cover you already have as a platform resource",
/// which is what every fallback path collapses to.
pub fn resolve_for_platform(dark: bool) -> String {
    let data_dir = crate::app::data_dir().unwrap_or_default();
    let choice = select(dark, data_dir.clone());

    let image = match &choice.image {
        SplashImage::Bundled => None,
        SplashImage::Cached(key) => {
            let path = data_dir.join(CACHE_SUBDIR).join(format!("{key}.png"));
            path.is_file().then(|| path.to_string_lossy().into_owned())
        }
        SplashImage::Path(path) => path.is_file().then(|| path.to_string_lossy().into_owned()),
    };

    let min_duration = choice
        .min_duration_ms
        .map(u64::from)
        .unwrap_or_else(|| lingxia_app_context::splash_min_duration().as_millis() as u64);

    serde_json::json!({ "image": image, "minDuration": min_duration }).to_string()
}
