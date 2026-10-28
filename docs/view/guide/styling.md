# Styling

```css
@theme {
  --radius-card: 14px;
}
```

```tsx
import { useLocal } from '@lingxia/ui';
import './theme.css';

const tone = { neutral: 'bg-surface-raised text-text', accent: 'bg-accent text-on-accent' } as const;

export function render() {
  const selected = useLocal<'neutral' | 'accent'>('selected', 'neutral');
  return (
    <page className="bg-canvas p-4 gap-3">
      <column className={`rounded-card p-4 gap-1 shadow-md ${tone[selected.value]}`}>
        <text className="text-lg font-semibold">Plan</text>
        <text className="text-sm opacity-80">Renews monthly</text>
      </column>
      <button
        onClick={() => selected.set(selected.value === 'neutral' ? 'accent' : 'neutral')}
        className="rounded-full border border-accent text-accent bg-transparent active:bg-accent/10 md:self-start"
      >
        Switch tone
      </button>
      <view style={{ height: 4, borderRadius: 2, backgroundImage: { kind: 'linear', direction: 'right', stops: [{ offset: 0, color: 'accent' }, { offset: 1, color: 'chart-4' }] } }} />
    </page>
  );
}
```

Style with Tailwind utilities in `className`, or with CSS-named properties in `style`. The official
Tailwind compiler runs at build time; each utility is lowered to native style or refused. Nothing
CSS-related runs on the device.

## Utilities

Write complete utility names as string literals. The build reads class names from literals, both
branches of conditionals, `&&`/`||`/`??`, `const` aliases and object maps (also imported ones), without
running your code. A name assembled at runtime is not compiled and fails when it renders:

```tsx error UI_CLASS
import { useLocal } from '@lingxia/ui';

export function render() {
  const size = useLocal('size', 4);
  return <page className={`p-${size.value}`} />;
}
```

Choose between complete names instead: `size.value === 4 ? 'p-4' : 'p-8'`.

The order of classes in a string never matters; Tailwind's cascade decides, as in CSS. Every utility
the default theme defines is listed in [utilities](../reference/utilities.md) as supported or refused.

## Colours are roles

Colours name a theme role, never a value: `bg-surface`, `text-muted`, `border-line`,
`bg-accent text-on-accent`, with opacity modifiers (`bg-text/8`). The theme pack resolves roles per
appearance, so `dark:` on a colour, palette ladders (`gray-200`) and hex fail with `UI_COLOR`; so do hex
colours in `style`. Complete CSS gradients (`bg-[radial-gradient(…)]`) and shadow layers may use literal
colours. The roles are listed in [styles](../reference/styles.md#colour-roles).

On `<text>`, `bg-clip-text` draws its background (colour or gradient) through the glyphs instead of
behind the box: `bg-clip-text text-transparent bg-linear-to-r from-accent to-danger`. Each such text costs
two layers.

The system's increased-contrast setting (`env.highContrast`) resolves the pack's high-contrast palette.
`shadow-*` elevation layers take the `shadow` role at their alpha, so a pack can tint shadows.

An lxapp sets its own look with `theme/pack.json`. A pack may name only an accent seed and derive the
accent family from it; every role it leaves out comes from the default pack. `lingxia build` proves the
pack's contrast pairs and ships it; the host resolves it when a page opens:

```json
{ "name": "brand", "version": 1, "accent": { "seed": "#0f9d58", "derive": true } }
```

```tsx error UI_COLOR
export function render() {
  return <page className="bg-white"><text className="text-gray-500">Hex and ladders fail</text></page>;
}
```

## Arbitrary values

`w-[72px]`, `text-[13px]`, `grid-cols-[1fr_auto]`, `bg-(--brand-image)` and opacity modifiers
(`bg-text/50`) work wherever the family works. `calc()` folds when its operands share a unit.
`url()` backgrounds, `!important` and unresolved `var()` are refused.

## Themes and tokens

A page imports at most one stylesheet (it may `@import` relative token files). It holds only tokens:

- `@theme { … }` and `@theme inline { … }` define Tailwind tokens (`--radius-*`, `--text-*`, `--shadow-*`, …);
  colour tokens never override a role.
- `:root { … }` and `[data-theme='light' | 'dark'] { … }` define custom properties, such as shadow tints.

Rules, selectors and `@apply` are refused (`UI_CSS`). A token that differs between the two blocks
compiles into a light and a dark rule; the View picks one from `env.theme`. Font tokens accept only the
standard system sans and mono stacks.

## Variants

| Variant | Matches |
| --- | --- |
| `dark:` | `env.theme === 'dark'`; not on colours, which follow appearance |
| `sm:` … `xl:`, `min-[…]:`, `max-*:` | the View viewport width (`sm` = 640 px, `md` = 768, `lg` = 1024, `xl` = 1280) |
| `rtl:`, `ltr:` | the node's inline direction ([RTL and i18n](rtl-and-i18n.md)) |
| `first:` `last:` `only:` `odd:` `even:` `empty:` | the node's position among its rendered siblings; `empty:` a node with no children |
| `active:` | a pressed `<button>`, `<checkbox>`, `<switch>`, `<radio>` or `<link>` |
| `focus:` | a focused `<input>`, `<textarea>` or `<search>` |
| `hover:` | a hovered element, only when `env.pointer` is `fine` |
| `disabled:` | an element with `disabled` set; it replaces the whole class cascade |
| `placeholder:` | an editor's placeholder; `text-*` color only |
| `motion-reduce:`, `motion-safe:` | `env.reducedMotion` |
| `data-active:`, `data-[state=open]:` | the element's own `data-active` / `data-state="open"` prop (a string, number or `true`) |
| `has-checked:`, `has-disabled:` | a checked checkbox, switch or answered radio, or a disabled element, anywhere below it |
| `focus-within:`, `has-focus:` | it or an element below it has focus |

`data-*` resolves at render, so it may set any utility. `active:`, `focus:`, `hover:`, `has-*` and
`focus-within:` may change paint only: color, background color and image, opacity,
border color, box shadow, placeholder color and the static transforms (`active:scale-95`). They cannot
change size or spacing (`UI_CLASS_ELEMENT`). The change is immediate unless a `transition-*` utility
animates it ([motion](motion.md)).
`2xl:` is not recognized and fails as an unknown utility; write `min-[96rem]:`.

Variants that test DOM state are refused with the alternative named in the message: `group-*`, `peer-*`,
other `has-[…]` selectors and `has-*` combined with another state, `aria-*`, `checked:`, `focus-visible:`,
`nth-*`, `before:`, `after:` and container queries (`@md:`). Compute the condition in TypeScript and pick the class.
The full list is in [variants](../reference/variants.md).

## Transforms

Transforms are static paint: they never change layout.

- `translate-*` (pixels or `%` of the box) moves the box after layout; siblings stay where they were and
  taps land where the box is drawn.
- `rotate-*`, `scale-*`, `-scale-x-100` and `origin-*` transform the box and its subtree about the origin
  (the center by default); hit testing follows the transformed shape.
- Editors, scrolls and sliders may translate, but not rotate or scale.
- Skew, `rotate-x-*`, `perspective-*` and other 3D transforms are refused; `translate-z-*` and `scale-z-*` have
  no effect.

```tsx
export function render() {
  return (
    <page className="p-4">
      <view className="relative w-16 h-10 rounded-lg bg-line">
        <view className="absolute top-0 right-0 w-4 h-4 rounded-full bg-danger translate-x-1/2 -translate-y-1/2" />
      </view>
      <button onClick={() => {}} className="self-start rounded-xl bg-accent text-on-accent active:scale-95 active:opacity-90">Press</button>
    </page>
  );
}
```

## Each element renders its own styles

Every element accepts only the style properties it draws. A utility on an element that ignores it is
a build error, not a silent no-op:

```tsx error UI_CLASS_ELEMENT
export function render() {
  return <page><text className="gap-2">Text has no gap</text></page>;
}
```

The accepted properties per element are in [styles](../reference/styles.md). `<span>` and `<link>`
take only text paint; `<spacer>` only box properties.

## The `style` prop

`style` takes CSS property names in camelCase. Numbers are logical pixels; strings carry units
(`'50%'`, `'100vw'`, `'24px'` line height, `'0.05em'` letter spacing). Inline style overrides classes.
Within one object, longhands beat shorthands whatever their order; across layers a later shorthand
resets earlier longhands, as in CSS.

Structured values have object forms: `backgroundImage` takes `{ kind, stops, … }` (linear, radial,
conic), `boxShadow` an array of `{ offsetX, offsetY, blur, spread, color }`, `translate`/`scale` a
number or a pair, `rotate` degrees, `transformOrigin` a pair of box fractions. Colours in `style` are
roles (`'surface'`, `'accent/12'`), `currentColor` or `transparent`; gradient stops and shadows may
also be hex.

```tsx error UI_STYLE
export function render() {
  return <page><text style={{ alignItems: 'center' }}>Text is not a flex container</text></page>;
}
```

## Refused, and why

Each of these fails the build with `UI_CLASS_UNSUPPORTED` rather than rendering differently from CSS:

- Filters (`blur-*`, `brightness-*`, `drop-shadow-*`), except `backdrop-blur-*` and `backdrop-saturate-*`.
- Inset shadows, `ring-offset-*`, outlines, `text-shadow-*`, masks, blend modes.
- Skew and 3D transforms (`skew-*`, `rotate-x-*`, `perspective-*`).
- Per-edge border colors and styles (`border-t-red-500`); borders share one color and style.
- `cursor-*` and `pointer-events-*`: there is no cursor, and hit testing follows the rules in [Interaction](interaction.md).
- Floats, columns, tables, `visibility`, `static`, scroll snapping and scroll margins.
- Serif and custom font families.
- Layout transitions and exit animations ([Motion](motion.md)).

```tsx error UI_CLASS_UNSUPPORTED
export function render() {
  return <page><view className="h-10 blur-sm bg-line" /></page>;
}
```

A misspelled or invented utility is `UI_CLASS_UNKNOWN`; the message names the nearest supported one.

```tsx error UI_CLASS_UNKNOWN
export function render() {
  return <page><row className="flex-center" /></page>;
}
```
