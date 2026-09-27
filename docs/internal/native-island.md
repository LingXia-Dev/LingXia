# Inline native island

For contributors changing the `LxNativeRoot` island. The author-facing contract
is [`docs/skill/lxapp/components.md`](../skill/lxapp/components.md).

## Lifecycle

The host owns the island container. Removing an `LxNativeRoot`, or destroying
or replacing its page WebView, unmounts every node owned by that Root, stops
its native video resources, and resets its lease before accepting a new
commit. Navigating back therefore cannot leave a stale video surface or an
invisible touch-blocking overlay.

## Layout and scrolling

Layout snapshots use unscrolled document CSS coordinates plus the current page
viewport offset. The host moves native visuals with top-level and nested
scrolling, clips partially visible nodes against overflow containers inside
and outside Root, and removes fully offscreen nodes from paint and hit testing.
Clipping preserves the video's dimensions; it never resizes the picture.

## Factories

Cover and Button are author recipes that expand to `view` / `tappable` before
the host commit. Host factories are only `root`, `view`, `text`, `tappable`,
and `video`. `LxPicker`, `LxMediaSwiper`, and `LxNavigator` use the presenter
overlay channel instead.

## Style resolution

Buttons measure their label/icon and default to inline-flex; author CSS wins.
Text typography props take part in DOM measurement. Inherited font and colour
are resolved before nodes are sent. CSSOM updates, pseudo-classes, and
media-query paint changes are observed even without geometry changes.
Browser colours (including `oklch()` and Display P3) are converted to sRGB.
Ancestor opacity multiplies into each node's alpha; there is no offscreen group
compositing. An active Root suppresses the measurement DOM's paint so
translucent native content is not drawn twice; DOM layout stays available.

## Text input

There is deliberately no native text component: the WebView engine owns IME,
autofill, selection, and accessibility. Keyboard avoidance is window-level
host configuration per platform (Android consumes IME insets; Harmony sets the
Web component's `RESIZE_CONTENT` keyboard-avoid mode; iOS WKWebView handles it
natively). `position: fixed` inputs are the edge case to test.

## Event paths

A handler that is one of `useLxPage().actions` is routed native → Rust → Logic
through CLI-generated `pageFuncBindings`, skipping the WebView round trip. A
local View function is routed native → WebView `CustomEvent` → handler.
