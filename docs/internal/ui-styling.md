# GPU View styling

Authoring is TSX with Tailwind utilities or CSS-named inline `style`. The
official Tailwind compiler runs at build time; nothing CSS-related loads into
the View isolate. Pin Tailwind and review lowering tests before upgrades.

## Pipeline

1. `compiler/tailwind.mjs` compiles candidate classes, then lowers each rule's
   declarations to CSS-named style (`backgroundColor`, `marginTop`,
   `justifyContent: 'space-between'`, …). Gradients, shadows and backdrop
   filters keep Tailwind's variables until the cascade composes them.
2. `src/style/classes.js` matches rules by class name and View environment
   (theme, viewport width, pointer) and returns the cascade: base, `disabled`,
   paint deltas for `focus`/`active`/`hover`, and the child style of
   `space-*`/`divide-*` rules.
3. `src/style/lower.js` merges layers (child rule, classes, inline) the way the
   CSS cascade does — a later shorthand resets earlier longhands — checks every
   property against the element's style groups, and lowers to wire fields.
4. The native decoder validates the record again; layout and paint own the
   result. There is no per-platform style evaluator.

Class order in a string never changes Tailwind's cascade. Inline style wins
over classes. Unknown, unsupported and wrong-element utilities fail at build
time (see [ui-authoring.md](ui-authoring.md)) and again at runtime.

## Semantics worth knowing

- Unitless `lineHeight` is a multiplier and is recomputed per node from its
  font size; `'24px'` is absolute and inherits as pixels.
- `letterSpacing` in `em` resolves on its owner; descendants inherit pixels.
- `rem` is 16 logical pixels. `vw`/`vh` (and `d`/`s`/`l` variants) resolve against
  the page's safe content box (viewport minus insets, above an open keyboard).
- `hidden` is `display: none`: no layout, paint, hits or semantics. `block` and
  `inline-block` lay children in a stretched column; `inline-flex` and
  `inline-block` default `align-self` to start.
- `fixed` is absolute against the root `<page>`; it escapes ancestor scrolls and clips,
  and its length insets measure from the safe content box (percentages from the surface).
- `sticky` (pixel insets) offsets a box inside its nearest `<scroll>` at composition,
  bounded by its parent's content box, and lifts it above in-flow siblings; inside a
  `<list>` row it pins within its row; transformed boxes cannot stick.
- `translate-*` moves layout geometry after layout (siblings stay put, hits stay exact);
  `rotate-*`, `scale-*` and `origin-*` paint an affine about the origin and hit-test the
  transformed shape. Editors, scrolls and sliders may translate but not rotate or scale;
  state variants transform as paint (`active:scale-95`); skew and 3D fail.
- Percent sizes resolve in layout against the parent content box. `aspect-*` derives
  the free axis; `max-w-[Nch]` resolves against the advance of "0" in the node's font.
- `bg-current` paints the text color. `overscroll-contain`/`-none` stop a scroll or
  list from handing leftover distance to ancestors.
- `select-text` (inherited) makes static text long-press selectable by word;
  `select-all` selects the whole text first; `select-none` stops it. One selection per
  page, ≤64 selectable texts per frame; truncated text and editors never take it.
- `w-fit`, `w-auto`, `h-auto` map to native sizing keywords; `h-auto` keeps
  intrinsic height under min/max constraints.
- `whiteSpace` follows CSS: `normal`/`nowrap` collapse whitespace runs (newlines
  too), `pre-line` keeps newlines, `pre`/`pre-wrap` keep everything.
- `tabular-nums`, `slashed-zero` and the other numeric variants compose across
  classes into inherited OpenType features (`tnum`, `zero`, …).
- `truncate` and `line-clamp-N` lower to `overflow`, `whiteSpace`,
  `textOverflow` and `lineClamp`; Tailwind's `-webkit-box` declarations are
  absorbed by the clamp.
- `grid` makes any container a CSS Grid (`<grid>` is sugar for it). Track lists
  take lengths, `%`, `fr`, `auto`, `min-/max-content`, `minmax()`, `fit-content()` and
  `repeat(N | auto-fit | auto-fill, …)`, ≤64 tracks; named lines, areas, subgrid and
  `calc()` fail. A number `gridTemplateColumns` is N equal columns.
- `order` reorders flex and grid items for layout and paint, not reading or focus order.
- Shadow tokens and `shadow-[var(--x)]` expand every layer the variable holds
  (≤8); layers Tailwind did not wrap in `--tw-shadow-color` keep their color.
- `bg-linear-*`, `bg-radial-*`, `bg-conic-*` and complete `linear-|radial-|conic-gradient()`
  values (arbitrary or theme tokens via `bg-(image:--x)`) lower to one gradient; stop
  positions are percentages (conic also angles); repeating gradients fail.
- `border-dashed`/`border-dotted` follow `--tw-border-style`, so class order never
  matters; dashes are 3× the width (dots 1×) and divide each ring or edge evenly.
- `rounded-full` lowers to the native 16,384 radius bound, clamped by paint.
- Standard system sans/mono stacks lower to `sans-serif`/`monospace`;
  custom faces and serif fail.
- `placeholder:text-*` lowers to `placeholderColor` (editors only).

## Direction

`env.direction` derives from `displayLanguage`; `dir` sets it for a subtree. Taffy
mirrors rows, column cross-alignment, grid placement and absolute positions.
Flow-relative utilities (`ps`/`pe`, `ms`/`me`, `start-`/`end-`, `border-s`/`-e`,
`rounded-s`/`-e`, `text-start`/`-end`) stay logical on the wire and resolve natively
against each node's own direction, so a language switch relays out without the View.
When a node carries both, the flow-relative value wins on its side. `rtl:`/`ltr:`
match the inherited direction like `dark:` matches the theme; mirror icons with
`rtl:-scale-x-100`. Editors take their box's direction; `dir="auto"` (editors only)
follows the first strong character. A horizontal scroll in RTL starts at its
inline start (offset 0 = right edge) and advances leftward.

## State variants

| Variant | Element | Wire | May change |
| --- | --- | --- | --- |
| `focus:` | input, textarea, search | `focusStyle` | paint |
| `active:` | button, checkbox, switch, radio | `pressedStyle` | paint |
| `hover:` | any; only for `pointer: 'fine'` | `hoverStyle` | paint |
| `disabled:` | elements with `disabled` | replaces the base cascade | anything |

Paint properties are color, background color/image, opacity, border color,
box shadow and placeholder color. Native applies state deltas to a copy after
reconciliation, so a failed frame restores the previous state.

`first:`, `last:`, `odd:`, `even:`, `only:` and `empty:` match a node's position among
its rendered siblings (list rows: their row index of `count`) and combine with the
state variants. `has-[…]`, `data-[…]` and `aria-*` fail: choose classes in TS.

## Motion (gated)

Pages opt in with a `// @ui-experimental motion` source line until native
presentation passes device acceptance. `transition-*`, `duration-*`, `delay-*` and
`ease-*` compose into one `transition` field over opacity, transform, bg, fg,
border and shadow (≤10 s each); layout transitions fail naming
`transition-transform`/`transition-opacity`. `animate-*` keyframes may omit
endpoints (identity), rotate, translate by `%` (relative) and ease per keyframe.
`motion-reduce:`/`motion-safe:` follow `env.reducedMotion`; animations carry
`whenReduced: keep` when authored under either, otherwise reduced motion drops
them. Native multiplies durations by `env.animatorDurationScale` (0 = reduced). Transforms are paint, so
`active:scale-95` is legal.

## Themes

A page imports one stylesheet; it may `@import` relative token files, expanded
in source order (cycles rejected, ≤128 files). Use `@theme`/`@theme inline` for
Tailwind tokens and `:root` / `[data-theme='light'|'dark']` blocks for palette
custom properties. Appearance-specific values compile into separate rules;
the isolate selects them from `View.env.theme`. Palette blocks accept only
custom properties and a matching `color-scheme`.

Colours are roles from `contracts/schema.json` (`roles`, append-only slots). The compiler injects
`--color-<role>: role(<role>)` after the project theme, so classes lower to `role` or `role/NN` and
a project token cannot redefine one; the build refuses other colours (`UI_COLOR`) unless
`literalColors` is set for pixel fixtures. Native resolves roles against the environment's
internal `palette` push (`lingxia-theme` resolves packs), else the default pack for `theme`.

## Unsupported on purpose

Skew and 3D transforms, filters other than backdrop blur/saturation, masks, text and
drop shadows, inset shadows, ring offsets, per-edge border styles and colors, `*`, `**`,
`in-*`, `group-*` and `peer-*` selectors, container queries and plugins. Each fails with
`UI_CLASS_UNSUPPORTED` naming what to use, rather than rendering differently from CSS
or compiling to nothing: a utility that lowers to no drawn style is refused.
