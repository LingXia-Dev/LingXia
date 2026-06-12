//! Native video component handling.

use super::*;

/// `WM_TIMER` id driving `timeupdate` while a video component plays ("LXVT").
pub(super) const VIDEO_TIMER_ID: usize = 0x4C58_5654;
/// Video `timeupdate` cadence, matching the HTML media-event ballpark.
const VIDEO_TIMER_INTERVAL_MS: u32 = 250;

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
    let Some(player) = VideoPlayer::new(surface, sink) else {
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
        }),
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
            src_changed.then(|| entry.state.src.clone().unwrap_or_default()),
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
    if let Some(source) = source {
        if source.is_empty() {
            player.stop();
        } else {
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
fn video_event_sink(key: String, container: isize, surface: isize) -> VideoEventSink {
    Arc::new(move |event| {
        let container_hwnd = HWND(container as *mut _);
        let surface_hwnd = HWND(surface as *mut _);
        match event {
            VideoPlayerEvent::MediaLoaded { duration } => {
                emit_event(&key, "loadedmetadata", json!({ "duration": duration }));
            }
            VideoPlayerEvent::Play => {
                set_video_playing(&key, true);
                set_video_stopped(&key, false);
                unsafe {
                    // Bring the surface back after a stop hid it; the
                    // layout pass re-shows the container.
                    let _ = WindowsAndMessaging::ShowWindow(
                        surface_hwnd,
                        WindowsAndMessaging::SW_SHOWNA,
                    );
                    let _ = WindowsAndMessaging::SetTimer(
                        Some(container_hwnd),
                        VIDEO_TIMER_ID,
                        VIDEO_TIMER_INTERVAL_MS,
                        None,
                    );
                }
                apply_layout(&key);
                poke_video_controls(&key);
                emit_event(&key, "play", json!({}));
                emit_event(&key, "playing", json!({}));
            }
            VideoPlayerEvent::Pause => {
                set_video_playing(&key, false);
                stop_video_timer(container_hwnd);
                poke_video_controls(&key);
                emit_event(&key, "pause", json!({}));
            }
            VideoPlayerEvent::Stop => {
                set_video_playing(&key, false);
                set_video_stopped(&key, true);
                stop_video_timer(container_hwnd);
                // MFPlay's subclassed surface keeps blitting the released
                // frame; hide the whole component so the element's DOM
                // placeholder/poster shows instead.
                unsafe {
                    let _ =
                        WindowsAndMessaging::ShowWindow(surface_hwnd, WindowsAndMessaging::SW_HIDE);
                    let _ = WindowsAndMessaging::ShowWindow(
                        container_hwnd,
                        WindowsAndMessaging::SW_HIDE,
                    );
                }
                emit_event(&key, "stop", json!({}));
            }
            VideoPlayerEvent::Ended => {
                set_video_playing(&key, false);
                stop_video_timer(container_hwnd);
                emit_event(&key, "ended", json!({}));
            }
            VideoPlayerEvent::Error { message } => {
                set_video_playing(&key, false);
                set_video_stopped(&key, true);
                stop_video_timer(container_hwnd);
                unsafe {
                    let _ =
                        WindowsAndMessaging::ShowWindow(surface_hwnd, WindowsAndMessaging::SW_HIDE);
                    let _ = WindowsAndMessaging::ShowWindow(
                        container_hwnd,
                        WindowsAndMessaging::SW_HIDE,
                    );
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

/// The quality label matching the active source (falls back to the first
/// preset), shown on the controls bar.
fn active_quality_label(props: &ComponentProps) -> Option<String> {
    let qualities = props.qualities.as_ref()?;
    let src = props.src.as_deref();
    qualities
        .iter()
        .find(|(_, url)| url.as_deref().is_some_and(|url| Some(url) == src))
        .or_else(|| qualities.first())
        .map(|(label, _)| label.clone())
}

/// Routes bar interactions back into the player; runs on the UI thread
/// (the bar's window procedure).
fn video_controls_sink(key: String) -> crate::video_controls::ControlsActionSink {
    Arc::new(move |action| {
        let snapshot = {
            let components = components();
            components.get(&key).and_then(|entry| {
                entry.video.as_ref().map(|video| {
                    (
                        video.player.clone(),
                        video.playing,
                        video.muted,
                        video.fullscreen,
                    )
                })
            })
        };
        let Some((player, playing, muted, fullscreen)) = snapshot else {
            return;
        };
        match action {
            ControlsAction::TogglePlay => {
                if playing {
                    player.pause();
                } else {
                    player.play();
                }
            }
            ControlsAction::ToggleMute => {
                let muted = !muted;
                player.set_muted(muted);
                {
                    let mut components = components();
                    if let Some(video) = components
                        .get_mut(&key)
                        .and_then(|entry| entry.video.as_mut())
                    {
                        video.muted = muted;
                    }
                }
                update_video_controls(&key);
            }
            ControlsAction::ToggleFullscreen => set_video_fullscreen(&key, !fullscreen),
            ControlsAction::Seek(position) => player.seek(position),
            ControlsAction::SetVolume(volume) => {
                player.set_volume(volume);
                {
                    let mut components = components();
                    if let Some(video) = components
                        .get_mut(&key)
                        .and_then(|entry| entry.video.as_mut())
                    {
                        video.volume = volume;
                    }
                }
                update_video_controls(&key);
            }
            ControlsAction::QualityMenu { anchor } => show_quality_menu(&key, anchor),
            ControlsAction::RateMenu { anchor } => show_rate_menu(&key, anchor),
        }
    })
}

/// Pops the quality menu above the bar and switches the source keeping
/// the playback position; the macOS bar's quality selector.
fn show_quality_menu(key: &str, anchor: (i32, i32)) {
    let snapshot = {
        let components = components();
        components.get(key).and_then(|entry| {
            entry.video.as_ref().map(|video| {
                (
                    video.player.clone(),
                    video.surface,
                    video.playing,
                    video.current_quality.clone(),
                    entry.state.qualities.clone().unwrap_or_default(),
                )
            })
        })
    };
    let Some((player, surface, playing, current, qualities)) = snapshot else {
        return;
    };
    if qualities.is_empty() {
        return;
    }
    let labels: Vec<String> = qualities.iter().map(|(label, _)| label.clone()).collect();
    let Some(index) = popup_choice(surface, anchor, &labels, current.as_deref()) else {
        return;
    };
    let (label, url) = &qualities[index];
    if Some(label.as_str()) == current.as_deref() {
        return;
    }
    if let Some(url) = url {
        player.switch_source(url, player.position(), playing);
        let mut components = components();
        if let Some(entry) = components.get_mut(key) {
            entry.state.src = Some(url.clone());
            if let Some(video) = entry.video.as_mut() {
                video.current_quality = Some(label.clone());
            }
        }
    } else {
        let mut components = components();
        if let Some(video) = components
            .get_mut(key)
            .and_then(|entry| entry.video.as_mut())
        {
            video.current_quality = Some(label.clone());
        }
    }
    update_video_controls(key);
    emit_event(key, "qualitychange", json!({ "quality": label }));
}

/// Pops the playback-rate menu above the bar (macOS bar's rate selector).
fn show_rate_menu(key: &str, anchor: (i32, i32)) {
    let snapshot = {
        let components = components();
        components.get(key).and_then(|entry| {
            entry.video.as_ref().map(|video| {
                (
                    video.player.clone(),
                    video.surface,
                    video.current_rate,
                    entry.state.playback_rates.clone().unwrap_or_default(),
                )
            })
        })
    };
    let Some((player, surface, current, rates)) = snapshot else {
        return;
    };
    if rates.is_empty() {
        return;
    }
    let labels: Vec<String> = rates.iter().map(|rate| format!("{rate}x")).collect();
    let current_label = format!("{current}x");
    let Some(index) = popup_choice(surface, anchor, &labels, Some(&current_label)) else {
        return;
    };
    let rate = rates[index];
    player.set_rate(rate);
    {
        let mut components = components();
        if let Some(video) = components
            .get_mut(key)
            .and_then(|entry| entry.video.as_mut())
        {
            video.current_rate = rate;
        }
    }
    update_video_controls(key);
    emit_event(key, "ratechange", json!({ "rate": rate }));
}

/// Shows a checked single-choice popup above `anchor` (screen coords of
/// the slot's top edge); returns the selected index.
fn popup_choice(
    owner: isize,
    anchor: (i32, i32),
    labels: &[String],
    checked: Option<&str>,
) -> Option<usize> {
    let menu = unsafe { WindowsAndMessaging::CreatePopupMenu() }.ok()?;
    for (index, label) in labels.iter().enumerate() {
        let mut flags = WindowsAndMessaging::MF_STRING;
        if Some(label.as_str()) == checked {
            flags |= WindowsAndMessaging::MF_CHECKED;
        }
        let wide = to_wide(label);
        unsafe {
            let _ = WindowsAndMessaging::AppendMenuW(menu, flags, index + 1, PCWSTR(wide.as_ptr()));
        }
    }
    let owner_hwnd = HWND(owner as *mut _);
    let selected = unsafe {
        let _ = WindowsAndMessaging::SetForegroundWindow(owner_hwnd);
        WindowsAndMessaging::TrackPopupMenu(
            menu,
            WindowsAndMessaging::TPM_LEFTALIGN
                | WindowsAndMessaging::TPM_BOTTOMALIGN
                | WindowsAndMessaging::TPM_RETURNCMD
                | WindowsAndMessaging::TPM_NONOTIFY,
            anchor.0,
            anchor.1 - 4,
            None,
            owner_hwnd,
            None,
        )
    };
    unsafe {
        let _ = WindowsAndMessaging::DestroyMenu(menu);
    }
    let id = selected.0 as usize;
    (id >= 1 && id <= labels.len()).then(|| id - 1)
}

/// Pushes the current playback state into the bar (no-op without one).
fn update_video_controls(key: &str) {
    let snapshot = {
        let components = components();
        components.get(key).and_then(|entry| {
            entry.video.as_ref().and_then(|video| {
                video.controls.as_ref().map(|controls| {
                    (
                        VideoControls {
                            hwnd: controls.hwnd,
                        },
                        video.player.clone(),
                        ControlsState {
                            playing: video.playing,
                            muted: video.muted,
                            fullscreen: video.fullscreen,
                            position: 0.0,
                            duration: 0.0,
                            show_progress: entry.state.progress_bar != Some(false),
                            quality: entry
                                .state
                                .qualities
                                .as_ref()
                                .filter(|qualities| !qualities.is_empty())
                                .and_then(|_| video.current_quality.clone()),
                            rate: entry
                                .state
                                .playback_rates
                                .as_ref()
                                .filter(|rates| !rates.is_empty())
                                .map(|_| video.current_rate),
                            volume: video.volume,
                        },
                    )
                })
            })
        })
    };
    let Some((controls, player, mut state)) = snapshot else {
        return;
    };
    state.position = player.position();
    state.duration = player.duration();
    controls.update(state);
}

/// Reveals the bar on mouse activity over the video.
fn poke_video_controls(key: &str) {
    let controls = {
        let components = components();
        components.get(key).and_then(|entry| {
            entry.video.as_ref().and_then(|video| {
                video.controls.as_ref().map(|controls| VideoControls {
                    hwnd: controls.hwnd,
                })
            })
        })
    };
    if let Some(controls) = controls {
        update_video_controls(key);
        controls.poke();
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
            .map(|(key, entry)| (key.clone(), entry.parent))
    };
    let Some((key, parent)) = target else {
        return Err(format!("no native video component '{component_id}'"));
    };
    let command = command.clone();
    if run_on_window_thread(parent, move || apply_video_command(&key, &command)) {
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
