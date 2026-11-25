# View styling

How Tailwind utilities and CSS-named inline `style` become wire fields, and
what each construct means natively. Element and prop admission is in
[view-authoring.md](view-authoring.md); colour roles and scale steps are in
[theme.md](theme.md); motion timing is in [view-animation.md](view-animation.md).

Invariants:

- The official Tailwind compiler runs at build time; nothing CSS-related loads
  into the View isolate, and there is no per-platform style evaluator.
- A utility either lowers to drawn style that matches CSS or fails the build;
  nothing compiles to a silent no-op.
- Native re-validates every lowered record; it never trusts the runtime.

Pin Tailwind and review the lowering tests before upgrading it.

## Pipeline

1. The `@lingxia/view` compiler compiles candidate classes via Tailwind, then lowers each rule's
   declarations to CSS-named style (`backgroundColor`, `marginTop`,
   `justifyContent: 'space-between'`, …). Gradients, shadows and backdrop
   filters keep Tailwind's variables until the cascade composes them.
2. The class-matching stage in `@lingxia/view` matches rules by class name and View environment
   (theme, viewport width, pointer) and returns the cascade: base, `disabled`,
   paint deltas for `focus`/`active`/`hover`, and the child style of
   `space-*`/`divide-*` rules.
3. The style-lowering stage in `@lingxia/view` merges layers (child rule, classes, inline) the
   way the CSS cascade does — a later shorthand resets earlier longhands — checks every
   property against the element's style groups, and lowers to wire fields.
4. The native decoder validates the record again; layout and paint own the
   result.

Class order in a string never changes Tailwind's cascade. Inline style wins
over classes. Unknown, unsupported and wrong-element utilities fail at build
time (see [view-authoring.md](view-authoring.md)) and again at runtime.

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
- `grid` makes any container a CSS Grid. Track lists
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
- `font-sans`, `font-serif` and `font-mono` (and system stacks ending in their generic) lower to
  the platform family; a stack naming a bundled `@font-face` lowers to that face.
- `placeholder:text-*` lowers to `placeholderColor` (editors only).

## Direction

`env.direction` derives from `language`; `dir` sets it for a subtree. Taffy
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

`docs/view/reference/variants.md` (generated) lists every supported variant and the elements
it applies to. Each paint-only state lowers to its own wire field (`focusStyle`, `pressedStyle`,
`hoverStyle`, `focusVisibleStyle`, `focusWithinStyle`, `groupPressedStyle`, `hasCheckedStyle`, …);
`starting:`/`exit:` lower to `startingStyle`/`exitStyle`; `disabled:` replaces
the base cascade; `data-*` resolves in the isolate at render, so it may set any utility.

Paint properties are color, background color/image, opacity, border color,
box shadow, placeholder color and static transforms. Native applies state deltas to a copy after
reconciliation, so a failed frame restores the previous state.

`first:`, `last:`, `odd:`, `even:`, `only:` and `empty:` match a node's position among
its rendered siblings (list rows: their row index of `count`) and combine with the
state variants. `aria-*`, `peer-*`, `group-*` other than `group-active:`, and `has-[…]`
other than `has-checked:`/`has-disabled:`/`has-focus:` fail: choose classes in TS.

## Motion

`transition-*`, `duration-*`, `delay-*` and `ease-*` compose into one
`transition` field (≤10 s each). CSS transition properties map onto what
native can animate: opacity, transform, colours, border colour, shadow,
`width`/`height` (as `size`) and a vector's `d`. `transition-layout` adds
position (FLIP) motion and `transition-size` size motion to whatever
`transition-*` sets; `ease-spring` and `ease-spring-bouncy` time a transition
by a spring. A transition of any other layout property (padding, margin,
inset, gap, font size, …) fails naming those utilities, because animating it
would re-run layout every frame.

`animate-*` keyframes may omit endpoints (identity), rotate, translate by `%`
(relative), ease per keyframe and take a fill mode; one animation per element.
`motion-reduce:`/`motion-safe:` follow `env.reducedMotion`; animations carry
`whenReduced: keep` when authored under either, otherwise reduced motion shows
their base style. Native multiplies durations by `env.animatorDurationScale`
(0 = reduced). Transforms are paint, so `active:scale-95` is legal.
[view-animation.md](view-animation.md) has the payloads and timing.

## Themes

A page imports one stylesheet; it may `@import` relative token files, expanded
in source order (cycles rejected, ≤128 files). Use `@theme`/`@theme inline` for
Tailwind tokens and `:root` / `[data-theme='light'|'dark']` blocks for palette
custom properties. Appearance-specific values compile into separate rules;
the isolate selects them from `View.env.theme`. Palette blocks accept only
custom properties and a matching `color-scheme`.

Colours are roles from `contracts/view/schema.json` (`roles`). The compiler injects
`--color-<role>: role(<role>)` after the project theme, so classes lower to `role` or `role/NN` and
a project token cannot redefine one; the build refuses other colours (`UI_COLOR`) unless
`literalColors` is set for pixel fixtures. How roles resolve to colours at run time, and where the
pack comes from, is in [theme.md](theme.md).

A component package's `tokens.css` names steps, never values: an alias is `var(--<step>)` and a
size is `calc(var(--spacing) * N)`. A literal there lowers to a fixed number the pack cannot reach.

## Unsupported on purpose

`docs/view/reference/utilities.md`, generated from the schema and the compiler, lists every
Tailwind family as supported or refused with its reason; the selectors and variants are in
`variants.md`. A refused utility fails with `UI_CLASS_UNSUPPORTED` naming what to use, rather than
rendering differently from CSS or compiling to nothing: a utility that lowers to no drawn style is
refused.
