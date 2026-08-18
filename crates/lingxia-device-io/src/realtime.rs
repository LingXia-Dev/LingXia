//! Desktop adapter for the platform capture contract.
//!
//! Reuses the sessionless visual engine. Does not require a `Platform`
//! instance and does not enable system-audio or microphone paths.

use crate::engine::{self, EngineFrame};
use crate::geometry::identity_geometry;
use crate::model::CaptureTarget;
use lingxia_platform::capture::{
    CaptureCapabilities, CaptureError, CaptureFuture, CaptureSessionId, CaptureTimestamp,
    EncodedPacket, MediaCaptureProvider, ProviderCaptureRequest, ProviderCaptureSession,
    ProviderEvent, ProviderEventSink, Size, TrackId, TrackKind, VideoCodec, VisualGeometry,
    VisualTarget,
};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

static NEXT_SESSION: AtomicU64 = AtomicU64::new(1);

/// Visual-only desktop provider. Constructed by a host that declared capture.
pub struct DesktopRealtimeProvider {
    session_clock: Instant,
}

impl DesktopRealtimeProvider {
    pub fn new() -> Self {
        Self {
            session_clock: Instant::now(),
        }
    }
}

impl Default for DesktopRealtimeProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl MediaCaptureProvider for DesktopRealtimeProvider {
    fn id(&self) -> &'static str {
        "desktop-visual"
    }

    fn capabilities(&self) -> CaptureCapabilities {
        CaptureCapabilities::visual_only()
    }

    fn start(
        &self,
        request: ProviderCaptureRequest,
        events: Arc<dyn ProviderEventSink>,
    ) -> CaptureFuture<Result<Box<dyn ProviderCaptureSession>, CaptureError>> {
        let clock = self.session_clock;
        Box::pin(async move { start_session(request, events, clock) })
    }
}

fn start_session(
    request: ProviderCaptureRequest,
    events: Arc<dyn ProviderEventSink>,
    clock: Instant,
) -> Result<Box<dyn ProviderCaptureSession>, CaptureError> {
    let (target, fps) = visual_target_and_fps(&request)?;
    Ok(Box::new(DesktopSession::spawn(target, fps, events, clock)?))
}

fn visual_target_and_fps(
    request: &ProviderCaptureRequest,
) -> Result<(CaptureTarget, u32), CaptureError> {
    let visual = request
        .tracks
        .iter()
        .find(|track| track.kind == TrackKind::Visual)
        .ok_or_else(|| CaptureError::InvalidRequest("desktop provider is visual-only".into()))?;
    if request
        .tracks
        .iter()
        .any(|track| track.kind != TrackKind::Visual && track.required)
    {
        return Err(CaptureError::Unavailable {
            track: Some(TrackKind::SystemAudio),
            reason: "desktop realtime provider does not capture audio".into(),
        });
    }
    let target = visual
        .visual
        .as_ref()
        .map(|config| to_capture_target(&config.target))
        .transpose()?
        .unwrap_or(CaptureTarget::Screen);
    let fps = visual
        .visual
        .as_ref()
        .and_then(|config| config.fps)
        .unwrap_or(8)
        .clamp(1, 30);
    Ok((target, fps))
}

fn to_capture_target(target: &VisualTarget) -> Result<CaptureTarget, CaptureError> {
    match target {
        VisualTarget::Screen => Ok(CaptureTarget::Screen),
        VisualTarget::Display { id } => resolve_display(id),
        VisualTarget::Window { id } => Ok(CaptureTarget::Window(id.clone())),
        VisualTarget::Region {
            x,
            y,
            width,
            height,
        } => Ok(CaptureTarget::Region {
            x: *x,
            y: *y,
            w: *width as i32,
            h: *height as i32,
        }),
    }
}

fn resolve_display(id: &str) -> Result<CaptureTarget, CaptureError> {
    if let Some(index) = parse_display_index(id) {
        return Ok(CaptureTarget::Display(index));
    }
    let displays = crate::displays().map_err(|error| CaptureError::Failed {
        reason: error.to_string(),
    })?;
    displays
        .iter()
        .position(|display| display.id == id)
        .map(|position| CaptureTarget::Display(position + 1))
        .ok_or_else(|| CaptureError::InvalidRequest(format!("unknown display '{id}'")))
}

fn parse_display_index(id: &str) -> Option<usize> {
    id.strip_prefix("display-")
        .and_then(|rest| rest.parse().ok())
        .or_else(|| id.parse().ok())
        .filter(|index| *index > 0)
}

struct Shared {
    stop: AtomicBool,
    suspended: AtomicBool,
    keyframe: AtomicBool,
    /// Set by reconfigure. The worker owns `geometry_generation`; this asks it
    /// to open a new generation and re-announce geometry even when the source
    /// rectangle did not move, so packets never carry an unannounced value.
    refresh_geometry: AtomicBool,
    format_generation: AtomicU64,
    geometry_generation: AtomicU64,
    target: Mutex<CaptureTarget>,
    period_ms: AtomicU64,
}

struct DesktopSession {
    shared: Arc<Shared>,
    worker: Mutex<Option<JoinHandle<()>>>,
}

impl DesktopSession {
    fn spawn(
        target: CaptureTarget,
        fps: u32,
        events: Arc<dyn ProviderEventSink>,
        clock: Instant,
    ) -> Result<Self, CaptureError> {
        let shared = Arc::new(Shared {
            stop: AtomicBool::new(false),
            suspended: AtomicBool::new(false),
            keyframe: AtomicBool::new(true),
            refresh_geometry: AtomicBool::new(false),
            format_generation: AtomicU64::new(1),
            geometry_generation: AtomicU64::new(1),
            target: Mutex::new(target),
            period_ms: AtomicU64::new(u64::from(1000 / fps.max(1))),
        });
        let worker_shared = Arc::clone(&shared);
        let handle = thread::Builder::new()
            .name("lingxia-desktop-capture".into())
            .spawn(move || run_loop(events, clock, worker_shared))
            .map_err(|error| CaptureError::Failed {
                reason: format!("could not start desktop capture: {error}"),
            })?;
        Ok(Self {
            shared,
            worker: Mutex::new(Some(handle)),
        })
    }
}

impl ProviderCaptureSession for DesktopSession {
    fn reconfigure(
        &self,
        request: ProviderCaptureRequest,
    ) -> CaptureFuture<Result<(), CaptureError>> {
        let result = (|| {
            let (target, fps) = visual_target_and_fps(&request)?;
            *self
                .shared
                .target
                .lock()
                .unwrap_or_else(|error| error.into_inner()) = target;
            self.shared
                .period_ms
                .store(u64::from(1000 / fps.max(1)), Ordering::SeqCst);
            self.shared.refresh_geometry.store(true, Ordering::SeqCst);
            self.shared.keyframe.store(true, Ordering::SeqCst);
            Ok(())
        })();
        Box::pin(async move { result })
    }

    fn request_keyframe(&self, _track: TrackId) -> Result<(), CaptureError> {
        self.shared.keyframe.store(true, Ordering::SeqCst);
        Ok(())
    }

    fn suspend(&self) -> Result<(), CaptureError> {
        self.shared.suspended.store(true, Ordering::SeqCst);
        Ok(())
    }

    fn resume(&self) -> Result<(), CaptureError> {
        self.shared.suspended.store(false, Ordering::SeqCst);
        Ok(())
    }

    fn stop(&self) {
        self.shared.stop.store(true, Ordering::SeqCst);
        if let Some(worker) = self.worker.lock().unwrap_or_else(|e| e.into_inner()).take() {
            let _ = worker.join();
        }
    }
}

impl Drop for DesktopSession {
    fn drop(&mut self) {
        self.stop();
    }
}

fn run_loop(events: Arc<dyn ProviderEventSink>, clock: Instant, shared: Arc<Shared>) {
    let session_id = CaptureSessionId(NEXT_SESSION.fetch_add(1, Ordering::Relaxed));
    let track_id = TrackId(1);
    let mut last_source: Option<crate::model::Rect> = None;
    let mut last_size: Option<(u32, u32)> = None;
    let mut sequence = 0u64;

    while !shared.stop.load(Ordering::Relaxed) {
        let period = Duration::from_millis(shared.period_ms.load(Ordering::Relaxed).max(1));
        if shared.suspended.load(Ordering::Relaxed) {
            thread::sleep(period);
            continue;
        }
        let target = shared
            .target
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .clone();
        match engine::capture_frame(&target) {
            Ok(frame) => {
                let size = (frame.width, frame.height);
                if last_size != Some(size) {
                    if last_size.is_some() {
                        shared.format_generation.fetch_add(1, Ordering::SeqCst);
                    }
                    last_size = Some(size);
                    events.emit(ProviderEvent::TrackConfigured {
                        track_id,
                        kind: TrackKind::Visual,
                        format_generation: shared.format_generation.load(Ordering::Relaxed),
                        video_codec: Some(VideoCodec::Png),
                        audio_codec: None,
                        sample_rate: None,
                        channels: None,
                        size: Some(Size {
                            width: frame.width,
                            height: frame.height,
                        }),
                    });
                }
                let refresh = shared.refresh_geometry.swap(false, Ordering::SeqCst);
                if refresh || last_source != Some(frame.source) {
                    if last_source.is_some() {
                        shared.geometry_generation.fetch_add(1, Ordering::SeqCst);
                    }
                    last_source = Some(frame.source);
                    events.emit(ProviderEvent::VisualGeometry {
                        track_id,
                        geometry: geometry_from_frame(
                            session_id,
                            shared.geometry_generation.load(Ordering::Relaxed),
                            &frame,
                        ),
                    });
                    shared.keyframe.store(true, Ordering::SeqCst);
                }
                let keyframe = shared.keyframe.swap(false, Ordering::SeqCst);
                if let Ok(payload) = engine::encode_png(frame.width, frame.height, frame.rgba) {
                    sequence += 1;
                    events.emit(ProviderEvent::EncodedPacket(EncodedPacket {
                        track_id,
                        kind: TrackKind::Visual,
                        timestamp: timestamp(clock),
                        sequence,
                        format_generation: shared.format_generation.load(Ordering::Relaxed),
                        geometry_generation: Some(
                            shared.geometry_generation.load(Ordering::Relaxed),
                        ),
                        keyframe,
                        video_codec: Some(VideoCodec::Png),
                        audio_codec: None,
                        payload,
                    }));
                }
            }
            Err(error) => {
                events.emit(ProviderEvent::Failed {
                    track: Some(TrackKind::Visual),
                    error: CaptureError::Failed {
                        reason: error.to_string(),
                    },
                });
                break;
            }
        }
        thread::sleep(period);
    }
    events.emit(ProviderEvent::Stopped);
}

fn geometry_from_frame(
    session_id: CaptureSessionId,
    generation: u64,
    frame: &EngineFrame,
) -> VisualGeometry {
    let mut geometry = identity_geometry(
        session_id,
        generation,
        lingxia_platform::capture::Rect {
            x: frame.source.x,
            y: frame.source.y,
            width: frame.source.w,
            height: frame.source.h,
        },
    );
    geometry.output = Size {
        width: frame.width,
        height: frame.height,
    };
    geometry.content_in_output = lingxia_platform::capture::Rect {
        x: 0,
        y: 0,
        width: frame.width as i32,
        height: frame.height as i32,
    };
    geometry.scale = frame.scale;
    geometry
}

fn timestamp(start: Instant) -> CaptureTimestamp {
    CaptureTimestamp {
        monotonic_nanos: start.elapsed().as_nanos().min(u128::from(u64::MAX)) as u64,
        wall_unix_nanos: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .ok()
            .and_then(|duration| i64::try_from(duration.as_nanos()).ok()),
    }
}

#[cfg(test)]
mod tests {
    use super::parse_display_index;

    #[test]
    fn display_ids_use_the_one_based_suffix() {
        assert_eq!(parse_display_index("display-1"), Some(1));
        assert_eq!(parse_display_index("display-2"), Some(2));
        assert_eq!(parse_display_index("3"), Some(3));
        assert_eq!(parse_display_index("display-0"), None);
        assert_eq!(parse_display_index("main"), None);
    }
}
