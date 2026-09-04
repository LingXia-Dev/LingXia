//! Host-wide display-language state and Runner session overrides.

use super::runtime_registry::{get_lxapps_manager, get_platform};
use crate::error::LxAppError;
use language_tags::LanguageTag as ParsedLanguageTag;
use lingxia_platform::traits::app_runtime::AppRuntime;
use lingxia_webview::WebViewController;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::collections::VecDeque;
use std::fmt;
use std::path::Path;
use std::str::FromStr;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use tokio::sync::mpsc;

const FALLBACK_LANGUAGE: &str = "en-US";

/// A validated, canonical BCP-47 language tag.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct LanguageTag(String);

impl LanguageTag {
    /// Parse, validate, and canonicalize a BCP-47 language tag.
    pub fn parse(value: &str) -> Result<Self, String> {
        let value = value.trim();
        if value.is_empty() {
            return Err("language tag must not be empty".to_string());
        }
        if value.eq_ignore_ascii_case("auto") {
            return Err("'auto' is reserved for the automatic display-language preference".into());
        }
        let parsed = ParsedLanguageTag::parse(value)
            .map_err(|error| format!("invalid BCP-47 language tag '{value}': {error}"))?;
        parsed
            .validate()
            .map_err(|error| format!("invalid BCP-47 language tag '{value}': {error}"))?;
        let canonical = parsed
            .canonicalize()
            .map_err(|error| format!("invalid BCP-47 language tag '{value}': {error}"))?
            .into_string();
        if canonical.eq_ignore_ascii_case("auto") {
            return Err("'auto' is reserved for the automatic display-language preference".into());
        }
        Ok(Self(canonical))
    }

    /// Return the canonical wire value.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    fn from_system(value: &str) -> Result<Self, String> {
        Self::parse(&value.replace('_', "-"))
    }
}

impl fmt::Display for LanguageTag {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for LanguageTag {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::parse(value)
    }
}

impl Serialize for LanguageTag {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for LanguageTag {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Self::parse(&String::deserialize(deserializer)?).map_err(serde::de::Error::custom)
    }
}

/// Persisted host display-language preference.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum DisplayLanguagePreference {
    /// Follow the current system language.
    Auto,
    /// Pin a canonical BCP-47 language tag.
    LanguageTag(LanguageTag),
}

impl DisplayLanguagePreference {
    /// Return the persistence/wire representation.
    pub fn as_str(&self) -> &str {
        match self {
            Self::Auto => "auto",
            Self::LanguageTag(tag) => tag.as_str(),
        }
    }

    fn persisted_tag(&self) -> Option<&str> {
        match self {
            Self::Auto => None,
            Self::LanguageTag(tag) => Some(tag.as_str()),
        }
    }
}

impl fmt::Display for DisplayLanguagePreference {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for DisplayLanguagePreference {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let value = value.trim();
        if value.eq_ignore_ascii_case("auto") {
            Ok(Self::Auto)
        } else {
            LanguageTag::parse(value).map(Self::LanguageTag)
        }
    }
}

impl Serialize for DisplayLanguagePreference {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for DisplayLanguagePreference {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        String::deserialize(deserializer)?
            .parse()
            .map_err(serde::de::Error::custom)
    }
}

/// The highest-priority input that produced the effective language.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum DisplayLanguageEffectiveSource {
    /// The persisted preference follows the system language.
    System,
    /// A persisted language tag won.
    Preference,
    /// A Runner session override won, including an `auto` override.
    SessionOverride,
}

/// Complete host display-language state.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DisplayLanguageState {
    /// Persisted preference, even while a Runner override shadows it.
    pub preference: DisplayLanguagePreference,
    /// Resolved canonical language tag rendered by every host surface.
    pub effective: LanguageTag,
    /// The winning input layer.
    pub effective_source: DisplayLanguageEffectiveSource,
}

/// Opaque owner of a Runner session-only override.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct DisplayLanguageSessionOwner(u64);

impl DisplayLanguageSessionOwner {
    fn next() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(1);
        Self(NEXT.fetch_add(1, Ordering::Relaxed))
    }
}

#[derive(Debug, Clone)]
struct SessionOverride {
    owner: DisplayLanguageSessionOwner,
    preference: DisplayLanguagePreference,
}

type StateListener = Arc<dyn Fn(DisplayLanguageState) + Send + Sync>;
type EffectiveListener = Arc<dyn Fn(LanguageTag) + Send + Sync>;

/// A revisioned state update used to make stream subscription atomic.
#[doc(hidden)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DisplayLanguageStateUpdate {
    pub revision: u64,
    pub state: DisplayLanguageState,
}

/// A revisioned effective-language update used to make stream subscription atomic.
#[doc(hidden)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DisplayLanguageEffectiveUpdate {
    pub revision: u64,
    pub effective: LanguageTag,
}

struct ServiceInner {
    preference: DisplayLanguagePreference,
    system: LanguageTag,
    session_override: Option<SessionOverride>,
    state: DisplayLanguageState,
    revision: u64,
    effective_revision: u64,
    pending: VecDeque<Transition>,
    publishing: bool,
    state_subscribers: Vec<mpsc::UnboundedSender<DisplayLanguageStateUpdate>>,
    effective_subscribers: Vec<mpsc::UnboundedSender<DisplayLanguageEffectiveUpdate>>,
    state_listeners: Vec<StateListener>,
    effective_listeners: Vec<EffectiveListener>,
}

impl ServiceInner {
    fn resolve(&self) -> DisplayLanguageState {
        let (effective, effective_source) = match &self.session_override {
            Some(SessionOverride {
                preference: DisplayLanguagePreference::LanguageTag(tag),
                ..
            }) => (tag.clone(), DisplayLanguageEffectiveSource::SessionOverride),
            Some(SessionOverride {
                preference: DisplayLanguagePreference::Auto,
                ..
            }) => (
                self.system.clone(),
                DisplayLanguageEffectiveSource::SessionOverride,
            ),
            None => match &self.preference {
                DisplayLanguagePreference::LanguageTag(tag) => {
                    (tag.clone(), DisplayLanguageEffectiveSource::Preference)
                }
                DisplayLanguagePreference::Auto => {
                    (self.system.clone(), DisplayLanguageEffectiveSource::System)
                }
            },
        };
        DisplayLanguageState {
            preference: self.preference.clone(),
            effective,
            effective_source,
        }
    }
}

struct DisplayLanguageService {
    inner: Mutex<ServiceInner>,
}

struct Transition {
    revision: u64,
    state: DisplayLanguageState,
    state_listeners: Vec<StateListener>,
    effective_listeners: Vec<EffectiveListener>,
    effective_changed: bool,
}

impl DisplayLanguageService {
    fn new(preference: DisplayLanguagePreference, system: LanguageTag) -> Self {
        let initial = DisplayLanguageState {
            preference: DisplayLanguagePreference::Auto,
            effective: system.clone(),
            effective_source: DisplayLanguageEffectiveSource::System,
        };
        let mut inner = ServiceInner {
            preference,
            system,
            session_override: None,
            state: initial,
            revision: 0,
            effective_revision: 0,
            pending: VecDeque::new(),
            publishing: false,
            state_subscribers: Vec::new(),
            effective_subscribers: Vec::new(),
            state_listeners: Vec::new(),
            effective_listeners: Vec::new(),
        };
        inner.state = inner.resolve();
        Self {
            inner: Mutex::new(inner),
        }
    }

    fn state(&self) -> DisplayLanguageState {
        self.inner
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .state
            .clone()
    }

    fn snapshot(&self) -> DisplayLanguageStateUpdate {
        let inner = self.inner.lock().unwrap_or_else(|error| error.into_inner());
        DisplayLanguageStateUpdate {
            revision: inner.revision,
            state: inner.state.clone(),
        }
    }

    fn subscribe_state(
        &self,
    ) -> (
        DisplayLanguageStateUpdate,
        mpsc::UnboundedReceiver<DisplayLanguageStateUpdate>,
    ) {
        let mut inner = self.inner.lock().unwrap_or_else(|error| error.into_inner());
        let (sender, receiver) = mpsc::unbounded_channel();
        inner
            .state_subscribers
            .retain(|subscriber| !subscriber.is_closed());
        inner.state_subscribers.push(sender);
        (
            DisplayLanguageStateUpdate {
                revision: inner.revision,
                state: inner.state.clone(),
            },
            receiver,
        )
    }

    fn subscribe_effective(
        &self,
    ) -> (
        DisplayLanguageEffectiveUpdate,
        mpsc::UnboundedReceiver<DisplayLanguageEffectiveUpdate>,
    ) {
        let mut inner = self.inner.lock().unwrap_or_else(|error| error.into_inner());
        let (sender, receiver) = mpsc::unbounded_channel();
        inner
            .effective_subscribers
            .retain(|subscriber| !subscriber.is_closed());
        inner.effective_subscribers.push(sender);
        (
            DisplayLanguageEffectiveUpdate {
                revision: inner.effective_revision,
                effective: inner.state.effective.clone(),
            },
            receiver,
        )
    }

    fn enqueue_locked(inner: &mut ServiceInner, previous: DisplayLanguageState) -> bool {
        let next = inner.resolve();
        if previous == next {
            return false;
        }
        let effective_changed = previous.effective != next.effective;
        inner.revision = inner
            .revision
            .checked_add(1)
            .expect("display-language revision exhausted");
        if effective_changed {
            inner.effective_revision = inner.revision;
        }
        inner.state = next.clone();
        let transition = Transition {
            revision: inner.revision,
            state: next,
            state_listeners: inner.state_listeners.clone(),
            effective_listeners: if effective_changed {
                inner.effective_listeners.clone()
            } else {
                Vec::new()
            },
            effective_changed,
        };
        inner.pending.push_back(transition);
        if inner.publishing {
            false
        } else {
            inner.publishing = true;
            true
        }
    }

    fn update(&self, mutate: impl FnOnce(&mut ServiceInner)) -> bool {
        let mut inner = self.inner.lock().unwrap_or_else(|error| error.into_inner());
        let previous = inner.state.clone();
        mutate(&mut inner);
        Self::enqueue_locked(&mut inner, previous)
    }

    fn set_preference_persisted(
        &self,
        preference: DisplayLanguagePreference,
        persist: impl FnOnce(&DisplayLanguagePreference) -> Result<(), LxAppError>,
    ) -> Result<bool, LxAppError> {
        // Keep persistence and publication serialized. A failed write never
        // mutates memory or emits either event.
        let mut inner = self.inner.lock().unwrap_or_else(|error| error.into_inner());
        persist(&preference)?;
        let previous = inner.state.clone();
        inner.preference = preference;
        Ok(Self::enqueue_locked(&mut inner, previous))
    }

    fn install_session_override(
        &self,
        preference: DisplayLanguagePreference,
    ) -> (DisplayLanguageSessionOwner, bool) {
        let owner = DisplayLanguageSessionOwner::next();
        let transition = self.update(|inner| {
            inner.session_override = Some(SessionOverride { owner, preference });
        });
        (owner, transition)
    }

    fn clear_session_override(&self, owner: DisplayLanguageSessionOwner) -> bool {
        self.update(|inner| {
            if inner
                .session_override
                .as_ref()
                .is_some_and(|active| active.owner == owner)
            {
                inner.session_override = None;
            }
        })
    }

    fn clear_active_session_override(&self) -> bool {
        self.update(|inner| inner.session_override = None)
    }

    fn refresh_system(&self, system: LanguageTag) -> bool {
        self.update(|inner| inner.system = system)
    }

    fn add_state_listener(&self, listener: StateListener) {
        self.inner
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .state_listeners
            .push(listener);
    }

    fn add_effective_listener(&self, listener: EffectiveListener) {
        self.inner
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .effective_listeners
            .push(listener);
    }

    fn send_state_update(&self, update: &DisplayLanguageStateUpdate) {
        self.inner
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .state_subscribers
            .retain(|subscriber| subscriber.send(update.clone()).is_ok());
    }

    fn send_effective_update(&self, update: &DisplayLanguageEffectiveUpdate) {
        self.inner
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .effective_subscribers
            .retain(|subscriber| subscriber.send(update.clone()).is_ok());
    }

    fn drain(&self, mut publish: impl FnMut(Transition)) {
        loop {
            let transition = {
                let mut inner = self.inner.lock().unwrap_or_else(|error| error.into_inner());
                match inner.pending.pop_front() {
                    Some(transition) => transition,
                    None => {
                        inner.publishing = false;
                        return;
                    }
                }
            };
            publish(transition);
        }
    }
}

fn service() -> &'static DisplayLanguageService {
    static SERVICE: OnceLock<DisplayLanguageService> = OnceLock::new();
    SERVICE.get_or_init(|| {
        DisplayLanguageService::new(
            DisplayLanguagePreference::Auto,
            LanguageTag::parse(FALLBACK_LANGUAGE).expect("fallback language is valid"),
        )
    })
}

fn dispatch(should_drain: bool) {
    if !should_drain {
        return;
    }
    service().drain(publish_transition);
}

fn publish_transition(transition: Transition) {
    let state_update = DisplayLanguageStateUpdate {
        revision: transition.revision,
        state: transition.state.clone(),
    };
    service().send_state_update(&state_update);
    publish_state(&state_update);
    for listener in transition.state_listeners {
        listener(transition.state.clone());
    }
    if transition.effective_changed {
        let effective_update = DisplayLanguageEffectiveUpdate {
            revision: transition.revision,
            effective: transition.state.effective.clone(),
        };
        service().send_effective_update(&effective_update);
        publish_effective(&effective_update);
        for listener in transition.effective_listeners {
            listener(transition.state.effective.clone());
        }
    }
}

fn publish_state(update: &DisplayLanguageStateUpdate) {
    let Some(manager) = get_lxapps_manager() else {
        return;
    };
    let Ok(payload) = serde_json::to_string(&serde_json::json!({
        "revision": update.revision,
        "state": update.state,
    })) else {
        return;
    };
    let appids: Vec<_> = manager
        .lxapps
        .iter()
        .map(|entry| entry.key().clone())
        .collect();
    for appid in appids {
        crate::appservice::event_bus::publish_app_event(
            &appid,
            crate::DISPLAY_LANGUAGE_STATE_CHANGE_EVENT,
            Some(payload.clone()),
        );
    }
}

/// Initialize the service from persisted state and the current host locale.
///
/// The returned owner belongs to the native Runner session. The host retains
/// it through graceful teardown; a crash or process exit drops the entire
/// in-memory service, and a later takeover receives a distinct owner token.
pub fn initialize_display_language(
    persisted: Option<String>,
    system: &str,
    runner_override: Option<DisplayLanguagePreference>,
) -> Result<Option<DisplayLanguageSessionOwner>, LxAppError> {
    let preference = persisted
        .as_deref()
        .unwrap_or("auto")
        .parse::<DisplayLanguagePreference>()
        .map_err(LxAppError::InvalidParameter)?;
    let system = LanguageTag::from_system(system).map_err(LxAppError::InvalidParameter)?;
    dispatch(service().update(|inner| {
        inner.preference = preference;
        inner.system = system;
        inner.session_override = None;
    }));
    Ok(runner_override.map(install_display_language_session_override))
}

/// Snapshot the complete host display-language state.
pub fn display_language_state() -> DisplayLanguageState {
    service().state()
}

/// Snapshot the state and its revision in one linearized read.
#[doc(hidden)]
pub fn display_language_state_update() -> DisplayLanguageStateUpdate {
    service().snapshot()
}

/// Subscribe without a gap between the initial snapshot and later updates.
#[doc(hidden)]
pub fn subscribe_display_language_state() -> (
    DisplayLanguageStateUpdate,
    mpsc::UnboundedReceiver<DisplayLanguageStateUpdate>,
) {
    service().subscribe_state()
}

/// Subscribe to effective-language changes without an initial-snapshot race.
#[doc(hidden)]
pub fn subscribe_display_language_effective() -> (
    DisplayLanguageEffectiveUpdate,
    mpsc::UnboundedReceiver<DisplayLanguageEffectiveUpdate>,
) {
    service().subscribe_effective()
}

/// Effective canonical language tag rendered by every host surface.
pub fn display_language() -> String {
    display_language_state().effective.to_string()
}

/// Persist a host preference, then atomically publish its resulting state.
pub fn set_display_language_preference(
    preference: DisplayLanguagePreference,
) -> Result<(), LxAppError> {
    let dir = get_platform()
        .ok_or_else(|| LxAppError::Runtime("SDK has not been initialized".to_string()))?
        .app_data_dir();
    set_display_language_preference_in(&dir, preference)
}

/// Persist a host preference in an explicit app-data directory.
pub fn set_display_language_preference_in(
    app_data_dir: &Path,
    preference: DisplayLanguagePreference,
) -> Result<(), LxAppError> {
    let transition = service().set_preference_persisted(preference, |preference| {
        lingxia_settings::set_display_language(app_data_dir, preference.persisted_tag())
            .map_err(|error| LxAppError::Runtime(error.to_string()))
    })?;
    dispatch(transition);
    Ok(())
}

/// Install a Runner session-only override and return its independent owner.
/// Installing another override is a takeover; stale owners cannot clear it.
pub fn install_display_language_session_override(
    preference: DisplayLanguagePreference,
) -> DisplayLanguageSessionOwner {
    let (owner, transition) = service().install_session_override(preference);
    dispatch(transition);
    owner
}

/// Clear an override only when `owner` is still the active Runner session.
pub fn clear_display_language_session_override(owner: DisplayLanguageSessionOwner) {
    dispatch(service().clear_session_override(owner));
}

/// Clear whichever Runner session override is active during host teardown.
#[doc(hidden)]
pub fn clear_active_display_language_session_override() {
    dispatch(service().clear_active_session_override());
}

/// Refresh the system input after the native host reports a locale change.
pub fn refresh_display_language_system(system: &str) -> Result<(), LxAppError> {
    let system = LanguageTag::from_system(system).map_err(LxAppError::InvalidParameter)?;
    dispatch(service().refresh_system(system));
    Ok(())
}

/// Register a listener for every actual state change.
pub fn add_display_language_state_listener(
    listener: Box<dyn Fn(DisplayLanguageState) + Send + Sync>,
) {
    service().add_state_listener(Arc::from(listener));
}

/// Register a listener only for actual effective-tag changes.
pub fn add_display_language_effective_listener(listener: Box<dyn Fn(LanguageTag) + Send + Sync>) {
    service().add_effective_listener(Arc::from(listener));
}

fn publish_effective(update: &DisplayLanguageEffectiveUpdate) {
    let Some(manager) = get_lxapps_manager() else {
        return;
    };
    let quoted = serde_json::to_string(update.effective.as_str())
        .unwrap_or_else(|_| format!("\"{FALLBACK_LANGUAGE}\""));
    let script = format!("var f = globalThis.__lingxiaApplyDisplayLanguage; if (f) f({quoted});");
    let apps: Vec<_> = manager
        .lxapps
        .iter()
        .map(|entry| (entry.key().clone(), entry.value().clone()))
        .collect();
    for (appid, app) in apps {
        for page in app.live_page_instances() {
            if let Some(webview) = page.webview() {
                let _ = webview.exec_js(&script);
            }
        }
        crate::appservice::event_bus::publish_app_event(
            &appid,
            crate::DISPLAY_LANGUAGE_CHANGE_EVENT,
            Some(
                serde_json::to_string(&serde_json::json!({
                    "revision": update.revision,
                    "effective": update.effective,
                }))
                .unwrap_or_else(|_| {
                    format!(
                        "{{\"revision\":{},\"effective\":{quoted}}}",
                        update.revision
                    )
                }),
            ),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::{Barrier, Mutex as TestMutex};

    fn tag(value: &str) -> LanguageTag {
        LanguageTag::parse(value).unwrap()
    }

    fn service_with(preference: &str, system: &str) -> DisplayLanguageService {
        DisplayLanguageService::new(preference.parse().unwrap(), tag(system))
    }

    fn counts(service: &DisplayLanguageService) -> (Arc<AtomicUsize>, Arc<AtomicUsize>) {
        let states = Arc::new(AtomicUsize::new(0));
        let effective = Arc::new(AtomicUsize::new(0));
        let state_count = states.clone();
        service.add_state_listener(Arc::new(move |_| {
            state_count.fetch_add(1, Ordering::Relaxed);
        }));
        let effective_count = effective.clone();
        service.add_effective_listener(Arc::new(move |_| {
            effective_count.fetch_add(1, Ordering::Relaxed);
        }));
        (states, effective)
    }

    fn deliver(service: &DisplayLanguageService, should_drain: bool) {
        if !should_drain {
            return;
        }
        service.drain(|transition| {
            service.send_state_update(&DisplayLanguageStateUpdate {
                revision: transition.revision,
                state: transition.state.clone(),
            });
            if transition.effective_changed {
                service.send_effective_update(&DisplayLanguageEffectiveUpdate {
                    revision: transition.revision,
                    effective: transition.state.effective.clone(),
                });
            }
            for listener in transition.state_listeners {
                listener(transition.state.clone());
            }
            for listener in transition.effective_listeners {
                listener(transition.state.effective.clone());
            }
        });
    }

    #[test]
    fn validates_and_canonicalizes_arbitrary_bcp47_tags() {
        assert_eq!(tag("JA-jp").as_str(), "ja-JP");
        assert_eq!(tag("sr-latn-rs").as_str(), "sr-Latn-RS");
        assert_eq!(tag("de-DE-u-co-phonebk").as_str(), "de-DE-u-co-phonebk");
        assert!(LanguageTag::parse("").is_err());
        assert!(LanguageTag::parse("en--US").is_err());
        assert!(LanguageTag::parse("zzz-Latn-RS").is_err());
        assert!(LanguageTag::parse("auto").is_err());
        assert!(LanguageTag::parse("AUTO").is_err());
    }

    #[test]
    fn preference_and_state_use_string_wire_contract() {
        let state = DisplayLanguageState {
            preference: "ja-jp".parse().unwrap(),
            effective: tag("ja-JP"),
            effective_source: DisplayLanguageEffectiveSource::Preference,
        };
        assert_eq!(
            serde_json::to_value(state).unwrap(),
            serde_json::json!({
                "preference": "ja-JP",
                "effective": "ja-JP",
                "effectiveSource": "preference"
            })
        );
        for wire in ["auto", "AUTO"] {
            let preference: DisplayLanguagePreference =
                serde_json::from_str(&format!("\"{wire}\"")).unwrap();
            assert_eq!(preference, DisplayLanguagePreference::Auto);
            assert_eq!(serde_json::to_string(&preference).unwrap(), "\"auto\"");
        }
        let explicit: DisplayLanguagePreference = serde_json::from_str("\"JA-jp\"").unwrap();
        assert_eq!(serde_json::to_string(&explicit).unwrap(), "\"ja-JP\"");
        assert!(serde_json::from_str::<LanguageTag>("\"auto\"").is_err());
    }

    #[test]
    fn persistence_failure_does_not_publish() {
        let service = service_with("auto", "en-US");
        let (states, effective) = counts(&service);
        let before = service.state();
        let result = service.set_preference_persisted("zh-CN".parse().unwrap(), |_| {
            Err(LxAppError::Runtime("disk full".to_string()))
        });
        assert!(result.is_err());
        assert_eq!(service.state(), before);
        assert_eq!(states.load(Ordering::Relaxed), 0);
        assert_eq!(effective.load(Ordering::Relaxed), 0);
    }

    #[test]
    fn auto_tracks_system_and_deduplicates_refresh() {
        let service = service_with("auto", "en-US");
        let (states, effective) = counts(&service);
        let drain = service.refresh_system(tag("en-us"));
        deliver(&service, drain);
        assert_eq!(states.load(Ordering::Relaxed), 0);
        let drain = service.refresh_system(tag("zh-CN"));
        deliver(&service, drain);
        assert_eq!(states.load(Ordering::Relaxed), 1);
        assert_eq!(effective.load(Ordering::Relaxed), 1);
        assert_eq!(service.state().effective.as_str(), "zh-CN");
    }

    #[test]
    fn explicit_preference_ignores_system_refresh() {
        let service = service_with("ja-JP", "en-US");
        let (states, effective) = counts(&service);
        let drain = service.refresh_system(tag("zh-CN"));
        deliver(&service, drain);
        assert_eq!(service.state().effective.as_str(), "ja-JP");
        assert_eq!(states.load(Ordering::Relaxed), 0);
        assert_eq!(effective.load(Ordering::Relaxed), 0);
    }

    #[test]
    fn repeated_preference_is_persisted_without_duplicate_observer_events() {
        let service = service_with("ja-JP", "en-US");
        let (states, effective) = counts(&service);
        let mut persisted = false;
        let transition = service
            .set_preference_persisted("JA-jp".parse().unwrap(), |_| {
                persisted = true;
                Ok(())
            })
            .unwrap();
        deliver(&service, transition);
        assert!(persisted);
        assert_eq!(states.load(Ordering::Relaxed), 0);
        assert_eq!(effective.load(Ordering::Relaxed), 0);
    }

    #[test]
    fn runner_override_shadows_preference_updates_until_normal_end() {
        let service = service_with("en-US", "de-DE");
        let (states, effective) = counts(&service);
        let (owner, transition) = service.install_session_override("ja-JP".parse().unwrap());
        deliver(&service, transition);
        let transition = service
            .set_preference_persisted("zh-CN".parse().unwrap(), |_| Ok(()))
            .unwrap();
        deliver(&service, transition);
        assert_eq!(service.state().effective.as_str(), "ja-JP");
        assert_eq!(service.state().preference.as_str(), "zh-CN");
        assert_eq!(states.load(Ordering::Relaxed), 2);
        assert_eq!(effective.load(Ordering::Relaxed), 1);
        let drain = service.clear_session_override(owner);
        deliver(&service, drain);
        assert_eq!(service.state().effective.as_str(), "zh-CN");
        assert_eq!(states.load(Ordering::Relaxed), 3);
        assert_eq!(effective.load(Ordering::Relaxed), 2);
    }

    #[test]
    fn session_auto_keeps_session_source_and_follows_system() {
        let service = service_with("zh-CN", "en-US");
        let (owner, transition) = service.install_session_override(DisplayLanguagePreference::Auto);
        deliver(&service, transition);
        assert_eq!(
            service.state().effective_source,
            DisplayLanguageEffectiveSource::SessionOverride
        );
        assert_eq!(service.state().effective.as_str(), "en-US");
        let drain = service.refresh_system(tag("fr-FR"));
        deliver(&service, drain);
        assert_eq!(service.state().effective.as_str(), "fr-FR");
        let drain = service.clear_session_override(owner);
        deliver(&service, drain);
    }

    #[test]
    fn stale_crash_cleanup_cannot_clear_takeover_owner() {
        let service = service_with("auto", "en-US");
        let (crashed_owner, first) = service.install_session_override("ja-JP".parse().unwrap());
        deliver(&service, first);
        let (takeover_owner, second) = service.install_session_override("fr-FR".parse().unwrap());
        deliver(&service, second);
        let drain = service.clear_session_override(crashed_owner);
        deliver(&service, drain);
        assert_eq!(service.state().effective.as_str(), "fr-FR");
        let drain = service.clear_session_override(takeover_owner);
        deliver(&service, drain);
        assert_eq!(service.state().effective.as_str(), "en-US");
    }

    #[test]
    fn same_effective_override_changes_state_only_once() {
        let service = service_with("en-US", "zh-CN");
        let (states, effective) = counts(&service);
        let (owner, transition) = service.install_session_override("en-us".parse().unwrap());
        deliver(&service, transition);
        assert_eq!(states.load(Ordering::Relaxed), 1);
        assert_eq!(effective.load(Ordering::Relaxed), 0);
        let drain = service.clear_session_override(owner);
        deliver(&service, drain);
        assert_eq!(states.load(Ordering::Relaxed), 2);
        assert_eq!(effective.load(Ordering::Relaxed), 0);
    }

    #[test]
    fn subscribe_snapshot_deduplicates_a_pending_boundary_update() {
        let service = service_with("auto", "en-US");
        let should_drain = service.refresh_system(tag("ja-JP"));
        let (initial, mut receiver) = service.subscribe_state();
        assert_eq!(initial.revision, 1);
        assert_eq!(initial.state.effective.as_str(), "ja-JP");

        deliver(&service, should_drain);
        let queued = receiver.try_recv().unwrap();
        assert_eq!(queued.revision, initial.revision);
        assert!(queued.revision <= initial.revision);

        let should_drain = service.refresh_system(tag("fr-FR"));
        deliver(&service, should_drain);
        let next = receiver.try_recv().unwrap();
        assert_eq!(next.revision, initial.revision + 1);
        assert_eq!(next.state.effective.as_str(), "fr-FR");
    }

    #[test]
    fn concurrent_transitions_publish_in_strict_revision_order() {
        let service = Arc::new(service_with("auto", "en-US"));
        let (initial, mut receiver) = service.subscribe_state();
        let barrier = Arc::new(Barrier::new(65));
        let mut threads = Vec::new();
        for index in 0..64 {
            let service = service.clone();
            let barrier = barrier.clone();
            threads.push(std::thread::spawn(move || {
                barrier.wait();
                let should_drain = service.refresh_system(tag(&format!("en-x-t{index}")));
                deliver(&service, should_drain);
            }));
        }
        barrier.wait();
        for thread in threads {
            thread.join().unwrap();
        }

        let mut revisions = Vec::new();
        while let Ok(update) = receiver.try_recv() {
            revisions.push(update.revision);
        }
        assert_eq!(revisions.len(), 64);
        assert_eq!(
            revisions,
            (initial.revision + 1..=initial.revision + 64).collect::<Vec<_>>()
        );
    }

    #[test]
    fn reentrant_transition_is_queued_after_the_current_publication() {
        let service = Arc::new(service_with("auto", "en-US"));
        let observed = Arc::new(TestMutex::new(Vec::new()));
        let service_in_listener = service.clone();
        let observed_in_listener = observed.clone();
        service.add_effective_listener(Arc::new(move |language| {
            observed_in_listener
                .lock()
                .unwrap()
                .push(language.to_string());
            if language.as_str() == "ja-JP" {
                assert!(!service_in_listener.refresh_system(tag("fr-FR")));
            }
        }));

        let should_drain = service.refresh_system(tag("ja-JP"));
        deliver(&service, should_drain);
        assert_eq!(&*observed.lock().unwrap(), &["ja-JP", "fr-FR"]);
    }

    #[test]
    fn disconnect_reconnect_and_takeover_do_not_retain_stale_override() {
        let service = service_with("auto", "en-US");
        let (disconnected, drain) = service.install_session_override("ja-JP".parse().unwrap());
        deliver(&service, drain);
        let drain = service.clear_session_override(disconnected);
        deliver(&service, drain);
        assert_eq!(service.state().effective.as_str(), "en-US");

        let (reconnected, drain) = service.install_session_override("fr-FR".parse().unwrap());
        deliver(&service, drain);
        let (takeover, drain) = service.install_session_override("de-DE".parse().unwrap());
        deliver(&service, drain);
        let drain = service.clear_session_override(reconnected);
        deliver(&service, drain);
        assert_eq!(service.state().effective.as_str(), "de-DE");
        let drain = service.clear_session_override(takeover);
        deliver(&service, drain);
        assert_eq!(service.state().effective.as_str(), "en-US");
    }
}
