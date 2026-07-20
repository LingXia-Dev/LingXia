# Adaptive LxApp Views

Use LingXia's surface context when an lxapp must change its component tree or
interaction model for different available sizes. Do not infer a device family
from the user agent, `screen.width`, or a browser-only media query.

## Surface context

The generated `@lingxia/types` declarations are authoritative:

```ts
type SurfaceContext = {
  sizeClass: 'compact' | 'medium' | 'expanded';
  width: number;
  height: number;
};

lx.onSurfaceContext(
  handler: (context: SurfaceContext) => void,
): () => void;
```

The subscription invokes the handler immediately, then only when the actual
surface viewport changes. `width` and `height` use logical pixels. `sizeClass`
uses the following ranges with platform-managed hysteresis:

| Size class | Actual surface viewport width |
|---|---:|
| `compact` | less than 600 |
| `medium` | 600 through 840 |
| `expanded` | greater than 840 |

Content size class is scoped to the lxapp surface. It is not the shell size
class. An aside inside an expanded desktop shell can receive `compact`.

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

Page<PageData>({
  data: {
    surfaceContext: {
      sizeClass: 'compact',
      width: 0,
      height: 0,
    },
  },

  onLoad() {
    const unsubscribe = lx.onSurfaceContext((surfaceContext) => {
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
const CompactView = lazy(() => import('./views/compact-view'));
const WorkspaceView = lazy(() => import('./views/workspace-view'));

export default function PageView() {
  const page = useLxPage<Partial<PageData>, PageActions>();
  if (!page.data.surfaceContext) {
    return <PageSkeleton />;
  }
  const View = page.data.surfaceContext.sizeClass === 'compact'
    ? CompactView
    : WorkspaceView;

  return (
    <Suspense fallback={<PageSkeleton />}>
      <View page={page} />
    </Suspense>
  );
}
```

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
const phone = devices.find((device) => device.width < 600)!;
const desktop = devices.find((device) => device.width > 840)!;

await auto.device.set({ id: phone.id });
await app.page.waitFor({ css: '[data-view="compact"]' });

await auto.device.set({ id: desktop.id });
await app.page.waitFor({ css: '[data-view="workspace"]' });
```

Assert that the old View is absent from the DOM and that Logic-owned state is
still visible after each switch. Add a medium preset when the product gives
`medium` distinct behavior.
