//! Android MediaProjection provider.
//!
//! A fresh authorization token is required for every [`MediaCaptureProvider::start`].
//! Tokens are not cached, and consent is held on the Java side.
//!
//! The encoder half does not exist yet: there is no `VirtualDisplay` and no
//! `MediaCodec` pipeline, so a start that gets past authorization reports
//! [`CaptureError::Unavailable`] rather than handing back a session that would
//! sit in `Running` and never produce a packet.

use super::{
    CaptureCapabilities, CaptureError, CaptureFuture, MediaCaptureProvider, ProviderCaptureRequest,
    ProviderCaptureSession, ProviderEventSink, TrackKind, TrackSet,
};
use std::sync::Arc;

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
        _events: Arc<dyn ProviderEventSink>,
    ) -> CaptureFuture<Result<Box<dyn ProviderCaptureSession>, CaptureError>> {
        // Any screen or system-audio track needs a projection, required or
        // not: an optional track cannot quietly capture without consent.
        let needs_projection = request
            .tracks
            .iter()
            .any(|track| matches!(track.kind, TrackKind::Visual | TrackKind::SystemAudio));
        if needs_projection {
            let authorized = request.authorization.as_ref().is_some_and(|token| {
                token.kind == super::AuthorizationKind::AndroidMediaProjection
            });
            if !authorized {
                return Box::pin(async {
                    Err(CaptureError::AuthorizationRequired {
                        track: TrackKind::Visual,
                    })
                });
            }
        }
        let track = request.tracks.first().map(|track| track.kind);
        Box::pin(async move {
            Err(CaptureError::Unavailable {
                track,
                reason: "android capture has no encoder yet".into(),
            })
        })
    }
}
