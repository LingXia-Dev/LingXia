//! Native media-swiper component (`<lx-media-swiper>`): a paged carousel of
//! image and video items.
//!
//! Mirrors `lingxia-sdk/apple/Sources/macOS/NativeComponents/Components/
//! MacMediaSwiperComponent.swift`: the mounted container window hosts one child
//! page window per item - an image rendered with GDI+ (`cover`/`contain`/`fill`)
//! or a reused [`VideoPlayer`] surface - laid out side by side at the current
//! index with `peek`/`direction`/`slide` animation, a row of circular dot
//! windows for the page indicator, and an autoplay timer.
//!
//! Events (`change`/`transitionend`/`endreached`/`tap`/`videoended`/`error`)
//! and the imperative `next`/`previous`/`goToIndex` commands match the macOS
//! component, routed back through [`emit_event`] like the text/video hosts.
use std::time::Instant;

use windows::Win32::Graphics::Gdi::{
    BeginPaint, CreateEllipticRgn, CreateSolidBrush, EndPaint, FillRect, PAINTSTRUCT,
};
use windows::Win32::Graphics::GdiPlus::{
    GdipCreateBitmapFromFile, GdipCreateFromHDC, GdipDeleteGraphics, GdipDisposeImage,
    GdipDrawImageRectI, GdipGetImageHeight, GdipGetImageWidth, GdipSetInterpolationMode, GpBitmap,
    GpGraphics, GpImage, InterpolationModeHighQualityBicubic,
};

use super::*;

/// `WM_TIMER` id for the autoplay tick ("LXSA").
pub(super) const SWIPER_AUTOPLAY_TIMER_ID: usize = 0x4C58_5341;
/// `WM_TIMER` id driving the slide animation ("LXSN").
pub(super) const SWIPER_ANIM_TIMER_ID: usize = 0x4C58_534E;
/// Slide-animation frame cadence (~60fps).
const SWIPER_ANIM_INTERVAL_MS: u32 = 16;

// ---------------------------------------------------------------------------
// Config + item model (mirrors MacMediaSwiperConfig / MacMediaSwiperItem)
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq)]
enum ItemKind {
    Image,
    Video,
}

impl ItemKind {
    fn as_str(self) -> &'static str {
        match self {
            ItemKind::Image => "image",
            ItemKind::Video => "video",
        }
    }
}

#[derive(Clone, PartialEq)]
struct SwiperItem {
    id: String,
    kind: ItemKind,
    src: String,
    poster: Option<String>,
    controls: Option<bool>,
    muted: Option<bool>,
}

impl SwiperItem {
    fn payload(&self) -> Value {
        let mut map = json!({ "id": self.id, "type": self.kind.as_str(), "src": self.src });
        if let Some(poster) = &self.poster {
            map["poster"] = json!(poster);
        }
        if let Some(controls) = self.controls {
            map["controls"] = json!(controls);
        }
        if let Some(muted) = self.muted {
            map["muted"] = json!(muted);
        }
        map
    }
}

#[derive(Clone, Copy, PartialEq)]
enum ObjectFit {
    Cover,
    Contain,
    Fill,
}

#[derive(Clone, Copy, PartialEq)]
enum Direction {
    Horizontal,
    Vertical,
}

#[derive(Clone, Copy, PartialEq)]
enum Animation {
    Slide,
    None,
}

#[derive(Clone)]
pub(super) struct SwiperConfig {
    items: Vec<SwiperItem>,
    index: Option<usize>,
    initial_index: usize,
    looping: bool,
    autoplay: bool,
    interval: u32,
    animation: Animation,
    animation_duration: u32,
    direction: Direction,
    object_fit: ObjectFit,
    controls: bool,
    muted: bool,
    dots_enabled: bool,
    dots_color: u32,
    dots_active_color: u32,
    #[allow(dead_code)]
    swipe_enabled: bool,
    peek_previous: f64,
    peek_next: f64,
}

impl Default for SwiperConfig {
    fn default() -> Self {
        Self {
            items: Vec::new(),
            index: None,
            initial_index: 0,
            looping: false,
            autoplay: false,
            interval: 5000,
            animation: Animation::Slide,
            animation_duration: 300,
            direction: Direction::Horizontal,
            object_fit: ObjectFit::Cover,
            controls: false,
            muted: true,
            dots_enabled: false,
            // White at 0.4 alpha, premultiplied over the black backdrop.
            dots_color: blend_over_black(0xff, 0xff, 0xff, 0.4),
            dots_active_color: blend_over_black(0xff, 0xff, 0xff, 1.0),
            swipe_enabled: true,
            peek_previous: 0.0,
            peek_next: 0.0,
        }
    }
}

impl SwiperConfig {
    /// Parses the JS `props` bag, merging onto `previous` (the view resends
    /// the full bag, but partial merges keep parity with the macOS parser).
    pub(super) fn parse(props: &Value, previous: &SwiperConfig) -> SwiperConfig {
        let mut next = previous.clone();
        let Some(map) = props.as_object() else {
            return next;
        };

        if let Some(raw) = map.get("items").and_then(Value::as_array) {
            next.items = parse_items(raw);
        }
        match map.get("index") {
            Some(Value::Null) => next.index = None,
            Some(value) => {
                if let Some(n) = value.as_i64() {
                    next.index = Some(n.max(0) as usize);
                }
            }
            None => {}
        }
        if let Some(n) = map.get("initialIndex").and_then(Value::as_i64) {
            next.initial_index = n.max(0) as usize;
        }
        if let Some(v) = map.get("loop").and_then(Value::as_bool) {
            next.looping = v;
        }
        if let Some(v) = map.get("autoplay").and_then(Value::as_bool) {
            next.autoplay = v;
        }
        if let Some(n) = map.get("interval").and_then(Value::as_i64) {
            next.interval = (n.max(500)) as u32;
        }
        if let Some(n) = map.get("animationDuration").and_then(Value::as_i64) {
            next.animation_duration = n.max(0) as u32;
        }
        if let Some(s) = map.get("animation").and_then(Value::as_str) {
            next.animation = if s == "none" {
                Animation::None
            } else {
                Animation::Slide
            };
        }
        if let Some(s) = map.get("direction").and_then(Value::as_str) {
            next.direction = if s == "vertical" {
                Direction::Vertical
            } else {
                Direction::Horizontal
            };
        }
        if let Some(s) = map.get("objectFit").and_then(Value::as_str) {
            next.object_fit = match s.to_ascii_lowercase().as_str() {
                "contain" | "fit" => ObjectFit::Contain,
                "fill" => ObjectFit::Fill,
                _ => ObjectFit::Cover,
            };
        }
        if let Some(v) = map.get("controls").and_then(Value::as_bool) {
            next.controls = v;
        }
        if let Some(v) = map.get("muted").and_then(Value::as_bool) {
            next.muted = v;
        }
        if let Some(v) = map.get("swipeEnabled").and_then(Value::as_bool) {
            next.swipe_enabled = v;
        }

        match map.get("dots") {
            Some(Value::Bool(v)) => next.dots_enabled = *v,
            Some(Value::Object(dots)) => {
                next.dots_enabled = true;
                if let Some(c) = dots
                    .get("color")
                    .and_then(Value::as_str)
                    .and_then(parse_dot_color)
                {
                    next.dots_color = c;
                }
                if let Some(c) = dots
                    .get("activeColor")
                    .and_then(Value::as_str)
                    .and_then(parse_dot_color)
                {
                    next.dots_active_color = c;
                }
            }
            _ => {}
        }

        match map.get("peek") {
            Some(Value::Number(n)) => {
                let value = n.as_f64().unwrap_or(0.0).max(0.0);
                next.peek_previous = value;
                next.peek_next = value;
            }
            Some(Value::Object(peek)) => {
                if let Some(prev) = peek.get("previous").and_then(Value::as_f64) {
                    next.peek_previous = prev.max(0.0);
                }
                if let Some(nx) = peek.get("next").and_then(Value::as_f64) {
                    next.peek_next = nx.max(0.0);
                }
            }
            Some(Value::Null) => {
                next.peek_previous = 0.0;
                next.peek_next = 0.0;
            }
            _ => {}
        }

        next
    }
}

fn parse_items(raw: &[Value]) -> Vec<SwiperItem> {
    raw.iter()
        .enumerate()
        .filter_map(|(index, entry)| {
            let map = entry.as_object()?;
            let type_raw = map.get("type").and_then(Value::as_str)?;
            let kind = match type_raw {
                "image" => ItemKind::Image,
                "video" => ItemKind::Video,
                _ => return None,
            };
            let src = map.get("src").and_then(Value::as_str)?.trim().to_string();
            if src.is_empty() {
                return None;
            }
            let id = map
                .get("id")
                .and_then(Value::as_str)
                .map(str::to_string)
                .unwrap_or_else(|| format!("{type_raw}:{src}:{index}"));
            Some(SwiperItem {
                id,
                kind,
                src,
                poster: map
                    .get("poster")
                    .and_then(Value::as_str)
                    .map(str::to_string),
                controls: map.get("controls").and_then(Value::as_bool),
                muted: map.get("muted").and_then(Value::as_bool),
            })
        })
        .collect()
}

/// Parses a dot color (`#rgb`, `#rrggbb`, `#rrggbbaa`, or a basic name) and
/// premultiplies any alpha over black into a `COLORREF` (0x00BBGGRR); child
/// dot windows are opaque, so the configured translucency is approximated
/// against the carousel's black backdrop.
fn parse_dot_color(raw: &str) -> Option<u32> {
    let value = raw.trim();
    let (r, g, b, a) = if let Some(hex) = value.strip_prefix('#') {
        match hex.len() {
            3 => {
                let expand = |c: u8| {
                    let d = (c as char).to_digit(16).unwrap_or(0) as u8;
                    d << 4 | d
                };
                let bytes = hex.as_bytes();
                (expand(bytes[0]), expand(bytes[1]), expand(bytes[2]), 1.0)
            }
            6 => {
                let n = u32::from_str_radix(hex, 16).ok()?;
                (
                    ((n >> 16) & 0xff) as u8,
                    ((n >> 8) & 0xff) as u8,
                    (n & 0xff) as u8,
                    1.0,
                )
            }
            8 => {
                let n = u32::from_str_radix(hex, 16).ok()?;
                (
                    ((n >> 24) & 0xff) as u8,
                    ((n >> 16) & 0xff) as u8,
                    ((n >> 8) & 0xff) as u8,
                    (n & 0xff) as f64 / 255.0,
                )
            }
            _ => return None,
        }
    } else {
        match value.to_ascii_lowercase().as_str() {
            "white" => (0xff, 0xff, 0xff, 1.0),
            "black" => (0, 0, 0, 1.0),
            "red" => (0xff, 0, 0, 1.0),
            "green" => (0, 0x80, 0, 1.0),
            "blue" => (0, 0, 0xff, 1.0),
            _ => return None,
        }
    };
    Some(blend_over_black(r as u32, g as u32, b as u32, a))
}

fn blend_over_black(r: u32, g: u32, b: u32, alpha: f64) -> u32 {
    let alpha = alpha.clamp(0.0, 1.0);
    let scale = |c: u32| ((c as f64) * alpha).round() as u32 & 0xff;
    (scale(b) << 16) | (scale(g) << 8) | scale(r)
}

// ---------------------------------------------------------------------------
// Component + page state
// ---------------------------------------------------------------------------

/// In-flight slide animation: `anim_offset` interpolates from `from` to `to`
/// over `duration_ms`; on completion `transitionend` fires for `target`.
struct AnimState {
    from: f64,
    to: f64,
    start: Instant,
    duration_ms: u32,
    previous_index: usize,
    target_index: usize,
    source: String,
}

pub(super) struct MediaSwiperComponent {
    config: SwiperConfig,
    current_index: usize,
    /// Fractional index used for layout (equal to `current_index` when idle,
    /// interpolated during a slide animation).
    anim_offset: f64,
    pages: Vec<SwiperPage>,
    /// Circular page-indicator child windows.
    dots: Vec<isize>,
    /// Last container content size laid out, for animation ticks.
    last_size: (i32, i32),
    anim: Option<AnimState>,
    /// Monotonic token source for async image loads (drops stale results).
    load_seq: u64,
}

struct SwiperPage {
    kind: ItemKind,
    /// Child page window: the GDI+ image canvas, or the MFPlay video surface.
    window: isize,
    /// Decoded GDI+ image as `isize` (`0` = none); kept as `isize` so the
    /// component stays `Send` for the process-wide registry.
    image: isize,
    /// Playback engine for a video item (`None` for images).
    player: Option<Arc<VideoPlayer>>,
    /// Guards async image loads against page rebuilds reusing this slot.
    load_token: u64,
}

impl Drop for SwiperPage {
    fn drop(&mut self) {
        // The page window itself is destroyed with the container (or
        // explicitly on rebuild); only the decoded image needs releasing
        // here. The player `Arc` shuts MFPlay down on its own drop.
        if self.image != 0 {
            unsafe {
                let _ = GdipDisposeImage(self.image as *mut GpImage);
            }
            self.image = 0;
        }
    }
}

fn with_swiper<R>(key: &str, f: impl FnOnce(&mut MediaSwiperComponent) -> R) -> Option<R> {
    let mut components = components();
    components
        .get_mut(key)
        .and_then(|entry| entry.swiper.as_mut())
        .map(f)
}

pub(super) fn container_is_swiper(container: HWND) -> bool {
    component_key_for_container(container)
        .map(|key| {
            components()
                .get(&key)
                .is_some_and(|entry| entry.swiper.is_some())
        })
        .unwrap_or(false)
}

// ---------------------------------------------------------------------------
// Mount / update
// ---------------------------------------------------------------------------

pub(super) fn mount_swiper_on_ui(
    context: PageContext,
    component_id: String,
    parent: isize,
    doc_rect: DocRect,
    config: SwiperConfig,
    corner_radius: Option<f64>,
) {
    let key = component_key(&context.page_key, &component_id);
    // Remount of a live id replaces the previous component.
    destroy_component(&key);

    if !is_window(parent) {
        return;
    }
    let Some(container) = create_container(parent, &component_id) else {
        return;
    };
    crate::media_preview::ensure_gdiplus();

    let current_index = resolve_initial_index(&config);
    let state = ComponentProps {
        corner_radius,
        ..ComponentProps::default()
    };

    let entry = ComponentEntry {
        context,
        component_id,
        multiline: false,
        parent,
        container: container.0 as isize,
        edit: 0,
        font: 0,
        video: None,
        swiper: Some(MediaSwiperComponent {
            config,
            current_index,
            anim_offset: current_index as f64,
            pages: Vec::new(),
            dots: Vec::new(),
            last_size: (0, 0),
            anim: None,
            load_seq: 0,
        }),
        doc_rect,
        state,
        last_value: String::new(),
        ready: ready_keys().contains(&key),
        pending: Vec::new(),
    };
    components().insert(key.clone(), entry);
    containers().insert(container.0 as isize, key.clone());

    rebuild_pages(&key);
    rebuild_dots(&key);
    apply_layout(&key);
    refresh_playback(&key);
    schedule_autoplay(&key);
}

pub(super) fn apply_swiper_update(
    key: &str,
    doc_rect: Option<DocRect>,
    props: Option<Value>,
    corner_radius: Option<f64>,
) {
    {
        let mut components = components();
        let Some(entry) = components.get_mut(key) else {
            return;
        };
        if let Some(rect) = doc_rect {
            entry.doc_rect = rect;
        }
        if let Some(radius) = corner_radius {
            entry.state.corner_radius = Some(radius);
        }
    }

    let Some(props) = props else {
        // Geometry-only update.
        apply_layout(key);
        return;
    };

    let Some((previous, previous_index, prior_item)) = with_swiper(key, |swiper| {
        (
            swiper.config.clone(),
            swiper.current_index,
            swiper.config.items.get(swiper.current_index).cloned(),
        )
    }) else {
        return;
    };

    let next = SwiperConfig::parse(&props, &previous);
    let items_changed = previous.items != next.items;
    let layout_changed = !items_changed
        && (previous.peek_previous != next.peek_previous
            || previous.peek_next != next.peek_next
            || previous.direction != next.direction);
    let controlled = next.index;
    let count = next.items.len();

    with_swiper(key, |swiper| swiper.config = next.clone());

    if items_changed {
        let new_index =
            resolve_index_for_items_change(&next, &previous.items, previous_index, prior_item);
        with_swiper(key, |swiper| {
            swiper.current_index = new_index;
            swiper.anim_offset = new_index as f64;
            swiper.anim = None;
        });
        rebuild_pages(key);
        rebuild_dots(key);
        apply_layout(key);
        refresh_playback(key);
    } else {
        apply_pages_config(key);
        if layout_changed {
            apply_layout(key);
        }
        if let Some(controlled) = controlled {
            let resolved = clamp_index(controlled, count);
            let changed = with_swiper(key, |swiper| {
                if resolved != swiper.current_index {
                    swiper.current_index = resolved;
                    swiper.anim_offset = resolved as f64;
                    swiper.anim = None;
                    true
                } else {
                    false
                }
            })
            .unwrap_or(false);
            if changed {
                apply_layout(key);
                refresh_playback(key);
            }
        }
    }

    schedule_autoplay(key);
}

/// Reapplies the config to existing pages without rebuilding them: images
/// repaint (object-fit may have changed), videos pick up the muted flag.
fn apply_pages_config(key: &str) {
    let updates: Vec<(isize, Option<Arc<VideoPlayer>>, bool)> = {
        let components = components();
        let Some(swiper) = components.get(key).and_then(|entry| entry.swiper.as_ref()) else {
            return;
        };
        swiper
            .pages
            .iter()
            .enumerate()
            .map(|(index, page)| {
                let muted = swiper
                    .config
                    .items
                    .get(index)
                    .and_then(|item| item.muted)
                    .unwrap_or(swiper.config.muted);
                (page.window, page.player.clone(), muted)
            })
            .collect()
    };
    for (window, player, muted) in updates {
        if let Some(player) = player {
            player.set_muted(muted);
        } else {
            unsafe {
                let _ = InvalidateRect(Some(HWND(window as *mut _)), None, true);
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Page / dot lifecycle
// ---------------------------------------------------------------------------

fn rebuild_pages(key: &str) {
    let (container, parent, appid, items, config_muted) = {
        let components = components();
        let Some(entry) = components.get(key) else {
            return;
        };
        let appid = entry.context.appid.clone();
        let Some(swiper) = entry.swiper.as_ref() else {
            return;
        };
        (
            HWND(entry.container as *mut _),
            entry.parent,
            appid,
            swiper.config.items.clone(),
            swiper.config.muted,
        )
    };

    // Tear down existing pages (destroy the windows, then dropping the state
    // releases the decoded images and shuts MFPlay down).
    let old = with_swiper(key, |swiper| std::mem::take(&mut swiper.pages)).unwrap_or_default();
    for page in &old {
        unsafe {
            let _ = WindowsAndMessaging::DestroyWindow(HWND(page.window as *mut _));
        }
    }
    drop(old);

    let mut new_pages = Vec::with_capacity(items.len());
    for (index, item) in items.iter().enumerate() {
        let Some(window) = create_page_window(container) else {
            new_pages.push(SwiperPage {
                kind: item.kind,
                window: 0,
                image: 0,
                player: None,
                load_token: 0,
            });
            continue;
        };
        let player = if item.kind == ItemKind::Video {
            build_video_player(&appid, key.to_string(), window, index, item, config_muted)
        } else {
            None
        };
        new_pages.push(SwiperPage {
            kind: item.kind,
            window: window.0 as isize,
            image: 0,
            player,
            load_token: 0,
        });
    }

    // Publish the pages, assign image-load tokens, then kick off the async
    // loads so the completion handlers find the pages already in place.
    let mut image_loads: Vec<(usize, isize, u64, String, bool)> = Vec::new();
    with_swiper(key, |swiper| {
        swiper.pages = new_pages;
        for (index, item) in items.iter().enumerate() {
            if item.kind != ItemKind::Image {
                continue;
            }
            let Some(page) = swiper.pages.get_mut(index) else {
                continue;
            };
            swiper.load_seq += 1;
            page.load_token = swiper.load_seq;
            let is_http = item.src.starts_with("http://") || item.src.starts_with("https://");
            image_loads.push((
                index,
                page.window,
                page.load_token,
                item.src.clone(),
                is_http,
            ));
        }
    });

    for (index, window, token, src, is_http) in image_loads {
        begin_image_load(
            key.to_string(),
            appid.clone(),
            index,
            window,
            token,
            parent,
            src,
            is_http,
        );
    }
}

fn rebuild_dots(key: &str) {
    let (container, count) = {
        let components = components();
        let Some(swiper) = components.get(key).and_then(|entry| entry.swiper.as_ref()) else {
            return;
        };
        let container = components
            .get(key)
            .map(|entry| entry.container)
            .unwrap_or(0);
        (HWND(container as *mut _), swiper.config.items.len())
    };

    let old = with_swiper(key, |swiper| std::mem::take(&mut swiper.dots)).unwrap_or_default();
    for dot in old {
        unsafe {
            let _ = WindowsAndMessaging::DestroyWindow(HWND(dot as *mut _));
        }
    }

    let mut new_dots = Vec::with_capacity(count);
    for _ in 0..count {
        if let Some(dot) = create_dot_window(container) {
            new_dots.push(dot.0 as isize);
        }
    }
    with_swiper(key, |swiper| swiper.dots = new_dots);
}

fn build_video_player(
    appid: &str,
    key: String,
    window: HWND,
    index: usize,
    item: &SwiperItem,
    config_muted: bool,
) -> Option<Arc<VideoPlayer>> {
    let sink = swiper_video_sink(key, index);
    let player = VideoPlayer::new(window, sink)?;
    player.set_looping(false);
    player.set_muted(item.muted.unwrap_or(config_muted));
    if let Some(source) = resolve_native_media_source(appid, &item.src) {
        player.set_source(&source);
    }
    Some(Arc::new(player))
}

fn swiper_video_sink(key: String, index: usize) -> VideoEventSink {
    Arc::new(move |event| match event {
        VideoPlayerEvent::Ended => emit_video_event(&key, index, "videoended", None),
        VideoPlayerEvent::Error { message } => {
            emit_video_event(&key, index, "error", Some(message))
        }
        _ => {}
    })
}

fn emit_video_event(key: &str, index: usize, event: &str, message: Option<String>) {
    let Some(item) = with_swiper(key, |swiper| swiper.config.items.get(index).cloned()).flatten()
    else {
        return;
    };
    let detail = match event {
        "error" => json!({
            "index": index,
            "item": item.payload(),
            "code": "unknown",
            "message": message.unwrap_or_else(|| "video error".to_string()),
        }),
        _ => json!({ "index": index, "item": item.payload() }),
    };
    emit_event(key, event, detail);
}

// ---------------------------------------------------------------------------
// Async image loading
// ---------------------------------------------------------------------------

/// Resolves `src` (downloading remote sources through the URL cache) off the
/// UI thread, then decodes and stores the image back on the UI thread so the
/// GDI+ object stays thread-affine.
fn begin_image_load(
    key: String,
    appid: String,
    index: usize,
    window: isize,
    token: u64,
    dispatch_window: isize,
    src: String,
    is_http: bool,
) {
    let spawned = std::thread::Builder::new()
        .name("lingxia-swiper-img".to_string())
        .spawn(move || {
            // http(s) downloads through the URL cache; `lx://`/local sources
            // resolve to a real filesystem path GDI+ can decode.
            let resolved = if is_http {
                crate::media_preview::resolve_media_path(&src)
            } else {
                resolve_native_media_source(&appid, &src)
            };
            run_on_window_thread(dispatch_window, move || {
                finish_image_load(&key, index, window, token, resolved, is_http);
            });
        });
    if let Err(err) = spawned {
        log::warn!("failed to spawn swiper image loader: {err}");
    }
}

fn finish_image_load(
    key: &str,
    index: usize,
    window: isize,
    token: u64,
    path: Option<String>,
    is_http: bool,
) {
    // Bail if the slot was rebuilt while the load was in flight.
    let still_current = with_swiper(key, |swiper| {
        swiper
            .pages
            .get(index)
            .is_some_and(|page| page.window == window && page.load_token == token)
    })
    .unwrap_or(false);
    if !still_current {
        return;
    }

    let item = with_swiper(key, |swiper| swiper.config.items.get(index).cloned()).flatten();

    let Some(path) = path else {
        if let Some(item) = item {
            let code = if is_http { "network" } else { "not_found" };
            emit_event(
                key,
                "error",
                json!({ "index": index, "item": item.payload(), "code": code, "message": "image source not found" }),
            );
        }
        return;
    };

    let wide: Vec<u16> = path.encode_utf16().chain(std::iter::once(0)).collect();
    let mut bitmap: *mut GpBitmap = std::ptr::null_mut();
    let status = unsafe { GdipCreateBitmapFromFile(PCWSTR(wide.as_ptr()), &mut bitmap) };
    if status.0 != 0 || bitmap.is_null() {
        if let Some(item) = item {
            emit_event(
                key,
                "error",
                json!({ "index": index, "item": item.payload(), "code": "decode", "message": "image decode failed" }),
            );
        }
        return;
    }

    let stored = with_swiper(key, |swiper| {
        let Some(page) = swiper.pages.get_mut(index) else {
            return false;
        };
        if page.window != window || page.load_token != token {
            return false;
        }
        if page.image != 0 {
            unsafe {
                let _ = GdipDisposeImage(page.image as *mut GpImage);
            }
        }
        page.image = bitmap as isize;
        true
    })
    .unwrap_or(false);

    if stored {
        unsafe {
            let _ = InvalidateRect(Some(HWND(window as *mut _)), None, true);
        }
    } else {
        unsafe {
            let _ = GdipDisposeImage(bitmap as *mut GpImage);
        }
    }
}

// ---------------------------------------------------------------------------
// Layout
// ---------------------------------------------------------------------------

/// Positions every page window at its index offset (with peek/direction/
/// animation) and lays the page-indicator dots over the bottom/side edge.
/// Called by the host's `apply_layout` after the container is placed.
pub(super) fn layout_swiper_children(key: &str, width: i32, height: i32) {
    if width <= 0 || height <= 0 {
        return;
    }

    struct Snapshot {
        page_key: String,
        pages: Vec<isize>,
        dots: Vec<isize>,
        direction: Direction,
        peek_previous: f64,
        peek_next: f64,
        dots_enabled: bool,
        dots_color: u32,
        dots_active_color: u32,
        current_index: usize,
        anim_offset: f64,
    }

    let snapshot = (|| {
        let mut components = components();
        let entry = components.get_mut(key)?;
        let page_key = entry.context.page_key.clone();
        let swiper = entry.swiper.as_mut()?;
        swiper.last_size = (width, height);
        Some(Snapshot {
            page_key,
            pages: swiper.pages.iter().map(|page| page.window).collect(),
            dots: swiper.dots.clone(),
            direction: swiper.config.direction,
            peek_previous: swiper.config.peek_previous,
            peek_next: swiper.config.peek_next,
            dots_enabled: swiper.config.dots_enabled,
            dots_color: swiper.config.dots_color,
            dots_active_color: swiper.config.dots_active_color,
            current_index: swiper.current_index,
            anim_offset: swiper.anim_offset,
        })
    })();
    let Some(snapshot) = snapshot else {
        return;
    };

    let scale = page_views()
        .get(&snapshot.page_key)
        .map(|view| view.target.scale)
        .filter(|scale| *scale > 0.0)
        .unwrap_or(1.0);

    let peek_prev = (snapshot.peek_previous * scale).round() as i32;
    let peek_next = (snapshot.peek_next * scale).round() as i32;
    let horizontal = snapshot.direction == Direction::Horizontal;
    let main = if horizontal { width } else { height };
    let breadth = if horizontal { height } else { width };
    let stride = (main - peek_prev - peek_next).max(1);

    for (index, &window) in snapshot.pages.iter().enumerate() {
        if window == 0 {
            continue;
        }
        let offset = ((index as f64 - snapshot.anim_offset) * stride as f64).round() as i32;
        let (x, y, w, h) = if horizontal {
            (peek_prev + offset, 0, stride, breadth)
        } else {
            (0, peek_prev + offset, breadth, stride)
        };
        unsafe {
            let _ = WindowsAndMessaging::MoveWindow(HWND(window as *mut _), x, y, w, h, false);
        }
    }

    // Dots: 6px circles, 8px apart, 12px from the trailing edge (scaled).
    let diam = ((6.0 * scale).round() as i32).max(4);
    let gap = (8.0 * scale).round() as i32;
    let margin = (12.0 * scale).round() as i32;
    let count = snapshot.dots.len() as i32;
    let show_dots = snapshot.dots_enabled && snapshot.pages.len() > 1;
    let total = count * diam + (count - 1).max(0) * gap;

    for (index, &dot) in snapshot.dots.iter().enumerate() {
        let dot_hwnd = HWND(dot as *mut _);
        if !show_dots {
            unsafe {
                let _ = WindowsAndMessaging::ShowWindow(dot_hwnd, WindowsAndMessaging::SW_HIDE);
            }
            continue;
        }
        let i = index as i32;
        let (x, y) = if horizontal {
            (
                (width - total) / 2 + i * (diam + gap),
                height - margin - diam,
            )
        } else {
            (
                width - margin - diam,
                (height - total) / 2 + i * (diam + gap),
            )
        };
        let color = if index == snapshot.current_index {
            snapshot.dots_active_color
        } else {
            snapshot.dots_color
        };
        unsafe {
            let _ = WindowsAndMessaging::MoveWindow(dot_hwnd, x, y, diam, diam, false);
            // End coordinates exclusive, hence +1; the system owns the
            // region after SetWindowRgn succeeds.
            let region = CreateEllipticRgn(0, 0, diam + 1, diam + 1);
            if SetWindowRgn(dot_hwnd, Some(region), true) == 0 {
                let _ = DeleteObject(HGDIOBJ(region.0));
            }
            WindowsAndMessaging::SetWindowLongPtrW(
                dot_hwnd,
                WindowsAndMessaging::GWLP_USERDATA,
                color as isize,
            );
            let _ = InvalidateRect(Some(dot_hwnd), None, true);
            let _ = WindowsAndMessaging::SetWindowPos(
                dot_hwnd,
                Some(WindowsAndMessaging::HWND_TOP),
                0,
                0,
                0,
                0,
                WindowsAndMessaging::SWP_NOMOVE
                    | WindowsAndMessaging::SWP_NOSIZE
                    | WindowsAndMessaging::SWP_NOACTIVATE,
            );
            let _ = WindowsAndMessaging::ShowWindow(dot_hwnd, WindowsAndMessaging::SW_SHOWNA);
        }
    }
}

// ---------------------------------------------------------------------------
// Index transitions / commands
// ---------------------------------------------------------------------------

fn clamp_index(value: usize, count: usize) -> usize {
    if count == 0 { 0 } else { value.min(count - 1) }
}

fn resolve_initial_index(config: &SwiperConfig) -> usize {
    let raw = config.index.unwrap_or(config.initial_index);
    clamp_index(raw, config.items.len())
}

fn resolve_index_for_items_change(
    next: &SwiperConfig,
    previous_items: &[SwiperItem],
    previous_index: usize,
    prior_item: Option<SwiperItem>,
) -> usize {
    if let Some(controlled) = next.index {
        return clamp_index(controlled, next.items.len());
    }
    if let Some(prior) = prior_item {
        let _ = previous_index;
        if !previous_items.is_empty()
            && let Some(matched) = next.items.iter().position(|item| item.id == prior.id)
        {
            return matched;
        }
    }
    resolve_initial_index(next)
}

pub(super) fn handle_swiper_command(key: &str, name: &str, params: Option<&Value>) {
    let is_swiper = components()
        .get(key)
        .is_some_and(|entry| entry.swiper.is_some());
    if !is_swiper {
        return;
    }
    match name {
        "next" => go_by(key, 1, "api"),
        "previous" => go_by(key, -1, "api"),
        "goToIndex" => {
            let Some(index) = params
                .and_then(|params| params.get("index"))
                .and_then(Value::as_i64)
            else {
                return;
            };
            let count = with_swiper(key, |swiper| swiper.config.items.len()).unwrap_or(0) as i64;
            if index < 0 || index >= count {
                return;
            }
            let animated = with_swiper(key, |swiper| swiper.config.animation == Animation::Slide)
                .unwrap_or(false);
            go_to(key, index as usize, "api", animated);
        }
        _ => {}
    }
}

fn go_by(key: &str, delta: i64, source: &str) {
    let Some((count, current, looping, animated)) = with_swiper(key, |swiper| {
        (
            swiper.config.items.len(),
            swiper.current_index,
            swiper.config.looping,
            swiper.config.animation == Animation::Slide,
        )
    }) else {
        return;
    };
    if count == 0 {
        return;
    }
    let target = current as i64 + delta;
    if target < 0 || target >= count as i64 {
        if looping && count > 1 {
            let wrapped = if delta > 0 { 0 } else { count - 1 };
            go_to(key, wrapped, source, animated);
        } else {
            emit_end_reached(key, current, source);
            if source == "autoplay" {
                stop_autoplay(key);
            }
        }
        return;
    }
    let target = target as usize;
    go_to(key, target, source, animated);
    if source == "autoplay" && !looping && target == count - 1 {
        emit_end_reached(key, target, source);
        stop_autoplay(key);
    }
}

fn go_to(key: &str, target: usize, source: &str, animated: bool) {
    let Some(current) = with_swiper(key, |swiper| swiper.current_index) else {
        return;
    };
    if target == current {
        return;
    }
    with_swiper(key, |swiper| swiper.current_index = target);
    emit_change(key, target, current, source);

    if animated {
        // A re-entrant transition: snap the in-flight one to its target (its
        // `transitionend` is flushed below) before starting the new one.
        let superseded = with_swiper(key, |swiper| {
            let superseded = swiper.anim.take().map(|anim| {
                swiper.anim_offset = anim.to;
                (anim.target_index, anim.previous_index, anim.source)
            });
            let duration = swiper.config.animation_duration.max(1);
            swiper.anim = Some(AnimState {
                from: swiper.anim_offset,
                to: target as f64,
                start: Instant::now(),
                duration_ms: duration,
                previous_index: current,
                target_index: target,
                source: source.to_string(),
            });
            superseded
        })
        .flatten();
        if let Some((ti, tp, ts)) = superseded {
            emit_transition_end(key, ti, tp, &ts);
        }
        let container = components()
            .get(key)
            .map(|entry| entry.container)
            .unwrap_or(0);
        unsafe {
            let _ = WindowsAndMessaging::SetTimer(
                Some(HWND(container as *mut _)),
                SWIPER_ANIM_TIMER_ID,
                SWIPER_ANIM_INTERVAL_MS,
                None,
            );
        }
    } else {
        with_swiper(key, |swiper| {
            swiper.anim = None;
            swiper.anim_offset = target as f64;
        });
        apply_layout(key);
        emit_transition_end(key, target, current, source);
        refresh_playback(key);
    }

    schedule_autoplay(key);
}

pub(super) fn on_swiper_anim_timer(container: HWND) {
    let Some(key) = component_key_for_container(container) else {
        unsafe {
            let _ = WindowsAndMessaging::KillTimer(Some(container), SWIPER_ANIM_TIMER_ID);
        }
        return;
    };

    let Some((done, size)) = with_swiper(&key, |swiper| {
        let Some(anim) = swiper.anim.as_ref() else {
            return (true, swiper.last_size);
        };
        let elapsed = anim.start.elapsed().as_millis() as f64;
        let t = (elapsed / anim.duration_ms as f64).clamp(0.0, 1.0);
        let eased = smoothstep(t);
        swiper.anim_offset = anim.from + (anim.to - anim.from) * eased;
        (t >= 1.0, swiper.last_size)
    }) else {
        unsafe {
            let _ = WindowsAndMessaging::KillTimer(Some(container), SWIPER_ANIM_TIMER_ID);
        }
        return;
    };

    layout_swiper_children(&key, size.0, size.1);

    if done {
        unsafe {
            let _ = WindowsAndMessaging::KillTimer(Some(container), SWIPER_ANIM_TIMER_ID);
        }
        let finished = with_swiper(&key, |swiper| {
            swiper.anim.take().map(|anim| {
                swiper.anim_offset = anim.to;
                (anim.target_index, anim.previous_index, anim.source)
            })
        })
        .flatten();
        apply_layout(&key);
        if let Some((ti, tp, ts)) = finished {
            emit_transition_end(&key, ti, tp, &ts);
        }
        refresh_playback(&key);
    }
}

fn smoothstep(t: f64) -> f64 {
    t * t * (3.0 - 2.0 * t)
}

// ---------------------------------------------------------------------------
// Playback / autoplay / visibility
// ---------------------------------------------------------------------------

/// Plays the current page's video (when the page is in the foreground) and
/// pauses every other video; the swiper's analogue of macOS
/// `refreshVisiblePagesPlayback`.
fn refresh_playback(key: &str) {
    let Some((context, current, players)) = (|| {
        let components = components();
        let entry = components.get(key)?;
        let swiper = entry.swiper.as_ref()?;
        Some((
            entry.context.clone(),
            swiper.current_index,
            swiper
                .pages
                .iter()
                .enumerate()
                .map(|(index, page)| (index, page.player.clone()))
                .collect::<Vec<_>>(),
        ))
    })() else {
        return;
    };
    let foreground = page_is_foreground(&context);
    for (index, player) in players {
        let Some(player) = player else {
            continue;
        };
        if index == current && foreground {
            player.play();
        } else {
            player.pause();
        }
    }
}

/// Resumes/pauses on page foreground changes (mirrors the macOS manager's
/// active/inactive handling, which pauses the current video when hidden).
pub(super) fn swiper_on_visible(key: &str, active: bool) {
    if active {
        refresh_playback(key);
        schedule_autoplay(key);
    } else {
        let players = with_swiper(key, |swiper| {
            swiper
                .pages
                .iter()
                .filter_map(|page| page.player.clone())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
        for player in players {
            player.pause();
        }
        stop_autoplay(key);
    }
}

fn schedule_autoplay(key: &str) {
    let Some((container, autoplay, count, looping, current, interval, context)) = (|| {
        let components = components();
        let entry = components.get(key)?;
        let swiper = entry.swiper.as_ref()?;
        Some((
            entry.container,
            swiper.config.autoplay,
            swiper.config.items.len(),
            swiper.config.looping,
            swiper.current_index,
            swiper.config.interval,
            entry.context.clone(),
        ))
    })() else {
        return;
    };

    unsafe {
        let _ = WindowsAndMessaging::KillTimer(
            Some(HWND(container as *mut _)),
            SWIPER_AUTOPLAY_TIMER_ID,
        );
    }

    let can_advance = looping || current < count.saturating_sub(1);
    if autoplay && count > 1 && can_advance && page_is_foreground(&context) {
        unsafe {
            let _ = WindowsAndMessaging::SetTimer(
                Some(HWND(container as *mut _)),
                SWIPER_AUTOPLAY_TIMER_ID,
                interval,
                None,
            );
        }
    }
}

fn stop_autoplay(key: &str) {
    if let Some(container) = components().get(key).map(|entry| entry.container) {
        unsafe {
            let _ = WindowsAndMessaging::KillTimer(
                Some(HWND(container as *mut _)),
                SWIPER_AUTOPLAY_TIMER_ID,
            );
        }
    }
}

pub(super) fn on_swiper_autoplay_timer(container: HWND) {
    unsafe {
        // One-shot: `go_to` reschedules the next tick.
        let _ = WindowsAndMessaging::KillTimer(Some(container), SWIPER_AUTOPLAY_TIMER_ID);
    }
    let Some(key) = component_key_for_container(container) else {
        return;
    };
    go_by(&key, 1, "autoplay");
}

// ---------------------------------------------------------------------------
// Events
// ---------------------------------------------------------------------------

fn item_payload_at(key: &str, index: usize) -> Value {
    with_swiper(key, |swiper| {
        swiper
            .config
            .items
            .get(index)
            .map(SwiperItem::payload)
            .unwrap_or_else(|| json!({}))
    })
    .unwrap_or_else(|| json!({}))
}

fn emit_change(key: &str, index: usize, previous: usize, source: &str) {
    let item = item_payload_at(key, index);
    emit_event(
        key,
        "change",
        json!({ "index": index, "previousIndex": previous, "item": item, "source": source }),
    );
}

fn emit_transition_end(key: &str, index: usize, previous: usize, source: &str) {
    let item = item_payload_at(key, index);
    emit_event(
        key,
        "transitionend",
        json!({ "index": index, "previousIndex": previous, "item": item, "source": source }),
    );
}

fn emit_end_reached(key: &str, index: usize, source: &str) {
    let item = item_payload_at(key, index);
    emit_event(
        key,
        "endreached",
        json!({ "index": index, "item": item, "source": source }),
    );
}

// ---------------------------------------------------------------------------
// Page / dot windows
// ---------------------------------------------------------------------------

fn create_page_window(container: HWND) -> Option<HWND> {
    unsafe {
        WindowsAndMessaging::CreateWindowExW(
            WINDOW_EX_STYLE::default(),
            swiper_page_class(),
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
    }
    .ok()
}

fn create_dot_window(container: HWND) -> Option<HWND> {
    unsafe {
        WindowsAndMessaging::CreateWindowExW(
            WINDOW_EX_STYLE::default(),
            swiper_dot_class(),
            PCWSTR::null(),
            WINDOW_STYLE(WindowsAndMessaging::WS_CHILD.0 | WindowsAndMessaging::WS_CLIPSIBLINGS.0),
            0,
            0,
            8,
            8,
            Some(container),
            None,
            GetModuleHandleW(None)
                .ok()
                .map(|module| HINSTANCE(module.0)),
            None,
        )
    }
    .ok()
}

fn swiper_page_class() -> PCWSTR {
    static REGISTERED: OnceLock<()> = OnceLock::new();
    REGISTERED.get_or_init(|| {
        let class = WNDCLASSW {
            style: WindowsAndMessaging::CS_DBLCLKS,
            lpfnWndProc: Some(swiper_page_proc),
            hInstance: unsafe { GetModuleHandleW(None) }
                .map(|module| HINSTANCE(module.0))
                .unwrap_or_default(),
            lpszClassName: w!("LingXiaSwiperPage"),
            hbrBackground: HBRUSH(unsafe { GetStockObject(BLACK_BRUSH) }.0),
            ..Default::default()
        };
        unsafe {
            WindowsAndMessaging::RegisterClassW(&class);
        }
    });
    w!("LingXiaSwiperPage")
}

fn swiper_dot_class() -> PCWSTR {
    static REGISTERED: OnceLock<()> = OnceLock::new();
    REGISTERED.get_or_init(|| {
        let class = WNDCLASSW {
            lpfnWndProc: Some(swiper_dot_proc),
            hInstance: unsafe { GetModuleHandleW(None) }
                .map(|module| HINSTANCE(module.0))
                .unwrap_or_default(),
            lpszClassName: w!("LingXiaSwiperDot"),
            ..Default::default()
        };
        unsafe {
            WindowsAndMessaging::RegisterClassW(&class);
        }
    });
    w!("LingXiaSwiperDot")
}

/// Resolves a page window back to its component key and item index.
fn page_lookup(page: HWND) -> Option<(String, usize)> {
    let container = unsafe { WindowsAndMessaging::GetParent(page) }.ok()?;
    let key = component_key_for_container(container)?;
    let index = {
        let components = components();
        let swiper = components.get(&key)?.swiper.as_ref()?;
        swiper
            .pages
            .iter()
            .position(|p| p.window == page.0 as isize)?
    };
    Some((key, index))
}

fn page_is_image(page: HWND) -> bool {
    page_lookup(page)
        .and_then(|(key, index)| {
            with_swiper(&key, |swiper| {
                swiper.pages.get(index).map(|p| p.kind == ItemKind::Image)
            })
            .flatten()
        })
        .unwrap_or(false)
}

unsafe extern "system" fn swiper_page_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        // Image pages paint their decoded bitmap; video pages let MFPlay's
        // subclass paint (it forwards everything else here).
        WindowsAndMessaging::WM_PAINT if page_is_image(hwnd) => {
            paint_image_page(hwnd);
            LRESULT(0)
        }
        WindowsAndMessaging::WM_LBUTTONUP => {
            handle_page_tap(hwnd);
            LRESULT(0)
        }
        _ => unsafe { WindowsAndMessaging::DefWindowProcW(hwnd, msg, wparam, lparam) },
    }
}

fn paint_image_page(hwnd: HWND) {
    let Some((key, index)) = page_lookup(hwnd) else {
        return;
    };
    let Some((image, fit)) = with_swiper(&key, |swiper| {
        swiper
            .pages
            .get(index)
            .map(|page| (page.image, swiper.config.object_fit))
    })
    .flatten() else {
        return;
    };

    unsafe {
        let mut paint = PAINTSTRUCT::default();
        let dc = BeginPaint(hwnd, &mut paint);
        let mut client = RECT::default();
        let _ = WindowsAndMessaging::GetClientRect(hwnd, &mut client);
        let _ = FillRect(dc, &client, HBRUSH(GetStockObject(BLACK_BRUSH).0));

        if image != 0 && client.right > 0 && client.bottom > 0 {
            let image = image as *mut GpImage;
            let mut width = 0u32;
            let mut height = 0u32;
            let _ = GdipGetImageWidth(image, &mut width);
            let _ = GdipGetImageHeight(image, &mut height);
            if width > 0 && height > 0 {
                let client_w = client.right as f64;
                let client_h = client.bottom as f64;
                let iw = width as f64;
                let ih = height as f64;
                let (draw_w, draw_h) = match fit {
                    ObjectFit::Fill => (client_w, client_h),
                    ObjectFit::Contain => {
                        let scale = (client_w / iw).min(client_h / ih);
                        (iw * scale, ih * scale)
                    }
                    ObjectFit::Cover => {
                        let scale = (client_w / iw).max(client_h / ih);
                        (iw * scale, ih * scale)
                    }
                };
                // Centered; `cover` overflow is clipped to the page's client
                // DC, `contain` letterboxes onto the black fill.
                let x = ((client_w - draw_w) / 2.0).round() as i32;
                let y = ((client_h - draw_h) / 2.0).round() as i32;
                let mut graphics: *mut GpGraphics = std::ptr::null_mut();
                if GdipCreateFromHDC(dc, &mut graphics).0 == 0 && !graphics.is_null() {
                    let _ = GdipSetInterpolationMode(graphics, InterpolationModeHighQualityBicubic);
                    let _ = GdipDrawImageRectI(
                        graphics,
                        image,
                        x,
                        y,
                        draw_w.round() as i32,
                        draw_h.round() as i32,
                    );
                    let _ = GdipDeleteGraphics(graphics);
                }
            }
        }

        let _ = EndPaint(hwnd, &paint);
    }
}

fn handle_page_tap(hwnd: HWND) {
    let Some((key, index)) = page_lookup(hwnd) else {
        return;
    };
    let Some((item, suppress)) = with_swiper(&key, |swiper| {
        let item = swiper.config.items.get(index).cloned()?;
        // A video with native controls owns its own interaction (parity with
        // macOS, which installs no tap gesture in that case).
        let effective_controls = item.controls.unwrap_or(swiper.config.controls);
        let suppress = item.kind == ItemKind::Video && effective_controls;
        Some((item, suppress))
    })
    .flatten() else {
        return;
    };
    if suppress {
        return;
    }
    emit_event(
        &key,
        "tap",
        json!({ "index": index, "item": item.payload() }),
    );
}

unsafe extern "system" fn swiper_dot_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        WindowsAndMessaging::WM_ERASEBKGND => {
            let color = unsafe {
                WindowsAndMessaging::GetWindowLongPtrW(hwnd, WindowsAndMessaging::GWLP_USERDATA)
            } as u32;
            let hdc = HDC(wparam.0 as *mut _);
            let mut rect = RECT::default();
            unsafe {
                let _ = WindowsAndMessaging::GetClientRect(hwnd, &mut rect);
                let brush = CreateSolidBrush(COLORREF(color));
                let _ = FillRect(hdc, &rect, brush);
                let _ = DeleteObject(HGDIOBJ(brush.0));
            }
            LRESULT(1)
        }
        // Dots are indicators only; let clicks fall through to the page
        // beneath so taps still register.
        WindowsAndMessaging::WM_NCHITTEST => LRESULT(WindowsAndMessaging::HTTRANSPARENT as isize),
        _ => unsafe { WindowsAndMessaging::DefWindowProcW(hwnd, msg, wparam, lparam) },
    }
}
