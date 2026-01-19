use lingxia_platform::PlatformError;
use lingxia_platform::traits::stream_decoder::{
    AudioFrame, AudioStreamConfig, VideoFrame, VideoStreamConfig, VideoStreamDecoderHandle,
};
use serde_json::Value;
use std::collections::HashMap;
use std::fmt;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};

#[derive(Debug, Clone)]
pub struct StreamError {
    message: String,
}

impl StreamError {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl fmt::Display for StreamError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for StreamError {}

impl From<PlatformError> for StreamError {
    fn from(err: PlatformError) -> Self {
        StreamError::new(err.to_string())
    }
}

pub trait StreamSession: Send + Sync {
    fn stop(&self) -> Result<(), StreamError>;
    fn pause(&self) -> Result<(), StreamError>;
    fn resume(&self) -> Result<(), StreamError>;
    fn seek(&self, position: f64) -> Result<(), StreamError>;
}

pub trait StreamProvider: Send + Sync {
    fn name(&self) -> &'static str;
    fn start(&self, params: Value, sink: FrameSink) -> Result<Box<dyn StreamSession>, StreamError>;

    fn should_force_hard_switch(&self, _prev_params: Option<&Value>, _next_params: &Value) -> bool {
        false
    }
}

#[derive(Clone)]
pub struct FrameSink {
    decoder: Arc<dyn VideoStreamDecoderHandle>,
    epoch_token: Option<Arc<AtomicU64>>,
    epoch: u64,
    stale_logged: Arc<std::sync::atomic::AtomicBool>,
    component_id: Option<String>,
    duration_reporter: Option<Arc<dyn Fn(u64) + Send + Sync>>,
    ended_reporter: Option<Arc<dyn Fn() + Send + Sync>>,
}

impl FrameSink {
    pub fn new(decoder: Box<dyn VideoStreamDecoderHandle>) -> Self {
        Self {
            decoder: decoder.into(),
            epoch_token: None,
            epoch: 0,
            stale_logged: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            component_id: None,
            duration_reporter: None,
            ended_reporter: None,
        }
    }

    pub fn from_arc(decoder: Arc<dyn VideoStreamDecoderHandle>) -> Self {
        Self {
            decoder,
            epoch_token: None,
            epoch: 0,
            stale_logged: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            component_id: None,
            duration_reporter: None,
            ended_reporter: None,
        }
    }

    pub fn from_arc_with_epoch(
        decoder: Arc<dyn VideoStreamDecoderHandle>,
        epoch_token: Arc<AtomicU64>,
        epoch: u64,
    ) -> Self {
        Self {
            decoder,
            epoch_token: Some(epoch_token),
            epoch,
            stale_logged: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            component_id: None,
            duration_reporter: None,
            ended_reporter: None,
        }
    }

    pub fn with_component_id(mut self, component_id: impl Into<String>) -> Self {
        self.component_id = Some(component_id.into());
        self
    }

    pub fn with_duration_reporter<F>(mut self, reporter: F) -> Self
    where
        F: Fn(u64) + Send + Sync + 'static,
    {
        self.duration_reporter = Some(Arc::new(reporter));
        self
    }

    pub fn with_ended_reporter<F>(mut self, reporter: F) -> Self
    where
        F: Fn() + Send + Sync + 'static,
    {
        self.ended_reporter = Some(Arc::new(reporter));
        self
    }

    pub fn component_id(&self) -> Option<&str> {
        self.component_id.as_deref()
    }

    fn is_current(&self, op: &'static str) -> bool {
        let Some(token) = &self.epoch_token else {
            return true;
        };
        let current = token.load(Ordering::Relaxed);
        if current == self.epoch {
            return true;
        }
        if self
            .stale_logged
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_ok()
        {
            let component_id = self.component_id.as_deref().unwrap_or("-");
            crate::warn!(
                "FrameSink dropped stale {}: component_id={} sink_epoch={} current_epoch={}",
                op,
                component_id,
                self.epoch,
                current
            );
        }
        false
    }

    pub fn configure_video(&self, config: VideoStreamConfig) -> Result<(), StreamError> {
        if !self.is_current("configure_video") {
            return Ok(());
        }
        self.decoder
            .configure_video(config)
            .map_err(StreamError::from)
    }

    pub fn configure_audio(&self, config: AudioStreamConfig) -> Result<(), StreamError> {
        if !self.is_current("configure_audio") {
            return Ok(());
        }
        self.decoder
            .configure_audio(config)
            .map_err(StreamError::from)
    }

    pub fn push_video(&self, frame: VideoFrame) -> Result<(), StreamError> {
        if !self.is_current("push_video") {
            return Ok(());
        }
        self.decoder.push_video(frame).map_err(StreamError::from)
    }

    pub fn push_audio(&self, frame: AudioFrame) -> Result<(), StreamError> {
        if !self.is_current("push_audio") {
            return Ok(());
        }
        self.decoder.push_audio(frame).map_err(StreamError::from)
    }

    pub fn stop(&self) -> Result<(), StreamError> {
        if !self.is_current("stop") {
            return Ok(());
        }
        self.decoder.stop().map_err(StreamError::from)
    }

    pub fn report_duration_ms(&self, duration_ms: u64) {
        if duration_ms == 0 {
            return;
        }
        if !self.is_current("report_duration_ms") {
            return;
        }
        if let Some(reporter) = &self.duration_reporter {
            reporter(duration_ms);
        }
    }

    pub fn report_ended(&self) {
        if !self.is_current("report_ended") {
            return;
        }
        if let Some(reporter) = &self.ended_reporter {
            reporter();
        }
    }
}

type ProviderRegistry = HashMap<String, Arc<dyn StreamProvider>>;

static STREAM_PROVIDERS: OnceLock<Mutex<ProviderRegistry>> = OnceLock::new();

pub fn register_stream_provider(provider: Box<dyn StreamProvider>) {
    let registry = STREAM_PROVIDERS.get_or_init(|| Mutex::new(HashMap::new()));
    let mut providers = registry
        .lock()
        .expect("Stream provider registry mutex is poisoned");
    providers.insert(provider.name().to_string(), provider.into());
}

pub fn get_stream_provider(name: &str) -> Option<Arc<dyn StreamProvider>> {
    STREAM_PROVIDERS
        .get()
        .and_then(|registry| registry.lock().ok())
        .and_then(|providers| providers.get(name).cloned())
}

// Stream seek callback registry - allows FFI layer to seek without depending on logic layer
type SeekCallback = Arc<dyn Fn(f64) -> bool + Send + Sync>;
type SeekCallbackRegistry = HashMap<String, SeekCallback>;

static STREAM_SEEK_CALLBACKS: OnceLock<Mutex<SeekCallbackRegistry>> = OnceLock::new();

fn seek_callback_registry() -> &'static Mutex<SeekCallbackRegistry> {
    STREAM_SEEK_CALLBACKS.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Register a stream seek callback for a component.
/// The callback takes position in seconds and returns true if seek was successful.
pub fn register_stream_seek_callback<F>(component_id: &str, callback: F)
where
    F: Fn(f64) -> bool + Send + Sync + 'static,
{
    if let Ok(mut registry) = seek_callback_registry().lock() {
        crate::info!(
            "[StreamSource] register_stream_seek_callback: component_id={}",
            component_id
        );
        registry.insert(component_id.to_string(), Arc::new(callback));
    }
}

/// Unregister a stream seek callback.
pub fn unregister_stream_seek_callback(component_id: &str) {
    if let Ok(mut registry) = seek_callback_registry().lock() {
        registry.remove(component_id);
    }
}

/// Seek stream session by component_id. Returns true if seek was successful.
pub fn seek_stream_session(component_id: &str, position_seconds: f64) -> bool {
    crate::info!(
        "[StreamSource] seek_stream_session: component_id={} position_seconds={}",
        component_id,
        position_seconds
    );

    // Clone the Arc callback while holding the lock, then call it outside the lock
    let callback = {
        let Ok(registry) = seek_callback_registry().lock() else {
            crate::warn!(
                "[StreamSource] seek_stream_session: failed to lock registry for component_id={}",
                component_id
            );
            return false;
        };
        let cb = registry.get(component_id).cloned();
        if cb.is_none() {
            crate::warn!(
                "[StreamSource] seek_stream_session: no callback found for component_id={}, registered_ids={:?}",
                component_id,
                registry.keys().collect::<Vec<_>>()
            );
        }
        cb
    };

    if let Some(cb) = callback {
        crate::info!(
            "[StreamSource] seek_stream_session: invoking callback for component_id={}",
            component_id
        );
        return cb(position_seconds);
    }
    false
}
