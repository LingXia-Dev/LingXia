//! Native video component handling.

use super::*;

mod controls;

use controls::*;

/// `WM_TIMER` id driving `timeupdate` while a video component plays ("LXVT").
pub(super) const VIDEO_TIMER_ID: usize = 0x4C58_5654;
/// Island EVR frame pump ("LXFR") — copies GetCurrentImage onto the DComp visual.
pub(super) const ISLAND_FRAME_TIMER_ID: usize = 0x4C58_4652;
/// Video `timeupdate` cadence, matching the HTML media-event ballpark.
const VIDEO_TIMER_INTERVAL_MS: u32 = 250;
const ISLAND_FRAME_INTERVAL_MS: u32 = 66;

pub(super) struct VideoComponent {
    pub(super) player: Arc<VideoPlayer>,
    /// Last observed cursor position over the surface — repaints under a
    /// resting cursor synthesize WM_MOUSEMOVEs that must not count as
    /// activity (or the controls bar never auto-hides).
    last_surface_mouse: Option<(i32, i32)>,
    /// Inner child window MFPlay renders into (and subclasses for its
    /// repaints). Hidden while stopped so the retained last frame never
    /// shows.
    pub(super) surface: isize,
    /// No media is presented (initial state, after `stop()`, on error).
    /// The whole container hides so the element's DOM placeholder/poster
    /// shows through; playing reveals it again.
    pub(super) stopped: bool,
    /// Native playback controls (`controls` prop), floating over the
    /// surface; auto-hides while playing.
    pub(super) controls: Option<VideoControls>,
    /// Mirrors the `muted` prop and the bar's mute toggle.
    muted: bool,
    /// Active quality label (bar quality menu).
    current_quality: Option<String>,
    /// Active playback rate (bar rate menu).
    current_rate: f64,
    /// Volume in `0.0..=1.0` (volume prop and the bar's slider).
    volume: f64,
    /// Fullscreen plays in a borderless topmost window covering the
    /// monitor (the macOS player's screen-sized fullscreen window).
    pub(super) fullscreen: bool,
    /// The fullscreen host window; `0` while not fullscreen. The
    /// container reparents into it and back.
    pub(super) fullscreen_host: isize,
    /// Mirrors the player's play/pause transitions (sink updates).
    pub(super) playing: bool,
    /// Was playing when its page left the foreground; auto-resumes when
    /// the page returns (mirrors the macOS manager).
    pub(super) resume_on_show: bool,
    /// The view may have remounted after we last emitted `play`/`playing`.
    /// The next island rematerialize re-delivers those events so
    /// `data-lx-playing` can land on the new element.
    pub(super) view_needs_playback_event: bool,
    /// True once this ready session has been told about live playback.
    pub(super) view_ack_playing: bool,
}

/// Mounts a `video.native` component: an MFPlay player rendering into the
/// container window. Playback transitions and the play-timer drive the
/// element's media events; the document rect only positions the surface.
pub(super) fn mount_video_on_ui(
    context: PageContext,
    component_id: String,
    parent: isize,
    container: HWND,
    doc_rect: DocRect,
    props: ComponentProps,
) {
    let key = component_key(&context.page_key, &component_id);
    let surface = unsafe {
        WindowsAndMessaging::CreateWindowExW(
            WINDOW_EX_STYLE::default(),
            video_surface_class(),
            PCWSTR::null(),
            WINDOW_STYLE(
                WindowsAndMessaging::WS_CHILD.0
                    | WindowsAndMessaging::WS_VISIBLE.0
                    | WindowsAndMessaging::WS_CLIPSIBLINGS.0,
            ),
            0,
            0,
            16,
            16,
            Some(container),
            None,
            GetModuleHandleW(None)
                .ok()
                .map(|module| HINSTANCE(module.0)),
            None,
        )
    };
    let Ok(surface) = surface else {
        log::warn!("failed to create video surface for {component_id}");
        unsafe {
            let _ = WindowsAndMessaging::DestroyWindow(container);
        }
        return;
    };
    let sink = video_event_sink(key.clone(), container.0 as isize, surface.0 as isize);
    let Some(player) = VideoPlayer::new(Some(surface), sink) else {
        log::warn!("failed to create video player for {component_id}");
        unsafe {
            let _ = WindowsAndMessaging::DestroyWindow(container);
        }
        return;
    };

    // Native playback controls (the macOS player's bottom bar) when the
    // element asks for them.
    let controls = (props.controls == Some(true))
        .then(|| VideoControls::create(container, video_controls_sink(key.clone())))
        .flatten();

    let entry = ComponentEntry {
        context,
        component_id,
        multiline: false,
        parent,
        container: container.0 as isize,
        edit: 0,
        font: 0,
        video: Some(VideoComponent {
            player: Arc::new(player),
            last_surface_mouse: None,
            surface: surface.0 as isize,
            stopped: true,
            fullscreen: false,
            fullscreen_host: 0,
            controls,
            muted: props.muted == Some(true),
            current_quality: active_quality_label(&props),
            current_rate: 1.0,
            volume: props.volume.unwrap_or(1.0).clamp(0.0, 1.0),
            playing: false,
            resume_on_show: false,
            view_needs_playback_event: false,
            view_ack_playing: false,
        }),
        swiper: None,
        island_kind: None,
        doc_rect,
        state: ComponentProps::default(),
        last_value: String::new(),
        ready: ready_keys().contains(&key),
        pending: Vec::new(),
    };
    components().insert(key.clone(), entry);
    containers().insert(container.0 as isize, key.clone());

    apply_video_props(&key, &props);
    apply_layout(&key);
}

/// Island video: off-screen top-level EVR HWND (not a host child) so
/// `GetCurrentImage` can feed the DComp visual. Never `HWND_TOP`.
pub(super) fn mount_windowless_island_video(
    context: PageContext,
    author_id: String,
    parent: isize,
    doc_rect: DocRect,
    props: ComponentProps,
) -> bool {
    let key = component_key(&context.page_key, &author_id);
    let Some(evr) = create_island_evr_window() else {
        log::warn!("failed to create island EVR window for {author_id}");
        return false;
    };
    let evr_handle = evr.0 as isize;
    let sink = video_event_sink(key.clone(), 0, evr_handle);
    let Some(player) = VideoPlayer::new(Some(evr), sink) else {
        log::warn!("failed to create island player for {author_id}");
        unsafe {
            let _ = WindowsAndMessaging::DestroyWindow(evr);
        }
        return false;
    };
    let player = Arc::new(player);
    let should_play =
        props.src.as_deref().is_some_and(|src| !src.is_empty()) && props.autoplay != Some(false);
    let entry = ComponentEntry {
        context,
        component_id: author_id,
        multiline: false,
        parent,
        container: 0,
        edit: 0,
        font: 0,
        video: Some(VideoComponent {
            player: player.clone(),
            last_surface_mouse: None,
            surface: evr_handle,
            stopped: true,
            fullscreen: false,
            fullscreen_host: 0,
            controls: None,
            muted: props.muted == Some(true),
            current_quality: active_quality_label(&props),
            current_rate: 1.0,
            volume: props.volume.unwrap_or(1.0).clamp(0.0, 1.0),
            playing: false,
            resume_on_show: false,
            view_needs_playback_event: false,
            view_ack_playing: false,
        }),
        swiper: None,
        island_kind: Some("video".into()),
        doc_rect,
        state: ComponentProps::default(),
        last_value: String::new(),
        // Same handshake as overlay mount: `play`/`playing` queue until
        // `lx-video` sends `component.ready` for this author id.
        ready: ready_keys().contains(&key),
        pending: Vec::new(),
    };
    components().insert(key.clone(), entry);
    apply_video_props(&key, &props);
    if should_play {
        player.play();
    }
    true
}

/// Re-issue autoplay / replay `playing` after an island rematerialize.
///
/// `play`/`playing` that landed before `component.ready` sit in `pending`.
/// A later commit still re-emits them once the view is ready so a leaf
/// that connected after the first Play still gets `data-lx-playing`.
pub(super) fn sync_island_video_playback(key: &str, props: &ComponentProps) {
    let (player, should_play_now, should_emit) = {
        let mut components = components();
        let Some(entry) = components.get_mut(key) else {
            return;
        };
        let Some(video) = entry.video.as_mut() else {
            return;
        };
        let src = props
            .src
            .as_deref()
            .or(entry.state.src.as_deref())
            .unwrap_or("");
        let autoplay = props.autoplay.or(entry.state.autoplay);
        let wants_autoplay = !src.is_empty() && autoplay != Some(false);
        let resume = std::mem::take(&mut video.resume_on_show);
        let needs_event = std::mem::take(&mut video.view_needs_playback_event);
        let live = video.playing || video.player.wants_playback();
        let should_play_now = (resume || (video.stopped && wants_autoplay)) && !video.playing;
        // Re-emit whenever the view is ready and playback is live — not
        // only after hide/show. Latch so geometry snapshots don't spam.
        let should_emit =
            !should_play_now && entry.ready && live && (needs_event || !video.view_ack_playing);
        if should_emit {
            video.view_ack_playing = true;
        }
        (video.player.clone(), should_play_now, should_emit)
    };
    if should_play_now {
        player.play();
        return;
    }
    if should_emit {
        emit_event(key, "play", json!({}));
        emit_event(key, "playing", json!({}));
    }
}

/// After `component.ready`, replay live playback so a handler that
/// registered after the first Play still sees `playing`.
pub(super) fn replay_island_playback_after_ready(key: &str) {
    let live = {
        let mut components = components();
        let Some(entry) = components.get_mut(key) else {
            return;
        };
        if !super::island::is_island_component_key(key) {
            return;
        }
        let Some(video) = entry.video.as_mut() else {
            return;
        };
        let live = video.playing || video.player.wants_playback();
        if live {
            video.view_ack_playing = true;
        }
        live
    };
    if live {
        emit_event(key, "play", json!({}));
        emit_event(key, "playing", json!({}));
    }
}

fn create_island_evr_window() -> Option<HWND> {
    let hwnd = unsafe {
        WindowsAndMessaging::CreateWindowExW(
            WINDOW_EX_STYLE(
                WindowsAndMessaging::WS_EX_TOOLWINDOW.0 | WindowsAndMessaging::WS_EX_NOACTIVATE.0,
            ),
            island_evr_class(),
            PCWSTR::null(),
            WINDOW_STYLE(WindowsAndMessaging::WS_POPUP.0 | WindowsAndMessaging::WS_DISABLED.0),
            -32_000,
            -32_000,
            320,
            180,
            None,
            None,
            GetModuleHandleW(None)
                .ok()
                .map(|module| HINSTANCE(module.0)),
            None,
        )
    };
    let hwnd = hwnd.ok()?;
    unsafe {
        let _ = WindowsAndMessaging::ShowWindow(hwnd, WindowsAndMessaging::SW_SHOWNA);
    }
    Some(hwnd)
}

fn island_evr_class() -> PCWSTR {
    static REGISTERED: OnceLock<()> = OnceLock::new();
    REGISTERED.get_or_init(|| {
        let class = WNDCLASSW {
            lpfnWndProc: Some(island_evr_proc),
            hInstance: unsafe { GetModuleHandleW(None) }
                .map(|module| HINSTANCE(module.0))
                .unwrap_or_default(),
            lpszClassName: w!("LingXiaIslandEvr"),
            hbrBackground: HBRUSH(unsafe { GetStockObject(BLACK_BRUSH) }.0),
            ..Default::default()
        };
        unsafe {
            WindowsAndMessaging::RegisterClassW(&class);
        }
    });
    w!("LingXiaIslandEvr")
}

unsafe extern "system" fn island_evr_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    if msg == WindowsAndMessaging::WM_TIMER && wparam.0 == ISLAND_FRAME_TIMER_ID {
        on_island_frame_timer(hwnd);
        return LRESULT(0);
    }
    unsafe { WindowsAndMessaging::DefWindowProcW(hwnd, msg, wparam, lparam) }
}

fn on_island_frame_timer(hwnd: HWND) {
    let handle = hwnd.0 as isize;
    let snapshot = {
        let components = components();
        components.iter().find_map(|(key, entry)| {
            let video = entry.video.as_ref()?;
            if video.surface != handle {
                return None;
            }
            Some((
                key.clone(),
                entry.context.clone(),
                entry.component_id.clone(),
                entry.doc_rect,
                video.player.clone(),
                video.playing,
            ))
        })
    };
    let Some((key, context, author_id, doc_rect, player, playing)) = snapshot else {
        return;
    };
    if playing {
        super::island::present_decoded_island_frame(
            &context, &author_id, &doc_rect, &player, handle,
        );
        let current_time = player.position();
        let duration = player.duration();
        emit_event(
            &key,
            "timeupdate",
            json!({ "currentTime": current_time, "duration": duration }),
        );
    }
}

/// Registers (once) and returns the video-surface window class: the inner
/// child MFPlay renders into. Black background (the element's placeholder
/// color), double clicks toggling fullscreen and Escape leaving it.
fn video_surface_class() -> PCWSTR {
    static REGISTERED: OnceLock<()> = OnceLock::new();
    REGISTERED.get_or_init(|| {
        let class = WNDCLASSW {
            style: WindowsAndMessaging::CS_DBLCLKS,
            lpfnWndProc: Some(video_surface_proc),
            hInstance: unsafe { GetModuleHandleW(None) }
                .map(|module| HINSTANCE(module.0))
                .unwrap_or_default(),
            lpszClassName: w!("LingXiaVideoSurface"),
            hbrBackground: HBRUSH(unsafe { GetStockObject(BLACK_BRUSH) }.0),
            ..Default::default()
        };
        unsafe {
            WindowsAndMessaging::RegisterClassW(&class);
        }
    });
    w!("LingXiaVideoSurface")
}

fn component_key_for_surface(surface: HWND) -> Option<String> {
    let container = unsafe { WindowsAndMessaging::GetParent(surface) }.ok()?;
    component_key_for_container(container)
}

/// Window procedure of the video surface (MFPlay subclasses it for its
/// repaints and forwards what it does not handle here).
unsafe extern "system" fn video_surface_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        // Take focus so Escape reaches the surface.
        WindowsAndMessaging::WM_LBUTTONDOWN => {
            unsafe {
                let _ = SetFocus(Some(hwnd));
            }
            if let Some(key) = component_key_for_surface(hwnd) {
                poke_video_controls(&key);
            }
            LRESULT(0)
        }
        // Real mouse movement over the video reveals the controls bar
        // (repaints under a resting cursor synthesize this message).
        WindowsAndMessaging::WM_MOUSEMOVE => {
            let x = (lparam.0 & 0xffff) as i16 as i32;
            let y = ((lparam.0 >> 16) & 0xffff) as i16 as i32;
            if let Some(key) = component_key_for_surface(hwnd) {
                let moved = {
                    let mut components = components();
                    components
                        .get_mut(&key)
                        .and_then(|entry| entry.video.as_mut())
                        .map(|video| {
                            let moved = video.last_surface_mouse != Some((x, y));
                            video.last_surface_mouse = Some((x, y));
                            moved
                        })
                        .unwrap_or(false)
                };
                if moved {
                    poke_video_controls(&key);
                }
            }
            LRESULT(0)
        }
        WindowsAndMessaging::WM_LBUTTONDBLCLK => {
            if let Some(key) = component_key_for_surface(hwnd) {
                let fullscreen = {
                    let components = components();
                    components
                        .get(&key)
                        .and_then(|entry| entry.video.as_ref())
                        .map(|video| video.fullscreen)
                };
                if let Some(fullscreen) = fullscreen {
                    set_video_fullscreen(&key, !fullscreen);
                }
            }
            LRESULT(0)
        }
        WindowsAndMessaging::WM_KEYDOWN if wparam.0 == VK_ESCAPE.0 as usize => {
            if let Some(key) = component_key_for_surface(hwnd) {
                let fullscreen = {
                    let components = components();
                    components
                        .get(&key)
                        .and_then(|entry| entry.video.as_ref())
                        .is_some_and(|video| video.fullscreen)
                };
                if fullscreen {
                    set_video_fullscreen(&key, false);
                    return LRESULT(0);
                }
            }
            unsafe { WindowsAndMessaging::DefWindowProcW(hwnd, msg, wparam, lparam) }
        }
        _ => unsafe { WindowsAndMessaging::DefWindowProcW(hwnd, msg, wparam, lparam) },
    }
}

/// Snapshot of a video component's layout-relevant state.
pub(super) struct VideoLayout {
    pub(super) player: Arc<VideoPlayer>,
    pub(super) surface: isize,
    pub(super) stopped: bool,
    pub(super) fullscreen_host: isize,
    pub(super) controls: Option<isize>,
}

impl VideoLayout {
    /// Sizes the surface (and the controls bar over it) to `width`x`height`
    /// inside the container, then nudges MFPlay to repaint.
    pub(super) fn layout_children(&self, width: i32, height: i32) {
        unsafe {
            let _ = WindowsAndMessaging::MoveWindow(
                HWND(self.surface as *mut _),
                0,
                0,
                width,
                height,
                true,
            );
        }
        if let Some(controls) = self.controls {
            VideoControls { hwnd: controls }.layout(width, height);
        }
        self.player.update_video();
    }
}

/// Merges `props` into a video component's stored state and applies the
/// changes to its player. The player calls run after the registry lock is
/// dropped (they are COM calls into MFPlay).
pub(super) fn apply_video_props(key: &str, props: &ComponentProps) {
    let pending = {
        let mut components = components();
        let Some(entry) = components.get_mut(key) else {
            return;
        };
        let Some(video) = entry.video.as_ref() else {
            return;
        };
        let src_changed = props.src.is_some() && props.src != entry.state.src;
        entry.state.merge_from(props);
        (
            video.player.clone(),
            src_changed.then(|| {
                (
                    entry.context.appid.clone(),
                    entry.state.src.clone().unwrap_or_default(),
                )
            }),
            src_changed && entry.state.autoplay == Some(true),
        )
    };
    if props.muted.is_some() || props.volume.is_some() {
        let mut components = components();
        if let Some(video) = components
            .get_mut(key)
            .and_then(|entry| entry.video.as_mut())
        {
            if let Some(muted) = props.muted {
                video.muted = muted;
            }
            if let Some(volume) = props.volume {
                video.volume = volume.clamp(0.0, 1.0);
            }
        }
    }
    let (player, source, autoplay) = pending;

    if let Some(looping) = props.looping {
        player.set_looping(looping);
    }
    if let Some(volume) = props.volume {
        player.set_volume(volume);
    }
    if let Some(muted) = props.muted {
        player.set_muted(muted);
    }
    if let Some((appid, source)) = source {
        if source.is_empty() {
            player.stop();
        } else if let Some(source) = resolve_native_media_source(&appid, &source) {
            player.set_source(&source);
            if autoplay {
                player.play();
            }
        }
    }
}

/// Builds the sink translating player transitions into the element's media
/// events and driving the `timeupdate` timer. MFPlay delivers these on the
/// UI thread that owns the container window.
pub(super) fn video_event_sink(key: String, container: isize, surface: isize) -> VideoEventSink {
    Arc::new(move |event| {
        let container_hwnd = HWND(container as *mut _);
        let surface_hwnd = HWND(surface as *mut _);
        match event {
            VideoPlayerEvent::MediaLoaded { duration } => {
                log::info!("island/video {key} media loaded duration={duration}");
                emit_event(&key, "loadedmetadata", json!({ "duration": duration }));
            }
            VideoPlayerEvent::Play => {
                set_video_playing(&key, true);
                set_video_stopped(&key, false);
                unsafe {
                    if !super::island::is_island_component_key(&key) {
                        // Bring the surface back after a stop hid it; the
                        // layout pass re-shows the container.
                        let _ = WindowsAndMessaging::ShowWindow(
                            surface_hwnd,
                            WindowsAndMessaging::SW_SHOWNA,
                        );
                    }
                    if super::island::is_island_component_key(&key) {
                        if surface != 0 {
                            let _ = WindowsAndMessaging::SetTimer(
                                Some(surface_hwnd),
                                ISLAND_FRAME_TIMER_ID,
                                ISLAND_FRAME_INTERVAL_MS,
                                None,
                            );
                        }
                    } else if container != 0 {
                        let _ = WindowsAndMessaging::SetTimer(
                            Some(container_hwnd),
                            VIDEO_TIMER_ID,
                            VIDEO_TIMER_INTERVAL_MS,
                            None,
                        );
                    }
                }
                apply_layout(&key);
                poke_video_controls(&key);
                emit_event(&key, "play", json!({}));
                emit_event(&key, "playing", json!({}));
            }
            VideoPlayerEvent::Pause => {
                set_video_playing(&key, false);
                stop_video_timer(container_hwnd);
                stop_island_frame_timer(surface_hwnd);
                poke_video_controls(&key);
                emit_event(&key, "pause", json!({}));
            }
            VideoPlayerEvent::Stop => {
                set_video_playing(&key, false);
                set_video_stopped(&key, true);
                stop_video_timer(container_hwnd);
                stop_island_frame_timer(surface_hwnd);
                // Overlay videos hide so the DOM poster shows. Island
                // players stay cloaked — hiding would drop DWM frames.
                if !super::island::is_island_component_key(&key) {
                    unsafe {
                        let _ = WindowsAndMessaging::ShowWindow(
                            surface_hwnd,
                            WindowsAndMessaging::SW_HIDE,
                        );
                        let _ = WindowsAndMessaging::ShowWindow(
                            container_hwnd,
                            WindowsAndMessaging::SW_HIDE,
                        );
                    }
                }
                emit_event(&key, "stop", json!({}));
            }
            VideoPlayerEvent::Ended => {
                set_video_playing(&key, false);
                stop_video_timer(container_hwnd);
                stop_island_frame_timer(surface_hwnd);
                emit_event(&key, "ended", json!({}));
            }
            VideoPlayerEvent::Error { message } => {
                set_video_playing(&key, false);
                set_video_stopped(&key, true);
                stop_video_timer(container_hwnd);
                stop_island_frame_timer(surface_hwnd);
                if !super::island::is_island_component_key(&key) {
                    unsafe {
                        let _ = WindowsAndMessaging::ShowWindow(
                            surface_hwnd,
                            WindowsAndMessaging::SW_HIDE,
                        );
                        let _ = WindowsAndMessaging::ShowWindow(
                            container_hwnd,
                            WindowsAndMessaging::SW_HIDE,
                        );
                    }
                }
                log::warn!("native video component {key}: {message}");
                emit_event(&key, "error", json!({ "errMsg": message }));
            }
        }
    })
}

fn set_video_playing(key: &str, playing: bool) {
    let mut components = components();
    if let Some(video) = components
        .get_mut(key)
        .and_then(|entry| entry.video.as_mut())
    {
        video.playing = playing;
    }
}

fn set_video_stopped(key: &str, stopped: bool) {
    let mut components = components();
    if let Some(video) = components
        .get_mut(key)
        .and_then(|entry| entry.video.as_mut())
    {
        video.stopped = stopped;
    }
}

fn stop_video_timer(container: HWND) {
    unsafe {
        let _ = WindowsAndMessaging::KillTimer(Some(container), VIDEO_TIMER_ID);
    }
}

fn stop_island_frame_timer(surface: HWND) {
    if surface.0.is_null() {
        return;
    }
    unsafe {
        let _ = WindowsAndMessaging::KillTimer(Some(surface), ISLAND_FRAME_TIMER_ID);
    }
}

/// Emits `timeupdate` while a video plays (container `WM_TIMER` tick).
pub(super) fn on_video_timer(container: HWND) {
    let Some(key) = component_key_for_container(container) else {
        stop_video_timer(container);
        return;
    };
    let player = {
        let components = components();
        components
            .get(&key)
            .and_then(|entry| entry.video.as_ref())
            .map(|video| video.player.clone())
    };
    let Some(player) = player else {
        stop_video_timer(container);
        return;
    };
    let current_time = player.position();
    let duration = player.duration();
    update_video_controls(&key);
    emit_event(
        &key,
        "timeupdate",
        json!({ "currentTime": current_time, "duration": duration }),
    );
}

/// Routes a video-context command (`lx.createVideoContext`) to the mounted
/// `video.native` component with that id. Registered with the platform
/// layer at [`install`]; called from logic threads.
pub(super) fn dispatch_video_command(
    component_id: &str,
    command: &VideoPlayerCommand,
) -> Result<(), String> {
    let target = {
        let components = components();
        components
            .iter()
            .find(|(_, entry)| entry.video.is_some() && entry.component_id == component_id)
            .map(|(key, entry)| {
                // Island players may be windowless (`container == 0`); the
                // host window still owns the UI thread they were created on.
                let thread_window = if entry.parent != 0 {
                    entry.parent
                } else {
                    entry.container
                };
                (key.clone(), thread_window)
            })
    };
    let Some((key, parent)) = target else {
        return match command {
            // Island video mounts after the WebView2 message turn. Pause/stop
            // from `lx.createVideoContext` must not throw while that apply is
            // still queued (the shape-fixture spec clicks pause immediately).
            VideoPlayerCommand::Pause | VideoPlayerCommand::Stop => Ok(()),
            _ => Err(format!("no native video component '{component_id}'")),
        };
    };
    let command = command.clone();
    let quiet = matches!(
        command,
        VideoPlayerCommand::Pause | VideoPlayerCommand::Stop
    );
    let posted = run_on_window_thread(parent, move || apply_video_command(&key, &command));
    if posted || quiet {
        Ok(())
    } else {
        Err(format!(
            "window of video component '{component_id}' is gone"
        ))
    }
}

fn apply_video_command(key: &str, command: &VideoPlayerCommand) {
    let player = {
        let components = components();
        let Some(video) = components.get(key).and_then(|entry| entry.video.as_ref()) else {
            return;
        };
        video.player.clone()
    };
    match command {
        VideoPlayerCommand::Play => player.play(),
        VideoPlayerCommand::Pause => player.pause(),
        VideoPlayerCommand::Stop => player.stop(),
        VideoPlayerCommand::Seek { position } => player.seek(*position),
        VideoPlayerCommand::NotifyEnded => {
            // Stream providers surface an authoritative end-of-stream.
            player.stop();
            emit_event(key, "ended", json!({}));
        }
        VideoPlayerCommand::SetDuration { .. } => {
            // Stream-piped duration; file/URL playback reads it from the
            // media item instead.
        }
        VideoPlayerCommand::EnterFullscreen => set_video_fullscreen(key, true),
        VideoPlayerCommand::ExitFullscreen => set_video_fullscreen(key, false),
    }
}

/// Registers (once) and returns the fullscreen host class: a black
/// borderless topmost window covering the monitor (the macOS player's
/// screen-sized fullscreen window).
fn fullscreen_host_class() -> PCWSTR {
    static REGISTERED: OnceLock<()> = OnceLock::new();
    REGISTERED.get_or_init(|| {
        let class = WNDCLASSW {
            lpfnWndProc: Some(fullscreen_host_proc),
            hInstance: unsafe { GetModuleHandleW(None) }
                .map(|module| HINSTANCE(module.0))
                .unwrap_or_default(),
            lpszClassName: w!("LingXiaVideoFullscreenHost"),
            hbrBackground: HBRUSH(unsafe { GetStockObject(BLACK_BRUSH) }.0),
            ..Default::default()
        };
        unsafe {
            WindowsAndMessaging::RegisterClassW(&class);
        }
    });
    w!("LingXiaVideoFullscreenHost")
}

fn component_key_for_fullscreen_host(host: HWND) -> Option<String> {
    let host = host.0 as isize;
    let components = components();
    components
        .iter()
        .find(|(_, entry)| {
            entry
                .video
                .as_ref()
                .is_some_and(|video| video.fullscreen_host == host)
        })
        .map(|(key, _)| key.clone())
}

unsafe extern "system" fn fullscreen_host_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        WindowsAndMessaging::WM_CLOSE => {
            if let Some(key) = component_key_for_fullscreen_host(hwnd) {
                set_video_fullscreen(&key, false);
            }
            LRESULT(0)
        }
        _ => unsafe { WindowsAndMessaging::DefWindowProcW(hwnd, msg, wparam, lparam) },
    }
}

pub(super) fn set_video_fullscreen(key: &str, fullscreen: bool) {
    let Some((surface, container, parent)) = ({
        let components = components();
        components.get(key).and_then(|entry| {
            entry
                .video
                .as_ref()
                .filter(|video| video.fullscreen != fullscreen)
                .map(|video| (video.surface, entry.container, entry.parent))
        })
    }) else {
        return;
    };

    let container_hwnd = HWND(container as *mut _);
    if fullscreen {
        // A borderless topmost window covering the monitor the app sits
        // on; the container reparents into it and fills it.
        let monitor =
            unsafe { MonitorFromWindow(HWND(parent as *mut _), MONITOR_DEFAULTTONEAREST) };
        let mut info = MONITORINFO {
            cbSize: std::mem::size_of::<MONITORINFO>() as u32,
            ..Default::default()
        };
        unsafe {
            let _ = GetMonitorInfoW(monitor, &mut info);
        }
        let area = info.rcMonitor;
        let host = unsafe {
            WindowsAndMessaging::CreateWindowExW(
                WindowsAndMessaging::WS_EX_TOPMOST,
                fullscreen_host_class(),
                PCWSTR::null(),
                WINDOW_STYLE(
                    WindowsAndMessaging::WS_POPUP.0
                        | WindowsAndMessaging::WS_VISIBLE.0
                        | WindowsAndMessaging::WS_CLIPCHILDREN.0,
                ),
                area.left,
                area.top,
                area.right - area.left,
                area.bottom - area.top,
                None,
                None,
                GetModuleHandleW(None)
                    .ok()
                    .map(|module| HINSTANCE(module.0)),
                None,
            )
        };
        let Ok(host) = host else {
            log::warn!("failed to create video fullscreen window");
            return;
        };
        {
            let mut components = components();
            let Some(video) = components
                .get_mut(key)
                .and_then(|entry| entry.video.as_mut())
            else {
                unsafe {
                    let _ = WindowsAndMessaging::DestroyWindow(host);
                }
                return;
            };
            video.fullscreen = true;
            video.fullscreen_host = host.0 as isize;
        }
        unsafe {
            let _ = WindowsAndMessaging::SetParent(container_hwnd, Some(host));
        }
    } else {
        let host = {
            let mut components = components();
            let Some(video) = components
                .get_mut(key)
                .and_then(|entry| entry.video.as_mut())
            else {
                return;
            };
            video.fullscreen = false;
            std::mem::take(&mut video.fullscreen_host)
        };
        unsafe {
            let _ = WindowsAndMessaging::SetParent(container_hwnd, Some(HWND(parent as *mut _)));
            if host != 0 {
                let _ = WindowsAndMessaging::DestroyWindow(HWND(host as *mut _));
            }
        }
    }

    apply_layout(key);
    // The fullscreen window covers everything; focus the surface so
    // Escape dismisses, and hand focus back when leaving.
    unsafe {
        let surface_hwnd = HWND(surface as *mut _);
        if fullscreen {
            let _ = SetFocus(Some(surface_hwnd));
        } else if GetFocus() == surface_hwnd {
            let _ = SetFocus(Some(HWND(parent as *mut _)));
        }
    }
    emit_event(key, "fullscreenchange", json!({ "fullScreen": fullscreen }));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pause_without_a_mounted_video_is_a_noop() {
        assert_eq!(
            dispatch_video_command("missing-video", &VideoPlayerCommand::Pause),
            Ok(())
        );
        assert_eq!(
            dispatch_video_command("missing-video", &VideoPlayerCommand::Stop),
            Ok(())
        );
        assert!(
            dispatch_video_command("missing-video", &VideoPlayerCommand::Play)
                .unwrap_err()
                .contains("no native video component")
        );
    }
}
