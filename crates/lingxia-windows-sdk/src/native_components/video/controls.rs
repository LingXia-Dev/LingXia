//! Native video controls bar and menu handling.

use super::*;

/// The quality label matching the active source (falls back to the first
/// preset), shown on the controls bar.
pub(super) fn active_quality_label(props: &ComponentProps) -> Option<String> {
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
pub(super) fn video_controls_sink(key: String) -> crate::video_controls::ControlsActionSink {
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
pub(super) fn update_video_controls(key: &str) {
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
pub(super) fn poke_video_controls(key: &str) {
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
