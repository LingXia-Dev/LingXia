# Adaptive UI

Change a page's layout by the room it has. Never infer a device from the user agent,
`screen.width`, or a media query.

## Size class

| `sizeClass` | Page width (logical px) |
|---|---:|
| `compact` | less than 600 |
| `regular` | 600 and above |

- Near 600 the class holds until the width is 24 px past it, so a width breakpoint
  (`min-[600px]:`) and a `sizeClass` branch disagree there: choose one, `sizeClass`.
- It is the lxapp's own room, not the window: an lxapp in a narrow region of a wide desktop is
  `compact`. A page in its own window (`as: 'window'`) reads its lxapp's value; size that page
  from its own viewport.
- `regular` is room, not desktop: an unfolded fold, a tablet and a desktop workspace are all
  `regular`. Do not branch on the device for layout, and do not add a third class.
- `canDock` is whether the window can dock an `as: 'aside'` beside the main right now. Read it,
  never infer it from width.

## GPU View

One View adapts. Read `env.sizeClass` and `env.canDock` from the render's `env` (or
`useView().env` in a component); they are there before the first frame, and a change re-renders
in place, keeping `data` and `useLocal` state.

```tsx
export function render({ data, env }: View) {
  const regular = env.sizeClass === 'regular';
  return (
    <page className="p-4">
      <view className={regular ? 'grid grid-cols-[18rem_1fr] gap-6' : 'gap-4'}>
        <OrderList orders={data.orders} />
        {regular ? <OrderDetail order={data.selected} /> : null}
      </view>
    </page>
  );
}
```

- Give both classes the same `data-testid`s, so one test covers both.
- `env.pointer` (`fine` or `coarse`) decides hover and density, independently of size.
- Size-based availability is a product rule, not authorization: Logic still rejects
  unavailable actions.

## Logic and WebView pages

- Logic reads `page.env.get()` (`{ sizeClass, canDock }`) and calls `page.env.watch(cb)` only
  when Logic itself acts on a change; `cb` runs at once, then on each class or `canDock` change,
  and stops when the page unloads. Never forward the class to the View with `page.update`.
- A WebView page reads `useLxHost().sizeClass` and `.canDock`
  ([host facts](./webview/guide.md#host-facts)); size spacing and columns with CSS and container
  queries. Older hosts report `compact`, so keep `minRuntime` current.

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

Other page-chrome variables: [Page chrome CSS](./webview/guide.md#page-chrome-css).

## Test runtime switching

Runner device changes report a new viewport. Switch in one test:

```ts
test('lays out for compact and regular', async (t) => {
  const devices = await t.automation.device.list();
  const phone = devices.find((device) => device.group === 'phone')!;
  const desktop = devices.find((device) => device.group === 'desktop')!;

  await t.automation.device.set({ id: phone.id });
  await expect(t.app.view.testId('order-detail')).toHaveCount(0);

  await t.automation.device.set({ id: desktop.id });
  await expect(t.app.view.testId('order-detail')).toBeVisible();
});
```

- `device.set` is partial; `appearance: "light" | "dark" | "system"` pins the
  simulated scheme, `textScale` (0.5 to 3) and `contrast: "standard" | "high"` the
  system text size and contrast.
- Assert that Logic-owned state survived the switch.
