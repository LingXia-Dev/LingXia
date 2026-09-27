//! The isolated data profile one automation run owns.
//!
//! The host switches the target lxapp onto the profile before the run starts
//! and hands it to the run here. Whatever way the run ends, its finalization
//! returns the app to its own data before the run reports its end or releases
//! the automation slot, then deletes the profile or keeps it for a bounded
//! time for export. A teardown that fails ends the run as a failure and keeps
//! the profile aside ([`Quarantine`]) until a retry succeeds.

use lxapp::data_profile::{self, RunProfile};
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
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

/// Runs a teardown off the calling thread. Swappable so tests can make it
/// fail to start.
pub(crate) type SpawnFn = fn(Box<dyn FnOnce() + Send>) -> std::io::Result<()>;

pub(crate) const DEFAULT_SPAWN: SpawnFn = |job| {
    std::thread::Builder::new()
        .name("lingxia-profile-teardown".to_string())
        .spawn(job)
        .map(|_| ())
};

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

/// What a finished teardown reports to its run.
pub(crate) type TeardownDone = Box<dyn FnOnce(Result<(), String>) + Send>;

/// A run's profile, and whether its teardown started.
pub(crate) struct RunProfileSlot {
    profile: Option<AutomationProfile>,
    started: AtomicBool,
}

impl RunProfileSlot {
    pub(crate) fn new(profile: Option<AutomationProfile>) -> Self {
        Self {
            profile,
            started: AtomicBool::new(false),
        }
    }

    pub(crate) fn profile(&self) -> Option<&AutomationProfile> {
        self.profile.as_ref()
    }

    /// Return the app to its own data once, off the calling thread:
    /// finalization runs on the automation worker and the watchdog, neither
    /// of which may block. `done` gets the outcome; a run without a profile
    /// gets `Ok` at once.
    pub(crate) fn begin_teardown(
        &self,
        run_id: &str,
        teardown: TeardownFn,
        spawn: SpawnFn,
        quarantine: &Arc<Quarantine>,
        done: TeardownDone,
    ) {
        if self.started.swap(true, Ordering::SeqCst) {
            return;
        }
        let Some(profile) = self.profile.clone() else {
            done(Ok(()));
            return;
        };
        let done = Arc::new(Mutex::new(Some(done)));
        let job = {
            let done = done.clone();
            let run_id = run_id.to_string();
            let profile = profile.clone();
            let quarantine = quarantine.clone();
            Box::new(move || {
                let result = finish_teardown(&run_id, &profile, teardown, &quarantine);
                if let Some(done) = done.lock().unwrap_or_else(|err| err.into_inner()).take() {
                    done(result);
                }
            })
        };
        if let Err(err) = spawn(job) {
            let message = format!("the profile teardown could not start: {err}");
            log::error!("automation run {run_id}: {message}");
            // Nothing returned the app to its own data: it may still run on
            // the profile.
            quarantine.hold(run_id, profile, message.clone());
            if let Some(done) = done.lock().unwrap_or_else(|err| err.into_inner()).take() {
                done(Err(message));
            }
        }
    }
}

fn finish_teardown(
    run_id: &str,
    profile: &AutomationProfile,
    teardown: TeardownFn,
    quarantine: &Quarantine,
) -> Result<(), String> {
    if let Err(err) = teardown(profile) {
        log::error!(
            "automation run {run_id}: returning {} to its own data: {err}",
            profile.appid
        );
        // Nothing proves its app stopped using the profile: keep it on disk
        // and admit no new run until a retry succeeds.
        quarantine.hold(run_id, profile.clone(), err.clone());
        return Err(err);
    }
    if profile.retain {
        retain(run_id, profile.clone());
    } else if let Err(err) = profile.profile.remove() {
        log::warn!("automation run {run_id}: removing its profile: {err}");
    }
    Ok(())
}

/// Profiles whose app could not be shown to have left them. Each stays on
/// disk until a retried teardown returns its app to its own data; until
/// then the runtime admits no new run.
#[derive(Default)]
pub(crate) struct Quarantine {
    held: Mutex<Vec<Held>>,
}

struct Held {
    run_id: String,
    profile: AutomationProfile,
    error: String,
    retrying: bool,
}

impl Quarantine {
    fn held(&self) -> std::sync::MutexGuard<'_, Vec<Held>> {
        self.held.lock().unwrap_or_else(|err| err.into_inner())
    }

    fn hold(&self, run_id: &str, profile: AutomationProfile, error: String) {
        self.held().push(Held {
            run_id: run_id.to_string(),
            profile,
            error,
            retrying: false,
        });
    }

    /// `Ok` when no profile is held. Otherwise retry each held teardown off
    /// this thread and refuse, naming the first.
    pub(crate) fn admit(
        self: &Arc<Self>,
        teardown: TeardownFn,
        spawn: SpawnFn,
    ) -> Result<(), String> {
        let (refusal, retries) = {
            let mut held = self.held();
            let Some(first) = held.first() else {
                return Ok(());
            };
            let refusal = format!(
                "automation_profile_unrecovered: run {} could not return {} to its own data ({}); \
                 retrying now. Start the run again shortly; if this persists, restart the app \
                 (`lxdev lxapp restart`)",
                first.run_id, first.profile.appid, first.error
            );
            let mut retries = Vec::new();
            for entry in held.iter_mut().filter(|entry| !entry.retrying) {
                entry.retrying = true;
                retries.push((entry.run_id.clone(), entry.profile.clone()));
            }
            (refusal, retries)
        };
        // Outside the lock: a retry settles by taking it.
        for (run_id, profile) in retries {
            let quarantine = self.clone();
            let job_run = run_id.clone();
            let job = Box::new(move || {
                let result = teardown(&profile);
                quarantine.settle(&job_run, result);
            });
            if let Err(err) = spawn(job) {
                self.settle(
                    &run_id,
                    Err(format!("the retried teardown could not start: {err}")),
                );
            }
        }
        Err(refusal)
    }

    fn settle(&self, run_id: &str, result: Result<(), String>) {
        let mut held = self.held();
        let Some(position) = held.iter().position(|entry| entry.run_id == run_id) else {
            return;
        };
        match result {
            Ok(()) => {
                let entry = held.remove(position);
                if let Err(err) = entry.profile.profile.remove() {
                    log::warn!("automation run {run_id}: removing its recovered profile: {err}");
                }
            }
            Err(err) => {
                let entry = &mut held[position];
                entry.error = err;
                entry.retrying = false;
            }
        }
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
    use std::sync::mpsc;

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

    fn failing_teardown(_: &AutomationProfile) -> Result<(), String> {
        Err("timed out waiting for app.lingxia.profile-test to close".to_string())
    }

    fn no_threads(_: Box<dyn FnOnce() + Send>) -> std::io::Result<()> {
        Err(std::io::Error::other("no threads left"))
    }

    /// Run the teardown and wait for what it reports.
    fn tear_down(
        slot: &RunProfileSlot,
        run_id: &str,
        teardown: TeardownFn,
        spawn: SpawnFn,
        quarantine: &Arc<Quarantine>,
    ) -> Result<(), String> {
        let (sender, receiver) = mpsc::channel();
        slot.begin_teardown(
            run_id,
            teardown,
            spawn,
            quarantine,
            Box::new(move |result| sender.send(result).unwrap()),
        );
        receiver
            .recv_timeout(Duration::from_secs(5))
            .expect("teardown reported")
    }

    #[test]
    fn teardown_deletes_an_unretained_profile_once() {
        let (base, profile) = profile(false);
        let dir = profile.profile.dir().to_path_buf();
        let slot = RunProfileSlot::new(Some(profile));
        let quarantine = Arc::new(Quarantine::default());
        assert_eq!(
            tear_down(&slot, "run-a", ok_teardown, DEFAULT_SPAWN, &quarantine),
            Ok(())
        );
        assert!(!dir.exists());
        // A second finalization path does not start another teardown.
        let (sender, receiver) = mpsc::channel();
        slot.begin_teardown(
            "run-a",
            ok_teardown,
            DEFAULT_SPAWN,
            &quarantine,
            Box::new(move |result| sender.send(result).unwrap()),
        );
        assert!(receiver.recv_timeout(Duration::from_millis(100)).is_err());
        assert!(quarantine.admit(ok_teardown, DEFAULT_SPAWN).is_ok());
        let _ = std::fs::remove_dir_all(base);
    }

    #[test]
    fn a_failed_teardown_keeps_the_profile_and_admits_no_run_until_a_retry_succeeds() {
        let (base, profile) = profile(true);
        let dir = profile.profile.dir().to_path_buf();
        let slot = RunProfileSlot::new(Some(profile));
        let quarantine = Arc::new(Quarantine::default());
        let result = tear_down(&slot, "run-b", failing_teardown, DEFAULT_SPAWN, &quarantine);
        assert!(result.unwrap_err().contains("to close"));
        assert!(dir.exists(), "its app may still use it: not deleted");
        assert!(export_retained("run-b").is_err(), "nor offered for export");

        // Admission retries the teardown and refuses meanwhile.
        let refused = quarantine
            .admit(failing_teardown, DEFAULT_SPAWN)
            .unwrap_err();
        assert!(
            refused.starts_with("automation_profile_unrecovered: run run-b"),
            "{refused}"
        );
        let deadline = Instant::now() + Duration::from_secs(5);
        while quarantine.held().iter().any(|held| held.retrying) {
            assert!(Instant::now() < deadline, "retry did not settle");
            std::thread::sleep(Duration::from_millis(10));
        }
        assert!(
            quarantine.admit(ok_teardown, DEFAULT_SPAWN).is_err(),
            "still held after a failed retry"
        );
        let deadline = Instant::now() + Duration::from_secs(5);
        while quarantine.admit(ok_teardown, DEFAULT_SPAWN).is_err() {
            assert!(
                Instant::now() < deadline,
                "a successful retry must readmit runs"
            );
            std::thread::sleep(Duration::from_millis(10));
        }
        assert!(!dir.exists(), "a recovered profile is deleted");
        let _ = std::fs::remove_dir_all(base);
    }

    #[test]
    fn a_teardown_that_cannot_start_is_a_failure_and_keeps_the_profile() {
        let (base, profile) = profile(false);
        let dir = profile.profile.dir().to_path_buf();
        let slot = RunProfileSlot::new(Some(profile));
        let quarantine = Arc::new(Quarantine::default());
        let result = tear_down(&slot, "run-c", ok_teardown, no_threads, &quarantine);
        assert!(
            result
                .unwrap_err()
                .contains("could not start: no threads left")
        );
        assert!(dir.exists());
        assert!(quarantine.admit(ok_teardown, no_threads).is_err());
        let _ = std::fs::remove_dir_all(base);
    }

    #[test]
    fn a_retained_profile_exports_then_discards() {
        let (base, profile) = profile(true);
        let dir = profile.profile.dir().to_path_buf();
        let slot = RunProfileSlot::new(Some(profile));
        let quarantine = Arc::new(Quarantine::default());
        assert_eq!(
            tear_down(&slot, "run-d", ok_teardown, DEFAULT_SPAWN, &quarantine),
            Ok(())
        );
        assert!(dir.exists(), "retained for export");

        let first = export_retained("run-d").unwrap();
        let again = export_retained("run-d").unwrap();
        assert!(Arc::ptr_eq(&first.bytes, &again.bytes), "packed once");
        assert_eq!(first.sha256.len(), 64);
        let manifest = data_profile::read_manifest(&first.bytes).unwrap();
        assert_eq!(manifest.appid, "app.lingxia.profile-test");

        assert!(discard_retained("run-d"));
        assert!(!dir.exists());
        assert!(!discard_retained("run-d"));
        assert!(export_retained("run-d").is_err());
        let _ = std::fs::remove_dir_all(base);
    }
}
