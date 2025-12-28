use lingxia_platform::{
    AudioFrame, AudioStreamConfig, PlatformError, VideoFrame, VideoStreamConfig,
    VideoStreamDecoderHandle,
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

    fn abort(&self) -> Result<(), StreamError> {
        self.stop()
    }
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
}

impl FrameSink {
    pub fn new(decoder: Box<dyn VideoStreamDecoderHandle>) -> Self {
        Self {
            decoder: decoder.into(),
            epoch_token: None,
            epoch: 0,
        }
    }

    pub fn from_arc(decoder: Arc<dyn VideoStreamDecoderHandle>) -> Self {
        Self {
            decoder,
            epoch_token: None,
            epoch: 0,
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
        }
    }

    fn is_current(&self) -> bool {
        match &self.epoch_token {
            Some(token) => token.load(Ordering::Relaxed) == self.epoch,
            None => true,
        }
    }

    pub fn configure_video(&self, config: VideoStreamConfig) -> Result<(), StreamError> {
        if !self.is_current() {
            return Ok(());
        }
        self.decoder
            .configure_video(config)
            .map_err(StreamError::from)
    }

    pub fn configure_audio(&self, config: AudioStreamConfig) -> Result<(), StreamError> {
        if !self.is_current() {
            return Ok(());
        }
        self.decoder
            .configure_audio(config)
            .map_err(StreamError::from)
    }

    pub fn push_video(&self, frame: VideoFrame) -> Result<(), StreamError> {
        if !self.is_current() {
            return Ok(());
        }
        self.decoder.push_video(frame).map_err(StreamError::from)
    }

    pub fn push_audio(&self, frame: AudioFrame) -> Result<(), StreamError> {
        if !self.is_current() {
            return Ok(());
        }
        self.decoder.push_audio(frame).map_err(StreamError::from)
    }

    pub fn stop(&self) -> Result<(), StreamError> {
        if !self.is_current() {
            return Ok(());
        }
        self.decoder.stop().map_err(StreamError::from)
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
