//! Realtime multi-track capture pipeline.
//!
//! Providers are passed in explicitly. Absence is a composition error before
//! the lifecycle starts — there is no always-unsupported placeholder.

use std::collections::{HashMap, VecDeque};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

pub use lingxia_platform::capture::{
    AudioCodec, AudioTrackConfig, AuthorizationKind, CaptureAuthorization, CaptureCapabilities,
    CaptureError, CaptureSessionId, CaptureTimestamp, DiscontinuityReason, EncodedPacket,
    MediaCaptureProvider, ProviderCaptureRequest, ProviderCaptureSession, ProviderEvent,
    ProviderEventSink, Size, TrackId, TrackKind, TrackRequest, TrackSet, VideoCodec,
    VisualGeometry, VisualTarget, VisualTrackConfig,
};

/// Explicit, instance-local set of providers. Not a process-global registry.
#[derive(Clone, Default)]
pub struct CaptureProviderSet {
    providers: Vec<Arc<dyn MediaCaptureProvider>>,
}

impl CaptureProviderSet {
    pub fn new(providers: impl IntoIterator<Item = Arc<dyn MediaCaptureProvider>>) -> Self {
        Self {
            providers: providers.into_iter().collect(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.providers.is_empty()
    }

    pub fn len(&self) -> usize {
        self.providers.len()
    }

    pub fn iter(&self) -> impl Iterator<Item = &Arc<dyn MediaCaptureProvider>> {
        self.providers.iter()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PipelineState {
    Authorizing,
    Starting,
    Running,
    Suspended,
    AuthorizationRevoked,
    Failed,
    Stopped,
}

impl PipelineState {
    pub fn is_active(self) -> bool {
        matches!(
            self,
            Self::Authorizing | Self::Starting | Self::Running | Self::Suspended
        )
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct CaptureRequest {
    pub tracks: Vec<TrackRequest>,
    pub authorization: Option<CaptureAuthorization>,
}

#[derive(Debug, Clone)]
pub enum CaptureEvent {
    State(PipelineState),
    TrackConfigured {
        track_id: TrackId,
        kind: TrackKind,
        format_generation: u64,
        video_codec: Option<VideoCodec>,
        audio_codec: Option<AudioCodec>,
        sample_rate: Option<u32>,
        channels: Option<u8>,
        size: Option<Size>,
    },
    VisualGeometry {
        track_id: TrackId,
        geometry: VisualGeometry,
    },
    EncodedPacket(EncodedPacket),
    Discontinuity {
        track_id: TrackId,
        kind: TrackKind,
        reason: DiscontinuityReason,
        format_generation: u64,
        timestamp: CaptureTimestamp,
    },
    AuthorizationRevoked {
        track: TrackKind,
    },
    Failed {
        track: Option<TrackKind>,
        error: CaptureError,
    },
}

pub(crate) struct SessionClock {
    start: Instant,
}

impl SessionClock {
    fn new() -> Self {
        Self {
            start: Instant::now(),
        }
    }

    fn now(&self) -> CaptureTimestamp {
        let monotonic_nanos = self.start.elapsed().as_nanos().min(u128::from(u64::MAX)) as u64;
        let wall_unix_nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .ok()
            .and_then(|d| i64::try_from(d.as_nanos()).ok());
        CaptureTimestamp {
            monotonic_nanos,
            wall_unix_nanos,
        }
    }
}

struct TrackQueue {
    kind: TrackKind,
    packets: VecDeque<EncodedPacket>,
    bytes: usize,
    waiting_keyframe: bool,
    max_packets: usize,
    max_bytes: usize,
    max_audio_duration: Duration,
}

impl TrackQueue {
    fn video() -> Self {
        Self {
            kind: TrackKind::Visual,
            packets: VecDeque::new(),
            bytes: 0,
            waiting_keyframe: false,
            max_packets: 8,
            max_bytes: 8 * 1024 * 1024,
            max_audio_duration: Duration::from_millis(250),
        }
    }

    fn audio(kind: TrackKind) -> Self {
        Self {
            kind,
            packets: VecDeque::new(),
            bytes: 0,
            waiting_keyframe: false,
            max_packets: 32,
            max_bytes: 512 * 1024,
            max_audio_duration: Duration::from_millis(250),
        }
    }

    fn push(
        &mut self,
        packet: EncodedPacket,
        clock: &SessionClock,
    ) -> Result<Option<DiscontinuityReason>, EncodedPacket> {
        match self.kind {
            TrackKind::Visual => self.push_video(packet),
            TrackKind::SystemAudio | TrackKind::Microphone => self.push_audio(packet, clock),
        }
    }

    fn push_video(
        &mut self,
        packet: EncodedPacket,
    ) -> Result<Option<DiscontinuityReason>, EncodedPacket> {
        if self.waiting_keyframe && !packet.keyframe {
            return Err(packet);
        }
        let overflow = self.packets.len() >= self.max_packets
            || self.bytes.saturating_add(packet.payload.len()) > self.max_bytes;
        if overflow {
            self.packets.clear();
            self.bytes = 0;
            if packet.keyframe {
                self.waiting_keyframe = false;
                self.enqueue(packet);
                return Ok(Some(DiscontinuityReason::Overflow));
            }
            self.waiting_keyframe = true;
            return Err(packet);
        }
        self.waiting_keyframe = false;
        self.enqueue(packet);
        Ok(None)
    }

    fn push_audio(
        &mut self,
        packet: EncodedPacket,
        _clock: &SessionClock,
    ) -> Result<Option<DiscontinuityReason>, EncodedPacket> {
        let overflow = self.packets.len() >= self.max_packets
            || self.bytes.saturating_add(packet.payload.len()) > self.max_bytes
            || audio_span(&self.packets, &packet) > self.max_audio_duration;
        if overflow {
            self.packets.clear();
            self.bytes = 0;
            self.enqueue(packet);
            return Ok(Some(DiscontinuityReason::Overflow));
        }
        self.enqueue(packet);
        Ok(None)
    }

    fn enqueue(&mut self, packet: EncodedPacket) {
        self.bytes = self.bytes.saturating_add(packet.payload.len());
        self.packets.push_back(packet);
    }

    fn pop(&mut self) -> Option<EncodedPacket> {
        let packet = self.packets.pop_front()?;
        self.bytes = self.bytes.saturating_sub(packet.payload.len());
        Some(packet)
    }
}

fn audio_span(queued: &VecDeque<EncodedPacket>, next: &EncodedPacket) -> Duration {
    let Some(first) = queued.front() else {
        return Duration::ZERO;
    };
    let start = first.timestamp.monotonic_nanos;
    let end = next.timestamp.monotonic_nanos.max(start);
    Duration::from_nanos(end.saturating_sub(start))
}

struct TrackRuntime {
    kind: TrackKind,
    format_generation: u64,
    geometry_generation: u64,
    configured: bool,
    geometry_ready: bool,
    queue: TrackQueue,
}

impl TrackRuntime {
    fn new(kind: TrackKind) -> Self {
        Self {
            kind,
            format_generation: 0,
            geometry_generation: 0,
            configured: false,
            geometry_ready: kind != TrackKind::Visual,
            queue: match kind {
                TrackKind::Visual => TrackQueue::video(),
                other => TrackQueue::audio(other),
            },
        }
    }
}

struct LiveSession {
    kinds: Vec<TrackKind>,
    session: Box<dyn ProviderCaptureSession>,
}

struct Inner {
    state: PipelineState,
    clock: SessionClock,
    tracks: HashMap<TrackId, TrackRuntime>,
    subscribers: Vec<Sender<CaptureEvent>>,
    sessions: Vec<LiveSession>,
    request: CaptureRequest,
    unauthorized: Vec<Assignment>,
    event_tx: Sender<ProviderEvent>,
}

impl Inner {
    fn emit(&mut self, event: CaptureEvent) {
        self.subscribers.retain(|tx| tx.send(event.clone()).is_ok());
    }

    fn set_state(&mut self, state: PipelineState) {
        if self.state == state {
            return;
        }
        self.state = state;
        self.emit(CaptureEvent::State(state));
    }
}

struct Sink {
    tx: Sender<ProviderEvent>,
}

impl ProviderEventSink for Sink {
    fn emit(&self, event: ProviderEvent) {
        let _ = self.tx.send(event);
    }
}

/// Authorization-aware multi-track capture session.
pub struct Pipeline {
    inner: Arc<Mutex<Inner>>,
    stop: Arc<AtomicBool>,
    worker: Mutex<Option<JoinHandle<()>>>,
}

impl Pipeline {
    /// Compose providers and begin the lifecycle. An empty set, or a required
    /// track no provider can start, fails before `Authorizing`.
    pub fn start(
        providers: CaptureProviderSet,
        request: CaptureRequest,
    ) -> Result<Self, CaptureError> {
        if providers.is_empty() {
            return Err(CaptureError::Composition(
                "no capture providers were supplied".into(),
            ));
        }
        if request.tracks.is_empty() {
            return Err(CaptureError::InvalidRequest(
                "at least one track is required".into(),
            ));
        }

        let assignments = assign_providers(&providers, &request)?;
        let (event_tx, event_rx) = mpsc::channel();
        let sink: Arc<dyn ProviderEventSink> = Arc::new(Sink {
            tx: event_tx.clone(),
        });

        let mut sessions = Vec::new();
        let tracks = HashMap::new();
        let mut unauthorized = Vec::new();

        for assignment in assignments {
            let provider_request = ProviderCaptureRequest {
                tracks: assignment.tracks.clone(),
                authorization: request.authorization.clone(),
            };
            match block_on(
                assignment
                    .provider
                    .start(provider_request, Arc::clone(&sink)),
            ) {
                Ok(session) => sessions.push(LiveSession {
                    kinds: assignment.tracks.iter().map(|track| track.kind).collect(),
                    session,
                }),
                Err(CaptureError::AuthorizationRequired { .. }) => {
                    unauthorized.push(assignment);
                }
                Err(error) => {
                    if assignment.tracks.iter().any(|track| track.required) {
                        return Err(error);
                    }
                }
            }
        }
        let authorizing = !unauthorized.is_empty();

        if sessions.is_empty() && !authorizing {
            return Err(CaptureError::Composition(
                "no provider could start the requested tracks".into(),
            ));
        }

        // Constructed directly in the steady state: flipping to Running after
        // the worker starts could overwrite a Failed the worker already set.
        let initial = if authorizing {
            PipelineState::Authorizing
        } else {
            PipelineState::Running
        };
        let inner = Arc::new(Mutex::new(Inner {
            state: initial,
            clock: SessionClock::new(),
            tracks,
            subscribers: Vec::new(),
            sessions,
            request,
            unauthorized,
            event_tx,
        }));
        let stop = Arc::new(AtomicBool::new(false));
        let worker_inner = Arc::clone(&inner);
        let worker_stop = Arc::clone(&stop);
        let worker = thread::Builder::new()
            .name("lingxia-capture".into())
            .spawn(move || pump(worker_inner, event_rx, worker_stop))
            .map_err(|error| CaptureError::Failed {
                reason: format!("could not start capture worker: {error}"),
            })?;

        Ok(Self {
            inner,
            stop,
            worker: Mutex::new(Some(worker)),
        })
    }

    pub fn state(&self) -> PipelineState {
        lock(&self.inner).state
    }

    pub fn subscribe(&self) -> Receiver<CaptureEvent> {
        let (tx, rx) = mpsc::channel();
        let mut inner = lock(&self.inner);
        let _ = tx.send(CaptureEvent::State(inner.state));
        inner.subscribers.push(tx);
        rx
    }

    pub fn reconfigure(&self, request: CaptureRequest) -> Result<(), CaptureError> {
        let inner = lock(&self.inner);
        if !inner.state.is_active() {
            return Err(CaptureError::Failed {
                reason: format!("cannot reconfigure from {:?}", inner.state),
            });
        }
        let results: Vec<_> = inner
            .sessions
            .iter()
            .filter_map(|live| {
                let tracks: Vec<_> = request
                    .tracks
                    .iter()
                    .filter(|track| live.kinds.contains(&track.kind))
                    .cloned()
                    .collect();
                if tracks.is_empty() {
                    return None;
                }
                Some(block_on(live.session.reconfigure(ProviderCaptureRequest {
                    tracks,
                    authorization: request.authorization.clone(),
                })))
            })
            .collect();
        drop(inner);
        lock(&self.inner).request = request;
        for result in results {
            result?;
        }
        Ok(())
    }

    /// Supply a fresh authorization token and retry providers that needed one.
    pub fn provide_authorization(
        &self,
        authorization: CaptureAuthorization,
    ) -> Result<(), CaptureError> {
        let (pending, sink) = {
            let mut inner = lock(&self.inner);
            if inner.state != PipelineState::Authorizing {
                return Err(CaptureError::Failed {
                    reason: format!("cannot authorize from {:?}", inner.state),
                });
            }
            inner.request.authorization = Some(authorization.clone());
            (
                std::mem::take(&mut inner.unauthorized),
                Arc::new(Sink {
                    tx: inner.event_tx.clone(),
                }) as Arc<dyn ProviderEventSink>,
            )
        };
        let mut started = Vec::new();
        let mut still_unauthorized = Vec::new();
        for assignment in pending {
            let provider_request = ProviderCaptureRequest {
                tracks: assignment.tracks.clone(),
                authorization: Some(authorization.clone()),
            };
            match block_on(
                assignment
                    .provider
                    .start(provider_request, Arc::clone(&sink)),
            ) {
                Ok(session) => started.push(LiveSession {
                    kinds: assignment.tracks.iter().map(|track| track.kind).collect(),
                    session,
                }),
                Err(CaptureError::AuthorizationRequired { .. }) => {
                    still_unauthorized.push(assignment);
                }
                Err(error) if assignment.tracks.iter().any(|track| track.required) => {
                    return Err(error);
                }
                Err(_) => {}
            }
        }
        let mut inner = lock(&self.inner);
        inner.sessions.extend(started);
        inner.unauthorized = still_unauthorized;
        if inner.unauthorized.is_empty() && !inner.sessions.is_empty() {
            inner.set_state(PipelineState::Running);
        }
        Ok(())
    }

    pub fn request_keyframe(&self, track: TrackId) -> Result<(), CaptureError> {
        let inner = lock(&self.inner);
        for live in &inner.sessions {
            live.session.request_keyframe(track)?;
        }
        Ok(())
    }

    pub fn suspend(&self) -> Result<(), CaptureError> {
        let mut inner = lock(&self.inner);
        if inner.state != PipelineState::Running {
            return Err(CaptureError::Failed {
                reason: format!("cannot suspend from {:?}", inner.state),
            });
        }
        for live in &inner.sessions {
            live.session.suspend()?;
        }
        inner.set_state(PipelineState::Suspended);
        Ok(())
    }

    pub fn resume(&self) -> Result<(), CaptureError> {
        let mut inner = lock(&self.inner);
        if inner.state != PipelineState::Suspended {
            return Err(CaptureError::Failed {
                reason: format!("cannot resume from {:?}", inner.state),
            });
        }
        for live in &inner.sessions {
            live.session.resume()?;
        }
        inner.set_state(PipelineState::Running);
        Ok(())
    }

    pub fn stop(&self) {
        self.stop.store(true, Ordering::SeqCst);
        let mut inner = lock(&self.inner);
        for live in &inner.sessions {
            live.session.stop();
        }
        inner.set_state(PipelineState::Stopped);
        drop(inner);
        if let Some(worker) = self.worker.lock().unwrap_or_else(|e| e.into_inner()).take() {
            let _ = worker.join();
        }
    }
}

impl Drop for Pipeline {
    fn drop(&mut self) {
        self.stop();
    }
}

#[derive(Clone)]
struct Assignment {
    provider: Arc<dyn MediaCaptureProvider>,
    tracks: Vec<TrackRequest>,
}

fn assign_providers(
    providers: &CaptureProviderSet,
    request: &CaptureRequest,
) -> Result<Vec<Assignment>, CaptureError> {
    let mut remaining: Vec<TrackRequest> = request.tracks.clone();
    let mut assignments = Vec::new();

    for provider in providers.iter() {
        if remaining.is_empty() {
            break;
        }
        let caps = provider.capabilities();
        let mut sets = caps.combinations.clone();
        sets.sort_by_key(|set| std::cmp::Reverse(set.tracks.len()));
        // One assignment per matched set: a `TrackSet` is what the provider
        // can start together on one native session, so tracks from different
        // sets must become separate sessions.
        for set in sets {
            let remaining_kinds: Vec<_> = remaining.iter().map(|track| track.kind).collect();
            if !set.tracks.iter().all(|kind| remaining_kinds.contains(kind)) {
                continue;
            }
            let mut taken = Vec::new();
            remaining.retain(|track| {
                if set.tracks.contains(&track.kind) {
                    taken.push(track.clone());
                    false
                } else {
                    true
                }
            });
            if !taken.is_empty() {
                assignments.push(Assignment {
                    provider: Arc::clone(provider),
                    tracks: taken,
                });
            }
        }
    }

    let missing: Vec<_> = remaining
        .iter()
        .filter(|track| track.required)
        .map(|track| track.kind)
        .collect();
    if !missing.is_empty() {
        return Err(CaptureError::Composition(format!(
            "no provider can start required tracks {missing:?}"
        )));
    }
    Ok(assignments)
}

fn pump(inner: Arc<Mutex<Inner>>, rx: Receiver<ProviderEvent>, stop: Arc<AtomicBool>) {
    while !stop.load(Ordering::Relaxed) {
        match rx.recv_timeout(Duration::from_millis(20)) {
            Ok(event) => handle_provider_event(&inner, event),
            Err(RecvTimeoutError::Timeout) => drain_queues(&inner),
            Err(RecvTimeoutError::Disconnected) => break,
        }
    }
}

fn handle_provider_event(inner: &Arc<Mutex<Inner>>, event: ProviderEvent) {
    let mut inner = lock(inner);
    if !inner.state.is_active() {
        return;
    }
    match event {
        ProviderEvent::TrackConfigured {
            track_id,
            kind,
            format_generation,
            video_codec,
            audio_codec,
            sample_rate,
            channels,
            size,
        } => {
            let runtime = inner
                .tracks
                .entry(track_id)
                .or_insert_with(|| TrackRuntime::new(kind));
            runtime.format_generation = format_generation;
            runtime.configured = true;
            inner.emit(CaptureEvent::TrackConfigured {
                track_id,
                kind,
                format_generation,
                video_codec,
                audio_codec,
                sample_rate,
                channels,
                size,
            });
        }
        ProviderEvent::VisualGeometry { track_id, geometry } => {
            let runtime = inner
                .tracks
                .entry(track_id)
                .or_insert_with(|| TrackRuntime::new(TrackKind::Visual));
            runtime.geometry_generation = geometry.generation;
            runtime.geometry_ready = true;
            inner.emit(CaptureEvent::VisualGeometry { track_id, geometry });
        }
        ProviderEvent::EncodedPacket(packet) => {
            accept_packet(&mut inner, packet);
        }
        ProviderEvent::Discontinuity {
            track_id,
            kind,
            reason,
            format_generation,
            timestamp,
        } => {
            inner.emit(CaptureEvent::Discontinuity {
                track_id,
                kind,
                reason,
                format_generation,
                timestamp,
            });
        }
        ProviderEvent::AuthorizationRevoked { track } => {
            inner.emit(CaptureEvent::AuthorizationRevoked { track });
            inner.set_state(PipelineState::AuthorizationRevoked);
        }
        ProviderEvent::Failed { track, error } => {
            inner.emit(CaptureEvent::Failed {
                track,
                error: error.clone(),
            });
            inner.set_state(PipelineState::Failed);
        }
        ProviderEvent::Stopped => {
            inner.set_state(PipelineState::Stopped);
        }
    }
}

fn accept_packet(inner: &mut Inner, packet: EncodedPacket) {
    let track_id = packet.track_id;
    let Some(runtime) = inner.tracks.get_mut(&track_id) else {
        return;
    };
    if !runtime.configured || packet.format_generation != runtime.format_generation {
        return;
    }
    if runtime.kind == TrackKind::Visual {
        let Some(generation) = packet.geometry_generation else {
            return;
        };
        if !runtime.geometry_ready || generation != runtime.geometry_generation {
            return;
        }
    }
    let kind = runtime.kind;
    let format_generation = runtime.format_generation;
    match runtime.queue.push(packet, &inner.clock) {
        Ok(Some(reason)) => {
            let timestamp = inner.clock.now();
            inner.emit(CaptureEvent::Discontinuity {
                track_id,
                kind,
                reason,
                format_generation,
                timestamp,
            });
        }
        Ok(None) => {}
        Err(_) => {
            if kind == TrackKind::Visual {
                for live in &inner.sessions {
                    let _ = live.session.request_keyframe(track_id);
                }
            }
        }
    }
}

fn drain_queues(inner: &Arc<Mutex<Inner>>) {
    let mut inner = lock(inner);
    if inner.state != PipelineState::Running && inner.state != PipelineState::Suspended {
        return;
    }
    let ids: Vec<_> = inner.tracks.keys().copied().collect();
    for id in ids {
        while let Some(packet) = inner
            .tracks
            .get_mut(&id)
            .and_then(|runtime| runtime.queue.pop())
        {
            inner.emit(CaptureEvent::EncodedPacket(packet));
        }
    }
}

fn lock(inner: &Arc<Mutex<Inner>>) -> std::sync::MutexGuard<'_, Inner> {
    inner.lock().unwrap_or_else(|error| error.into_inner())
}

fn block_on<T>(future: impl std::future::Future<Output = T>) -> T {
    use std::sync::Arc;
    use std::task::{Context, Poll, RawWaker, RawWakerVTable, Waker};

    struct Parker(std::thread::Thread);

    fn raw(ptr: *const ()) -> RawWaker {
        RawWaker::new(ptr, &VTABLE)
    }
    unsafe fn clone(ptr: *const ()) -> RawWaker {
        unsafe {
            Arc::increment_strong_count(ptr as *const Parker);
        }
        raw(ptr)
    }
    unsafe fn wake(ptr: *const ()) {
        let parker = unsafe { Arc::from_raw(ptr as *const Parker) };
        parker.0.unpark();
    }
    unsafe fn wake_by_ref(ptr: *const ()) {
        let parker = unsafe { &*(ptr as *const Parker) };
        parker.0.unpark();
    }
    unsafe fn drop_waker(ptr: *const ()) {
        drop(unsafe { Arc::from_raw(ptr as *const Parker) });
    }
    static VTABLE: RawWakerVTable = RawWakerVTable::new(clone, wake, wake_by_ref, drop_waker);

    let parker = Arc::new(Parker(thread::current()));
    let waker = unsafe { Waker::from_raw(raw(Arc::into_raw(parker) as *const ())) };
    let mut future = std::pin::pin!(future);
    let mut context = Context::from_waker(&waker);
    loop {
        match future.as_mut().poll(&mut context) {
            Poll::Ready(value) => return value,
            Poll::Pending => thread::park(),
        }
    }
}

#[cfg(test)]
mod tests;
