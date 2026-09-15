# Adaptive LxApp Views

Use LingXia's surface context when an lxapp must change its component tree or
interaction model for different available sizes. Do not infer a device family
from the user agent, `screen.width`, or a browser-only media query.

## Surface context

The generated `@lingxia/types` declarations are authoritative:

```ts
type SurfaceContext = {
  sizeClass: 'compact' | 'regular';
  width: number;
  height: number;
};

lx.surface.onContext(
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

Content size class is scoped to the lxapp surface. It is not the shell size
class and it is not the host form. The shell still uses
`compact` / `medium` / `expanded` internally for sidebar and aside admission;
content never sees `medium` or `expanded`. An aside inside an expanded
desktop shell can receive `compact`.

`regular` means the surface is at least 600 wide. It does not mean desktop.
Pair it with `usePlatform().isDesktop` / `isMobile` (tablets and foldable
phones are mobile). Unfolding a fold flips `sizeClass` in place and does not
change host form:

| | mobile | desktop |
|---|---|---|
| `compact` | folded phone | narrow desktop window |
| `regular` | unfolded fold, tablet | desktop workspace |

Do not add a third size class for fold. Two-pane on a handheld is `regular`
on mobile — CSS or a product View, still not `medium`.

A tablet is `regular` + mobile (or `compact` + mobile in a narrow OS split).
The host shell stays device-compact: no sidebar, overlay asides. `is_pad`
only shows every tab-bar item; it is not `isDesktop()` and MUST NOT be used
to mount a desktop shell. Extra pad width is page two-pane, not WorkspaceView.

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
`lx.supports({ capability: 'surface', value: 'window', chrome: 'full' })`
before offering it.

## Runner safe areas and page chrome

Use the host's page-chrome snapshot/CSS insets for native chrome, including
custom-header pages in Runner; see [page chrome](guide.md#laying-out-under-immersive-chrome).
Browser `env(safe-area-inset-*)` alone does not describe simulated Runner
chrome. Do not compensate with a fixed phone/notch height. Capsule geometry
belongs to the View, through the framework's page-chrome helper.

## Subscribe in Logic

Keep the authoritative value in Page Logic and replicate it to the View. Store
the unsubscribe function per page instance so multiple instances of one route
do not overwrite each other.

```ts
import type { SurfaceContext } from '@lingxia/types';

type PageData = {
  surfaceContext: SurfaceContext;
};

const subscriptions = new WeakMap<object, () => void>();

Page({
  data: {
    surfaceContext: {
      sizeClass: 'compact',
      width: 0,
      height: 0,
    },
  } as PageData,

  onLoad() {
    const unsubscribe = lx.surface.onContext((surfaceContext) => {
      this.setData({ surfaceContext });
    });
    subscriptions.set(this, unsubscribe);
  },

  onUnload() {
    subscriptions.get(this)?.();
    subscriptions.delete(this);
  },
});
```

Do not store the unsubscribe function in `data`; bridge state must remain
serializable.

## Choose CSS or separate Views

Use CSS or container queries when only spacing, columns, wrapping, or alignment
changes. Use separate components when the interaction model or component tree
changes, such as cards versus a data table, a bottom action bar versus a
desktop toolbar, or a compact flow that omits workspace-only operations.

For React, keep the registered page entry stable and lazy-load one variant:

```tsx
import { useLxPage, usePlatform } from '@lingxia/react';

const CompactView = lazy(() => import('./views/compact-view'));
const WorkspaceView = lazy(() => import('./views/workspace-view'));

export default function PageView() {
  const page = useLxPage<Partial<PageData>, PageActions>();
  const { isDesktop } = usePlatform();
  if (!page.data.surfaceContext) {
    return <PageSkeleton />;
  }
  const View =
    page.data.surfaceContext.sizeClass === 'regular' && isDesktop
      ? WorkspaceView
      : CompactView;

  return (
    <Suspense fallback={<PageSkeleton />}>
      <View page={page} />
    </Suspense>
  );
}
```

Workspace is the desktop interaction, not "anything wider than a phone". A
`regular` mobile surface (unfolded fold, tablet) keeps CompactView; extra
columns there are CSS.

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

On macOS and Windows, changing between phone, tablet, and desktop presets also
updates the embedded browser identity used by external websites. The Runner
uses an engine-compatible synthetic UA (WebKit on macOS, Chromium/Android on
Windows), not the literal branded device name shown by the frame. This browser
emulation is for website compatibility only; lxapps must still use surface
context for layout and interaction decisions.

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
still visible after each switch. Do not add a third View for fold or the
shell's medium band.
