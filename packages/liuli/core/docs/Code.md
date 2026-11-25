<!-- Generated from the doc file by scripts/generate.mjs; do not edit. -->

# Code

Code as it was written, in a sentence or as a block. Status: preview.

```ts
import { Code, CodeBlock } from '@liulidesign/core/Code';
```

Code is a run inside a sentence, in the fixed-width face on a tint: a file name, a key, a value to type exactly. CodeBlock keeps lines of code as they were written: it scrolls sideways, or wraps them at its width with `isWrapped`, and it can carry a title, a copy button and one action of the app's. It can number its lines, mark some of them, show a change as a unified diff, cap its height with `maxHeight`, and colour the code by its `language`: keywords, strings, numbers, types and comments, the rest in the text colour.

## Examples

### A command to copy

```tsx
import { useLocal } from '@lingxia/view';
import { CodeBlock } from '@liulidesign/core/Code';

const install = `npm install @liulidesign/core
npx lingxia dev --platform ios`;

/** A command to copy: the title says what it is, and the block's copy button hands the code to the app's clipboard call. */
export function Command() {
  // Stands in for the app's clipboard call.
  const clipboard = useLocal('clipboard', '');
  return (
    <CodeBlock
      code={install}
      language="bash"
      label="Install commands"
      title="Terminal"
      onCopy={code => clipboard.set(code)}
      data-testid="install"
    />
  );
}
```

### A long file under a height cap

```tsx
import { useLocal } from '@lingxia/view';
import { CodeBlock } from '@liulidesign/core/Code';

const config = `export default {
  name: 'notes',
  version: '1.4.0',
  pages: ['pages/home/index', 'pages/note/index', 'pages/settings/index'],
  window: { title: 'Notes', theme: 'system' },
  network: { timeout: 15000, retries: 2 },
  storage: { quota: '50MB', encrypt: true },
  share: { targets: ['link', 'image'] },
  sync: { interval: 300, onCellular: false },
  logs: { level: 'warn' },
};`;

/** A long file in a short box: past `maxHeight` the code scrolls up and down under the title's line. */
export function Capped() {
  const clipboard = useLocal('clipboard', '');
  return <CodeBlock code={config} language="ts" label="lxapp.config.ts" title="lxapp.config.ts" maxHeight={160} onCopy={code => clipboard.set(code)} data-testid="capped" />;
}
```

### Long lines wrapped

```tsx
import { CodeBlock } from '@liulidesign/core/Code';

const prompt = 'Summarise the note in three short bullet points, keep every date and amount exactly as written, and end with one line listing anything that still needs an answer.';

/** Prose-like text, such as a prompt, wrapped at the block's width rather than scrolled. */
export function Wrapped() {
  return <CodeBlock code={prompt} label="Prompt" title="Prompt" isWrapped data-testid="wrapped" />;
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
  return <CodeBlock code={response} language="json" label="Example response" data-testid="response" />;
}
```

### Line numbers and marked lines

```tsx
import { CodeBlock } from '@liulidesign/core/Code';

const excerpt = `export function total(items: readonly Item[]) {
  let sum = 0;
  for (const item of items) sum += item.price * item.count;
  return Math.round(sum * 100) / 100;
}`;

/** An excerpt with its numbers in the file, two lines marked. */
export function Numbered() {
  return <CodeBlock code={excerpt} language="ts" label="total in cart.ts" title="cart.ts" hasLineNumbers startLineNumber={41} highlightLines={[43, 44]} data-testid="numbered" />;
}
```

### A change as a diff

```tsx
import { CodeBlock } from '@liulidesign/core/Code';

const change = `--- a/cart.ts
+++ b/cart.ts
@@ -42,4 +42,4 @@
   let sum = 0;
   for (const item of items)
-    sum += item.price;
+    sum += item.price * item.count;
   return Math.round(sum * 100) / 100;`;

/** A change as a unified diff: each sign in its own column, added and removed lines tinted. */
export function Diff() {
  return <CodeBlock code={change} language="ts" label="Change to cart.ts" title="cart.ts" isDiff data-testid="diff" />;
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
| `hasLineNumbers` | `boolean` | `false` | Numbers each line in a gutter before the code. |
| `startLineNumber` | `number` | `1` | The first line's number, so an excerpt shows its lines' numbers in the file. Below 1 reads as 1. |
| `highlightLines` | `readonly number[]` |  | Lines to mark, by the numbers the gutter shows (see `startLineNumber`). A number outside the code marks nothing; in a diff an added or removed line keeps its own tint. |
| `isDiff` | `boolean` | `false` | Reads the code as a unified diff: a line starting `+` is added and `-` removed, `@@` starts a hunk, and a `--- `/`+++ ` pair just before one names the files. The sign sits in its own column; `onCopy` and the app's action still get the whole text, signs included. |
| `isWrapped` | `boolean` | `false` | Long lines wrap at the block's width instead of scrolling sideways. |
| `code` (required) | `string` |  | The code, with its own line breaks and indentation. |
| `label` (required) | `string` |  | Names the block for assistive tech: "Shell command", "Example response". |
| `title` | `string` |  | Shown over the code: a file name, a language. |
| `language` | `string` |  | Colours the code by its language's grammar: `ts`, `tsx`, `js`, `json`, `css`, `html`, `xml`, `python`, `bash`, `yaml`, `toml`, `rust`, `kotlin`, `swift`, `java`, `go`, `c`, `cpp` or `sql`, by name or file extension. Any other stays plain. |
| `onCopy` | `((code: string) => void \| Promise<unknown>)` |  | Draws a copy button at the end of the title's line and runs with the code when it is pressed. Liuli has no clipboard: pass the call that puts text on it. When that call returns (a promise, once it resolves) the button says "Copied" for a moment. |
| `action` | `Child` |  | At the end of the title's line, before the copy button: a Button or IconButton the app draws. |
| `maxHeight` | `number` |  | The code area's tallest height, in points; longer code scrolls inside it, under the title's line. |
| `isSelectable` | `boolean` | `true` | Lets the code be selected. |
| `className` | `string` |  |  |
| `data-testid` | `string` |  |  |

## Anatomy

| Part | Element | Required | Description |
| --- | --- | --- | --- |
| inline | `<span>` | no | Code: a fixed-width run on a tint, inside a `<text>`. |
| block | `<view>` | no | CodeBlock: a tinted box, a group named by `label`. |
| header | `<view>` | no | The title, the action and the copy button, over a hairline. |
| copy | `<view>` | no | With `onCopy`: the copy button, which shows a tick and is named "Copied" for a moment after a copy. |
| copied | `<view>` | no | "Copied", in a bubble above the copy button, off the code, read as a status; it closes by itself. |
| title | `<text>` | no | A file name or a language. |
| scroll | `<scroll>` | no | The sideways scroll around the code. |
| wrap | `<view>` | no | With `isWrapped`: the box the wrapped code fills, in place of the sideways scroll. |
| capped | `<scroll>` | no | With `maxHeight`: the scroll up and down around the code, under the header. |
| tint | `<view>` | no | A row behind one line: marked, added, removed, or a diff's header. In a diff a mark tints only an unchanged line. |
| gutter | `<text>` | no | The line numbers, before a hairline. |
| signs | `<text>` | no | In a diff, each line's `+`, `-` or space, in its own column. |
| text | `<text>` | no | The code, fixed-width, with its own line breaks. |
| syntax | `<span>` | no | A token in its kind's colour: a keyword, a string, a number or literal, a type, a comment. |

## What an app may override

- `className`: classes merged after the recipe's, so they win.
- The recipe, `codeRecipe` from `@liulidesign/core/Code/recipe`: the classes of each part, to style an element of your own to match. The component does not read a changed copy.
- Scale tokens it reads, set in the app's stylesheet under `@theme`: `--text-footnote`, `--shadow-raised`, `--radius-control`.
- Colour roles it paints with, set by the theme pack: `accent`, `accent-soft`, `chart-4`, `chart-5`, `chart-7`, `danger`, `danger-soft`, `line`, `success`, `success-soft`, `surface`, `text`, `text-muted`.

## Accessibility

- Pattern: none.
- Name: label.
- Description: none.
- Press target: .
- A block is a group named by `label`; its code is read as text, line by line.
- The copy button is named "Copy" and, for a moment after a copy, "Copied"; the bubble saying so is a status, read once.
- The action is the app's own Button or IconButton, with its own name.
- Line numbers are not read; a diff's signs are, so a change reads without its colour.
- A marked line is marked by its tint alone; say in the text around the block why it is marked.
- Inline code is read with its sentence; nothing announces the change of face.

| Contrast | Pair | Minimum | Light | Dark |
| --- | --- | --- | --- | --- |
| code on its tint | `text` on `text/8` over `surface` | 4.5:1 | 13.91:1 | 11.29:1 |
| title and line numbers | `text-muted` on `text/6` over `surface` | 4.5:1 | 5.24:1 | 5.72:1 |
| code on an added line | `text` on `success-soft` | 4.5:1 | 14.33:1 | 11.76:1 |
| code on a removed line | `text` on `danger-soft` | 4.5:1 | 14.12:1 | 12.22:1 |
| code on a marked line | `text` on `accent-soft` | 4.5:1 | 14.25:1 | 11.26:1 |
| a sign and number on an added line | `success` on `success-soft` | 4.5:1 | 4.68:1 | 6.45:1 |
| a sign and number on a removed line | `danger` on `danger-soft` | 4.5:1 | 4.53:1 | 4.77:1 |
| a number on a marked line | `text-muted` on `accent-soft` | 4.5:1 | 5.16:1 | 5.39:1 |
| a hunk or file header on its tint, over the block | `text-muted` on `text/12` over `surface` | 4.5:1 | 4.64:1 | 4.81:1 |
| a keyword | `accent` on `text/6` over `surface` | 4.5:1 | 5.00:1 | 5.33:1 |
| a string | `chart-5` on `text/6` over `surface` | 4.5:1 | 5.03:1 | 7.16:1 |
| a number or literal | `chart-4` on `text/6` over `surface` | 4.5:1 | 5.17:1 | 6.12:1 |
| a type | `chart-7` on `text/6` over `surface` | 4.5:1 | 7.38:1 | 7.04:1 |
| "Copied" in its bubble | `surface` on `text` | 4.5:1 | 16.26:1 | 13.87:1 |

Ratios are computed from @lingxia/view/theme and rounded down.

## Guidance

- Do: Use Code for what must be typed exactly; use plain emphasis for a term.
- Do: Give a block `onCopy` when people will paste it somewhere.
- Do: Cap a long file with `maxHeight` so the page around it stays in reach.
- Don't: Put prose in a block to set it apart; that is a Card or a Banner.
- Don't: Break a long line by hand to make it fit; it scrolls, or use `isWrapped` for prose-like text such as a prompt.
- Don't: Wrap code whose line breaks carry meaning; a wrapped line reads as two.

## Limits

- Colouring reads tokens, not meaning: a name is coloured as a type by its capital, and a regular expression in JavaScript is not told from division.
- With numbers, a diff or marked lines, a line is never wrapped, even at large text; it scrolls. `isWrapped` does not go with them.
- Copying is the app's: Liuli has no clipboard, so `onCopy` makes the call that puts the text on it.
- Code past 64 KiB or about 800 coloured runs is several texts, one under another; a selection stays inside one of them.

Related: Markdown, Kbd.
