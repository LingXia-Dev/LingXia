This is `servo-paint` 0.7.0 with a temporary display-list batching patch.

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

Source: crates.io `servo-paint` 0.7.0, MPL-2.0. Changes are confined to
`painter.rs`, `paint.rs`, and `webview_renderer.rs`. This replaces an existing
dependency; it does not introduce another rendering component. Workspace
Cargo patches are not inherited by downstream crate consumers.

Remove when our Servo version batches these scene updates upstream. Before
removal, compare a many-iframe fixture and a real article page; verify Canvas
and DOM screenshot freshness, navigation/removal, hidden/resumed views, and
memory after closing tabs. Callback intervals are not screen frame-rate data.
