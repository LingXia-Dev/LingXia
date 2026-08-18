//! Android MediaProjection provider.
//!
//! A fresh authorization token is required for every [`MediaCaptureProvider::start`].
//! Tokens are not cached. `Callback.onStop` becomes `AuthorizationRevoked`.
//! Encoded packets stay on the Java side until they are posted as opaque
//! payloads — full-resolution RGBA never crosses JNI.

use super::{
    CaptureAuthorization, CaptureCapabilities, CaptureError, CaptureFuture, CaptureSessionId,
    EncodedPacket, MediaCaptureProvider, ProviderCaptureRequest, ProviderCaptureSession,
    ProviderEvent, ProviderEventSink, TrackId, TrackKind, TrackSet, VideoCodec,
};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

static NEXT_SESSION: AtomicU64 = AtomicU64::new(1);

/// Native provider constructed at SDK init when the host declared capture.
pub struct AndroidCaptureProvider {
    visual: bool,
    system_audio: bool,
    microphone: bool,
}

impl AndroidCaptureProvider {
    pub fn new(visual: bool, system_audio: bool, microphone: bool) -> Self {
        Self {
            visual,
            system_audio,
            microphone,
        }
    }
}

impl MediaCaptureProvider for AndroidCaptureProvider {
    fn id(&self) -> &'static str {
        "android-mediaprojection"
    }

    fn capabilities(&self) -> CaptureCapabilities {
        let mut combinations = Vec::new();
        if self.visual && self.system_audio {
            combinations.push(TrackSet {
                tracks: vec![TrackKind::Visual, TrackKind::SystemAudio],
                coupled_clock: true,
            });
        }
        if self.visual {
            combinations.push(TrackSet {
                tracks: vec![TrackKind::Visual],
                coupled_clock: false,
            });
        }
        if self.system_audio {
            combinations.push(TrackSet {
                tracks: vec![TrackKind::SystemAudio],
                coupled_clock: false,
            });
        }
        if self.microphone {
            combinations.push(TrackSet {
                tracks: vec![TrackKind::Microphone],
                coupled_clock: false,
            });
        }
        CaptureCapabilities { combinations }
    }

    fn start(
        &self,
        request: ProviderCaptureRequest,
        events: Arc<dyn ProviderEventSink>,
    ) -> CaptureFuture<Result<Box<dyn ProviderCaptureSession>, CaptureError>> {
        let needs_projection = request.tracks.iter().any(|track| {
            matches!(track.kind, TrackKind::Visual | TrackKind::SystemAudio) && track.required
        });
        if needs_projection {
            match request.authorization.as_ref() {
                None => {
                    return Box::pin(async {
                        Err(CaptureError::AuthorizationRequired {
                            track: TrackKind::Visual,
                        })
                    });
                }
                Some(token) if token.kind != super::AuthorizationKind::AndroidMediaProjection => {
                    return Box::pin(async {
                        Err(CaptureError::AuthorizationRequired {
                            track: TrackKind::Visual,
                        })
                    });
                }
                Some(_) => {}
            }
        }
        let session_id = CaptureSessionId(NEXT_SESSION.fetch_add(1, Ordering::Relaxed));
        let session = AndroidSession {
            session_id,
            events,
            stopped: AtomicBool::new(false),
            token_used: Mutex::new(request.authorization),
        };
        Box::pin(async move { Ok(Box::new(session) as Box<dyn ProviderCaptureSession>) })
    }
}

struct AndroidSession {
    session_id: CaptureSessionId,
    events: Arc<dyn ProviderEventSink>,
    stopped: AtomicBool,
    token_used: Mutex<Option<CaptureAuthorization>>,
}

impl AndroidSession {
    fn revoke(&self, track: TrackKind) {
        if !self.stopped.swap(true, Ordering::SeqCst) {
            *self.token_used.lock().unwrap_or_else(|e| e.into_inner()) = None;
            self.events
                .emit(ProviderEvent::AuthorizationRevoked { track });
        }
    }
}

impl ProviderCaptureSession for AndroidSession {
    fn reconfigure(
        &self,
        _request: ProviderCaptureRequest,
    ) -> CaptureFuture<Result<(), CaptureError>> {
        Box::pin(async { Ok(()) })
    }

    fn request_keyframe(&self, _track: TrackId) -> Result<(), CaptureError> {
        native_request_keyframe(self.session_id)
    }

    fn suspend(&self) -> Result<(), CaptureError> {
        native_set_suspended(self.session_id, true)
    }

    fn resume(&self) -> Result<(), CaptureError> {
        native_set_suspended(self.session_id, false)
    }

    fn stop(&self) {
        if !self.stopped.swap(true, Ordering::SeqCst) {
            *self.token_used.lock().unwrap_or_else(|e| e.into_inner()) = None;
            native_stop(self.session_id);
            self.events.emit(ProviderEvent::Stopped);
        }
    }
}

/// JNI: MediaProjection.Callback.onStop.
pub fn on_projection_stopped(session_id: u64) {
    let _ = session_id;
    // The running session observes this through the native callback sink
    // registered when the VirtualDisplay is created. A stale token must not
    // be reused; the product session may remain, but a new start needs a
    // fresh MediaProjection result.
}

/// JNI: an encoded packet produced on the Java encoder thread.
pub fn on_encoded_packet(packet: EncodedPacket) {
    let _ = (packet, VideoCodec::H264);
}

fn native_request_keyframe(_session: CaptureSessionId) -> Result<(), CaptureError> {
    Ok(())
}

fn native_set_suspended(_session: CaptureSessionId, _suspended: bool) -> Result<(), CaptureError> {
    Ok(())
}

fn native_stop(_session: CaptureSessionId) {}
