<!-- Generated from the doc file by scripts/generate.mjs; do not edit. -->

# Code

Code as it was written, in a sentence or as a block. Status: preview.

```ts
import { Code, CodeBlock } from '@liulidesign/core/Code';
```

Code is a run inside a sentence, in the fixed-width face on a tint: a file name, a key, a value to type exactly. CodeBlock keeps lines of code as they were written: it never wraps a line, it scrolls sideways, and it can carry a title and one action, usually copy. Neither colours the code by language.

## Examples

### A command to copy

```tsx
import { useLocal } from '@lingxia/view';
import { IconButton } from '@liulidesign/core/IconButton';
import { CodeBlock } from '@liulidesign/core/Code';

const install = `npm install @liulidesign/core
npx lingxia dev --platform ios`;

/** A command to copy: the title says what it is, and the app copies it. */
export function Command() {
  const copied = useLocal('copied', false);
  return (
    <view className="gap-2">
      <CodeBlock
        code={install}
        label="Install commands"
        title="Terminal"
        action={<IconButton label="Copy the commands" icon="copy" size="sm" variant="ghost" onClick={() => copied.set(true)} data-testid="copy" />}
        data-testid="install"
      />
      <text role="status" className="text-footnote text-muted">{copied.value ? 'Copied' : 'Not copied yet'}</text>
    </view>
  );
}
```

### A block without a header

```tsx
import { CodeBlock } from '@liulidesign/core/Code';

const response = `{
  "id": "note_8f2c",
  "title": "Trip to the coast",
  "updatedAt": "2026-11-14T09:42:00Z",
  "sharedWith": ["ana@example.com", "kai@example.com"]
}`;

/** Without a header: an example response whose long line scrolls sideways. */
export function Plain() {
  return <CodeBlock code={response} label="Example response" data-testid="response" />;
}
```

### In a sentence

```tsx
import { Code } from '@liulidesign/core/Code';

/** In a sentence: a file name and a value to type exactly. */
export function InSentence() {
  return (
    <text className="text-body text-text">
      Set <Code text="minRuntime" data-testid="key" /> in <Code text="lxapp.json" /> to the oldest runtime you test on.
    </text>
  );
}
```

## Props

| Prop | Type | Default | Description |
| --- | --- | --- | --- |
| `code` (required) | `string` |  | The code, with its own line breaks and indentation. |
| `label` (required) | `string` |  | Names the block for assistive tech: "Shell command", "Example response". |
| `title` | `string` |  | Shown over the code: a file name, a language. |
| `action` | `Child` |  | At the end of the title's line: a Button or IconButton that copies the code. The app does the copying. |
| `isSelectable` | `boolean` | `true` | Lets the code be selected. |
| `className` | `string` |  |  |
| `data-testid` | `string` |  |  |

## Anatomy

| Part | Element | Required | Description |
| --- | --- | --- | --- |
| inline | `<span>` | no | Code: a fixed-width run on a tint, inside a `<text>`. |
| block | `<view>` | no | CodeBlock: a tinted box, a group named by `label`. |
| header | `<view>` | no | The title and the action, over a hairline. |
| title | `<text>` | no | A file name or a language. |
| scroll | `<scroll>` | no | The sideways scroll around the code. |
| text | `<text>` | no | The code, fixed-width, with its own line breaks. |

## What an app may override

- `className`: classes merged after the recipe's, so they win.
- The recipe, `codeRecipe` from `@liulidesign/core/Code/recipe`: the classes of each part, to style an element of your own to match. The component does not read a changed copy.
- Scale tokens it reads, set in the app's stylesheet under `@theme`: `--text-footnote`, `--radius-control`.
- Colour roles it paints with, set by the theme pack: `text`, `text-muted`.

## Accessibility

- Pattern: none.
- Name: label.
- Description: none.
- Press target: .
- A block is a group named by `label`; its code is read as text, line by line.
- The action is the app's own Button or IconButton, with its own name.
- Inline code is read with its sentence; nothing announces the change of face.

| Contrast | Pair | Minimum | Light | Dark |
| --- | --- | --- | --- | --- |
| code on its tint | `text` on `text/8` over `surface` | 4.5:1 | 13.91:1 | 11.29:1 |
| title | `text-muted` on `text/6` over `surface` | 4.5:1 | 5.24:1 | 5.72:1 |

Ratios are computed from @lingxia/view/theme and rounded down.

## Guidance

- Do: Use Code for what must be typed exactly; use plain emphasis for a term.
- Do: Give a block a copy action when people will paste it somewhere.
- Don't: Put prose in a block to set it apart; that is a Card or a Banner.
- Don't: Break a long line by hand to make it fit; it scrolls.

## Limits

- No syntax colouring and no line numbers.
- Copying is the app's: Liuli has no clipboard.

Related: Markdown, Kbd.
