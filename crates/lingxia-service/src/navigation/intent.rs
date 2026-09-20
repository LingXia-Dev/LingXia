//! Activation intents: the OS carries an opaque token, the host keeps the target.
//!
//! Platform payload limits therefore never decide what a route may declare,
//! and no business parameter is exposed through a launch command line. A token
//! is single-use and bound to one generation of one public notification id, so
//! a stale banner can never resolve to whatever that id holds now.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::{Map, Value};
use sha2::{Digest, Sha256};

use super::target::{NavigationError, NavigationTarget};

const ENVELOPE_VERSION: u64 = 1;
const STORE_FILE: &str = "navigation-intents.json";
/// Reached only by a product that publishes far more live notifications than
/// it cancels; a new publish then fails rather than silently dropping one that
/// is still tappable.
const MAX_RECORDS: usize = 256;
/// A consumed token is kept only to merge the OS's repeat callbacks.
const CONSUMED_TTL_MS: u64 = 24 * 60 * 60 * 1000;
/// Same bound as the startup dispatch queue: a host that never inits must
/// not grow an unbounded tap list.
const MAX_DEFERRED: usize = 8;

/// A staged intent. Commit it once the OS accepted the notification, roll it
/// back otherwise: the record must never outlive a submission that failed.
#[derive(Debug, Clone)]
pub struct StagedIntent {
    pub token: String,
    id: String,
    generation: u64,
}

impl StagedIntent {
    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn generation(&self) -> u64 {
        self.generation
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Status {
    Pending,
    Posted,
    Consumed,
}

impl Status {
    fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Posted => "posted",
            Self::Consumed => "consumed",
        }
    }

    fn parse(value: &str) -> Option<Self> {
        match value {
            "pending" => Some(Self::Pending),
            "posted" => Some(Self::Posted),
            "consumed" => Some(Self::Consumed),
            _ => None,
        }
    }
}

#[derive(Debug, Clone)]
struct Record {
    scope: String,
    id: String,
    generation: u64,
    token: String,
    target: Value,
    created_ms: u64,
    status: Status,
}

impl Record {
    fn to_json(&self) -> Value {
        serde_json::json!({
            "v": ENVELOPE_VERSION,
            "scope": self.scope,
            "id": self.id,
            "generation": self.generation,
            "token": self.token,
            "target": self.target,
            "createdMs": self.created_ms,
            "status": self.status.as_str(),
        })
    }

    fn from_json(value: &Value) -> Option<Self> {
        let object = value.as_object()?;
        if object.get("v").and_then(Value::as_u64)? != ENVELOPE_VERSION {
            return None;
        }
        Some(Self {
            scope: object.get("scope")?.as_str()?.to_string(),
            id: object.get("id")?.as_str()?.to_string(),
            generation: object.get("generation").and_then(Value::as_u64)?,
            token: object.get("token")?.as_str()?.to_string(),
            target: object.get("target")?.clone(),
            created_ms: object.get("createdMs").and_then(Value::as_u64)?,
            status: Status::parse(object.get("status")?.as_str()?)?,
        })
    }
}

#[derive(Default)]
struct Store {
    dir: Option<PathBuf>,
    next_generation: u64,
    records: Vec<Record>,
    loaded: bool,
}

fn store() -> &'static Mutex<Store> {
    static STORE: OnceLock<Mutex<Store>> = OnceLock::new();
    STORE.get_or_init(|| Mutex::new(Store::default()))
}

fn locked() -> std::sync::MutexGuard<'static, Store> {
    store().lock().unwrap_or_else(|error| error.into_inner())
}

/// Point the store at the product's private state directory. Must not be a
/// cache directory: an ordinary cache sweep would strip live tap targets.
pub fn init(state_dir: PathBuf) {
    let mut store = locked();
    store.dir = Some(state_dir);
    store.loaded = false;
    store.records.clear();
    store.next_generation = 0;
}

pub fn is_initialized() -> bool {
    locked().dir.is_some()
}

fn deferred() -> &'static Mutex<Vec<String>> {
    static DEFERRED: OnceLock<Mutex<Vec<String>>> = OnceLock::new();
    DEFERRED.get_or_init(|| Mutex::new(Vec::new()))
}

/// Hold a tap that arrived before [`init`]. Duplicates merge.
pub fn defer(token: &str) {
    let token = token.trim();
    if token.is_empty() {
        return;
    }
    let mut queue = deferred().lock().unwrap_or_else(|error| error.into_inner());
    if queue.iter().any(|held| held == token) {
        return;
    }
    if queue.len() >= MAX_DEFERRED {
        log::warn!("dropping a notification tap that arrived before the intent store was ready");
        return;
    }
    queue.push(token.to_string());
}

pub fn take_deferred() -> Vec<String> {
    let mut queue = deferred().lock().unwrap_or_else(|error| error.into_inner());
    std::mem::take(&mut *queue)
}

/// Drop intents that were staged but never confirmed to the OS, including
/// those a previous run left behind when it exited mid-publish.
pub fn recover() {
    let mut store = locked();
    load(&mut store);
    let before = store.records.len();
    store
        .records
        .retain(|record| record.status != Status::Pending);
    if store.records.len() != before {
        log::info!(
            "dropped {} unconfirmed notification intent(s) from a previous run",
            before - store.records.len()
        );
        persist_logged(&store, "recover");
    }
}

/// Invalidate whatever `id` held and stage its replacement. The old generation
/// stops resolving here, before the OS is touched.
pub fn stage(id: &str, target: &NavigationTarget) -> Result<StagedIntent, NavigationError> {
    let mut store = locked();
    load(&mut store);
    store.records.retain(|record| record.id != id);
    gc(&mut store);
    if store.records.len() >= MAX_RECORDS {
        return Err(NavigationError::unavailable(format!(
            "the notification intent store is full ({MAX_RECORDS} live entries); cancel some first"
        )));
    }
    let generation = store.next_generation.wrapping_add(1);
    store.next_generation = generation;
    let created_ms = unix_now_ms();
    let token = derive_token(id, generation, created_ms);
    store.records.push(Record {
        scope: scope(),
        id: id.to_string(),
        generation,
        token: token.clone(),
        target: target.to_json(),
        created_ms,
        status: Status::Pending,
    });
    persist(&store).map_err(|error| {
        NavigationError::internal(format!(
            "failed to persist the notification target: {error}"
        ))
    })?;
    Ok(StagedIntent {
        token,
        id: id.to_string(),
        generation,
    })
}

/// The OS accepted the notification: the token may now resolve.
pub fn commit(staged: &StagedIntent) {
    let mut store = locked();
    if let Some(record) = store
        .records
        .iter_mut()
        .find(|record| record.token == staged.token)
    {
        record.status = Status::Posted;
    }
    persist_logged(&store, "commit");
}

/// The OS refused the notification, or a frontmost show suppressed it.
pub fn rollback(staged: &StagedIntent) {
    let mut store = locked();
    let before = store.records.len();
    store.records.retain(|record| record.token != staged.token);
    if store.records.len() != before {
        persist_logged(&store, "rollback");
    }
}

/// Forget `id`: a cancel, or a replacement that is about to be staged.
pub fn invalidate(id: &str) {
    let mut store = locked();
    load(&mut store);
    let before = store.records.len();
    store.records.retain(|record| record.id != id);
    if store.records.len() != before {
        persist_logged(&store, "invalidate");
    }
}

pub fn invalidate_all() {
    let mut store = locked();
    load(&mut store);
    if !store.records.is_empty() {
        store.records.clear();
        persist_logged(&store, "invalidate_all");
    }
}

/// What a consumed token pointed at.
#[derive(Debug, Clone, PartialEq)]
pub struct ResolvedIntent {
    /// The public notification id, for diagnostics and entry attribution.
    pub id: String,
    pub target: NavigationTarget,
}

/// Consume a token.
///
/// `Ok(None)` means this token was already consumed: the OS delivered the same
/// tap twice, and the second one merges into the first rather than navigating
/// again or telling the user anything went wrong.
pub fn resolve(token: &str) -> Result<Option<ResolvedIntent>, NavigationError> {
    let mut store = locked();
    load(&mut store);
    gc(&mut store);
    let scope = scope();
    let Some(index) = store
        .records
        .iter()
        .position(|record| record.token == token)
    else {
        persist_logged(&store, "resolve-miss");
        return Err(NavigationError::unavailable(
            "this notification's target is no longer available",
        ));
    };
    let record = &store.records[index];
    if record.scope != scope {
        return Err(NavigationError::unavailable(
            "this notification belongs to a different install of the product",
        ));
    }
    match record.status {
        Status::Consumed => return Ok(None),
        Status::Pending => {
            return Err(NavigationError::unavailable(
                "this notification was never confirmed by the system",
            ));
        }
        Status::Posted => {}
    }
    let target = NavigationTarget::from_json(&record.target).map_err(|error| {
        NavigationError::unavailable(format!("stored navigation target is unreadable: {error}"))
    })?;
    let id = record.id.clone();
    store.records[index].status = Status::Consumed;
    store.records[index].created_ms = unix_now_ms();
    persist_logged(&store, "resolve");
    Ok(Some(ResolvedIntent { id, target }))
}

/// Live entries, for tests and diagnostics.
pub fn live_count() -> usize {
    let mut store = locked();
    load(&mut store);
    store
        .records
        .iter()
        .filter(|record| record.status != Status::Consumed)
        .count()
}

fn scope() -> String {
    let product = lingxia_app_context::lingxia_id()
        .or_else(lingxia_app_context::product_name)
        .unwrap_or("lingxia");
    format!("{product}/{}", lingxia_app_context::env().as_str())
}

fn path(store: &Store) -> Option<PathBuf> {
    store.dir.as_ref().map(|dir| dir.join(STORE_FILE))
}

fn load(store: &mut Store) {
    if store.loaded {
        return;
    }
    store.loaded = true;
    let Some(path) = path(store) else {
        return;
    };
    let Ok(text) = std::fs::read_to_string(&path) else {
        return;
    };
    let Ok(value) = serde_json::from_str::<Value>(&text) else {
        log::warn!("navigation intent store is unreadable; starting empty");
        return;
    };
    store.next_generation = value
        .get("nextGeneration")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    store.records = value
        .get("records")
        .and_then(Value::as_array)
        .map(|records| records.iter().filter_map(Record::from_json).collect())
        .unwrap_or_default();
}

fn persist(store: &Store) -> Result<(), String> {
    let Some(path) = path(store) else {
        return Ok(());
    };
    let mut document = Map::new();
    document.insert("v".into(), Value::from(ENVELOPE_VERSION));
    document.insert("nextGeneration".into(), Value::from(store.next_generation));
    document.insert(
        "records".into(),
        Value::Array(store.records.iter().map(Record::to_json).collect()),
    );
    let text = serde_json::to_string(&Value::Object(document))
        .map_err(|error| format!("serialize: {error}"))?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|error| format!("create {parent:?}: {error}"))?;
    }
    // Replace whole: a half-written store would strand every live token.
    let temporary = path.with_extension("json.tmp");
    std::fs::write(&temporary, text).map_err(|error| format!("write: {error}"))?;
    match std::fs::rename(&temporary, &path) {
        Ok(()) => Ok(()),
        Err(error) => {
            // Windows refuses rename-over-existing; drop the old file and retry.
            #[cfg(windows)]
            {
                let _ = std::fs::remove_file(&path);
                std::fs::rename(&temporary, &path)
                    .map_err(|retry| format!("rename: {error}; retry after remove: {retry}"))
            }
            #[cfg(not(windows))]
            Err(format!("rename: {error}"))
        }
    }
}

fn persist_retry(store: &Store) -> Result<(), String> {
    match persist(store) {
        Ok(()) => Ok(()),
        Err(error) => {
            log::warn!("navigation intent store persist retrying after {error}");
            persist(store)
        }
    }
}

fn persist_logged(store: &Store, reason: &str) {
    if let Err(error) = persist_retry(store) {
        log::error!("navigation intent store persist failed ({reason}): {error}");
    }
}

fn gc(store: &mut Store) {
    let now = unix_now_ms();
    store.records.retain(|record| {
        record.status != Status::Consumed || now.saturating_sub(record.created_ms) < CONSUMED_TTL_MS
    });
}

/// Unguessable within a run and never reused: a caller cannot name someone
/// else's intent, and the public notification id stays a replace key only.
fn derive_token(id: &str, generation: u64, created_ms: u64) -> String {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let salt = process_salt();
    let sequence = COUNTER.fetch_add(1, Ordering::Relaxed);
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| elapsed.as_nanos())
        .unwrap_or(0);
    let mut hasher = Sha256::new();
    hasher.update(salt.to_le_bytes());
    hasher.update(sequence.to_le_bytes());
    hasher.update(generation.to_le_bytes());
    hasher.update(created_ms.to_le_bytes());
    hasher.update(nanos.to_le_bytes());
    hasher.update(id.as_bytes());
    hasher
        .finalize()
        .iter()
        .take(16)
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn process_salt() -> u64 {
    static SALT: OnceLock<u64> = OnceLock::new();
    *SALT.get_or_init(|| {
        let anchor = Box::new(0u8);
        let address = Box::into_raw(anchor) as u64;
        // SAFETY: the pointer came from `Box::into_raw` above and is reclaimed
        // exactly once, right here.
        drop(unsafe { Box::from_raw(address as *mut u8) });
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|elapsed| elapsed.as_nanos() as u64)
            .unwrap_or(0);
        address ^ nanos.rotate_left(17) ^ u64::from(std::process::id())
    })
}

fn unix_now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| elapsed.as_millis() as u64)
        .unwrap_or(0)
}

#[cfg(test)]
static SERIAL: Mutex<()> = Mutex::new(());

#[cfg(test)]
pub(super) fn with_uninitialized_store<T>(body: impl FnOnce() -> T) -> T {
    let _guard = SERIAL.lock().unwrap_or_else(|error| error.into_inner());
    {
        let mut store = locked();
        store.dir = None;
        store.loaded = false;
        store.records.clear();
        store.next_generation = 0;
    }
    let _ = take_deferred();
    let result = body();
    let _ = take_deferred();
    result
}

/// Snapshot of what is stored, for host diagnostics.
pub fn debug_summary() -> BTreeMap<String, usize> {
    let mut store = locked();
    load(&mut store);
    let mut summary = BTreeMap::new();
    for record in &store.records {
        *summary
            .entry(record.status.as_str().to_string())
            .or_insert(0) += 1;
    }
    summary
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The store is process-global, so the cases run under one lock and reset
    /// it themselves rather than racing over a shared temp directory.
    fn with_store<T>(body: impl FnOnce() -> T) -> T {
        let _guard = SERIAL.lock().unwrap_or_else(|error| error.into_inner());
        let dir = tempfile::tempdir().expect("temp dir");
        let _ = take_deferred();
        init(dir.path().to_path_buf());
        let result = body();
        init(dir.path().to_path_buf());
        let _ = take_deferred();
        result
    }

    fn target() -> NavigationTarget {
        NavigationTarget::route("downloads.detail")
    }

    #[test]
    fn a_committed_token_resolves_exactly_once() {
        with_store(|| {
            let staged = stage("download:1", &target()).unwrap();
            commit(&staged);
            assert_eq!(resolve(&staged.token).unwrap().unwrap().target, target());
            // A repeat callback for the same tap merges instead of navigating.
            assert_eq!(resolve(&staged.token).unwrap(), None);
        });
    }

    #[test]
    fn an_uncommitted_token_never_resolves() {
        with_store(|| {
            let staged = stage("download:1", &target()).unwrap();
            assert!(resolve(&staged.token).is_err());
            rollback(&staged);
            assert!(resolve(&staged.token).is_err());
        });
    }

    #[test]
    fn replacing_an_id_retires_the_old_token() {
        with_store(|| {
            let first = stage("download:1", &target()).unwrap();
            commit(&first);
            let second = stage("download:1", &NavigationTarget::Activate).unwrap();
            commit(&second);
            assert_ne!(first.token, second.token);
            assert!(resolve(&first.token).is_err());
            assert_eq!(
                resolve(&second.token).unwrap().unwrap().target,
                NavigationTarget::Activate
            );
        });
    }

    #[test]
    fn cancel_stops_a_posted_token() {
        with_store(|| {
            let staged = stage("download:1", &target()).unwrap();
            commit(&staged);
            invalidate("download:1");
            assert!(resolve(&staged.token).is_err());
        });
    }

    #[test]
    fn recover_drops_intents_that_were_never_confirmed() {
        with_store(|| {
            let confirmed = stage("a", &target()).unwrap();
            commit(&confirmed);
            let abandoned = stage("b", &target()).unwrap();
            recover();
            assert!(resolve(&abandoned.token).is_err());
            assert_eq!(resolve(&confirmed.token).unwrap().unwrap().target, target());
        });
    }

    #[test]
    fn records_survive_a_reload() {
        with_store(|| {
            let staged = stage("download:1", &target()).unwrap();
            commit(&staged);
            let dir = { locked().dir.clone().unwrap() };
            init(dir);
            assert_eq!(resolve(&staged.token).unwrap().unwrap().target, target());
        });
    }

    #[test]
    fn the_store_refuses_a_publish_instead_of_evicting_live_targets() {
        with_store(|| {
            for index in 0..MAX_RECORDS {
                let staged = stage(&format!("id:{index}"), &target()).unwrap();
                commit(&staged);
            }
            assert_eq!(live_count(), MAX_RECORDS);
            assert!(stage("one-too-many", &target()).is_err());
        });
    }

    #[test]
    fn a_tap_before_init_is_held_and_deduped() {
        with_uninitialized_store(|| {
            assert!(!is_initialized());
            defer("held-token");
            defer("held-token");
            assert_eq!(take_deferred(), vec!["held-token".to_string()]);
            assert!(take_deferred().is_empty());
        });
    }
}
