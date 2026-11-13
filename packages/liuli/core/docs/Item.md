<!-- Generated from the doc file by scripts/generate.mjs; do not edit. -->

# Item

One row of a list: a label, a second line, and content at either end. Status: preview.

```ts
import { Item } from '@liulidesign/core/Item';
```

Item is the row of a settings screen, an inbox or a file list. With `onClick` the whole row is one button that opens or selects something; without it the row is plain content and its `endContent` may hold its own control. Rows carry no border or fill, so the container decides how a list is grouped and divided.

## Examples

### Rows that open a page

```tsx
import { Item } from '@liulidesign/core/Item';

const notes = [
  { id: 'n1', title: 'Groceries', updated: 'Edited today' },
  { id: 'n2', title: 'Trip to the coast', updated: 'Edited yesterday' },
  { id: 'n3', title: 'Reading list', updated: 'Edited last week' },
] as const;

/** Rows that open a page: each is one button named by its title, with the second line as its description. */
export function Navigation() {
  return (
    <view className="overflow-hidden rounded-container border border-line bg-surface divide-y divide-line">
      {notes.map(note => (
        <Item key={note.id} label={note.title} description={note.updated} hasChevron onClick={() => {}} data-testid={`note-${note.id}`} />
      ))}
    </view>
  );
}
```

### Read-only rows with values

```tsx
import { Item } from '@liulidesign/core/Item';

const folder = [{ d: 'M3 7a2 2 0 0 1 2-2h4l2 2h8a2 2 0 0 1 2 2v8a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z', fill: 'none', stroke: 'currentColor', strokeWidth: 2, strokeLinejoin: 'round' }] as const;

/** A value at the end of a read-only row, and an icon at the start. */
export function Values() {
  return (
    <view className="overflow-hidden rounded-container border border-line bg-surface divide-y divide-line">
      <Item
        label="Version"
        endContent={<text className="text-footnote text-muted">2.4.1</text>}
        data-testid="version"
      />
      <Item
        label="Storage"
        description="Photos and documents"
        startContent={<vector aria-hidden paths={folder} viewBox={[0, 0, 24, 24]} className="size-5" />}
        endContent={<text className="text-footnote text-muted">7.2 GB</text>}
      />
    </view>
  );
}
```

### A control at the end

```tsx
import { useLocal } from '@lingxia/view';
import { Button } from '@liulidesign/core/Button';
import { Item } from '@liulidesign/core/Item';

/** A row without `onClick` can hold its own control at the end. */
export function WithControl() {
  const signedIn = useLocal('signedIn', true);
  return (
    <view className="overflow-hidden rounded-container border border-line bg-surface">
      <Item
        label="Account"
        description={signedIn.value ? 'Signed in' : 'Signed out'}
        endContent={
          <Button
            label={signedIn.value ? 'Sign out' : 'Sign in'}
            size="sm"
            onClick={() => signedIn.set(!signedIn.value)}
            data-testid="account-action"
          />
        }
        data-testid="account"
      />
    </view>
  );
}
```

### Disabled, with the reason

```tsx
import { Item } from '@liulidesign/core/Item';

export function Disabled() {
  return (
    <view className="overflow-hidden rounded-container border border-line bg-surface">
      <Item
        label="Export history"
        description="Download every change as a file"
        disabledMessage="Available once a change is saved"
        isDisabled
        hasChevron
        onClick={() => {}}
        data-testid="export"
      />
    </view>
  );
}
```

### A destructive row

```tsx
import { Item } from '@liulidesign/core/Item';

const notes = [
  { id: 'n1', title: 'Groceries', updated: 'Edited today' },
  { id: 'n2', title: 'Trip to the coast', updated: 'Edited yesterday' },
  { id: 'n3', title: 'Reading list', updated: 'Edited last week' },
] as const;

/** A row that deletes or ends something; the danger tone marks it before it is pressed. */
export function Destructive() {
  return (
    <view className="overflow-hidden rounded-container border border-line bg-surface">
      <Item label="Delete account" description="Removes your notes from every device" tone="danger" hasChevron onClick={() => {}} data-testid="delete-row" />
    </view>
  );
}
```

## Props

| Prop | Type | Default | Description |
| --- | --- | --- | --- |
| `onClick` | `(() => void)` |  | Makes the whole row one button. Its `endContent` is then drawn only, never pressed. |
| `hasChevron` | `boolean` | `false` | A chevron at the end, for a row that opens another page. |
| `isDisabled` | `boolean` | `false` |  |
| `disabledMessage` | `string` |  | Shown in place of `description` while disabled. |
| `label` (required) | `string` |  | The row's main text, and its accessible name when it is pressable. |
| `tone` | `ItemTone` | `'default'` | `danger` draws the label in the danger role, for a row that deletes or ends something. |
| `description` | `string` |  | A second line under the label; read as the row's description when it is pressable. |
| `startContent` | `Child` |  | Before the text: an icon or an avatar. Hidden from assistive tech on a pressable row. |
| `endContent` | `Child` |  | After the text: a value, a badge, or, on a row without `onClick`, a control such as a Button. |
| `className` | `string` |  |  |
| `data-testid` | `string` |  |  |

## Anatomy

| Part | Element | Required | Description |
| --- | --- | --- | --- |
| root | `<view>` | yes | A `button` when `onClick` is set, otherwise a `view`; at least the large control height. |
| start | `<view>` | no | `startContent`: an icon or avatar. |
| label | `<text>` | yes | The main text; it wraps rather than truncating. |
| description | `<text>` | no | `description`, or `disabledMessage` while disabled. |
| end | `<view>` | no | `endContent`: a value, a badge, or a control on a plain row. |
| chevron | `<vector>` | no | Shown with `hasChevron`; mirrors for right-to-left text. |

## Accessibility

- Pattern: button.
- Name: label.
- Description: description, or disabledMessage while disabled.
- States: disabled (from isDisabled).
- Press target: lg 48 px.
- The button pattern applies to a row with `onClick`. A plain row is not a control; its texts are read in order.
- On a pressable row the start and end content are drawn only, so the row is read as its label and second line.
- Every row is at least 48 px tall, above the 44 pt touch minimum.

| Contrast | Pair | Minimum | Light | Dark |
| --- | --- | --- | --- | --- |
| label | `text` on `surface` | 4.5:1 | 16.26:1 | 13.87:1 |
| second line and end values | `text-muted` on `surface` | 4.5:1 | 5.89:1 | 6.64:1 |
| chevron | `text-faint` on `surface` | 3:1 | 3.09:1 | 4.42:1 |

Ratios are computed from @lingxia/view/theme and rounded down.

## Guidance

- Do: Use `onClick` with `hasChevron` for a row that opens another page.
- Do: Keep the label short and put detail in `description`.
- Do: Group rows in a container with a border or fill and divide them (`divide-y divide-line`).
- Don't: Put a button or other control in `endContent` of a row with `onClick`; the View refuses a button inside a button.
- Don't: Use a row for a single action; that is a Button.
- Don't: Rely on the end value alone on a pressable row; it is not read, so repeat it in `description` when it matters.

## Not yet

| Missing | Needs | Meanwhile |
| --- | --- | --- |
| Rows reported as items of a list, with their position. | a listitem role on a pressable row, or list position on a button | Rows are buttons or plain views; the list itself is not announced. |
| Swipe to reveal row actions. | a horizontal drag with snap points that closes other open rows | Put actions in a DropdownMenu at the end of a plain row. |

Related: DropdownMenu, Card.
