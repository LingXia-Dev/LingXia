This is `servo-script` 0.7.0 with a temporary IntersectionObserver crash fix.

When an observed target lives inside an iframe whose container has no layout
box (for example, `display: none`), upstream unwraps the missing padding box
while mapping coordinates into the ancestor viewport. This panics the script
thread. It was reproduced on an ifeng article and with a local iframe fixture.

Return no intersection when that box is absent. The existing caller reports
`isIntersecting: false`; showing the iframe again resumes normal observation.
The original layout query and reflow behavior are preserved.

Source: crates.io `servo-script` 0.7.0, MPL-2.0. The only upstream source change
is in `dom/intersectionobserver/intersectionobserver.rs`. Both workspace and
standalone showcase manifests must select this patch while native
IntersectionObserver is enabled. Root Cargo patches are not inherited by
downstream crate consumers.

Remove this override when our Servo version handles a missing iframe padding
box. Verify visible → hidden → visible nested-frame observation and the
DeepSeek homepage before removal.
