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

use windows::Win32::Foundation::{COLORREF, HWND, RECT};
use windows::Win32::Graphics::Gdi::{
    BI_RGB, BITMAPINFO, BITMAPINFOHEADER, CreateCompatibleDC, CreateDIBSection, DIB_RGB_COLORS,
    DeleteDC, DeleteObject, GetDC, ReleaseDC, SelectObject,
};
use windows::Win32::Media::MediaFoundation::{
    IMFGetService, IMFPMediaPlayer, IMFPMediaPlayerCallback, IMFPMediaPlayerCallback_Impl,
    IMFVideoDisplayControl, MFP_EVENT_HEADER, MFP_EVENT_TYPE_ERROR,
    MFP_EVENT_TYPE_MEDIAITEM_CREATED, MFP_EVENT_TYPE_MEDIAITEM_SET, MFP_EVENT_TYPE_PAUSE,
    MFP_EVENT_TYPE_PLAY, MFP_EVENT_TYPE_PLAYBACK_ENDED, MFP_EVENT_TYPE_STOP,
    MFP_MEDIAITEM_CREATED_EVENT, MFP_OPTION_NONE, MFP_POSITIONTYPE_100NS, MFPCreateMediaPlayer,
    MR_VIDEO_RENDER_SERVICE,
};
use windows::Win32::Storage::Xps::{PRINT_WINDOW_FLAGS, PrintWindow};
use windows::Win32::System::Com::CoTaskMemFree;
use windows::Win32::System::Com::StructuredStorage::PROPVARIANT;
use windows::Win32::System::Variant::{VT_I8, VT_UI8};
use windows::Win32::UI::WindowsAndMessaging::{self, PW_RENDERFULLCONTENT};
use windows::core::{Interface, PCWSTR, implement};

/// Playback transitions reported to the component host, on the UI thread.
pub(crate) enum VideoPlayerEvent {
    /// The media item is attached and ready; duration in seconds
    /// (`0` when unknown, e.g. live sources).
    MediaLoaded {
        duration: f64,
    },
    Play,
    Pause,
    Stop,
    Ended,
    Error {
        message: String,
    },
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
    /// An async `CreateMediaItemFromURL` is in flight (since when — an
    /// open whose callbacks never came back must not wedge `play()`).
    opening: Option<std::time::Instant>,
    /// Position to restore once the next media item attaches (quality
    /// switches keep the playback position across the source change).
    pending_seek: Option<f64>,
    /// Playback rate, re-applied whenever a media item attaches.
    rate: f32,
}

/// An async open older than this is presumed dead and gets retried.
const OPEN_STALE_AFTER: std::time::Duration = std::time::Duration::from_secs(10);

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
    /// Creates a player. `video_window` is the EVR target; `None` is
    /// windowless (decode + events only — island nodes paint in DComp).
    /// `sink` receives playback transitions on this same thread.
    pub(crate) fn new(video_window: Option<HWND>, sink: VideoEventSink) -> Option<Self> {
        let shared = Arc::new(Mutex::new(SharedState {
            rate: 1.0,
            ..Default::default()
        }));
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
                video_window,
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
            shared.opening = None;
            shared.pending_seek = None;
        }
        self.open_current_source();
    }

    /// Switches to `url` keeping playback continuity: restores `position`
    /// once the new media item attaches and resumes when `resume` is set
    /// (quality switching).
    pub(crate) fn switch_source(&self, url: &str, position: f64, resume: bool) {
        {
            let mut shared = self.lock();
            shared.source = Some(url.to_string());
            shared.media_ready = false;
            shared.opening = None;
            shared.pending_seek = (position > 0.0).then_some(position);
            shared.pending_play = resume;
        }
        self.open_current_source();
    }

    /// Playback rate (1.0 = normal); survives source switches.
    pub(crate) fn set_rate(&self, rate: f64) {
        let rate = rate.clamp(0.1, 8.0) as f32;
        self.lock().rate = rate;
        unsafe {
            let _ = self.player.SetRate(rate);
        }
    }

    /// MFPlay's URL parser misreads extended-length paths (`\\?\C:\...`)
    /// as network paths; plain absolute paths work.
    fn normalize_source(url: &str) -> &str {
        url.strip_prefix(r"\\?\").unwrap_or(url)
    }

    /// Opens the stored source unless a (live) open is already in flight.
    fn open_current_source(&self) {
        let url = {
            let mut shared = self.lock();
            if shared
                .opening
                .is_some_and(|since| since.elapsed() < OPEN_STALE_AFTER)
            {
                return;
            }
            let Some(url) = shared.source.clone() else {
                return;
            };
            shared.opening = Some(std::time::Instant::now());
            shared.media_ready = false;
            url
        };
        let url = Self::normalize_source(&url).to_string();
        let wide: Vec<u16> = url.encode_utf16().chain(std::iter::once(0)).collect();
        log::info!("mfplay opening source {url}");
        unsafe {
            let _ = self.player.ClearMediaItem();
            if let Err(err) =
                self.player
                    .CreateMediaItemFromURL(PCWSTR(wide.as_ptr()), false, 0, None)
            {
                log::warn!("failed to open video source {url}: {err}");
                self.lock().opening = None;
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
                let opening_live = shared
                    .opening
                    .is_some_and(|since| since.elapsed() < OPEN_STALE_AFTER);
                if !opening_live && shared.source.is_some() {
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
            shared.opening = None;
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

    /// Copies the EVR HWND via PrintWindow (the same path that already
    /// showed decoded frames on the off-screen island window).
    pub(crate) fn capture_window_bgra(hwnd: HWND) -> Option<(u32, u32, Vec<u32>)> {
        unsafe {
            let mut rect = RECT::default();
            WindowsAndMessaging::GetClientRect(hwnd, &mut rect).ok()?;
            let width = rect.right.max(1);
            let height = rect.bottom.max(1);
            let screen = GetDC(None);
            if screen.0.is_null() {
                return None;
            }
            let mem = CreateCompatibleDC(Some(screen));
            let mut bits_ptr = std::ptr::null_mut();
            let info = BITMAPINFO {
                bmiHeader: BITMAPINFOHEADER {
                    biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                    biWidth: width,
                    biHeight: -height,
                    biPlanes: 1,
                    biBitCount: 32,
                    biCompression: BI_RGB.0,
                    ..Default::default()
                },
                ..Default::default()
            };
            let bmp =
                CreateDIBSection(Some(mem), &info, DIB_RGB_COLORS, &mut bits_ptr, None, 0).ok()?;
            if bits_ptr.is_null() {
                let _ = DeleteDC(mem);
                ReleaseDC(None, screen);
                return None;
            }
            let old = SelectObject(mem, bmp.into());
            let printed =
                PrintWindow(hwnd, mem, PRINT_WINDOW_FLAGS(PW_RENDERFULLCONTENT)).as_bool();
            let len = (width * height * 4) as usize;
            let slice = std::slice::from_raw_parts(bits_ptr as *const u8, len);
            let mut pixels = Vec::with_capacity((width * height) as usize);
            for px in slice.chunks_exact(4) {
                pixels.push(
                    0xff00_0000 | ((px[2] as u32) << 16) | ((px[1] as u32) << 8) | px[0] as u32,
                );
            }
            SelectObject(mem, old);
            let _ = DeleteObject(bmp.into());
            let _ = DeleteDC(mem);
            ReleaseDC(None, screen);
            if !printed {
                return None;
            }
            Some((width as u32, height as u32, pixels))
        }
    }

    /// Copies the EVR's current frame as top-down premultiplied BGRA.
    /// Requires a video HWND (windowless players have no renderer).
    pub(crate) fn current_frame(&self) -> Option<(u32, u32, Vec<u32>)> {
        if !self.lock().media_ready {
            return None;
        }
        let service: IMFGetService = self.player.cast().ok()?;
        let display: IMFVideoDisplayControl =
            unsafe { service.GetService(&MR_VIDEO_RENDER_SERVICE) }.ok()?;
        let mut header = BITMAPINFOHEADER {
            biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
            ..Default::default()
        };
        let mut bits = std::ptr::null_mut();
        let mut byte_len = 0u32;
        let mut timestamp = 0i64;
        if let Err(err) = unsafe {
            display.GetCurrentImage(&mut header, &mut bits, &mut byte_len, &mut timestamp)
        } {
            log::debug!("GetCurrentImage failed: {err}");
            return None;
        }
        if bits.is_null() || byte_len == 0 {
            return None;
        }
        let slice = unsafe { std::slice::from_raw_parts(bits, byte_len as usize) };
        let frame = bgra_from_dib(&header, slice);
        unsafe {
            CoTaskMemFree(Some(bits as *const _));
        }
        frame
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
        log::info!(
            "mfplay event type={} hr=0x{:08x}",
            header.eEventType.0,
            header.hrEvent.0
        );
        let player: Option<&IMFPMediaPlayer> = (*header.pMediaPlayer).as_ref();

        if let Err(err) = header.hrEvent.ok() {
            {
                let mut shared = self.lock();
                shared.pending_play = false;
                shared.opening = None;
            }
            (self.sink)(VideoPlayerEvent::Error {
                message: format!("playback failed: {err}"),
            });
            return;
        }

        match header.eEventType {
            MFP_EVENT_TYPE_MEDIAITEM_CREATED => {
                // The header is the first field of the created-item event.
                let created = unsafe {
                    &*(header as *const MFP_EVENT_HEADER).cast::<MFP_MEDIAITEM_CREATED_EVENT>()
                };
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
                let (pending_play, pending_seek, rate) = {
                    let mut shared = self.lock();
                    shared.media_ready = true;
                    shared.opening = None;
                    (
                        std::mem::take(&mut shared.pending_play),
                        std::mem::take(&mut shared.pending_seek),
                        shared.rate,
                    )
                };
                if let Some(player) = player {
                    unsafe {
                        if let Some(position) = pending_seek {
                            let target = propvariant_from_100ns((position.max(0.0) * 1e7) as i64);
                            let _ = player.SetPosition(&MFP_POSITIONTYPE_100NS, &target);
                        }
                        if rate != 0.0 && rate != 1.0 {
                            let _ = player.SetRate(rate);
                        }
                    }
                }
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
                if self.lock().looping
                    && let Some(player) = player
                {
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

/// Top-down premultiplied BGRA from an EVR `GetCurrentImage` DIB.
pub(crate) fn bgra_from_dib(
    header: &BITMAPINFOHEADER,
    bits: &[u8],
) -> Option<(u32, u32, Vec<u32>)> {
    let width = header.biWidth.unsigned_abs();
    let height = header.biHeight.unsigned_abs();
    if width == 0 || height == 0 {
        return None;
    }
    let top_down = header.biHeight < 0;
    let bpp = header.biBitCount;
    let stride = match bpp {
        32 => width.saturating_mul(4),
        24 => (width.saturating_mul(3) + 3) & !3,
        _ => return None,
    };
    if bits.len() < (stride * height) as usize {
        return None;
    }
    let mut pixels = Vec::with_capacity((width * height) as usize);
    for y in 0..height {
        let src_y = if top_down { y } else { height - 1 - y };
        let row = (src_y * stride) as usize;
        for x in 0..width {
            let pixel = match bpp {
                32 => {
                    let i = row + (x * 4) as usize;
                    let b = bits[i] as u32;
                    let g = bits[i + 1] as u32;
                    let r = bits[i + 2] as u32;
                    let a = bits[i + 3] as u32;
                    if a == 0 || a == 255 {
                        0xff00_0000 | (r << 16) | (g << 8) | b
                    } else {
                        let pre = |channel: u32| (channel * a + 127) / 255;
                        (a << 24) | (pre(r) << 16) | (pre(g) << 8) | pre(b)
                    }
                }
                _ => {
                    let i = row + (x * 3) as usize;
                    0xff00_0000
                        | ((bits[i + 2] as u32) << 16)
                        | ((bits[i + 1] as u32) << 8)
                        | bits[i] as u32
                }
            };
            pixels.push(pixel);
        }
    }
    Some((width, height, pixels))
}

/// Nearest-neighbor scale used so a modest EVR bitmap fills the island dest.
pub(crate) fn scale_bgra_nearest(
    src: &[u32],
    src_w: u32,
    src_h: u32,
    dest_w: u32,
    dest_h: u32,
) -> Vec<u32> {
    let dest_w = dest_w.max(1);
    let dest_h = dest_h.max(1);
    if src_w == 0 || src_h == 0 {
        return vec![0; (dest_w * dest_h) as usize];
    }
    if src_w == dest_w && src_h == dest_h {
        return src.to_vec();
    }
    let mut dest = vec![0u32; (dest_w * dest_h) as usize];
    for y in 0..dest_h {
        let src_y = (y as u64 * src_h as u64 / dest_h as u64) as u32;
        let src_row = (src_y.min(src_h - 1) * src_w) as usize;
        let dest_row = (y * dest_w) as usize;
        for x in 0..dest_w {
            let src_x = (x as u64 * src_w as u64 / dest_w as u64) as u32;
            dest[dest_row + x as usize] = src[src_row + src_x.min(src_w - 1) as usize];
        }
    }
    dest
}

fn seconds_from_propvariant(variant: &PROPVARIANT) -> f64 {
    unsafe {
        let inner = &*variant.Anonymous.Anonymous;
        // Positions come back as VT_I8, durations as VT_UI8; both are
        // 100ns counts in the same union slot.
        if inner.vt == VT_I8 || inner.vt == VT_UI8 {
            inner.Anonymous.hVal as f64 / 1e7
        } else {
            0.0
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{bgra_from_dib, scale_bgra_nearest};
    use windows::Win32::Graphics::Gdi::BITMAPINFOHEADER;

    #[test]
    fn converts_bottom_up_32bpp_dib_to_top_down_bgra() {
        // Memory order is bottom row first: blue then white, then red then green.
        let bits = [
            0xffu8, 0x00, 0x00, 0xff, 0xff, 0xff, 0xff, 0xff, 0x00, 0x00, 0xff, 0xff, 0x00, 0xff,
            0x00, 0xff,
        ];
        let header = BITMAPINFOHEADER {
            biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: 2,
            biHeight: 2,
            biPlanes: 1,
            biBitCount: 32,
            ..Default::default()
        };
        let (width, height, pixels) = bgra_from_dib(&header, &bits).expect("dib");
        assert_eq!((width, height), (2, 2));
        assert_eq!(pixels[0], 0xffff_0000, "top-left red");
        assert_eq!(pixels[1], 0xff00_ff00, "top-right green");
        assert_eq!(pixels[2], 0xff00_00ff, "bottom-left blue");
        assert_eq!(pixels[3], 0xffff_ffff, "bottom-right white");
    }

    #[test]
    fn scales_decoded_frame_to_the_island_dest() {
        let src = [0xffff_0000, 0xff00_ff00, 0xff00_00ff, 0xffff_ffff];
        let dest = scale_bgra_nearest(&src, 2, 2, 4, 4);
        assert_eq!(dest.len(), 16);
        assert_eq!(dest[0], 0xffff_0000);
        assert_eq!(dest[3], 0xff00_ff00);
        assert_eq!(dest[12], 0xff00_00ff);
        assert_eq!(dest[15], 0xffff_ffff);
    }
}
