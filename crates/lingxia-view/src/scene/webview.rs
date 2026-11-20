//! `<webview>` scene tracking: stable per-session `wv-<n>` ids, readiness tracking so a
//! placeholder `surface` fill shows until the live native WebView has painted (no black box),
//! and mounted geometry published to platform adapters on each presented frame.
use crate::scene::node::ClipSnapshot;
use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{LazyLock, Mutex};

static EPOCH: AtomicU64 = AtomicU64::new(0);
static READY: LazyLock<Mutex<HashSet<String>>> = LazyLock::new(Default::default);

/// Inbound event or readiness change from a live embedded WebView to its [`crate::session::host::ViewHost`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WebViewInbound {
    /// Native WebView committed and painted (`true`) or lost/cleared its document (`false`).
    Ready(bool),
    Load {
        url: String,
        title: String,
    },
    /// The loaded document's title changed.
    Title(String),
    Message {
        json: String,
        origin: String,
        queued: bool,
    },
    Blocked {
        url: String,
    },
    Error {
        code: String,
        message: String,
    },
    Fullscreen(bool),
}

/// Bumped whenever any `<webview>` flips its ready state, invalidating cached GPU scenes.
pub fn epoch() -> u64 {
    EPOCH.load(Ordering::Relaxed)
}

pub fn make_key(scope: u64, web_id: &str) -> String {
    format!("{scope}/{web_id}")
}

/// The image source a `<webview>`'s last picture is bound under, drawn in its place while the
/// live view cannot show (motion, opacity, masks, blur) and in captures.
pub fn picture_src(key: &str) -> String {
    format!("lx-webview-picture://{key}")
}

/// Marks `<webview>` `(scope, web_id)` ready (or not ready) for hole-punch composition.
/// Returns `true` when the state changed.
pub fn set_ready(scope: u64, web_id: &str, ready: bool) -> bool {
    let key = make_key(scope, web_id);
    let mut set = READY.lock().unwrap_or_else(|p| p.into_inner());
    let changed = if ready {
        set.insert(key)
    } else {
        set.remove(&key)
    };
    if changed {
        EPOCH.fetch_add(1, Ordering::Relaxed);
    }
    changed
}

pub fn is_ready_key(key: &str) -> bool {
    READY
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .contains(key)
}

pub fn forget_one(scope: u64, web_id: &str) {
    let _ = set_ready(scope, web_id, false);
    crate::renderer::video_forget_key(&make_key(scope, web_id));
}

pub fn forget_scope(scope: u64) {
    let prefix = format!("{scope}/");
    let mut set = READY.lock().unwrap_or_else(|p| p.into_inner());
    let before = set.len();
    set.retain(|entry| !entry.starts_with(&prefix));
    if set.len() != before {
        EPOCH.fetch_add(1, Ordering::Relaxed);
    }
}

/// Per-node `<webview>` metadata carried on `NodeSnapshot`.
#[derive(Clone, Debug, PartialEq)]
pub struct WebViewNode {
    pub scope: u64,
    pub web_id: String,
    pub src: String,
    /// `true` on the topmost visible `<webview>` node of this `webSession` (the one owning the live native view).
    pub active: bool,
    /// `true` when this `<webview>` sits inside an open `<dialog>` or popover top layer.
    pub in_top_layer: bool,
    pub degraded: bool,
    pub on_event: Option<u64>,
    /// The page handles its own drags: one starting on it never passes to the View page.
    pub handles_drags: bool,
    /// The View page's appearance, which the web page's `prefers-color-scheme` follows.
    pub dark: bool,
    /// The content box's own size, before any transform scales it.
    pub size: [f32; 2],
}

/// Mounted `<webview>` geometry and state delivered to [`crate::platform::HostEvents::sync_webviews`].
#[derive(Clone, Debug, PartialEq)]
pub struct WebViewMount {
    pub scope: u64,
    pub web_id: String,
    pub src: String,
    pub in_top_layer: bool,
    pub degraded: bool,
    pub accessibility_hidden: bool,
    pub handles_drags: bool,
    /// The View page's appearance, which the web page's `prefers-color-scheme` follows.
    pub dark: bool,
    /// Inner content box `[x, y, width, height]` in logical surface coordinates.
    pub bounds: [f32; 4],
    /// The content box's own size: a scale in motion changes `bounds`, not the page's layout.
    pub size: [f32; 2],
    /// Intersection of `bounds` with ancestor scroll/viewport clips.
    pub visible_bounds: [f32; 4],
    pub clips: Vec<ClipSnapshot>,
    pub radius: f32,
    pub radii: Option<[[f32; 2]; 4]>,
    /// Holes of `<webview>`s painted later that overlap this one: the native view is masked out
    /// there, since native views do not stack in paint order.
    pub occluders: Vec<ClipSnapshot>,
}

/// Fills each mount's `occluders` from the live, ready mounts painted after it; `mounts` is in
/// paint order.
pub fn occlude(mounts: &mut [WebViewMount]) {
    let shapes: Vec<Option<ClipSnapshot>> = mounts
        .iter()
        .map(|mount| {
            let [_, _, w, h] = mount.visible_bounds;
            let live = !mount.degraded
                && w > 0.0
                && h > 0.0
                && is_ready_key(&make_key(mount.scope, &mount.web_id));
            // Clipped by an ancestor, the hole's corners are the clip's: the visible box is the
            // nearest shape a mask can take.
            let whole = mount.visible_bounds == mount.bounds;
            live.then_some(ClipSnapshot {
                bounds: mount.visible_bounds,
                radius: if whole { mount.radius } else { 0.0 },
                radii: if whole { mount.radii } else { None },
                transform: None,
            })
        })
        .collect();
    for (index, mount) in mounts.iter_mut().enumerate() {
        mount.occluders = shapes[index + 1..]
            .iter()
            .flatten()
            .filter(|shape| overlaps(shape.bounds, mount.visible_bounds))
            .cloned()
            .collect();
    }
}

fn overlaps(a: [f32; 4], b: [f32; 4]) -> bool {
    a[0] < b[0] + b[2] && b[0] < a[0] + a[2] && a[1] < b[1] + b[3] && b[1] < a[1] + a[3]
}

/// Extracts [`WebViewMount`] geometry from the active `<webview>` [`crate::scene::node::NodeSnapshot`].
pub fn mount_of(node: &crate::scene::node::NodeSnapshot) -> Option<WebViewMount> {
    let wv = node.webview.as_ref().filter(|wv| wv.active)?;
    let (bounds, radius, radii) = match node.content_clip {
        Some(clip) => (clip.bounds, clip.radius, clip.radii),
        None => (node.bounds, node.radius, node.radii),
    };
    let visible_bounds = if node.visible_bounds[2] <= 0.0 || node.visible_bounds[3] <= 0.0 {
        [bounds[0], bounds[1], 0.0, 0.0]
    } else {
        let left = bounds[0].max(node.visible_bounds[0]);
        let top = bounds[1].max(node.visible_bounds[1]);
        let right = (bounds[0] + bounds[2]).min(node.visible_bounds[0] + node.visible_bounds[2]);
        let bottom = (bounds[1] + bounds[3]).min(node.visible_bounds[1] + node.visible_bounds[3]);
        [left, top, (right - left).max(0.0), (bottom - top).max(0.0)]
    };
    Some(WebViewMount {
        scope: wv.scope,
        web_id: wv.web_id.clone(),
        src: wv.src.clone(),
        in_top_layer: wv.in_top_layer,
        degraded: wv.degraded,
        accessibility_hidden: node.accessibility_hidden.unwrap_or(false) || wv.degraded,
        handles_drags: wv.handles_drags,
        dark: wv.dark,
        bounds,
        size: wv.size,
        visible_bounds,
        clips: node.clips.clone(),
        radius,
        radii,
        occluders: Vec::new(),
    })
}

/// Per-renderer `<webview>` id table keyed by `webSession` (`"name:<n>"` or `"el:<id>"`):
/// keeps `wv-<n>` stable across inline boxes and `<dialog>` / sheet handoffs while issuing a
/// fresh id if a session leaves the page and later remounts.
pub(crate) struct WebViews {
    scope: u64,
    next: u64,
    ids: HashMap<String, String>,
}

impl WebViews {
    pub(crate) fn new(scope: u64) -> Self {
        Self {
            scope,
            next: 1,
            ids: HashMap::new(),
        }
    }

    pub(crate) fn scope(&self) -> u64 {
        self.scope
    }

    pub(crate) fn id_for(&mut self, session: &str) -> String {
        if let Some(existing) = self.ids.get(session) {
            return existing.clone();
        }
        let id = format!("wv-{}", self.next);
        self.next += 1;
        self.ids.insert(session.to_owned(), id.clone());
        id
    }

    pub(crate) fn retain_live(&mut self, live: &HashSet<String>) {
        let scope = self.scope;
        self.ids.retain(|session, web_id| {
            let keep = live.contains(session);
            if !keep {
                forget_one(scope, web_id);
            }
            keep
        });
    }

    pub(crate) fn clear(&mut self) {
        self.ids.clear();
        forget_scope(self.scope);
    }
}

impl Drop for WebViews {
    fn drop(&mut self) {
        forget_scope(self.scope);
    }
}
