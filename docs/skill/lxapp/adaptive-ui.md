# Adaptive LxApp Views

Use LingXia's surface context when an lxapp must change its component tree or
interaction model for different available sizes. Do not infer a device family
from the user agent, `screen.width`, or a browser-only media query.

## Surface context

The generated `@lingxia/types` declarations are authoritative:

```ts
type SurfaceContext = {
  aside: boolean;                    // does the host currently offer a docked aside
  sizeClass: 'compact' | 'regular';
  width: number;
  height: number;
};

lx.surface.watchContext(
  handler: (context: SurfaceContext) => void,
): () => void;
```

The subscription invokes the handler immediately, then only when the actual
surface viewport changes. `width` and `height` use logical pixels. `sizeClass`
uses the following ranges with platform-managed hysteresis at 600:

| Size class | Actual surface viewport width |
|---|---:|
| `compact` | less than 600 |
| `regular` | 600 and above |

Content size class is scoped to the lxapp's own presentation, not the host
window: an lxapp in a narrow region of a wide desktop shell receives `compact`.
It is one value per lxapp — a page of that lxapp shown in an aside, float or
second window sees the main presentation's context, not its own. The shell's own
`medium` / `expanded` bands drive chrome admission and never reach content.
`aside` is live host docking availability, decided by the shell rather than
derived from the content viewport — read it from this context (in Logic or the
View), never infer it from the viewport.

`regular` is room, not desktop. Pair it with `usePlatform().isDesktop`;
tablets and foldable phones are mobile, and unfolding a fold flips
`sizeClass` without changing host form:

| | mobile | desktop |
|---|---|---|
| `compact` | folded phone, narrow tablet split | narrow desktop window |
| `regular` | unfolded fold, tablet | desktop workspace |

Do not add a third size class or View for fold or tablet. Extra width on a
mobile `regular` surface is page two-pane via CSS; the host shell there stays
mobile (full tab bar on a tablet, no desktop sidebar).

## Edge-to-edge windows

`lx.surface.openPage(page, { as: 'window', chrome: 'full' })` runs the page to
the window edge while the system keeps minimize, maximize, resize, and drag.
The runtime owns a native drag strip across the top and publishes its height as
`topInset` on the page-chrome snapshot, so the window stays movable whether or
not the page cooperates — nothing is asked of the page, and there is no
opt-in to forget.

```css
header {
  padding-top: var(--lx-page-chrome-top-inset);
}
```

`chrome` defaults to `'system'`, which is the standard title bar. Ask
`lx.supports('surface.window.fullChrome')`
before offering it.

## Runner safe areas and page chrome

Use the host's page-chrome snapshot/CSS insets for native chrome, including
custom-header pages in Runner; see [page chrome](guide.md#laying-out-under-immersive-chrome).
Browser `env(safe-area-inset-*)` alone does not describe simulated Runner
chrome. Do not compensate with a fixed phone/notch height. Capsule geometry
belongs to the View, through the framework's page-chrome helper.

## Read it in the View

A View reads the context itself: `useSurfaceContext()` from `@lingxia/react` /
`@lingxia/vue`, or `getSurfaceContext()` and `subscribeSurfaceContext(cb)` from
`@lingxia/html`. It is the same value `watchContext` delivers to Logic, seeded
into the page before its first frame and updated on every change, so a page
that only picks a layout needs no Logic subscription, no `setData`, nothing in
its `data` and no gate: the hook always has a value. It re-renders its
component on every width change, so read it where layout depends on it rather
than in every component. A host older than the release that added it reports
`compact` with a 0×0 viewport forever, so keep `lxapp.json` `minRuntime` at that
release or later — `lingxia` raises it when the project moves to this line.

Subscribe in Logic only when Logic itself acts on the context — say, fetching
less on `compact`. Then keep the unsubscribe per page instance, never in `data`:

```ts
const subscriptions = new WeakMap<object, () => void>();

Page({
  onLoad() {
    subscriptions.set(this, lx.surface.watchContext(({ sizeClass }) => {
      // Logic's own use of the size class.
    }));
  },
  onUnload() {
    subscriptions.get(this)?.();
    subscriptions.delete(this);
  },
});
```

## Choose CSS or separate Views

Use CSS or container queries when only spacing, columns, wrapping, or alignment
changes. Use separate components when the interaction model or component tree
changes, such as cards versus a data table, a bottom action bar versus a
desktop toolbar, or a compact flow that omits workspace-only operations.

For React, keep the registered page entry stable and lazy-load one variant:

```tsx
import { useLxPage, usePlatform, useSurfaceContext } from '@lingxia/react';

const CompactView = lazy(() => import('./views/compact-view'));
const WorkspaceView = lazy(() => import('./views/workspace-view'));

export default function PageView() {
  const page = useLxPage<Partial<PageData>, PageActions>(); // the page's own types
  const { isDesktop } = usePlatform();
  const surface = useSurfaceContext();
  const View =
    surface.sizeClass === 'regular' && isDesktop
      ? WorkspaceView
      : CompactView;

  return (
    <Suspense fallback={<PageSkeleton />}>
      <View page={page} />
    </Suspense>
  );
}
```

Workspace is the desktop interaction, not "anything wider than a phone": a
`regular` mobile surface keeps CompactView.

The React bridge snapshot is initially empty. Gate required nested data before
reading it; keep React hooks above the gate so hook order remains stable.

Only the selected component is mounted. Both dynamic chunks remain part of the
lxapp package. Confirm build output before claiming a first-load JavaScript
reduction.

Changing the selected component unmounts its local UI state. Keep business
state, drafts that must survive, locale selection, and feature availability in
Logic. Keep transient state such as hover or an open popover in the View.

Treat size-derived feature availability as a product rule, not authorization.
Logic should still reject an unavailable action, and services must enforce
real permissions.

## Test runtime switching

LingXia Runner device-frame changes report a new surface viewport. Exercise
them in one session through automation:

```ts
const auto = lx.automation();
const app = auto.lxapp();
const devices = await auto.device.list();
const phone = devices.find((device) => device.group === 'phone')!;
const desktop = devices.find((device) => device.group === 'desktop')!;

await auto.device.set({ id: phone.id });
await app.page.waitFor({ css: '[data-view="compact"]' });

await auto.device.set({ id: desktop.id });
await app.page.waitFor({ css: '[data-view="workspace"]' });
```

Switching form factor re-serves the page (`isMobile` / `isDesktop` are fixed
for a page lifetime). A tablet or fold frame is still mobile: expect Compact
View even when `sizeClass` is `regular`.

`device.set` is a partial update: omit `id` to keep the current device, and
pass `appearance: "light" | "dark" | "system"` to pin or release the simulated
color scheme for dual-theme assertions.

Assert that the old View is absent from the DOM and that Logic-owned state is
still visible after each switch.

On macOS and Windows, switching between phone, tablet, and desktop presets also
changes the embedded browser identity external websites see. The Runner sends an
engine-compatible synthetic UA (WebKit on macOS, Chromium/Android on Windows),
not the branded device name on the frame. That emulation is for website
compatibility only — lxapps still decide layout and interaction from surface
context.
