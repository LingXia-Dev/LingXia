This is `servo-paint` 0.7.0 with display-list batching and scroll preservation patches.

Each iframe can send a display list during one rendering update. Upstream
submits each list separately, making WebRender rebuild the whole scene for
each transaction. Many animated iframes can outpace the scene builder.

Buffer lists per Painter until Script requests a frame or another consumer
needs the scene. A shared timer schedules a flush after 16 ms for layout-query
paths that do not request a frame; this is a scheduling target, not a maximum
latency guarantee. Keep submission order at resource, input, screenshot,
geometry and teardown boundaries, and append the final scroll offsets once.

Normal flushes submit scene data without generating extra frames: Script and
scrolling retain their original presentation boundaries and Canvas barrier.
Image updates flush before changing Canvas readiness. All-hidden Painters
retain upstream immediate scene submission.
Native visibility must still be propagated by the embedder.

Pipeline initialization and removal also restore the remaining pipelines' scroll
offsets in the scene transaction. WebRender resets async offsets on a scene
rebuild; omitting them can flash a scrolled parent at the top for one frame when
an iframe appears or disappears.

Source: crates.io `servo-paint` 0.7.0, MPL-2.0. Changes are confined to
`painter.rs`, `paint.rs`, and `webview_renderer.rs`. This replaces an existing
dependency; it does not introduce another rendering component. Workspace
Cargo patches are not inherited by downstream crate consumers.

Remove when our Servo version batches these scene updates and preserves offsets
across pipeline changes upstream. Before
removal, compare a many-iframe fixture and a real article page; verify Canvas
and DOM screenshot freshness, navigation/removal, hidden/resumed views, and
memory after closing tabs. Callback intervals are not screen frame-rate data.

For scroll preservation, use native swipes to scroll a tall two-color page, then
insert/remove offscreen iframes while recording frames. Its top color must never
flash while DOM scrollY remains beyond the top band. Script-only scrolling
does not reproduce the asynchronous scroll reset.
