# Adaptive UI

Change an lxapp's component tree by available size. Never infer a device from
the user agent, `screen.width`, or a browser media query.

## Surface context

```ts
type SurfaceContext = {
  aside: boolean;                    // does the host currently offer a docked aside
  sizeClass: 'compact' | 'regular';
  width: number;
  height: number;
};
```

| Size class | Surface viewport width (logical px) |
|---|---:|
| `compact` | less than 600 |
| `regular` | 600 and above |

- The size class is the lxapp's own presentation, not the window: an lxapp in
  a narrow region of a wide desktop is `compact`. It is one value per lxapp.
- `aside` is whether the host offers a docked aside right now; read it, never
  infer it from width.
- `regular` is room, not desktop. Pair it with `formFactor`:

| | mobile | desktop |
|---|---|---|
| `compact` | folded phone, narrow tablet split | narrow desktop window |
| `regular` | unfolded fold, tablet | desktop workspace |

Do not add a third size class or View for folds or tablets; extra width on a
mobile `regular` surface is a CSS two-pane.

## Read it in the View

`useLxHost().sizeClass` and `.aside` ([host facts](./guide.md#host-facts)) are
present before the first frame and change only when the class changes. A page
that only picks a layout needs no Logic subscription. Size spacing and columns
with CSS and container queries. Older hosts report `compact`, so keep
`minRuntime` current.

Logic subscribes with `lx.surface.watchContext(cb)` only when Logic itself acts
on the context; it is called immediately, then on every viewport change, and
returns an unsubscribe (tear it down as in
[`lx.on*` subscriptions](./guide.md#lxon-subscriptions)).

## CSS or separate Views

Use CSS when only spacing, columns, or alignment change. Use separate
components when the interaction model changes (cards vs a data table, bottom
bar vs toolbar). Keep the page entry stable and lazy-load one variant:

```tsx
import { useLxHost } from '@lingxia/react';

const CompactView = lazy(() => import('./views/compact-view'));
const WorkspaceView = lazy(() => import('./views/workspace-view'));

export default function PageView() {
  const { sizeClass, formFactor } = useLxHost();
  const View =
    sizeClass === 'regular' && formFactor === 'desktop'
      ? WorkspaceView
      : CompactView;

  // Each View reads its own data with useLxPage(); nothing is passed down.
  return (
    <Suspense fallback={<PageSkeleton />}>
      <View />
    </Suspense>
  );
}
```

- Workspace is the desktop interaction; a `regular` mobile surface keeps
  CompactView.
- Switching unmounts local UI state. Keep business state and drafts in Logic;
  keep hover or an open popover in the View.
- Size-based availability is a product rule, not authorization: Logic still
  rejects unavailable actions.

## Edge-to-edge windows

`lx.surface.openPage(page, { as: 'window', chrome: 'full' })` runs the page to
the window edge; the system keeps window controls and the runtime keeps a drag
strip whose height is `--lx-page-chrome-top-inset`. Check
`lx.supports('surface.window.fullChrome')` first; the default is `'system'`.

```css
header {
  padding-top: var(--lx-page-chrome-top-inset);
}
```

Other page-chrome variables: [Page chrome CSS](./guide.md#page-chrome-css).

## Test runtime switching

Runner device changes report a new viewport. Switch in one spec:

```ts
spec('switches Compact and Workspace views', async (t) => {
  const devices = await t.automation.device.list();
  const phone = devices.find((device) => device.group === 'phone')!;
  const desktop = devices.find((device) => device.group === 'desktop')!;

  await t.automation.device.set({ id: phone.id });
  await expect(t.app.view.css('[data-view="compact"]')).toBeVisible();

  await t.automation.device.set({ id: desktop.id });
  await expect(t.app.view.css('[data-view="workspace"]')).toBeVisible();
});
```

- Switching form factor re-serves the page; a tablet or fold frame is still
  mobile, so expect CompactView there.
- `device.set` is partial; `appearance: "light" | "dark" | "system"` pins the
  simulated scheme.
- Assert the old View is gone and Logic-owned state survived.
