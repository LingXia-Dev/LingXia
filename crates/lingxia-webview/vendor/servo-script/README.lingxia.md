This is `servo-script` 0.7.0 with temporary observer and frame-scheduling fixes.

When an observed target lives inside an iframe whose container has no layout
box (for example, `display: none`), upstream unwraps the missing padding box
while mapping coordinates into the ancestor viewport. This panics the script
thread. It was reproduced on an ifeng article and with a local iframe fixture.

Return no intersection when that box is absent. The existing caller reports
`isIntersecting: false`; showing the iframe again resumes normal observation.
The original layout query and reflow behavior are preserved.

Removing the document root and querying layout can send an empty display list
without requesting a frame. Remember query-generated display lists until the
normal rendering update, so the empty scene replaces the old pixels without
presenting an intermediate state during script execution. Changes are in
`dom/document/document.rs` and `dom/window/window.rs`.

Source: crates.io `servo-script` 0.7.0, MPL-2.0. The observer change is in
`dom/intersectionobserver/intersectionobserver.rs`. Both workspace and
standalone showcase manifests must select this patch while native
IntersectionObserver is enabled. Root Cargo patches are not inherited by
downstream crate consumers.

Remove this override when our Servo version handles missing iframe padding
boxes and query-generated display lists. Verify visible → hidden → visible
nested-frame observation, the DeepSeek homepage, and root removal/reinsertion
with an intervening layout query before removal.
