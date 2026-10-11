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

### iOS composition

Each Root owns a shadow-DOM scroll anchor and a native container. Match the
anchor using its bounds, viewport position, and unique two-axis overflow extent;
never choose the nearest scroll view. Mount into a matching `WKChildScrollView`
so WebKit owns ancestor scrolling, clipping, and ordering against HTML. UIKit
operations are public, but WebKit's subview hierarchy is undocumented: missing,
ambiguous, replaced, transformed, zoomed, translucent, or overflowing anchors
use the overlay path. A bounded retry handles asynchronous layer creation.

Keep the Root itself visible; suppress only its slotted measurement children.
Composited node coordinates subtract the anchor's document origin, and only
clips inside Root are applied natively. Do not viewport-cull these nodes from a
JS snapshot: WebKit can scroll them into view before the next JS frame.
Snapshots must match the current Root generation and tree revision, and advance
that Root's geometry revision.

Run WebKit's hit test first. A composited native container may receive a touch
only when WebKit chose its anchor subtree; HTML above it keeps its touches.
The fallback overlay retains overlay ordering. Native hit tests honor ancestor
clipping and alpha, paint order, and transparent container holes. Restore the
anchor's pan recognizer when detaching; Root destroy/page teardown unregisters
the container and cancels layer retries.

On an iOS device or simulator, run `NATIVE-ISLAND-001` with `platform=ios`;
it requires `data-lx-native-presentation="same-layer"` on the video Root.
Also verify actual native taps with an HTML overlay above/below the Root,
nested inertial scrolling while JS is busy, overlapping Roots, clipping at
rounded ancestors, fullscreen video return, and page back/unmount in both themes.
`NativeRootCompositionTests` covers UIKit routing and fallback lifecycle;
DOM/Node tests alone cannot verify WebKit's layer hierarchy or visual smoothness.

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

Every native component event is routed native → WebView `CustomEvent` →
handler. A handler that is one of `useLxPage().actions` then calls Logic like
any other action.
