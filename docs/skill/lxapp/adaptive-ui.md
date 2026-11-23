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

### Parts and variants

- A list and the detail of the chosen item: `<ListDetail>` from `@liulidesign/core/ListDetail`,
  driven by the selection (`value` with `onChange`, or `bind`). On `compact` it drills into the
  detail and Back returns to the list; on `regular` the panes sit side by side.
- A main area with a companion pane: `<Split aside={…}>` from `@liulidesign/core/Split`; on
  `compact` the aside stacks below, or opens as a sheet with `compact="sheet"`.
- Columns and visibility by class: `grid-cols-1 regular:grid-cols-2`, `regular:hidden`.
  `regular:` follows the nearest `<SizeClassScope value>`, then `env.sizeClass`; the panes of
  `<ListDetail>` and `<Split>` set it, so a card inside a 360 px pane stays single-column.
- `useBackHandler(enabled, onBack)` takes Back (never Home) while `enabled`, the latest first; on
  the home app's first page it is the only thing that does (a leave guard there lets Back background the app).

```tsx
<ListDetail value={data.selectedId} onChange={actions.selectOrder} listLabel={t('orders')}>
  <ListDetail.List>
    <OrderList orders={data.orders} onSelect={actions.selectOrder} />
  </ListDetail.List>
  <ListDetail.Detail title={data.current?.title} placeholder={<EmptyState title={t('pickOrder')} />}>
    {data.current && <OrderDetail order={data.current} />}
  </ListDetail.Detail>
</ListDetail>
```

### Title, bleed and folds

- `<page title={…}>` sets the page's navigation bar title.
- `bleed` (`true`, `'top'`, `'bottom'`, `'horizontal'`) on a direct child of `<page>` runs it to
  the screen edge under bars and cutouts (a map, a video, a cover); elsewhere it throws
  `UI_BLEED_PARENT`. Only the first in-flow child reaches the top and the last the bottom.
- A custom header row is `<TopBar>` from `@liulidesign/core/TopBar`. The page already keeps
  clear of host chrome: never pad a header by `env.insets`.
- `env.insets` and `env.safeArea` also carry `inlineStart` and `inlineEnd`, which follow the
  page's direction.
- `env.hinge` is a fold's crease in page coordinates (`x`, `y`, `width`, `height`,
  `orientation`, `posture: 'flat' | 'half-opened'`), or `null`. `<ListDetail>` and `<Split>`
  already keep their seam on it; read it only for a layout of your own.

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
- `t.device.set({ preset, posture, orientation })` picks a device by form factor
  (`phone`, `fold`, `trifold`, `pad`, `desktop`) and a fold's posture (`outer`, `open`,
  `dual`, `tabletop`); a posture the device lacks fails the step.
- Assert that Logic-owned state survived the switch.
