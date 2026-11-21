//! `<webview>` host coordination: mount reconciliation, readiness hole-punch triggers,
//! per-frame message coalescing + 64-entry FIFO queue, and web-initiated full-screen sync.
use super::{Guard, Inner, ViewHost};
use crate::platform::PlatformAdapter;
use crate::protocol::limits::{WEB_MESSAGE_BYTES, WEB_MESSAGE_QUEUE};
use crate::scene::webview::{WebViewInbound, WebViewMount, mount_of};
use std::collections::{BTreeMap, HashMap, HashSet, VecDeque};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// How long a capture waits for the web views' pictures before it reports them.
const PICTURE_WAIT: Duration = Duration::from_secs(3);

#[derive(Default)]
struct Mailbox {
    queued: VecDeque<(String, String)>,
    coalesced: Option<(String, String)>,
    lifecycle: Vec<serde_json::Value>,
}

impl Mailbox {
    /// A coalesced message waiting when a queued one arrives was sent first: it moves into the
    /// queue ahead of it, so a drag's last move cannot land after the end that follows it.
    fn post(&mut self, json: String, origin: String, queued: bool) {
        if !queued {
            self.coalesced = Some((json, origin));
            return;
        }
        let earlier = self.coalesced.take();
        for message in earlier.into_iter().chain([(json, origin)]) {
            if self.queued.len() >= WEB_MESSAGE_QUEUE {
                self.queued.pop_front();
                if self.lifecycle.len() < WEB_MESSAGE_QUEUE {
                    self.lifecycle.push(serde_json::json!({
                        "type": "error",
                        "code": "WEBVIEW_MESSAGE_DROPPED",
                        "message": format!(
                            "queued message limit ({WEB_MESSAGE_QUEUE}) exceeded; oldest message dropped"
                        ),
                    }));
                }
            }
            self.queued.push_back(message);
        }
    }

    fn is_empty(&self) -> bool {
        self.queued.is_empty() && self.coalesced.is_none() && self.lifecycle.is_empty()
    }

    fn drain(&mut self) -> Vec<serde_json::Value> {
        let mut out = Vec::with_capacity(
            self.queued.len() + usize::from(self.coalesced.is_some()) + self.lifecycle.len(),
        );
        for (json, origin) in self.queued.drain(..) {
            out.push(serde_json::json!({
                "type": "message",
                "json": json,
                "origin": origin,
            }));
        }
        if let Some((json, origin)) = self.coalesced.take() {
            out.push(serde_json::json!({
                "type": "message",
                "json": json,
                "origin": origin,
            }));
        }
        out.append(&mut self.lifecycle);
        out
    }
}

#[derive(Default)]
pub(super) struct WebViewState {
    pub(super) scope: Option<u64>,
    pending_ready: BTreeMap<String, bool>,
    mailboxes: BTreeMap<String, Mailbox>,
    native_fullscreen: HashSet<String>,
    awaiting_fullscreen: HashSet<String>,
    last_mounts: Vec<WebViewMount>,
    /// The web views the last frame's scene degraded, picture or not.
    degraded: HashSet<String>,
    /// When each web view's latest picture was bound.
    pictured: HashMap<String, Instant>,
    /// The last presented frame.
    presented: Option<Instant>,
    /// A capture's request for pictures: when, and of which web views.
    capture_ask: Option<(Instant, Vec<String>)>,
}

impl WebViewState {
    pub(super) fn reset(&mut self) {
        if let Some(scope) = self.scope {
            crate::scene::webview::forget_scope(scope);
        }
        self.pending_ready.clear();
        self.mailboxes.clear();
        self.native_fullscreen.clear();
        self.awaiting_fullscreen.clear();
        self.last_mounts.clear();
        self.degraded.clear();
        self.pictured.clear();
        self.capture_ask = None;
    }

    pub(super) fn set_scope(&mut self, scope: u64) -> bool {
        self.scope = Some(scope);
        let mut changed = false;
        for (web_id, ready) in std::mem::take(&mut self.pending_ready) {
            changed |= crate::scene::webview::set_ready(scope, &web_id, ready);
        }
        changed
    }

    fn has_pending_events(&self) -> bool {
        self.mailboxes
            .values()
            .any(|box_state| !box_state.is_empty())
    }
}

impl<A: PlatformAdapter> ViewHost<A> {
    /// Delivers an inbound event or readiness update from embedded WebView `web_id`.
    /// Safe to call from any thread.
    pub fn webview_event(&self, web_id: String, event: WebViewInbound) {
        let inner = self.inner.clone();
        self.inner
            .adapter
            .post(Box::new(move || inner.on_webview_event(web_id, event)));
    }

    /// A PNG picture of `web_id`, asked for with
    /// [`crate::platform::HostEvents::picture_webviews`]. Safe to call from any thread.
    pub fn webview_picture(&self, web_id: String, png: Vec<u8>) {
        let inner = self.inner.clone();
        super::decode::decode(png, crate::scene::image::Bucket::FULL, move |bitmap| {
            let host = inner.clone();
            inner
                .adapter
                .post(Box::new(move || host.bind_webview_picture(web_id, bitmap)));
        });
    }

    /// Main thread, before an automation capture: `Ok` once every shown `<webview>` has a
    /// picture newer than this capture's request and a frame holding it has presented, so the
    /// capture draws them in place. An `E_WEBVIEW_NOT_READY` error names the web views that
    /// have not drawn yet; `allow_not_ready` captures those as their box's background.
    pub fn prepare_web_capture(&self, allow_not_ready: bool) -> Result<(), String> {
        self.web_capture(allow_not_ready, true)
    }

    /// Before a capture that shows web views as the screen does (Android's web textures,
    /// Harmony's composited snapshot): `E_WEBVIEW_NOT_READY` while a shown `<webview>` has
    /// not drawn, unless `allow_not_ready` takes its box's background, as the screen shows it.
    pub fn web_capture_ready(&self, allow_not_ready: bool) -> Result<(), String> {
        self.web_capture(allow_not_ready, false)
    }

    fn web_capture(&self, allow_not_ready: bool, pictures: bool) -> Result<(), String> {
        let inner = &self.inner;
        let mut main = inner.main();
        let Some(scope) = main.webviews.scope else {
            return Ok(());
        };
        let shown: Vec<&WebViewMount> = main
            .webviews
            .last_mounts
            .iter()
            .filter(|m| m.visible_bounds[2] > 0.0 && m.visible_bounds[3] > 0.0)
            .collect();
        let (ready, loading): (Vec<&WebViewMount>, Vec<&WebViewMount>) =
            shown.into_iter().partition(|m| {
                crate::scene::webview::is_ready_key(&crate::scene::webview::make_key(
                    scope, &m.web_id,
                ))
            });
        let not_ready = |ids: Vec<&str>| {
            format!(
                "E_WEBVIEW_NOT_READY: <webview> {} has not drawn its content yet; wait for it, or capture with allowNotReady",
                ids.join(", ")
            )
        };
        if !allow_not_ready && !loading.is_empty() {
            return Err(not_ready(
                loading.iter().map(|m| m.web_id.as_str()).collect(),
            ));
        }
        let ids: Vec<String> = ready.iter().map(|m| m.web_id.clone()).collect();
        if ids.is_empty() || !pictures {
            return Ok(());
        }
        let now = Instant::now();
        let asked = match &main.webviews.capture_ask {
            Some((at, asked)) if *asked == ids && now.duration_since(*at) < PICTURE_WAIT * 2 => *at,
            _ => {
                main.webviews.capture_ask = Some((now, ids.clone()));
                let scale = main.snapshot.as_ref().map_or(1.0, |s| s.scale);
                drop(main);
                inner.adapter.picture_webviews(&ids, scale);
                return Err(format!(
                    "E_WEBVIEW_NOT_READY: picturing <webview> {}",
                    ids.join(", ")
                ));
            }
        };
        let presented = main.webviews.presented;
        let missing: Vec<&str> =
            ids.iter()
                .filter(|id| {
                    !main.webviews.pictured.get(*id).is_some_and(|at| {
                        *at >= asked && presented.is_some_and(|shown| shown > *at)
                    })
                })
                .map(String::as_str)
                .collect();
        if missing.is_empty() {
            main.webviews.capture_ask = None;
            return Ok(());
        }
        if now.duration_since(asked) < PICTURE_WAIT {
            return Err(format!(
                "E_WEBVIEW_NOT_READY: picturing <webview> {}",
                missing.join(", ")
            ));
        }
        if allow_not_ready {
            main.webviews.capture_ask = None;
            return Ok(());
        }
        Err(not_ready(missing))
    }

    /// The live `<webview>` mount owning a pointer down at device-pixel `point_px`, or `None`
    /// when a View control, touch band, dialog, or clip claims or excludes the point.
    pub fn webview_at(&self, point_px: [f32; 2]) -> Option<WebViewMount> {
        let main = self.inner.main();
        if main.disposed || main.interaction.inert() {
            return None;
        }
        let snapshot = main.snapshot.as_ref()?;
        let scale = snapshot.scale.max(f32::MIN_POSITIVE);
        let logical = [point_px[0] / scale, point_px[1] / scale];
        let minimum = main.interaction.touch_target();
        let (_, node) = snapshot.frame.webview_at(logical, minimum)?;
        mount_of(node)
    }

    /// Same as [`Self::webview_at`], taking logical surface coordinates directly (e.g. UIKit `hitTest`).
    pub fn webview_at_logical(&self, point: [f32; 2]) -> Option<WebViewMount> {
        let main = self.inner.main();
        if main.disposed || main.interaction.inert() {
            return None;
        }
        let snapshot = main.snapshot.as_ref()?;
        let minimum = main.interaction.touch_target();
        let (_, node) = snapshot.frame.webview_at(point, minimum)?;
        mount_of(node)
    }
}

impl<A: PlatformAdapter> Inner<A> {
    fn bind_webview_picture(
        self: &Arc<Self>,
        web_id: String,
        bitmap: Result<crate::scene::image::Bitmap, String>,
    ) {
        let bitmap = match bitmap {
            Ok(bitmap) => bitmap,
            Err(error) => {
                self.adapter
                    .warn(&format!("UI_WEBVIEW: picture of {web_id} failed: {error}"));
                return;
            }
        };
        let Some(scope) = ({
            let main = self.main();
            if main.disposed {
                None
            } else {
                main.webviews.scope
            }
        }) else {
            return;
        };
        let key = crate::scene::webview::make_key(scope, &web_id);
        self.work(
            Guard::Page(self.lifecycle.page()),
            move |_, session| Ok(session.bind_web_picture(&key, bitmap)),
            move |inner, result| {
                if !matches!(result, Some(Ok(true))) {
                    return;
                }
                {
                    let mut main = inner.main();
                    if main.disposed {
                        return;
                    }
                    main.webviews.pictured.insert(web_id, Instant::now());
                    main.drop_prepared();
                    main.drop_ahead();
                    main.pending = true;
                }
                inner.schedule();
            },
        );
    }

    /// Asks the platform for pictures of `web_ids` at the presented frame's scale.
    fn picture(self: &Arc<Self>, web_ids: Vec<String>) {
        if web_ids.is_empty() {
            return;
        }
        let scale = self.main().snapshot.as_ref().map_or(1.0, |s| s.scale);
        self.adapter.picture_webviews(&web_ids, scale);
    }

    pub(super) fn on_webview_event(self: &Arc<Self>, web_id: String, event: WebViewInbound) {
        let mut flush_now = false;
        let mut ready_changed = false;
        let mut request_vsync = false;
        // A picture kept from the last paint shows at once when motion starts.
        let mut picture = None;
        {
            let mut main = self.main();
            if main.disposed {
                return;
            }
            match event {
                WebViewInbound::Ready(ready) => {
                    if ready {
                        picture = Some(web_id.clone());
                    }
                    if let Some(scope) = main.webviews.scope {
                        ready_changed = crate::scene::webview::set_ready(scope, &web_id, ready);
                    } else {
                        main.webviews.pending_ready.insert(web_id, ready);
                    }
                    if ready_changed {
                        main.drop_prepared();
                        main.drop_ahead();
                        main.pending = true;
                    }
                }
                WebViewInbound::Load { url, title } => {
                    picture = Some(web_id.clone());
                    main.webviews
                        .mailboxes
                        .entry(web_id)
                        .or_default()
                        .lifecycle
                        .push(serde_json::json!({
                            "type": "load",
                            "url": url,
                            "title": title,
                        }));
                    flush_now = true;
                }
                WebViewInbound::Title(title) => {
                    main.webviews
                        .mailboxes
                        .entry(web_id)
                        .or_default()
                        .lifecycle
                        .push(serde_json::json!({"type": "title", "title": title}));
                    flush_now = true;
                }
                WebViewInbound::Blocked { url } => {
                    main.webviews
                        .mailboxes
                        .entry(web_id)
                        .or_default()
                        .lifecycle
                        .push(serde_json::json!({
                            "type": "blocked",
                            "url": url,
                        }));
                    flush_now = true;
                }
                WebViewInbound::Error { code, message } => {
                    let mailbox = main.webviews.mailboxes.entry(web_id).or_default();
                    if mailbox.lifecycle.len() < WEB_MESSAGE_QUEUE * 2 {
                        mailbox.lifecycle.push(serde_json::json!({
                            "type": "error",
                            "code": code,
                            "message": message,
                        }));
                    }
                    flush_now = true;
                }
                WebViewInbound::Fullscreen(fullscreen) => {
                    if fullscreen {
                        main.webviews.native_fullscreen.insert(web_id.clone());
                        main.webviews.awaiting_fullscreen.insert(web_id.clone());
                    } else {
                        main.webviews.native_fullscreen.remove(&web_id);
                        main.webviews.awaiting_fullscreen.remove(&web_id);
                    }
                    main.webviews
                        .mailboxes
                        .entry(web_id)
                        .or_default()
                        .lifecycle
                        .push(serde_json::json!({
                            "type": "fullscreenchange",
                            "fullscreen": fullscreen,
                        }));
                    flush_now = true;
                }
                WebViewInbound::Message {
                    json,
                    origin,
                    queued,
                } => {
                    let mailbox = main.webviews.mailboxes.entry(web_id).or_default();
                    if json.len() > WEB_MESSAGE_BYTES {
                        if mailbox.lifecycle.len() < WEB_MESSAGE_QUEUE * 2 {
                            mailbox.lifecycle.push(serde_json::json!({
                                "type": "error",
                                "code": "WEBVIEW_MESSAGE_SIZE",
                                "message": format!("web message exceeds {WEB_MESSAGE_BYTES} bytes"),
                            }));
                        }
                        flush_now = true;
                    } else {
                        mailbox.post(json, origin, queued);
                        request_vsync = true;
                    }
                }
            }
        }
        if ready_changed {
            self.schedule();
        }
        if let Some(web_id) = picture {
            self.picture(vec![web_id]);
        }
        if flush_now {
            self.flush_webviews();
        } else if request_vsync {
            self.request_frame();
        }
    }

    pub(super) fn flush_webviews(self: &Arc<Self>) {
        let (batches, fullscreen_asked) = {
            let mut main = self.main();
            if main.disposed || !main.webviews.has_pending_events() {
                return;
            }
            let fullscreen_asked: Vec<String> =
                main.webviews.awaiting_fullscreen.iter().cloned().collect();
            let mut batches = Vec::new();
            for (web_id, mut mailbox) in std::mem::take(&mut main.webviews.mailboxes) {
                let events = mailbox.drain();
                if !events.is_empty() {
                    batches.push((web_id, events));
                }
            }
            (batches, fullscreen_asked)
        };
        if batches.is_empty() {
            return;
        }
        let page = self.lifecycle.page();
        self.work(
            Guard::Page(page),
            move |inner, session| {
                let changed = session.dispatch_webview_events(&batches)?;
                let update = if changed {
                    inner.drained(session)?
                } else {
                    None
                };
                Ok((changed, update))
            },
            move |inner, result| match result {
                Some(Ok((changed, update))) => {
                    if let Some((connection, update)) = update {
                        inner.publish(connection, update);
                    }
                    let mut reject_fullscreen = Vec::new();
                    {
                        let mut main = inner.main();
                        if main.disposed || !inner.lifecycle.is_current_page(page) {
                            return;
                        }
                        for web_id in &fullscreen_asked {
                            main.webviews.awaiting_fullscreen.remove(web_id);
                        }
                        if changed {
                            main.dirty = true;
                        } else {
                            for web_id in fullscreen_asked {
                                let mounted_top = main
                                    .webviews
                                    .last_mounts
                                    .iter()
                                    .any(|m| m.web_id == web_id && m.in_top_layer);
                                if !mounted_top && main.webviews.native_fullscreen.remove(&web_id) {
                                    reject_fullscreen.push(web_id);
                                }
                            }
                        }
                    }
                    for web_id in reject_fullscreen {
                        inner.adapter.exit_webview_fullscreen(&web_id);
                    }
                    if changed {
                        inner.schedule();
                    }
                }
                Some(Err(error)) if inner.lifecycle.is_current_page(page) => inner.fail(error),
                _ => {}
            },
        );
    }

    pub(super) fn sync_webviews_on_present(self: &Arc<Self>, snapshot: &super::FrameSnapshot) {
        let mut nodes: Vec<(usize, &crate::scene::node::NodeSnapshot)> = snapshot
            .frame
            .nodes
            .iter()
            .enumerate()
            .filter(|(_, node)| node.webview.is_some())
            .collect();
        nodes.sort_by_key(|(index, node)| (node.order, *index));
        let mut mounts: Vec<WebViewMount> = nodes
            .into_iter()
            .filter_map(|(_, node)| mount_of(node))
            .collect();
        let (changed, exit_fullscreen, degrading) = {
            let mut main = self.main();
            if main.disposed {
                return;
            }
            if let Some(first) = mounts.first()
                && main.webviews.scope.is_none()
            {
                let _ = main.webviews.set_scope(first.scope);
            }
            let live_ids: HashSet<&str> = mounts.iter().map(|m| m.web_id.as_str()).collect();
            main.webviews
                .mailboxes
                .retain(|id, _| live_ids.contains(id.as_str()));
            main.webviews
                .native_fullscreen
                .retain(|id| live_ids.contains(id.as_str()));
            main.webviews
                .awaiting_fullscreen
                .retain(|id| live_ids.contains(id.as_str()));

            let mut exit_fullscreen = Vec::new();
            for mount in &mounts {
                if !mount.in_top_layer
                    && !main.webviews.awaiting_fullscreen.contains(&mount.web_id)
                    && main.webviews.native_fullscreen.remove(&mount.web_id)
                {
                    exit_fullscreen.push(mount.web_id.clone());
                }
            }
            main.webviews.presented = Some(std::time::Instant::now());
            // Starting motion: picture the view as it is now, over the one kept from its load.
            let degrading: Vec<String> = mounts
                .iter()
                .filter(|m| m.degraded && !main.webviews.degraded.contains(&m.web_id))
                .map(|m| m.web_id.clone())
                .collect();
            main.webviews.degraded = mounts
                .iter()
                .filter(|m| m.degraded)
                .map(|m| m.web_id.clone())
                .collect();
            // With no picture yet the live view stays: a box that does not follow the motion
            // for a moment beats an empty one.
            for mount in &mut mounts {
                if mount.degraded && !main.webviews.pictured.contains_key(&mount.web_id) {
                    mount.degraded = false;
                }
            }
            crate::scene::webview::occlude(&mut mounts);
            let same = main.webviews.last_mounts == mounts;
            if !same {
                main.webviews.last_mounts = mounts.clone();
            }
            (!same, exit_fullscreen, degrading)
        };
        if changed {
            self.adapter.sync_webviews(&mounts);
        }
        self.picture(degrading);
        for web_id in exit_fullscreen {
            self.adapter.exit_webview_fullscreen(&web_id);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Mailbox;

    fn sent(mailbox: &mut Mailbox) -> Vec<String> {
        mailbox
            .drain()
            .into_iter()
            .map(|event| event["json"].as_str().unwrap().to_owned())
            .collect()
    }

    /// The pin page's moves are coalesced and its end is queued: within one frame the end must
    /// still arrive last, or the page reads a drag that never ended.
    #[test]
    fn a_queued_message_never_overtakes_a_coalesced_one_sent_before_it() {
        let mut mailbox = Mailbox::default();
        let origin = || "https://example.test".to_owned();
        mailbox.post("move1".into(), origin(), false);
        mailbox.post("move2".into(), origin(), false);
        mailbox.post("done".into(), origin(), true);
        assert_eq!(sent(&mut mailbox), ["move2", "done"]);
        mailbox.post("queued".into(), origin(), true);
        mailbox.post("later".into(), origin(), false);
        assert_eq!(sent(&mut mailbox), ["queued", "later"]);
    }
}
