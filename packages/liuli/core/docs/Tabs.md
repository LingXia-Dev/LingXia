<!-- Generated from the doc file by scripts/generate.mjs; do not edit. -->

# Tabs

A row of tabs over one panel, as an underline or a segmented control. Status: preview.

```ts
import { Tabs } from '@liulidesign/core/Tabs';
```

Tabs switch one area between a few peers: sections of a page, or modes of one view. The tabs share the row equally, and a single indicator slides to the chosen one.

## Examples

### Sections of a page

```tsx
import { useLocal } from '@lingxia/view';
import { Tabs } from '@liulidesign/core/Tabs';

const sections = [
  { value: 'overview', label: 'Overview' },
  { value: 'activity', label: 'Activity' },
  { value: 'members', label: 'Members' },
];
const summaries: Record<string, string> = {
  overview: 'Three notebooks, 128 notes, last edited this morning.',
  activity: 'Ana edited "Q3 plan" and two others today.',
  members: 'Ana Lima, Kai Chen and Mira Patel can edit.',
};

/** Sections of one page: the underline slides to the chosen tab. */
export function Sections() {
  const section = useLocal('section', 'overview');
  return (
    <Tabs label="Project" items={sections} value={section.value} onChange={next => section.set(next)} data-testid="project">
      <text className="text-body text-muted">{summaries[section.value]}</text>
    </Tabs>
  );
}
```

### Modes of one view

```tsx
import { useLocal } from '@lingxia/view';
import { Tabs } from '@liulidesign/core/Tabs';

const ranges = [
  { value: 'day', label: 'Day' },
  { value: 'week', label: 'Week' },
  { value: 'month', label: 'Month' },
];

/** Modes of one view: the thumb slides to the chosen segment. */
export function Segmented() {
  const range = useLocal('range', 'week');
  const totals: Record<string, string> = { day: '42 minutes', week: '5 hours 10 minutes', month: '21 hours' };
  return (
    <Tabs label="Range" items={ranges} value={range.value} onChange={next => range.set(next)} variant="segmented" data-testid="range">
      <view className="gap-0.5">
        <text className="text-footnote text-muted">Time spent writing</text>
        <text className="text-title font-semibold text-text tabular-nums">{totals[range.value]}</text>
      </view>
    </Tabs>
  );
}
```

### A segmented filter over a list

```tsx
import { useLocal } from '@lingxia/view';
import { Item } from '@liulidesign/core/Item';
import { Tabs } from '@liulidesign/core/Tabs';

const notes = {
  all: ['Q3 plan', 'Reading list', 'Interview notes'],
  pinned: ['Q3 plan'],
  shared: ['Interview notes'],
};

/** A list filtered by a segmented control above it. */
export function Filter() {
  const filter = useLocal('filter', 'all');
  return (
    <Tabs
      label="Show"
      items={[{ value: 'all', label: 'All' }, { value: 'pinned', label: 'Pinned' }, { value: 'shared', label: 'Shared' }]}
      value={filter.value}
      onChange={next => filter.set(next)}
      variant="segmented"
      className="gap-3"
      data-testid="filter"
    >
      <view className="-mx-5 -mb-5 divide-y divide-text/8 border-t border-text/8">
        {notes[filter.value as keyof typeof notes].map(note => <Item key={note} label={note} />)}
      </view>
    </Tabs>
  );
}
```

## Props

| Prop | Type | Default | Description |
| --- | --- | --- | --- |
| `label` (required) | `string` |  | Names the tab list for assistive tech: "Range", "Sections". |
| `items` (required) | `readonly TabItem[]` |  | The tabs, in order; they share the row equally. |
| `value` (required) | `string` |  | The chosen tab's value. |
| `onChange` (required) | `(value: string) => void` |  | Runs with the tab chosen by a tap or by arrowing onto it. |
| `variant` | `TabsVariant` | `'underline'` | `underline` switches between sections of a page; `segmented` switches one view between modes. |
| `children` | `Child` |  | The chosen tab's panel. |
| `className` | `string` |  |  |
| `data-testid` | `string` |  | On the tab list; each tab gets `<testid>-<value>`, the panel `<testid>-panel`. |

## Anatomy

| Part | Element | Required | Description |
| --- | --- | --- | --- |
| frame | `<view>` | yes | The underline rule, or the sunken track of the segmented control. |
| list | `<view>` | yes | The tab list: one Tab stop, named by `label`. |
| track | `<view>` | yes | Holds the indicator; mirrors in right-to-left so it slides toward the end. |
| indicator | `<view>` | yes | The underline or thumb, one tab wide, sliding to the chosen tab. |
| tab | `<button>` | yes | One tab per item; its text is its name. |
| panel | `<view>` | no | The chosen tab's content from `children`, named after that tab. |

## Accessibility

- Pattern: tabs.
- Name: label.
- Description: none.
- States: selected (from value); disabled (from items[].isDisabled).
- Tab: Into the row, on the chosen tab; again, on to the panel..
- Left, Right: The previous or next tab, chosen on arrival; wraps at the ends..
- Home, End: The first or last tab..
- Press target: md 40 px.
- The chosen state is in the text weight and colour and in the indicator, never in colour alone.
- The segmented control is 36 px tall, with its 32 px segments; it suits pointer and compact layouts.
- The segmented thumb's edge is decorative and has no contrast minimum: the chosen segment is carried by its label and the raised thumb.

| Contrast | Pair | Minimum | Light | Dark |
| --- | --- | --- | --- | --- |
| chosen tab text | `text` on `surface` | 4.5:1 | 16.26:1 | 13.87:1 |
| other tabs' text | `text-muted` on `surface` | 4.5:1 | 5.89:1 | 6.64:1 |
| underline | `accent` on `surface` | 3:1 | needs the resolved palette | needs the resolved palette |
| chosen segment text | `text` on `surface-raised` | 4.5:1 | 16.26:1 | 11.83:1 |
| other segments' text | `text-muted` on `surface-sunken` | 4.5:1 | 5.20:1 | 6.93:1 |

Ratios are computed from crates/lingxia-theme/packs/default.json and rounded down.

## Guidance

- Do: Keep to two to five short labels that fit the row without truncating.
- Do: Use the underline for sections of a page and the segmented control for modes of one view.
- Don't: Use tabs for steps someone must finish in order.
- Don't: Put actions in a tab row; a tab only switches what is shown.

## Not yet

| Missing | Needs | Meanwhile |
| --- | --- | --- |
| Tabs as wide as their labels, scrolling when they overflow, with the indicator following. | a way to read a laid-out box's position and width from View, or a shared-element layout motion between siblings | Tabs share the row equally, so the indicator moves by a tab's width. |

Related: Button, DropdownMenu.
