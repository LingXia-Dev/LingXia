//! The isolated data profile one automation run owns.
//!
//! The host switches the target lxapp onto the profile before the run starts
//! and hands it to the run here. Whatever way the run ends, its finalization
//! returns the app to its own data before the automation slot is released,
//! then deletes the profile or keeps it for a bounded time for export.

use lxapp::data_profile::{self, RunProfile};
use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

/// A retained profile nobody exported is deleted after this long.
pub(crate) const RETAINED_TTL: Duration = Duration::from_secs(300);
/// Upper bound on one teardown; the override is cleared before it starts.
const TEARDOWN_BUDGET: Duration = Duration::from_secs(60);

/// Profile handed to [`super::AutomationRuntime::start`].
#[derive(Debug, Clone)]
pub struct AutomationProfile {
    /// The lxapp that runs on the profile.
    pub appid: String,
    pub profile: RunProfile,
    /// Keep the profile after the run for `session.profile.export`.
    pub retain: bool,
}

/// Returns the app to its own data. Swappable so tests can observe the
/// slot barrier without a live lxapp host.
pub(crate) type TeardownFn = fn(&AutomationProfile) -> Result<(), String>;

#[cfg(not(test))]
pub(crate) const DEFAULT_TEARDOWN: TeardownFn = leave_profile;
// Unit-test binaries do not link a native LingXia host's Swift bridge, which
// reopening an lxapp reaches.
#[cfg(test)]
pub(crate) const DEFAULT_TEARDOWN: TeardownFn = |_| Ok(());

#[cfg_attr(test, allow(dead_code))]
fn leave_profile(profile: &AutomationProfile) -> Result<(), String> {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|err| err.to_string())?;
    runtime.block_on(async {
        tokio::time::timeout(TEARDOWN_BUDGET, data_profile::leave(&profile.appid))
            .await
            .map_err(|_| "timed out returning the app to its own data".to_string())?
            .map_err(|err| err.to_string())
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TeardownState {
    None,
    Pending,
    Done,
}

/// A run's profile and where its teardown stands.
pub(crate) struct RunProfileSlot {
    profile: Option<AutomationProfile>,
    state: Arc<Mutex<TeardownState>>,
}

impl RunProfileSlot {
    pub(crate) fn new(profile: Option<AutomationProfile>) -> Self {
        Self {
            profile,
            state: Arc::new(Mutex::new(TeardownState::None)),
        }
    }

    pub(crate) fn profile(&self) -> Option<&AutomationProfile> {
        self.profile.as_ref()
    }

    /// True from the run's finalization until its app is back on its own
    /// data. The run keeps the automation slot for that whole time.
    pub(crate) fn pending(&self) -> bool {
        *self.state.lock().unwrap() == TeardownState::Pending
    }

    /// Start the teardown once, off the calling thread: finalization runs
    /// on the automation worker and the watchdog, neither of which may block.
    pub(crate) fn begin_teardown(&self, run_id: &str, teardown: TeardownFn) {
        let Some(profile) = self.profile.clone() else {
            return;
        };
        {
            let mut state = self.state.lock().unwrap();
            if *state != TeardownState::None {
                return;
            }
            *state = TeardownState::Pending;
        }
        let state = self.state.clone();
        let run_id = run_id.to_string();
        let spawned = std::thread::Builder::new()
            .name("lingxia-profile-teardown".to_string())
            .spawn(move || {
                finish_teardown(&run_id, &profile, teardown);
                *state.lock().unwrap() = TeardownState::Done;
            });
        if let Err(err) = spawned {
            log::error!("automation run profile teardown could not start: {err}");
            *self.state.lock().unwrap() = TeardownState::Done;
        }
    }
}

fn finish_teardown(run_id: &str, profile: &AutomationProfile, teardown: TeardownFn) {
    if let Err(err) = teardown(profile) {
        log::error!(
            "automation run {run_id}: returning {} to its own data: {err}",
            profile.appid
        );
    }
    if profile.retain {
        retain(run_id, profile.clone());
    } else if let Err(err) = profile.profile.remove() {
        log::warn!("automation run {run_id}: removing its profile: {err}");
    }
}

/// A packed retained profile, ready to hand out in chunks.
#[derive(Clone)]
pub struct ProfileExport {
    pub bytes: Arc<Vec<u8>>,
    pub sha256: String,
}

struct Retained {
    profile: AutomationProfile,
    since: Instant,
    packed: Option<ProfileExport>,
}

fn retained() -> &'static Mutex<HashMap<String, Retained>> {
    static RETAINED: OnceLock<Mutex<HashMap<String, Retained>>> = OnceLock::new();
    RETAINED.get_or_init(|| Mutex::new(HashMap::new()))
}

fn retain(run_id: &str, profile: AutomationProfile) {
    let mut retained = retained().lock().unwrap();
    expire(&mut retained);
    retained.insert(
        run_id.to_string(),
        Retained {
            profile,
            since: Instant::now(),
            packed: None,
        },
    );
}

fn expire(retained: &mut HashMap<String, Retained>) {
    retained.retain(|run_id, entry| {
        let keep = entry.since.elapsed() < RETAINED_TTL;
        if !keep && let Err(err) = entry.profile.profile.remove() {
            log::warn!("automation run {run_id}: removing its expired profile: {err}");
        }
        keep
    });
}

/// Pack the profile a finished run retained. Packing happens once; later
/// calls return the same bytes.
pub fn export_retained(run_id: &str) -> Result<ProfileExport, String> {
    let mut retained = retained().lock().unwrap();
    expire(&mut retained);
    let entry = retained
        .get_mut(run_id)
        .ok_or_else(|| format!("run {run_id} has no retained profile (expired or discarded)"))?;
    if let Some(packed) = &entry.packed {
        return Ok(packed.clone());
    }
    let manifest = data_profile::manifest_for(&entry.profile.appid);
    let bytes = data_profile::pack(&entry.profile.profile.live(), &manifest)
        .map_err(|err| err.to_string())?;
    let packed = ProfileExport {
        sha256: data_profile::sha256_hex(&bytes),
        bytes: Arc::new(bytes),
    };
    entry.packed = Some(packed.clone());
    Ok(packed)
}

/// Delete a retained profile now. `false` when there was none.
pub fn discard_retained(run_id: &str) -> bool {
    let mut retained = retained().lock().unwrap();
    expire(&mut retained);
    let Some(entry) = retained.remove(run_id) else {
        return false;
    };
    if let Err(err) = entry.profile.profile.remove() {
        log::warn!("automation run {run_id}: discarding its profile: {err}");
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    fn profile(retain: bool) -> (std::path::PathBuf, AutomationProfile) {
        let base = std::env::temp_dir().join(format!(
            "lingxia-automation-profile-{}",
            uuid::Uuid::new_v4()
        ));
        let profile = RunProfile::create(&base).unwrap();
        std::fs::write(profile.live().join("storage.redb"), b"state").unwrap();
        (
            base,
            AutomationProfile {
                appid: "app.lingxia.profile-test".to_string(),
                profile,
                retain,
            },
        )
    }

    fn ok_teardown(_: &AutomationProfile) -> Result<(), String> {
        Ok(())
    }

    fn wait_done(slot: &RunProfileSlot) {
        let deadline = Instant::now() + Duration::from_secs(5);
        while slot.pending() {
            assert!(Instant::now() < deadline, "teardown did not finish");
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    #[test]
    fn teardown_deletes_an_unretained_profile_once() {
        let (base, profile) = profile(false);
        let dir = profile.profile.dir().to_path_buf();
        let slot = RunProfileSlot::new(Some(profile));
        slot.begin_teardown("run-a", ok_teardown);
        wait_done(&slot);
        assert!(!dir.exists());
        // A second finalization path does not start another teardown.
        slot.begin_teardown("run-a", ok_teardown);
        assert!(!slot.pending());
        let _ = std::fs::remove_dir_all(base);
    }

    #[test]
    fn a_failed_teardown_still_releases_the_slot() {
        fn failing(_: &AutomationProfile) -> Result<(), String> {
            Err("reopen failed".to_string())
        }
        let (base, profile) = profile(false);
        let slot = RunProfileSlot::new(Some(profile));
        slot.begin_teardown("run-b", failing);
        wait_done(&slot);
        let _ = std::fs::remove_dir_all(base);
    }

    #[test]
    fn a_retained_profile_exports_then_discards() {
        let (base, profile) = profile(true);
        let dir = profile.profile.dir().to_path_buf();
        let slot = RunProfileSlot::new(Some(profile));
        slot.begin_teardown("run-c", ok_teardown);
        wait_done(&slot);
        assert!(dir.exists(), "retained for export");

        let first = export_retained("run-c").unwrap();
        let again = export_retained("run-c").unwrap();
        assert!(Arc::ptr_eq(&first.bytes, &again.bytes), "packed once");
        assert_eq!(first.sha256.len(), 64);
        let manifest = data_profile::read_manifest(&first.bytes).unwrap();
        assert_eq!(manifest.appid, "app.lingxia.profile-test");

        assert!(discard_retained("run-c"));
        assert!(!dir.exists());
        assert!(!discard_retained("run-c"));
        assert!(export_retained("run-c").is_err());
        let _ = std::fs::remove_dir_all(base);
    }
}
