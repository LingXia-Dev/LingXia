<!-- Generated from the doc file by scripts/generate.mjs; do not edit. -->

# Link

A link among the words of a sentence. Status: preview.

```ts
import { Link } from '@liulidesign/core/Link';
```

Link is a run of text that goes somewhere: it sits inside a `<text>`, takes the size and weight of the words around it, and is told apart by the accent colour and an underline. It calls `onClick`; the app opens the page or the address. A link that stands alone, outside a sentence, is a Button with `variant="link"`.

## Examples

### In a sentence

```tsx
import { useLocal } from '@lingxia/view';
import { Link } from '@liulidesign/core/Link';

/** In a sentence, under a form. */
export function InSentence() {
  const opened = useLocal('opened', '');
  return (
    <view className="gap-2">
      <text className="text-footnote text-muted">
        By creating an account you accept the <Link label="terms of service" onClick={() => opened.set('terms')} data-testid="terms" /> and the{' '}
        <Link label="privacy notice" onClick={() => opened.set('privacy')} data-testid="privacy" />.
      </text>
      <text data-testid="opened" className="text-caption text-muted">{opened.value === '' ? 'Nothing opened' : `Opened: ${opened.value}`}</text>
    </view>
  );
}
```

### In body text

```tsx
import { Link } from '@liulidesign/core/Link';

/** In body text, at its size and weight. */
export function InBody() {
  return (
    <text className="text-body text-text">
      Exports are kept for seven days. You can <Link label="download the last export" onClick={() => {}} data-testid="download" /> or start a new one from Settings.
    </text>
  );
}
```

### Not available yet

```tsx
import { Link } from '@liulidesign/core/Link';

/** A link that cannot be followed yet. */
export function Disabled() {
  return (
    <text className="text-footnote text-muted">
      No code yet? <Link label="Send it again" isDisabled data-testid="resend" /> in 30 seconds.
    </text>
  );
}
```

## Props

| Prop | Type | Default | Description |
| --- | --- | --- | --- |
| `label` (required) | `string` |  | The linked words. They say where the link goes: "terms of service", never "here". |
| `onClick` | `(() => void)` |  | Called when the link is pressed. The app opens the page or the address. |
| `aria-label` | `string` |  | Replaces the label for assistive tech when the words alone are not enough out of their sentence. |
| `isDisabled` | `boolean` | `false` | Takes no press, is drawn faint and is read as unavailable. |
| `data-testid` | `string` |  |  |

## Anatomy

| Part | Element | Required | Description |
| --- | --- | --- | --- |
| root | `<link>` | yes | The linked words, underlined in the accent colour. |

## What an app may override

- No class props: it takes its look from its props, the tokens and the roles below.
- The recipe, `linkRecipe` from `@liulidesign/core/Link/recipe`: the classes of each part, to style an element of your own to match. The component does not read a changed copy.
- It reads no Liuli scale token.
- Colour roles it paints with, set by the theme pack: `accent`, `text-faint`.

## Accessibility

- Pattern: none.
- Name: label.
- Description: none.
- States: disabled (from isDisabled).
- Tab: Moves to the link.
- Enter: Follows it.
- Press target: .
- It is read as a link, by its label. Write the label so it makes sense out of its sentence, or give `aria-label`.
- The underline tells it from the text around it without colour.
- A link in a line of text is as tall as the line; a finger press that misses goes to the nearest link within reach.
- A disabled link is read as unavailable and takes no press.

| Contrast | Pair | Minimum | Light | Dark |
| --- | --- | --- | --- | --- |
| link | `accent` on `surface` | 4.5:1 | 5.62:1 | 6.19:1 |
| link on the page | `accent` on `canvas` | 4.5:1 | 5.25:1 | 6.83:1 |

Ratios are computed from @lingxia/view/theme and rounded down.

## Guidance

- Do: Link the words that name the destination: "terms of service".
- Do: Keep a link to a few words; a whole sentence underlined is hard to read.
- Don't: Link "here" or "click here".
- Don't: Use a link for an action that changes something; that is a Button.
- Don't: Set a link apart on its own line; use Button with `variant="link"`.

## Limits

- A link has no hover or focus style of its own beyond the system focus ring.

Related: Button, Breadcrumb, Markdown.
