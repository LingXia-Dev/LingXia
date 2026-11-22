<!-- Generated from the doc file by scripts/generate.mjs; do not edit. -->

# Section

A titled part of a page: a heading, then its content. Status: preview.

```ts
import { Section } from '@liulidesign/core/Section';
```

Section gives a part of a page its heading, an optional line of explanation and an action at the end of the heading's line, and keeps one distance between the heading and what follows. It draws no frame: put a Card, an ItemList or plain content inside.

## Examples

### A heading, a line and an action

```tsx
import { Said } from '../example';
import { Button } from '@liulidesign/core/Button';
import { Item, ItemList } from '@liulidesign/core/Item';
import { Section } from '@liulidesign/core/Section';

/** A part of a page with its heading, a line about it and a way to see more. */
export function Recent() {
  return (
    <Said>
      {say => (
        <Section title="Recent notes" description="Edited in the last week" action={<Button label="See all" variant="link" size="sm" onClick={() => say('Opened all recent notes')} />} data-testid="recent">
          <ItemList label="Recent notes">
            <Item label="Trip to the coast" description="Edited yesterday" hasChevron onClick={() => say('Opened Trip to the coast')} />
            <Item label="Reading list" description="Edited on Monday" hasChevron onClick={() => say('Opened Reading list')} />
          </ItemList>
        </Section>
      )}
    </Said>
  );
}
```

### Sections inside a section

```tsx
import { Section } from '@liulidesign/core/Section';

/** Sections inside a section step their headings down. */
export function Nested() {
  return (
    <Section title="Notifications" data-testid="outer">
      <Section title="Email" headingLevel={3} description="Sent to ana@example.com" data-testid="inner">
        <text className="text-body text-muted">A digest every Monday.</text>
      </Section>
      <Section title="On this device" headingLevel={3}>
        <text className="text-body text-muted">Mentions, at once.</text>
      </Section>
    </Section>
  );
}
```

## Props

| Prop | Type | Default | Description |
| --- | --- | --- | --- |
| `title` (required) | `string` |  | The section's heading. |
| `description` | `string` |  | A line under the heading: what the section is for. |
| `action` | `Child` |  | At the end of the heading's line: a Button or a link to see everything. |
| `headingLevel` | `2 \| 3 \| 4` | `2` | The heading's level in the page outline. |
| `children` (required) | `Child` |  |  |
| `className` | `string` |  |  |
| `data-testid` | `string` |  |  |

## Anatomy

| Part | Element | Required | Description |
| --- | --- | --- | --- |
| root | `<view>` | yes | The heading and the content, a fixed distance apart. |
| title | `<text>` | yes | The heading, at `headingLevel`. |
| description | `<text>` | no | A line under the heading. |
| action | `<view>` | no | At the end of the title's first line, centred on it, however the title or its line wrap. |

## What an app may override

- `className`: classes merged after the recipe's, so they win.
- The recipe, `sectionRecipe` from `@liulidesign/core/Section/recipe`: the classes of each part, to style an element of your own to match. The component does not read a changed copy.
- Scale tokens it reads, set in the app's stylesheet under `@theme`: `--text-footnote`, `--text-body`, `--text-callout`.
- Colour roles it paints with, set by the theme pack: `text`, `text-muted`.

## Accessibility

- Pattern: none.
- Name: none; its content is read.
- Description: none.
- Press target: .
- The title is a heading at `headingLevel`, so a screen reader can move through a page by its sections.
- Step the level down inside another section: 2, then 3, then 4.

| Contrast | Pair | Minimum | Light | Dark |
| --- | --- | --- | --- | --- |
| heading | `text` on `canvas` | 4.5:1 | 15.19:1 | 15.31:1 |
| line under it, and a level-4 heading | `text-muted` on `canvas` | 4.5:1 | 5.50:1 | 7.33:1 |

Ratios are computed from @lingxia/view/theme and rounded down.

## Guidance

- Do: Name the section by what it holds: "Recent notes", not "Section 2".
- Do: Keep one action at most on the heading's line, and make it about the whole section.
- Don't: Wrap a single field or row in a section; a heading over one thing is noise.
- Don't: Skip a heading level to get a smaller size.

Related: Card, Item, Divider.
