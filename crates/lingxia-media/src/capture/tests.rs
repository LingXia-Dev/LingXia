use super::*;
use lingxia_platform::capture::{
    CaptureFuture, CaptureSessionId, ProviderCaptureRequest, Rect, VisualTrackConfig,
};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

struct MockProvider {
    id: &'static str,
    capabilities: CaptureCapabilities,
    start_result: Mutex<StartBehavior>,
    events: Mutex<Option<Arc<dyn ProviderEventSink>>>,
    session: Arc<MockSession>,
}

enum StartBehavior {
    Ok,
    AuthorizationRequired(TrackKind),
    #[allow(dead_code)]
    Fail(CaptureError),
}

struct MockSession {
    keyframes: AtomicU64,
    stopped: AtomicBool,
    suspended: AtomicBool,
    last_reconfigure: Mutex<Option<ProviderCaptureRequest>>,
}

impl Default for MockSession {
    fn default() -> Self {
        Self {
            keyframes: AtomicU64::new(0),
            stopped: AtomicBool::new(false),
            suspended: AtomicBool::new(false),
            last_reconfigure: Mutex::new(None),
        }
    }
}

impl MockProvider {
    fn visual() -> Arc<Self> {
        Arc::new(Self {
            id: "mock-visual",
            capabilities: CaptureCapabilities::visual_only(),
            start_result: Mutex::new(StartBehavior::Ok),
            events: Mutex::new(None),
            session: Arc::new(MockSession::default()),
        })
    }

    fn microphone() -> Arc<Self> {
        Arc::new(Self {
            id: "mock-mic",
            capabilities: CaptureCapabilities {
                combinations: vec![TrackSet {
                    tracks: vec![TrackKind::Microphone],
                    coupled_clock: false,
                }],
            },
            start_result: Mutex::new(StartBehavior::Ok),
            events: Mutex::new(None),
            session: Arc::new(MockSession::default()),
        })
    }

    fn audio_and_visual() -> Arc<Self> {
        Arc::new(Self {
            id: "mock-av",
            capabilities: CaptureCapabilities {
                combinations: vec![
                    TrackSet {
                        tracks: vec![TrackKind::Visual, TrackKind::SystemAudio],
                        coupled_clock: true,
                    },
                    TrackSet {
                        tracks: vec![TrackKind::Microphone],
                        coupled_clock: false,
                    },
                ],
            },
            start_result: Mutex::new(StartBehavior::Ok),
            events: Mutex::new(None),
            session: Arc::new(MockSession::default()),
        })
    }

    fn emit(&self, event: ProviderEvent) {
        if let Some(sink) = self.events.lock().unwrap().as_ref() {
            sink.emit(event);
        }
    }
}

impl MediaCaptureProvider for MockProvider {
    fn id(&self) -> &'static str {
        self.id
    }

    fn capabilities(&self) -> CaptureCapabilities {
        self.capabilities.clone()
    }

    fn start(
        &self,
        _request: ProviderCaptureRequest,
        events: Arc<dyn ProviderEventSink>,
    ) -> CaptureFuture<Result<Box<dyn ProviderCaptureSession>, CaptureError>> {
        *self.events.lock().unwrap() = Some(events);
        let result = match &*self.start_result.lock().unwrap() {
            StartBehavior::Ok => {
                Ok(Box::new(Handle(Arc::clone(&self.session))) as Box<dyn ProviderCaptureSession>)
            }
            StartBehavior::AuthorizationRequired(track) => {
                Err(CaptureError::AuthorizationRequired { track: *track })
            }
            StartBehavior::Fail(error) => Err(error.clone()),
        };
        Box::pin(async move { result })
    }
}

struct Handle(Arc<MockSession>);

impl ProviderCaptureSession for Handle {
    fn reconfigure(
        &self,
        request: ProviderCaptureRequest,
    ) -> CaptureFuture<Result<(), CaptureError>> {
        *self.0.last_reconfigure.lock().unwrap() = Some(request);
        Box::pin(async { Ok(()) })
    }

    fn request_keyframe(&self, _track: TrackId) -> Result<(), CaptureError> {
        self.0.keyframes.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }

    fn suspend(&self) -> Result<(), CaptureError> {
        self.0.suspended.store(true, Ordering::SeqCst);
        Ok(())
    }

    fn resume(&self) -> Result<(), CaptureError> {
        self.0.suspended.store(false, Ordering::SeqCst);
        Ok(())
    }

    fn stop(&self) {
        self.0.stopped.store(true, Ordering::SeqCst);
    }
}

fn visual_request() -> CaptureRequest {
    CaptureRequest {
        tracks: vec![TrackRequest {
            kind: TrackKind::Visual,
            required: true,
            visual: Some(VisualTrackConfig {
                target: VisualTarget::Screen,
                max_size: None,
                fps: Some(15),
                bitrate_kbps: None,
                codec: Some(VideoCodec::Png),
            }),
            audio: None,
        }],
        authorization: None,
    }
}

fn geometry(generation: u64) -> VisualGeometry {
    VisualGeometry {
        session_id: CaptureSessionId(1),
        generation,
        source: Rect {
            x: 0,
            y: 0,
            width: 100,
            height: 80,
        },
        target_local: Rect {
            x: 0,
            y: 0,
            width: 100,
            height: 80,
        },
        output: Size {
            width: 100,
            height: 80,
        },
        content_in_output: Rect {
            x: 0,
            y: 0,
            width: 100,
            height: 80,
        },
        scale: 1.0,
        rotation_degrees: 0,
        mirrored: false,
    }
}

fn packet(
    generation: u64,
    geometry_generation: u64,
    sequence: u64,
    keyframe: bool,
) -> EncodedPacket {
    EncodedPacket {
        track_id: TrackId(1),
        kind: TrackKind::Visual,
        timestamp: CaptureTimestamp {
            monotonic_nanos: sequence * 16_000_000,
            wall_unix_nanos: None,
        },
        sequence,
        format_generation: generation,
        geometry_generation: Some(geometry_generation),
        keyframe,
        video_codec: Some(VideoCodec::Png),
        audio_codec: None,
        payload: vec![sequence as u8],
    }
}

fn collect_until(
    rx: &Receiver<CaptureEvent>,
    pred: impl Fn(&CaptureEvent) -> bool,
) -> Vec<CaptureEvent> {
    let mut events = Vec::new();
    let deadline = Instant::now() + Duration::from_secs(1);
    while Instant::now() < deadline {
        match rx.recv_timeout(Duration::from_millis(20)) {
            Ok(event) => {
                let done = pred(&event);
                events.push(event);
                if done {
                    break;
                }
            }
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {}
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break,
        }
    }
    events
}

fn configured(track_id: TrackId, generation: u64) -> ProviderEvent {
    ProviderEvent::TrackConfigured {
        track_id,
        kind: TrackKind::Visual,
        format_generation: generation,
        video_codec: Some(VideoCodec::Png),
        audio_codec: None,
        sample_rate: None,
        channels: None,
        size: Some(Size {
            width: 100,
            height: 80,
        }),
    }
}

#[test]
fn empty_provider_set_is_a_composition_error() {
    let error = match Pipeline::start(CaptureProviderSet::new([]), visual_request()) {
        Err(error) => error,
        Ok(_) => panic!("empty provider set must fail"),
    };
    assert!(matches!(error, CaptureError::Composition(_)));
}

#[test]
fn missing_required_track_is_a_composition_error() {
    let provider = MockProvider::visual();
    let request = CaptureRequest {
        tracks: vec![TrackRequest {
            kind: TrackKind::Microphone,
            required: true,
            visual: None,
            audio: None,
        }],
        authorization: None,
    };
    let error = match Pipeline::start(
        CaptureProviderSet::new([provider as Arc<dyn MediaCaptureProvider>]),
        request,
    ) {
        Err(error) => error,
        Ok(_) => panic!("missing required track must fail"),
    };
    assert!(matches!(error, CaptureError::Composition(_)));
}

#[test]
fn optional_track_may_be_omitted() {
    let provider = MockProvider::visual();
    let request = CaptureRequest {
        tracks: vec![
            TrackRequest {
                kind: TrackKind::Visual,
                required: true,
                visual: Some(VisualTrackConfig {
                    target: VisualTarget::Screen,
                    max_size: None,
                    fps: None,
                    bitrate_kbps: None,
                    codec: None,
                }),
                audio: None,
            },
            TrackRequest {
                kind: TrackKind::Microphone,
                required: false,
                visual: None,
                audio: None,
            },
        ],
        authorization: None,
    };
    let pipeline = Pipeline::start(
        CaptureProviderSet::new([provider as Arc<dyn MediaCaptureProvider>]),
        request,
    )
    .unwrap();
    assert_eq!(pipeline.state(), PipelineState::Running);
    pipeline.stop();
}

#[test]
fn authorization_required_starts_authorizing() {
    let provider = MockProvider::visual();
    *provider.start_result.lock().unwrap() =
        StartBehavior::AuthorizationRequired(TrackKind::Visual);
    let pipeline = Pipeline::start(
        CaptureProviderSet::new([Arc::clone(&provider) as Arc<dyn MediaCaptureProvider>]),
        visual_request(),
    )
    .unwrap();
    assert_eq!(pipeline.state(), PipelineState::Authorizing);
    *provider.start_result.lock().unwrap() = StartBehavior::Ok;
    pipeline
        .provide_authorization(CaptureAuthorization {
            kind: AuthorizationKind::AndroidMediaProjection,
            payload: vec![1],
        })
        .unwrap();
    assert_eq!(pipeline.state(), PipelineState::Running);
    pipeline.stop();
}

#[test]
fn config_and_geometry_must_precede_the_first_packet() {
    let provider = MockProvider::visual();
    let pipeline = Pipeline::start(
        CaptureProviderSet::new([Arc::clone(&provider) as _]),
        visual_request(),
    )
    .unwrap();
    let rx = pipeline.subscribe();

    provider.emit(ProviderEvent::EncodedPacket(packet(1, 1, 1, true)));
    let early = collect_until(&rx, |_| false);
    assert!(
        !early
            .iter()
            .any(|event| matches!(event, CaptureEvent::EncodedPacket(_))),
        "packet before config must be dropped: {early:?}"
    );

    provider.emit(configured(TrackId(1), 1));
    provider.emit(ProviderEvent::VisualGeometry {
        track_id: TrackId(1),
        geometry: geometry(1),
    });
    provider.emit(ProviderEvent::EncodedPacket(packet(1, 1, 2, true)));
    let events = collect_until(&rx, |event| matches!(event, CaptureEvent::EncodedPacket(_)));
    assert!(
        events
            .iter()
            .any(|event| matches!(event, CaptureEvent::EncodedPacket(p) if p.sequence == 2))
    );
    pipeline.stop();
}

#[test]
fn geometry_may_arrive_before_track_configured() {
    let provider = MockProvider::visual();
    let pipeline = Pipeline::start(
        CaptureProviderSet::new([Arc::clone(&provider) as _]),
        visual_request(),
    )
    .unwrap();
    let rx = pipeline.subscribe();
    provider.emit(ProviderEvent::VisualGeometry {
        track_id: TrackId(1),
        geometry: geometry(1),
    });
    provider.emit(configured(TrackId(1), 1));
    provider.emit(ProviderEvent::EncodedPacket(packet(1, 1, 2, true)));
    let events = collect_until(&rx, |event| matches!(event, CaptureEvent::EncodedPacket(_)));
    assert!(
        events
            .iter()
            .any(|event| matches!(event, CaptureEvent::EncodedPacket(p) if p.sequence == 2))
    );
    pipeline.stop();
}

#[test]
fn reconfigure_routes_tracks_to_the_owning_session() {
    let visual = MockProvider::visual();
    let microphone = MockProvider::microphone();
    let request = CaptureRequest {
        tracks: vec![
            TrackRequest {
                kind: TrackKind::Visual,
                required: true,
                visual: Some(VisualTrackConfig {
                    target: VisualTarget::Screen,
                    max_size: None,
                    fps: Some(15),
                    bitrate_kbps: None,
                    codec: Some(VideoCodec::Png),
                }),
                audio: None,
            },
            TrackRequest {
                kind: TrackKind::Microphone,
                required: true,
                visual: None,
                audio: None,
            },
        ],
        authorization: None,
    };
    let pipeline = Pipeline::start(
        CaptureProviderSet::new([
            Arc::clone(&visual) as Arc<dyn MediaCaptureProvider>,
            Arc::clone(&microphone) as Arc<dyn MediaCaptureProvider>,
        ]),
        request.clone(),
    )
    .unwrap();
    pipeline.reconfigure(request).unwrap();
    let visual_kinds: Vec<_> = visual
        .session
        .last_reconfigure
        .lock()
        .unwrap()
        .as_ref()
        .expect("visual session must be reconfigured")
        .tracks
        .iter()
        .map(|track| track.kind)
        .collect();
    let mic_kinds: Vec<_> = microphone
        .session
        .last_reconfigure
        .lock()
        .unwrap()
        .as_ref()
        .expect("microphone session must be reconfigured")
        .tracks
        .iter()
        .map(|track| track.kind)
        .collect();
    assert_eq!(visual_kinds, vec![TrackKind::Visual]);
    assert_eq!(mic_kinds, vec![TrackKind::Microphone]);
    pipeline.stop();
}

#[test]
fn old_generation_packets_are_dropped_after_reconfigure() {
    let provider = MockProvider::visual();
    let pipeline = Pipeline::start(
        CaptureProviderSet::new([Arc::clone(&provider) as _]),
        visual_request(),
    )
    .unwrap();
    let rx = pipeline.subscribe();
    provider.emit(configured(TrackId(1), 1));
    provider.emit(ProviderEvent::VisualGeometry {
        track_id: TrackId(1),
        geometry: geometry(1),
    });
    provider.emit(configured(TrackId(1), 2));
    provider.emit(ProviderEvent::VisualGeometry {
        track_id: TrackId(1),
        geometry: geometry(2),
    });
    provider.emit(ProviderEvent::EncodedPacket(packet(1, 1, 3, true)));
    provider.emit(ProviderEvent::EncodedPacket(packet(2, 2, 4, true)));
    let events = collect_until(
        &rx,
        |event| matches!(event, CaptureEvent::EncodedPacket(p) if p.sequence == 4),
    );
    let sequences: Vec<_> = events
        .iter()
        .filter_map(|event| match event {
            CaptureEvent::EncodedPacket(packet) => Some(packet.sequence),
            _ => None,
        })
        .collect();
    assert_eq!(sequences, vec![4]);
    pipeline.stop();
}

#[test]
fn video_overflow_requests_a_keyframe_and_audio_overflow_is_explicit() {
    let provider = MockProvider::audio_and_visual();
    let request = CaptureRequest {
        tracks: vec![
            TrackRequest {
                kind: TrackKind::Visual,
                required: true,
                visual: Some(VisualTrackConfig {
                    target: VisualTarget::Screen,
                    max_size: None,
                    fps: None,
                    bitrate_kbps: None,
                    codec: None,
                }),
                audio: None,
            },
            TrackRequest {
                kind: TrackKind::SystemAudio,
                required: true,
                visual: None,
                audio: None,
            },
        ],
        authorization: None,
    };
    let pipeline = Pipeline::start(
        CaptureProviderSet::new([Arc::clone(&provider) as _]),
        request,
    )
    .unwrap();
    let rx = pipeline.subscribe();
    provider.emit(configured(TrackId(1), 1));
    provider.emit(ProviderEvent::VisualGeometry {
        track_id: TrackId(1),
        geometry: geometry(1),
    });
    provider.emit(ProviderEvent::TrackConfigured {
        track_id: TrackId(2),
        kind: TrackKind::SystemAudio,
        format_generation: 1,
        video_codec: None,
        audio_codec: Some(AudioCodec::Opus),
        sample_rate: Some(48_000),
        channels: Some(2),
        size: None,
    });

    for sequence in 0..12u64 {
        provider.emit(ProviderEvent::EncodedPacket(EncodedPacket {
            track_id: TrackId(1),
            kind: TrackKind::Visual,
            timestamp: CaptureTimestamp {
                monotonic_nanos: sequence,
                wall_unix_nanos: None,
            },
            sequence,
            format_generation: 1,
            geometry_generation: Some(1),
            keyframe: sequence == 0,
            video_codec: Some(VideoCodec::Png),
            audio_codec: None,
            payload: vec![0; 64],
        }));
    }
    let _ = collect_until(&rx, |_| false);
    assert!(
        provider.session.keyframes.load(Ordering::SeqCst) > 0,
        "video overflow must request a keyframe"
    );

    for sequence in 0..40u64 {
        provider.emit(ProviderEvent::EncodedPacket(EncodedPacket {
            track_id: TrackId(2),
            kind: TrackKind::SystemAudio,
            timestamp: CaptureTimestamp {
                monotonic_nanos: sequence * 10_000_000,
                wall_unix_nanos: None,
            },
            sequence,
            format_generation: 1,
            geometry_generation: None,
            keyframe: true,
            video_codec: None,
            audio_codec: Some(AudioCodec::Opus),
            payload: vec![1; 32],
        }));
    }
    let events = collect_until(&rx, |event| {
        matches!(
            event,
            CaptureEvent::Discontinuity {
                kind: TrackKind::SystemAudio,
                reason: DiscontinuityReason::Overflow,
                ..
            }
        )
    });
    assert!(
        events.iter().any(|event| matches!(
            event,
            CaptureEvent::Discontinuity {
                kind: TrackKind::SystemAudio,
                reason: DiscontinuityReason::Overflow,
                ..
            }
        )),
        "audio overflow must be an explicit discontinuity: {events:?}"
    );
    pipeline.stop();
}

#[test]
fn suspend_resume_and_revocation_follow_the_lifecycle() {
    let provider = MockProvider::visual();
    let pipeline = Pipeline::start(
        CaptureProviderSet::new([Arc::clone(&provider) as _]),
        visual_request(),
    )
    .unwrap();
    assert_eq!(pipeline.state(), PipelineState::Running);
    pipeline.suspend().unwrap();
    assert_eq!(pipeline.state(), PipelineState::Suspended);
    pipeline.resume().unwrap();
    assert_eq!(pipeline.state(), PipelineState::Running);
    provider.emit(ProviderEvent::AuthorizationRevoked {
        track: TrackKind::Visual,
    });
    let deadline = Instant::now() + Duration::from_secs(1);
    while pipeline.state() != PipelineState::AuthorizationRevoked && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(10));
    }
    assert_eq!(pipeline.state(), PipelineState::AuthorizationRevoked);
    pipeline.stop();
    assert_eq!(pipeline.state(), PipelineState::Stopped);
}

#[test]
fn wall_clock_is_metadata_only() {
    let clock = SessionClock::new();
    let first = clock.now();
    std::thread::sleep(Duration::from_millis(2));
    let second = clock.now();
    assert!(second.monotonic_nanos >= first.monotonic_nanos);
    assert!(first.wall_unix_nanos.is_some());
}
