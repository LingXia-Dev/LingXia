# Text

```tsx
import { useLocal } from '@lingxia/ui';

export function render() {
  const opened = useLocal('opened', 0);
  return (
    <page className="p-4 gap-3 text-neutral-900">
      <text className="text-2xl font-semibold tracking-tight">Order summary</text>
      <text className="text-sm leading-relaxed">
        Plans renew <span className="font-semibold">monthly</span> unless you{' '}
        <span className="font-mono bg-neutral-100">cancel</span> first. Read the{' '}
        <link onPress={() => opened.set(opened.value + 1)} className="active:text-blue-800">terms</link>
        {' '}before you continue.
      </text>
      <row className="justify-between">
        <text className="text-sm text-neutral-500">Total</text>
        <text className="text-sm font-semibold tabular-nums">1,204.50</text>
      </row>
      <text className="text-xs text-neutral-500 whitespace-pre-line">{'Line one\nLine two'}</text>
      <text className="text-sm line-clamp-2">A long description that stops after two lines and ends with an ellipsis when it would run longer than that.</text>
      <text className="font-mono text-sm select-all">LX-2024-88412</text>
    </page>
  );
}
```

All text is in a `<text>`. A bare string among element children becomes one. Containers pass text
style down: `text-*`, `font-*`, `leading-*`, `tracking-*` and the other typography utilities set on a
`<column>` apply to every `<text>` inside it unless overridden.

## Defaults

Text is 14 px, weight 400, in the system sans font, colored `#172033`. Unstyled buttons draw white
text on `#365cdd`, editors a white surface, and the root a `#f6f7fb` background. These defaults do not
follow `env.theme`; a page that supports dark mode sets its colors explicitly.

## Whitespace

Whitespace collapses as in CSS. Under `whitespace-normal` (the default) and `whitespace-nowrap`, every
run of spaces, tabs and newlines becomes one space. `whitespace-pre-line` keeps newlines and collapses
spaces; `whitespace-pre` and `whitespace-pre-wrap` keep everything.

JSX trims text at line breaks before the View sees it: a space you need between a run and the next
line must be written `{' '}`. A string with `\n` needs `whitespace-pre-line` to show the break.

## Wrapping and truncation

- `truncate` keeps one line and ends it with an ellipsis; `line-clamp-N` keeps N lines.
  Accessibility still reads the full string.
- `break-all`, `break-keep`, `wrap-break-word` and `wrap-anywhere` control where long words break.
- `text-start`, `text-end`, `text-left`, `text-center`, `text-right`, `text-justify` align lines; `start`/`end`
  follow the text direction.
- `uppercase`, `lowercase`, `capitalize` change only what is painted.
- `max-w-[24ch]` limits a line to 24 advances of "0" in the node's font.

A `<text>` in a row does not shrink by default; give it `grow basis-0` or `shrink min-w-0` to let it wrap
or truncate inside the row ([Layout](layout.md)).

## Size, line height and spacing

- `text-sm`, `text-[13px]`: font size in logical pixels.
- `leading-relaxed`, `leading-[1.4]`: a unitless line height is a multiplier of each node's own font size.
  `leading-6`, `leading-[24px]` are absolute and inherit as pixels.
- `tracking-wide`, `tracking-[0.05em]`: `em` resolves on the node that sets it; descendants inherit pixels.

Text follows the system font size setting: font size, line height and letter spacing scale together.
Fixed heights do not, so give text boxes `min-h-*`, not `h-*`.

## Fonts

`font-sans` (the default) and `font-mono` are the platform's sans and monospace families. Serif and
custom font families are refused. `font-thin` … `font-black` and `font-[550]` set any weight the font
supports; `italic`/`not-italic` set the style. The display language is the shaping locale, so Han
characters pick the Simplified, Traditional, Japanese or Korean face that language expects.

## Numerals

`tabular-nums`, `proportional-nums`, `lining-nums`, `oldstyle-nums`, `slashed-zero`, `ordinal`,
`diagonal-fractions` and `stacked-fractions` combine across classes into OpenType features, as in CSS.
Use `tabular-nums` for columns of numbers and counters.

## Rich text

A `<text>` may contain strings, `<span>` and `<link>`. Nested spans flatten into styled runs of one
paragraph, so wrapping, alignment and selection treat the whole `<text>` as one block. A `<text>` holds
at most 256 runs.

Runs take text paint only: color, size, weight, family, italic, decoration, numeric variants and a
background color. Layout, padding and borders on a run are refused.

```tsx error UI_CLASS_ELEMENT
export function render() {
  return <page><text>Pay <span className="px-2">now</span></text></page>;
}
```

`<span>` and `<link>` are valid only inside `<text>` (`UI_TEXT` elsewhere).

## Links

`<link onPress>` is a pressable run: blue (`#365cdd`) and underlined unless you style it, restyled by
`active:` while pressed, and exposed to accessibility as a link named by its text (or
`accessibilityLabel`). Only the link's own line pieces respond to taps. `disabled` removes its handler.
There is no `href`; navigate in `onPress` through a Logic action.

## Selection

Static text is not selectable unless you opt in:

| Utility | Effect |
| --- | --- |
| `select-text` | long-press selects a word; handles extend it; the system menu copies it |
| `select-all` | long-press selects the whole text |
| `select-none` | stops selection that an ancestor enabled |

The setting inherits, so `select-text` on a container covers every `<text>` inside. One selection exists
per page, and selection stays within one `<text>`: it cannot span several elements. A frame offers at most
64 selectable texts; truncated or clamped text is never selectable. Editors select their own text and
refuse `select-*`.
