# GPU View styling and motion

## Colours and steps

- Style with complete Tailwind classes. `style` (CSS-named: `backgroundColor`, `marginTop`,
  `gridTemplateColumns`) is only for values computed at render time; a `style` object of fixed values
  warns `UI_STYLE_STATIC`. Unitless `lineHeight` is a multiplier. Each element accepts only styles it renders.
- RTL: use `ps`/`ms`/`start-`/`text-start` etc.; they mirror with `env.direction` (from the
  display language) or a `dir` prop. Mirror icons with `rtl:-scale-x-100`.
- Colours are theme roles, never hex: `bg-surface`, `text-muted`, `border-line` (decorative) or
  `border-line-strong` (editors, controls; a border, divide or ring in another role at an opacity fails `UI_CLASS_EDGE`), `bg-accent-solid text-on-accent-solid` (pressed or switched controls; badges and labels keep `bg-accent text-on-accent`, `accent` text and rings), `bg-text/8`; plot series
  `chart-1`..`chart-8`; controls over video `bg-media text-on-media` (tracks `on-media/25`). Roles follow appearance, so `dark:` on a colour fails (`UI_COLOR`). Inside `bg-surface-raised` or `bg-surface-overlay`, `line` and `line-strong` draw with that surface's own pair, so keep writing `border-line`.
- Type, radius, spacing and motion are theme steps: `text-sm`, `rounded-lg`, `p-4`, `duration-fast|base|slow`,
  `ease-standard|enter|exit|spring` (not milliseconds). Liuli's names come from the same ramps:
  `text-caption|footnote|body|callout|title|display`, `rounded-checkbox|segment|control|container|dialog`, `*-control-sm|md|lg`.
  Arbitrary values (`p-[13px]`) stay fixed and do not follow the theme.

## Theme

- `theme.json` at the lxapp root gives the lxapp its own look: `{"name":"…","version":1,"accent":{"seed":"#…","derive":true}}`
  derives every accent role from one colour; `appearance` may also set surfaces, lines, text and status colours
  for light and dark, and `type`, `shape.radius`, `space.unit` and `motion` retune the steps. Every other role
  is derived to pass its contrast pairs.
- `lingxia build` checks every role pair in light, dark and high contrast and fails listing each pair that misses.
- App-owned colours go in `colors` (≤32): `{"trend-up":{"light":"#…","dark":"#…","use":"text|graphic|fill","on":["surface"]}}`;
  classes are role-shaped (`text-trend-up`, a fill's label `text-on-<name>`), style values and plot series `app.<name>`.
- A palette the host pushes wins over the pack. `env.highContrast` picks the high-contrast palette;
  `shadow-*` layers use the `shadow` role. Host chrome around the page (bars, window floor) takes the page's palette.

## Classes

- Supported variants: `dark:`, `rtl:`/`ltr:`, `regular:`, `motion-safe:`/`motion-reduce:`, `first:`/`last:`/`odd:`/`even:`/`only:`/`empty:`, `sm:`–`2xl:`, `focus:` (editors), `focus-visible:` (keyboard focus; replaces the ring), `active:`
  (controls), `hover:` (mouse, stylus or trackpad), `disabled:`, `indeterminate:`, `placeholder:`, `backdrop:` (a dialog's scrim), slider parts `track:`/`range:`/`secondary:`/`thumb:`
  (a state goes first: `active:thumb:size-4`; the touch target stays the slider's box), switch parts `track:`/`thumb:`
  (`checked:track:bg-success`, `track:w-11 track:h-6`; `checked:` works only before a part),
  `data-*`, `has-checked:`/`has-[:disabled]:`/`has-focus:`, `focus-within:`, `group-active:`. `space-*`/`divide-*` are utilities styling every child but the last. `*:` only places grid children; `**:`, `in-*:`, other `group-*:`, `peer-*:` fail naming the TS-side form.
- `translate`/`rotate`/`scale`/`origin` are static and state variants may set them; skew and 3D fail.
  `transition-*`/`animate-*` animate paint and transforms only; gradients and `scale-0` jump. A box that is or
  holds an editor/scroll/slider may only translate (build error; on `starting:`/`exit:` a warning). `starting:`/`exit:` animate enter and removal; `transition-layout` animates position; `transition-[d]` morphs a `<vector>`'s paths, stroke width, dashes and solid colours between sources with the same commands (pair by `key`); draw a line in with `pathLength: 1`, `strokeDasharray: [1]`, `strokeDashoffset` 1 → 0.
  `ease-spring`, `ease-spring-bouncy` or `ease-[spring(0.4)]` time a transition by a spring; `duration-*` stays the knob.
- Utilities that would draw nothing (filters but `blur-*`, radial masks, text shadows, cursors, outlines) fail naming what to use.

## Drag and snap

- Scroll snap: `snap-x`/`snap-y` on a `<scroll>` or `<List>`, `snap-start|center|end` on children; every snap is mandatory.
  Pager: `<scroll bind={page} className="overflow-x-auto snap-x snap-always">`; a flick stops one page on, `bind` is the resting page, two-way.
- `drag="x|y" dragSnapPoints={[0, 120, '50%']}` (≤8, px or % of its own size; `x` mirrors in RTL) lets the
  finger move an element; it springs (its `transition-transform duration-* ease-spring*`) to the nearest point, a flick one on.
  `bind={cell}` is the point's index (setting it springs there), `onDragStart()`, `onDragEnd({ index, velocity })`.
  Needs `aria-label` (it throws `UI_A11Y` without): screen readers and arrow keys step it; tests read the point as `toHaveValue('1')`.
  A same-axis scroll inside it scrolls only once it is fully open (lowest offset) and returns to its top before it closes.
- Swipe row: `<button drag="x" dragSnapPoints={[0, -144, '-100%']} aria-actions="archive-1 delete-1">` over `<button id="archive-1">`s;
  screen readers get those as the row's actions; tests call `locator.performAction('Delete')` (or `increment`/`decrement`/`dismiss`).
- Sheet detents: `<dialog drag="y" dragSnapPoints={[0, '50%', '100%']} bind={detent}>`; resting wholly off screen calls `onClose`, the scrim fades with it.
