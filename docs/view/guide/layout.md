# Layout

```tsx
const tiles = ['Calls', 'Data', 'SMS', 'Roaming', 'Top up', 'Bills'];

export function render() {
  return (
    <page className="bg-neutral-50">
      <scroll className="grow">
        <text className="sticky top-0 px-4 py-2 text-xs font-semibold bg-neutral-200">Services</text>
        <grid className="grid-cols-[repeat(auto-fit,minmax(6rem,1fr))] gap-2 p-4">
          {tiles.map(tile => (
            <column key={tile} className="h-16 items-center justify-center rounded-lg bg-white">
              <text className="text-sm">{tile}</text>
            </column>
          ))}
        </grid>
        <row className="items-center gap-3 px-4">
          <text className="grow text-sm">Balance</text>
          <text className="text-sm font-semibold tabular-nums">12.40</text>
        </row>
      </scroll>
      <button onClick={() => {}} className="fixed bottom-6 end-6 rounded-full bg-blue-600 text-white px-5">Top up</button>
    </page>
  );
}
```

Layout is CSS flexbox and CSS Grid, computed natively. There is no block or inline flow: every box is
a flex container unless it is a grid.

## Containers

| Element | Lays out children |
| --- | --- |
| `<column>`, `<view>`, `<page>` | flex column |
| `<row>` | flex row |
| `<grid>` | CSS Grid; the same as `grid` on any container |
| `<stack>` | every child in one cell, overlapping; place them with `self-*`, `justify-self-*` and insets |
| `<scroll>` | flex column that scrolls one axis (see [Lists and scrolling](lists-and-scrolling.md)) |
| `<spacer>` | nothing: an empty box; give it `grow` or a size |

`flex-row`/`flex-col` (and their `-reverse` forms) change the direction of a `<column>`, `<row>` or `<page>`.
`flex-wrap`, `items-*`, `justify-*`, `content-*` and `gap-*` behave as in CSS.

## Where native defaults differ from CSS

- **No implicit shrinking.** `flex-shrink` defaults to `0`. Only `<scroll>`, `<list>` and boxes with `grow`
  shrink by default. Add `shrink` (or `min-w-0` on text) where CSS would have shrunk.
- `grow` also sets a zero minimum size, so a growing box never overflows its line. `grow` alone means
  `flex-grow: 1` with an `auto` basis; write `grow basis-0` (or `flex-1`) for equal columns.
- A `<scroll>` or `<list>` directly in `<page>` fills the remaining height unless it has a height, `grow` or a basis.
- Controls carry intrinsic sizes: buttons, editors and toggles have 12 px padding, editors and toggles a
  44–48 px minimum height, `<textarea>` 120 px height, `<slider>` 120 × 48, `<image>` 64 × 64 and `<vector>`
  24 × 24 until you size them.
- `justify-center` and `content-center` are *safe*: content that does not fit aligns to the start instead of
  overflowing both edges.

## Display

| Utility | Effect |
| --- | --- |
| `flex` | the default |
| `grid` | CSS Grid on any container |
| `hidden` | removed: no layout, paint, hit testing or accessibility |
| `block` | children stack in a stretched column |
| `inline-flex`, `inline-block` | as `flex`/`block`, but the box keeps its content width (`self-start`) |

`inline`, `contents`, `table`, `flow-root` and `list-item` are not supported. `invisible` is not supported;
use `hidden`, or `opacity-0` to keep the space.

## Grid

`grid-cols-N`, `grid-rows-N`, `col-span-*`, `row-span-*`, `col-start-*`, `grid-flow-*`, `auto-cols-*`,
`auto-rows-*`, `justify-items-*` and `place-*` work as in CSS. Track lists take lengths, `%`, `fr`, `auto`,
`min-content`, `max-content`, `minmax()`, `fit-content()` and `repeat(N | auto-fit | auto-fill, …)`, up to
64 tracks. Named lines, `grid-template-areas`, `subgrid`, masonry and `calc()` in tracks are refused.

```tsx
export function render() {
  return (
    <page className="p-4">
      <grid className="grid-cols-3 grid-rows-[auto_auto] gap-2">
        <view className="col-span-2 h-12 rounded-lg bg-blue-500" />
        <view className="row-span-2 rounded-lg bg-rose-500" />
        <view className="h-12 rounded-lg bg-neutral-300" />
        <view className="h-12 rounded-lg bg-neutral-300 order-first" />
      </grid>
    </page>
  );
}
```

`flex-wrap` and `flex-direction` do nothing on a grid and are refused there; grid items wrap into rows on
their own, and `grid-flow-col` fills columns first. `order` changes layout and paint order, not reading or
focus order.

## Positioning

- `relative` (default) offsets a box by its insets without moving its siblings.
- `absolute` takes the box out of flow, positioned against its parent's padding box. Insets are pixels or `%`;
  `inset-auto` and friends are refused. Positioned boxes paint in `z-*` order within their parent.
- `fixed` positions against the root `<page>`: it ignores ancestor scrolling and clipping. Use it for floating
  buttons and toasts.
- `sticky` works only inside a `<scroll>`: the box scrolls normally until it reaches its pixel inset, then holds
  there while its parent is still in view, and paints above its in-flow siblings. A sticky box outside a scroll,
  inside a list row, with `%` insets or with a transform fails when the host lays it out (`UI_STYLE`)
  and the previous frame stays on screen.
- `static` is not supported; it is the same as the default `relative` without insets.

`overflow-hidden` clips painting and hit testing to the box (and its rounded corners). `overflow-visible`
is the default. `overflow-scroll` is not supported; use `<scroll>`.

## Insets and the keyboard

`<page>` adds the safe-area insets to its padding, and uses the larger of the bottom inset and the
keyboard height for the bottom. Content inside `<page>` therefore never sits under the status bar,
the gesture area or the keyboard. When you need the numbers, `view.env.insets` and
`view.env.keyboard.height` are in logical pixels, and the page re-renders when they change.

Insets of `absolute` and `fixed` boxes measure from the padding box, so they do not include the page's
safe-area padding: a `fixed bottom-4` box sits 16 px above the page's bottom edge. Add
`env.insets.bottom` (and `env.keyboard.height` if it must clear the keyboard) through `style` when it
has to stay clear of them.

## Sizes and units

| Unit | Meaning |
| --- | --- |
| number, `px` | logical pixels (`w-12` = 48) |
| `rem` | 16 logical pixels, resolved at build time |
| `%` | of the parent's content box |
| `vw`, `vh` | of `env.viewport`; `dvh`, `svh`, `lvh` (and `dvw`…) are the same as `vh` (`vw`) |
| `ch` | only in `max-w-[Nch]`: N advances of "0" in the box's font |
| `em` | only in `tracking-*` (letter spacing) |

`w-fit`, `w-min`, `w-max` (and `h-*`) are the CSS sizing keywords; `h-auto` keeps an intrinsic height
under `min-h-*`/`max-h-*`. `aspect-*` derives the free axis from the other. `calc()` is folded at build
time when its operands share a unit; `w-[calc(100%-2rem)]` mixes units and is refused (use padding or
`grow`).

Text follows the system font size. Prefer `min-h-*` over fixed heights for boxes that hold text.
