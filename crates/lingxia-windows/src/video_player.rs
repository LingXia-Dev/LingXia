//! MFPlay-backed playback engine for the `video.native` component.
//!
//! One [`VideoPlayer`] per mounted component: an `IMFPMediaPlayer` that
//! renders into the component's container window (EVR letterboxing on a
//! black background, matching the element's default `object-fit`). The
//! player is created and driven exclusively on the UI thread that owns
//! that window; MFPlay marshals its callbacks to the creating thread's
//! message loop, so [`VideoPlayerEvent`]s also arrive there.
//!
//! Media items are opened asynchronously (`CreateMediaItemFromURL` with
//! `fSync = FALSE` — network sources must not block the UI thread) and
//! attached in the `MEDIAITEM_CREATED` callback, the canonical MFPlay
//! sequence. A `play()` before the item is ready is remembered and issued
//! when `MEDIAITEM_SET` lands, which also serves `autoplay`.

use std::sync::{Arc, Mutex};

use windows::Win32::Foundation::{COLORREF, HWND};
use windows::Win32::Media::MediaFoundation::{
    IMFPMediaPlayer, IMFPMediaPlayerCallback, IMFPMediaPlayerCallback_Impl, MFP_EVENT_HEADER,
    MFP_EVENT_TYPE_ERROR, MFP_EVENT_TYPE_MEDIAITEM_CREATED, MFP_EVENT_TYPE_MEDIAITEM_SET,
    MFP_EVENT_TYPE_PAUSE, MFP_EVENT_TYPE_PLAY, MFP_EVENT_TYPE_PLAYBACK_ENDED,
    MFP_EVENT_TYPE_STOP, MFP_MEDIAITEM_CREATED_EVENT, MFP_OPTION_NONE, MFP_POSITIONTYPE_100NS,
    MFPCreateMediaPlayer,
};
use windows::Win32::System::Com::StructuredStorage::PROPVARIANT;
use windows::Win32::System::Variant::VT_I8;
use windows::core::{PCWSTR, implement};

/// Playback transitions reported to the component host, on the UI thread.
pub(crate) enum VideoPlayerEvent {
    /// The media item is attached and ready; duration in seconds
    /// (`0` when unknown, e.g. live sources).
    MediaLoaded { duration: f64 },
    Play,
    Pause,
    Stop,
    Ended,
    Error { message: String },
}

pub(crate) type VideoEventSink = Arc<dyn Fn(VideoPlayerEvent) + Send + Sync>;

/// State shared with the MFPlay callback object.
#[derive(Default)]
struct SharedState {
    /// A media item is attached; position/duration calls are meaningful
    /// and `Play` works directly.
    media_ready: bool,
    /// `play()` (or autoplay) requested before the media item finished
    /// opening; issued on `MEDIAITEM_SET`.
    pending_play: bool,
    /// Restart from the beginning instead of surfacing `Ended`.
    looping: bool,
    /// Current source URL. `stop()` clears the media item (releasing the
    /// decoder and the displayed frame); `play()` reopens from here.
    source: Option<String>,
    /// An async `CreateMediaItemFromURL` is in flight.
    opening: bool,
}

pub(crate) struct VideoPlayer {
    player: IMFPMediaPlayer,
    shared: Arc<Mutex<SharedState>>,
}

// COM interfaces are not Send/Sync, but the player is created and used only
// on the UI thread that owns its video window — the component registry that
// stores it (under a process-wide mutex, hence the `Sync` requirement on
// its `Arc`) is plain bookkeeping, the same contract as the raw window
// handles stored next to it.
unsafe impl Send for VideoPlayer {}
unsafe impl Sync for VideoPlayer {}

impl VideoPlayer {
    /// Creates a player rendering into `video_window`. `sink` receives
    /// playback transitions on this same (UI) thread.
    pub(crate) fn new(video_window: HWND, sink: VideoEventSink) -> Option<Self> {
        let shared = Arc::new(Mutex::new(SharedState::default()));
        let callback: IMFPMediaPlayerCallback = PlayerCallback {
            sink,
            shared: shared.clone(),
        }
        .into();
        let mut player = None;
        let created = unsafe {
            MFPCreateMediaPlayer(
                PCWSTR::null(),
                false,
                MFP_OPTION_NONE,
                &callback,
                Some(video_window),
                Some(&mut player),
            )
        };
        if let Err(err) = created {
            log::warn!("MFPCreateMediaPlayer failed: {err}");
            return None;
        }
        let player = player?;
        unsafe {
            // Letterbox bars match the element's black placeholder.
            let _ = player.SetBorderColor(COLORREF(0));
        }
        Some(Self { player, shared })
    }

    /// Starts opening `url` asynchronously; the item is attached (and any
    /// pending play issued) from the MFPlay callback.
    pub(crate) fn set_source(&self, url: &str) {
        {
            let mut shared = self.lock();
            shared.source = Some(url.to_string());
            shared.media_ready = false;
            shared.opening = false;
        }
        self.open_current_source();
    }

    /// Opens the stored source unless an open is already in flight.
    fn open_current_source(&self) {
        let url = {
            let mut shared = self.lock();
            if shared.opening {
                return;
            }
            let Some(url) = shared.source.clone() else {
                return;
            };
            shared.opening = true;
            shared.media_ready = false;
            url
        };
        let wide: Vec<u16> = url.encode_utf16().chain(std::iter::once(0)).collect();
        unsafe {
            let _ = self.player.ClearMediaItem();
            if let Err(err) =
                self.player
                    .CreateMediaItemFromURL(PCWSTR(wide.as_ptr()), false, 0, None)
            {
                log::warn!("failed to open video source {url}: {err}");
                self.lock().opening = false;
            }
        }
    }

    /// Plays now, or as soon as the media item finishes opening; after a
    /// `stop()` the stored source is reopened from the start.
    pub(crate) fn play(&self) {
        enum Action {
            Direct,
            Reopen,
            Wait,
        }
        let action = {
            let mut shared = self.lock();
            if shared.media_ready {
                Action::Direct
            } else {
                shared.pending_play = true;
                if !shared.opening && shared.source.is_some() {
                    Action::Reopen
                } else {
                    Action::Wait
                }
            }
        };
        match action {
            Action::Direct => unsafe {
                let _ = self.player.Play();
            },
            Action::Reopen => self.open_current_source(),
            Action::Wait => {}
        }
    }

    pub(crate) fn pause(&self) {
        self.lock().pending_play = false;
        unsafe {
            let _ = self.player.Pause();
        }
    }

    /// Stops playback and releases the media item: the decoder and the
    /// displayed frame go away (the surface falls back to the container's
    /// black background) and `play()` starts over from the source.
    pub(crate) fn stop(&self) {
        {
            let mut shared = self.lock();
            shared.pending_play = false;
            shared.media_ready = false;
            shared.opening = false;
        }
        unsafe {
            let _ = self.player.Stop();
            let _ = self.player.ClearMediaItem();
        }
    }

    /// Seeks to `seconds` from the start.
    pub(crate) fn seek(&self, seconds: f64) {
        if !self.lock().media_ready {
            return;
        }
        let position = propvariant_from_100ns((seconds.max(0.0) * 1e7) as i64);
        unsafe {
            let _ = self.player.SetPosition(&MFP_POSITIONTYPE_100NS, &position);
        }
    }

    /// Current position in seconds (`0` while no media is attached).
    pub(crate) fn position(&self) -> f64 {
        if !self.lock().media_ready {
            return 0.0;
        }
        unsafe { self.player.GetPosition(&MFP_POSITIONTYPE_100NS) }
            .map(|value| seconds_from_propvariant(&value))
            .unwrap_or(0.0)
    }

    /// Duration in seconds (`0` while unknown).
    pub(crate) fn duration(&self) -> f64 {
        if !self.lock().media_ready {
            return 0.0;
        }
        unsafe { self.player.GetDuration(&MFP_POSITIONTYPE_100NS) }
            .map(|value| seconds_from_propvariant(&value))
            .unwrap_or(0.0)
    }

    /// Volume in `0.0..=1.0`.
    pub(crate) fn set_volume(&self, volume: f64) {
        unsafe {
            let _ = self.player.SetVolume(volume.clamp(0.0, 1.0) as f32);
        }
    }

    pub(crate) fn set_muted(&self, muted: bool) {
        unsafe {
            let _ = self.player.SetMute(muted);
        }
    }

    pub(crate) fn set_looping(&self, looping: bool) {
        self.lock().looping = looping;
    }

    /// Repaints the video after the window was moved or resized.
    pub(crate) fn update_video(&self) {
        unsafe {
            let _ = self.player.UpdateVideo();
        }
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, SharedState> {
        self.shared
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}

impl Drop for VideoPlayer {
    fn drop(&mut self) {
        unsafe {
            let _ = self.player.Shutdown();
        }
    }
}

#[implement(IMFPMediaPlayerCallback)]
struct PlayerCallback {
    sink: VideoEventSink,
    shared: Arc<Mutex<SharedState>>,
}

impl PlayerCallback {
    fn lock(&self) -> std::sync::MutexGuard<'_, SharedState> {
        self.shared
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}

impl IMFPMediaPlayerCallback_Impl for PlayerCallback_Impl {
    fn OnMediaPlayerEvent(&self, header: *const MFP_EVENT_HEADER) {
        let Some(header) = (unsafe { header.as_ref() }) else {
            return;
        };
        let player: Option<&IMFPMediaPlayer> = (*header.pMediaPlayer).as_ref();

        if let Err(err) = header.hrEvent.ok() {
            {
                let mut shared = self.lock();
                shared.pending_play = false;
                shared.opening = false;
            }
            (self.sink)(VideoPlayerEvent::Error {
                message: format!("playback failed: {err}"),
            });
            return;
        }

        match header.eEventType {
            MFP_EVENT_TYPE_MEDIAITEM_CREATED => {
                // The header is the first field of the created-item event.
                let created =
                    unsafe { &*(header as *const MFP_EVENT_HEADER).cast::<MFP_MEDIAITEM_CREATED_EVENT>() };
                if let (Some(item), Some(player)) = ((*created.pMediaItem).as_ref(), player) {
                    unsafe {
                        if let Err(err) = player.SetMediaItem(item) {
                            (self.sink)(VideoPlayerEvent::Error {
                                message: format!("failed to attach media item: {err}"),
                            });
                        }
                    }
                }
            }
            MFP_EVENT_TYPE_MEDIAITEM_SET => {
                let pending_play = {
                    let mut shared = self.lock();
                    shared.media_ready = true;
                    shared.opening = false;
                    std::mem::take(&mut shared.pending_play)
                };
                let duration = player
                    .and_then(|player| unsafe { player.GetDuration(&MFP_POSITIONTYPE_100NS) }.ok())
                    .map(|value| seconds_from_propvariant(&value))
                    .unwrap_or(0.0);
                (self.sink)(VideoPlayerEvent::MediaLoaded { duration });
                if pending_play && let Some(player) = player {
                    unsafe {
                        let _ = player.Play();
                    }
                }
            }
            MFP_EVENT_TYPE_PLAY => (self.sink)(VideoPlayerEvent::Play),
            MFP_EVENT_TYPE_PAUSE => (self.sink)(VideoPlayerEvent::Pause),
            MFP_EVENT_TYPE_STOP => (self.sink)(VideoPlayerEvent::Stop),
            MFP_EVENT_TYPE_PLAYBACK_ENDED => {
                if self.lock().looping && let Some(player) = player {
                    unsafe {
                        let start = propvariant_from_100ns(0);
                        let _ = player.SetPosition(&MFP_POSITIONTYPE_100NS, &start);
                        let _ = player.Play();
                    }
                } else {
                    (self.sink)(VideoPlayerEvent::Ended);
                }
            }
            MFP_EVENT_TYPE_ERROR => (self.sink)(VideoPlayerEvent::Error {
                message: "playback error".to_string(),
            }),
            _ => {}
        }
    }
}

fn propvariant_from_100ns(value: i64) -> PROPVARIANT {
    let mut variant = PROPVARIANT::default();
    unsafe {
        let inner = &mut *variant.Anonymous.Anonymous;
        inner.vt = VT_I8;
        inner.Anonymous.hVal = value;
    }
    variant
}

fn seconds_from_propvariant(variant: &PROPVARIANT) -> f64 {
    unsafe {
        let inner = &*variant.Anonymous.Anonymous;
        if inner.vt == VT_I8 {
            inner.Anonymous.hVal as f64 / 1e7
        } else {
            0.0
        }
    }
}
