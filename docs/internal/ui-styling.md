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
- `rem` is 16 logical pixels. `vw`/`vh` resolve against the View viewport.
- Percent sizes resolve in layout against the parent content box.
- `w-fit`, `w-auto`, `h-auto` map to native sizing keywords; `h-auto` keeps
  intrinsic height under min/max constraints.
- `truncate` and `line-clamp-N` lower to `overflow`, `whiteSpace`,
  `textOverflow` and `lineClamp`; Tailwind's `-webkit-box` declarations are
  absorbed by the clamp.
- `grid-cols-N` lowers to `gridTemplateColumns: N` on `<grid>`.
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

## Themes

A page imports one stylesheet; it may `@import` relative token files, expanded
in source order (cycles rejected, ≤128 files). Use `@theme`/`@theme inline` for
Tailwind tokens and `:root` / `[data-theme='light'|'dark']` blocks for palette
custom properties. Appearance-specific values compile into separate rules;
the isolate selects them from `View.env.theme`. Palette blocks accept only
custom properties and a matching `color-scheme`.

## Unsupported on purpose

Transitions, transforms, filters other than backdrop blur/saturation, inset
shadows, ring offsets, per-edge border styles and colors, `group-*` and
`peer-*` selectors, container queries and plugins. Each fails with
`UI_CLASS_UNSUPPORTED` rather than rendering differently from CSS.
