# Adaptive UI

Write one declarative page for all screen sizes. Never check device type,
`formFactor`, user agent, or `screen.width` — branch only on `sizeClass`
(`compact < 600 dp` vs `regular >= 600 dp`, with `±24 dp` hysteresis).

**Three rules cover 95% of pages:**
- List-detail drill-down: use `<ListDetail>` from `@liulidesign/core/ListDetail` (or `@lingxia/react`).
- Primary + companion pane: use `<Split>` from `@liulidesign/core/Split` (or `@lingxia/react`).
- Responsive grids and visibility: use the scope-aware `regular:` variant (`grid-cols-1 regular:grid-cols-2`, `regular:hidden`).

## `<page>` title and `bleed`

Declare `<page title={t('orders')} headerActions={...}>` for standard pages.
Omit `title` and add `bleed` (`boolean | 'top' | 'bottom' | 'horizontal'`) on a
**direct child** of `<page>` for full-bleed maps, video, or hero covers;
non-bleed floating controls stay inside safe areas automatically. Placing
`bleed` on a non-direct child throws `UI_BLEED_PARENT`.

For custom headers (e.g. a search bar), use `<TopBar>` from `@liulidesign/core/TopBar`.
The page already keeps content clear of host chrome, so never pad a header by `env.insets`.

`bleed` on the first in-flow child reaches the top edge and on the last the bottom
edge; a child in between bleeds only sideways.

## `<ListDetail>`

```tsx
import { ListDetail } from '@liulidesign/core/ListDetail';

<page title={t('orders')}>
  <ListDetail value={data.selectedId} onChange={actions.selectOrder} listLabel={t('orders')}>
    <ListDetail.List>
      <OrderList items={data.orders} selectedId={data.selectedId} onSelect={actions.selectOrder} />
    </ListDetail.List>
    <ListDetail.Detail title={data.currentOrder?.title} placeholder={<EmptyState title={t('selectOrder')} />}>
      {data.currentOrder && <OrderDetail order={data.currentOrder} />}
    </ListDetail.Detail>
  </ListDetail>
</page>
```

- On `compact`, selecting an item drills into `Detail`, preserves list scroll
  position, and intercepts Back (never Home) via `useBackHandler` to return to
  the list.
- On `regular`, both panes sit side-by-side, snap to a vertical fold hinge
  (`env.hinge`) when present, and wrap each pane in `<SizeClassScope value="compact">`
  so nested cards never trigger wide-screen grids inside a 360 dp column.

## `<Split>`

```tsx
import { Split } from '@liulidesign/core/Split';

// compact="stack" (default): top/bottom on compact, left/right on regular,
// and upper-leaf / lower-leaf automatically on half-opened tabletop folds.
<Split aside={<LiveChat messages={data.messages} />} asideWidth={320}>
  <VideoPlayer src={data.streamUrl} />
</Split>

// compact="sheet": turns `aside` into a draggable BottomSheet on compact.
<Split compact="sheet" bindOpen={commentsOpen} asideTitle={t('comments')} aside={<CommentThread />}>
  <DocumentView />
</Split>
```

## Scope-aware `regular:` and `view.env`

`regular:` binds to the nearest `<SizeClassScope>` (falling back to
`env.sizeClass`), so a card using `grid-cols-1 regular:grid-cols-2` stays
single-column inside `<ListDetail.Detail>` or a narrow aside dock.

`view.env` (GPU) and `useLxViewEnvironment()` (React, field-tracked Proxy)
expose the same canvas facts: `sizeClass`, `viewport`, logical `insets` and
`safeArea` (`top`, `bottom`, `inlineStart`, `inlineEnd`), `hinge`, `keyboard`,
`pointer`, `direction`, `theme`, `highContrast`, `reducedMotion`,
`displayLanguage`, `contentLanguage`, and `platform` (for shortcut glyphs only).
React pages always read `hinge` as `null`; only GPU pages get fold geometry.

## Test runtime switching

Switch form factors and fold postures in specs or with `lxdev device set --preset fold --posture open` (Runner only; a posture the device lacks fails):

```ts
spec('adapts across compact, foldable, and desktop', async (t) => {
  await t.device.set({ preset: 'phone', orientation: 'portrait' });
  await t.device.set({ preset: 'fold', posture: 'open' });
  await t.device.set({ preset: 'fold', posture: 'tabletop' });
  await t.device.set({ preset: 'pad', orientation: 'landscape' });
});
```
