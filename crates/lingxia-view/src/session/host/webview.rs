//! `<webview>` host coordination: mount reconciliation, readiness hole-punch triggers,
//! per-frame message coalescing + 64-entry FIFO queue, and web-initiated full-screen sync.
use super::{Guard, Inner, ViewHost};
use crate::platform::PlatformAdapter;
use crate::protocol::limits::{WEB_MESSAGE_BYTES, WEB_MESSAGE_QUEUE};
use crate::scene::webview::{WebViewInbound, WebViewMount, mount_of};
use std::collections::{BTreeMap, HashSet, VecDeque};
use std::sync::Arc;

#[derive(Default)]
struct Mailbox {
    queued: VecDeque<(String, String)>,
    coalesced: Option<(String, String)>,
    lifecycle: Vec<serde_json::Value>,
}

impl Mailbox {
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
    pub(super) fn on_webview_event(self: &Arc<Self>, web_id: String, event: WebViewInbound) {
        let mut flush_now = false;
        let mut ready_changed = false;
        let mut request_vsync = false;
        {
            let mut main = self.main();
            if main.disposed {
                return;
            }
            match event {
                WebViewInbound::Ready(ready) => {
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
                    } else if queued {
                        if mailbox.queued.len() >= WEB_MESSAGE_QUEUE {
                            mailbox.queued.pop_front();
                            if mailbox.lifecycle.len() < WEB_MESSAGE_QUEUE {
                                mailbox.lifecycle.push(serde_json::json!({
                                    "type": "error",
                                    "code": "WEBVIEW_MESSAGE_DROPPED",
                                    "message": format!(
                                        "queued message limit ({WEB_MESSAGE_QUEUE}) exceeded; oldest message dropped"
                                    ),
                                }));
                            }
                        }
                        mailbox.queued.push_back((json, origin));
                        request_vsync = true;
                    } else {
                        mailbox.coalesced = Some((json, origin));
                        request_vsync = true;
                    }
                }
            }
        }
        if ready_changed {
            self.schedule();
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
        let mounts: Vec<WebViewMount> = nodes
            .into_iter()
            .filter_map(|(_, node)| mount_of(node))
            .collect();
        let (changed, exit_fullscreen) = {
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
            let same = main.webviews.last_mounts == mounts;
            if !same {
                main.webviews.last_mounts = mounts.clone();
            }
            (!same, exit_fullscreen)
        };
        if changed {
            self.adapter.sync_webviews(&mounts);
        }
        for web_id in exit_fullscreen {
            self.adapter.exit_webview_fullscreen(&web_id);
        }
    }
}
