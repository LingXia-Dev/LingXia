<!-- Generated from the doc file by scripts/generate.mjs; do not edit. -->

# Item

One row of a list: a label, a second line, and content at either end. Status: preview.

```ts
import { Item, ItemList } from '@liulidesign/core/Item';
```

Item is the row of a settings screen, an inbox or a file list. With `onClick` the whole row is one button that opens or selects something; without it the row is plain content and its `endContent` may hold its own control. Rows carry no border or fill; ItemList, from the same import, groups and divides them, reads them as a list, and can carry a heading and a line of explanation. A row can hide up to three actions behind its end, shown by dragging it aside, and a list can be made compact for a mouse.

## Examples

### Rows that open a page

```tsx
import { Said } from '../example';
import { Item, ItemList } from '@liulidesign/core/Item';

const notes = [
  { id: 'n1', title: 'Groceries', updated: 'Edited today' },
  { id: 'n2', title: 'Trip to the coast', updated: 'Edited yesterday' },
  { id: 'n3', title: 'Reading list', updated: 'Edited last week' },
] as const;

/** Rows that open a page, in a list: each is one button named by its title, read with its position. */
export function Navigation() {
  return (
    <Said>
      {say => (
        <ItemList label="Notes" data-testid="notes">
          {notes.map(note => (
            <Item key={note.id} label={note.title} description={note.updated} hasChevron onClick={() => say(`Opened ${note.title}`)} data-testid={`note-${note.id}`} />
          ))}
        </ItemList>
      )}
    </Said>
  );
}
```

### Rows that choose one

```tsx
import { useLocal } from '@lingxia/view';
import { Item, ItemList } from '@liulidesign/core/Item';

const mailboxes = [
  { id: 'inbox', label: 'Inbox', count: '12' },
  { id: 'drafts', label: 'Drafts', count: '3' },
  { id: 'sent', label: 'Sent', count: '' },
] as const;
const inbox = [
  { id: 'm1', from: 'Ana Lee', subject: 'Notes from Tuesday' },
  { id: 'm2', from: 'Ben Okafor', subject: 'Invoice for October' },
  { id: 'm3', from: 'Chloé Martin', subject: 'Photos from the coast' },
] as const;

/** Rows that choose one of several, as a sidebar does: the chosen row is selected, the others are not. */
export function Selection() {
  const open = useLocal<string>('open', 'inbox');
  return (
    <ItemList label="Mailboxes" data-testid="mailboxes">
      {mailboxes.map(box => (
        <Item
          key={box.id}
          label={box.label}
          endContent={box.count ? <text className="text-footnote text-muted">{box.count}</text> : undefined}
          isSelected={open.value === box.id}
          onClick={() => open.set(box.id)}
          data-testid={`mailbox-${box.id}`}
        />
      ))}
    </ItemList>
  );
}
```

### Read-only rows with values

```tsx
import { Item, ItemList } from '@liulidesign/core/Item';

const folder = [{ d: 'M3 7a2 2 0 0 1 2-2h4l2 2h8a2 2 0 0 1 2 2v8a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z', fill: 'none', stroke: 'currentColor', strokeWidth: 2, strokeLinejoin: 'round' }] as const;

/** A value at the end of a read-only row, and an icon at the start. */
export function Values() {
  return (
    <ItemList label="About">
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
    </ItemList>
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
    <view className="overflow-hidden rounded-container border border-text/12 bg-surface">
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
import { unused } from '../example';
import { Item } from '@liulidesign/core/Item';

export function Disabled() {
  return (
    <view className="overflow-hidden rounded-container border border-text/12 bg-surface">
      <Item
        label="Export history"
        description="Download every change as a file"
        disabledMessage="Available once a change is saved"
        isDisabled
        hasChevron
        onClick={unused}
        data-testid="export"
      />
    </view>
  );
}
```

### A destructive row

```tsx
import { Said } from '../example';
import { Item } from '@liulidesign/core/Item';

const notes = [
  { id: 'n1', title: 'Groceries', updated: 'Edited today' },
  { id: 'n2', title: 'Trip to the coast', updated: 'Edited yesterday' },
  { id: 'n3', title: 'Reading list', updated: 'Edited last week' },
] as const;

/** A row that deletes or ends something; the danger tone marks it before it is pressed. */
export function Destructive() {
  return (
    <Said>
      {say => (
        <view className="overflow-hidden rounded-container border border-text/12 bg-surface">
          <Item label="Delete account" description="Removes your notes from every device" tone="danger" hasChevron onClick={() => say('Opened Delete account')} data-testid="delete-row" />
        </view>
      )}
    </Said>
  );
}
```

### Actions behind a row, and a list with a heading

```tsx
import { useLocal } from '@lingxia/view';
import { Said } from '../example';
import { Item, ItemList } from '@liulidesign/core/Item';

const inbox = [
  { id: 'm1', from: 'Ana Lee', subject: 'Notes from Tuesday' },
  { id: 'm2', from: 'Ben Okafor', subject: 'Invoice for October' },
  { id: 'm3', from: 'Chloé Martin', subject: 'Photos from the coast' },
] as const;

/** Drag a row toward its start to show its actions; the list has a heading and a line under it. */
export function Swipe() {
  const gone = useLocal<readonly string[]>('gone', []);
  const flagged = useLocal<readonly string[]>('flagged', []);
  return (
    <Said>
      {say => (
        <ItemList title="Inbox" description="Drag a message to the side to flag or delete it." data-testid="inbox">
          {inbox.filter(mail => !gone.value.includes(mail.id)).map(mail => (
            <Item
              key={mail.id}
              label={mail.from}
              description={mail.subject}
              endContent={flagged.value.includes(mail.id) ? <text className="text-footnote text-muted">Flagged</text> : undefined}
              hasChevron
              onClick={() => say(`Opened ${mail.subject}`)}
              swipeActions={[
                { key: 'flag', label: 'Flag', icon: 'flag', onClick: () => flagged.set(flagged.value.includes(mail.id) ? flagged.value.filter(id => id !== mail.id) : [...flagged.value, mail.id]) },
                { key: 'delete', label: 'Delete', icon: 'delete', tone: 'danger', onClick: () => gone.set([...gone.value, mail.id]) },
              ]}
              data-testid={`mail-${mail.id}`}
            />
          ))}
        </ItemList>
      )}
    </Said>
  );
}
```

### A compact list

```tsx
import { Said } from '../example';
import { Item, ItemList } from '@liulidesign/core/Item';

const files = [
  { name: 'Budget.xlsx', size: '48 KB' }, { name: 'Contract.pdf', size: '1.2 MB' }, { name: 'Logo.svg', size: '6 KB' },
  { name: 'Notes.md', size: '3 KB' }, { name: 'Roadmap.key', size: '14 MB' },
] as const;

/** Compact rows for a long list read with a mouse. */
export function Compact() {
  return (
    <Said>
      {say => (
        <ItemList title="Files" density="compact" data-testid="files">
          {files.map(file => (
            <Item key={file.name} label={file.name} endContent={<text className="text-footnote text-muted tabular-nums">{file.size}</text>} onClick={() => say(`Opened ${file.name}`)} data-testid={`file-${file.name}`} />
          ))}
        </ItemList>
      )}
    </Said>
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
| `isSelected` | `boolean` |  | The current choice among rows that select, such as a sidebar's open page: an accent wash and a heavier label, read as selected. Pass `false` on the other rows so they read as not selected. |
| `label` (required) | `string` |  | The row's main text, and its accessible name when it is pressable. |
| `tone` | `ItemTone` | `'default'` | `danger` draws the label in the danger role, for a row that deletes or ends something. |
| `description` | `string` |  | A second line under the label; read as the row's description when it is pressable. |
| `startContent` | `Child` |  | Before the text: an icon or an avatar. Hidden from assistive tech on a pressable row. |
| `startIcon` | `IconRef` |  | An icon before the text, by name or as a definition, in a quiet tinted square: for rows that name a place or a kind. Not read. |
| `endContent` | `Child` |  | After the text: a value, a badge, or, on a row without `onClick`, a control such as a Button. |
| `density` | `ItemDensity` |  | The row's height and padding. By default the ItemList's, else `comfortable`. |
| `swipeActions` | `readonly ItemSwipeAction[]` |  | Up to three actions behind the row's end, shown by dragging the row toward its start. A screen reader offers them as the row's actions; with a keyboard the arrow keys open the row. |
| `className` | `string` |  |  |
| `data-testid` | `string` |  |  |

## ItemList props

| Prop | Type | Default | Description |
| --- | --- | --- | --- |
| `children` (required) | `Child` |  | Rows: Items, or row variants of Switch and Slider. |
| `label` | `string` |  | Names the list for assistive input, when neither `title` nor a heading above it does. |
| `title` | `string` |  | A heading over the list, in the quiet type of a settings group; it names the list. |
| `description` | `string` |  | A line under the list that explains the group. |
| `density` | `ItemDensity` | `the screen's density, 'comfortable' unless a DensityProvider says otherwise` | The rows' height and padding; a row's own `density` wins. |
| `isFlush` | `boolean` | `false` | Rows on the pane's own surface, with no border, corners or fill of the list's: for a list that fills a bordered pane. |
| `className` | `string` |  |  |
| `data-testid` | `string` |  |  |

## ItemSwipeAction props

| Prop | Type | Default | Description |
| --- | --- | --- | --- |
| `key` (required) | `string` |  | Identifies the action: its test id is `<row id>-<key>`. |
| `label` (required) | `string` |  | Drawn under the icon, and what a screen reader offers as the row's action. |
| `icon` | `IconRef` |  |  |
| `tone` | `ItemSwipeTone` | `'neutral'` |  |
| `onClick` (required) | `() => void` |  | Runs after the row has closed. |

## Anatomy

| Part | Element | Required | Description |
| --- | --- | --- | --- |
| group | `<view>` | no | ItemList with a `title` or `description`: the heading, the list and the line under it. |
| title | `<text>` | no | ItemList's `title`: a quiet heading that names the list. |
| note | `<text>` | no | ItemList's `description`, under the list. |
| list | `<view>` | no | ItemList: a bordered, divided list, read as a list. |
| listitem | `<view>` | no | Around each row in an ItemList, Item or Switch and Slider rows alike; it carries the row's position. |
| root | `<view>` | yes | A `button` when `onClick` is set, otherwise a `view`; at least the large control height. |
| iconTile | `<view>` | no | `startIcon`: an icon by name in a 32 px tinted square; not read. |
| start | `<view>` | no | `startContent`: an icon or avatar. |
| label | `<text>` | yes | The main text; it wraps rather than truncating. |
| description | `<text>` | no | `description`, or `disabledMessage` while disabled. |
| end | `<view>` | no | `endContent`: a value, a badge, or a control on a plain row. |
| chevron | `<vector>` | no | Shown with `hasChevron`; mirrors for right-to-left text. |
| swipe | `<view>` | no | Around a row with `swipeActions`: it clips the row as it slides. |
| slide | `<view>` | no | The part that moves: the row on the list's surface, dragged between closed and open. |
| actions | `<view>` | no | Behind the row's end: the actions, 72 px each. |
| action | `<button>` | no | One action: an icon over its label, in its tone. |

## What an app may override

- `className`: classes merged after the recipe's, so they win.
- `className` on ItemList: classes merged after the recipe's, so they win.
- The recipe, `itemRecipe` from `@liulidesign/core/Item/recipe`: the classes of each part, to style an element of your own to match. The component does not read a changed copy.
- Scale tokens it reads, set in the app's stylesheet under `@theme`: `--spacing-control-md`, `--spacing-control-lg`, `--text-caption`, `--text-footnote`, `--text-body`, `--radius-control`, `--radius-container`.
- Colour roles it paints with, set by the theme pack: `accent`, `accent-soft`, `accent-solid`, `accent-solid-hover`, `accent-solid-pressed`, `danger`, `danger-solid`, `danger-solid-hover`, `danger-solid-pressed`, `focus`, `on-accent-soft`, `on-accent-solid`, `on-danger-solid`, `surface`, `surface-overlay`, `text`, `text-faint`, `text-muted`.

## Accessibility

- Pattern: button.
- Name: label.
- Description: description, or disabledMessage while disabled.
- States: disabled (from isDisabled); selected (from isSelected).
- Left and Right: On a row with `swipeActions`: opens and closes the row; Tab then reaches the actions.
- Press target: md 40 px, lg 48 px.
- The button pattern applies to a row with `onClick`. A plain row is not a control; its texts are read in order.
- On a pressable row the start and end content are drawn only, so the row is read as its label and second line.
- In an ItemList each row is read with its position ("2 of 3"), whether it is a button or plain content.
- Every row is at least 48 px tall, above the 44 pt touch minimum; a `compact` row is 40 px, for lists read with a mouse.
- A row with `swipeActions` is read as an adjustable named by its label that offers each action by its label; while it is closed the action buttons are out of the reading order. The actions never depend on the drag: a screen reader runs them from the row, and a keyboard opens the row with the arrow keys.
- An ItemList's `title` names the list and is not read a second time; pass `label` as well to name the list differently.
- A row with `isSelected` is read as selected, and one with `isSelected={false}` as not selected; a row without the prop has no selected state.

| Contrast | Pair | Minimum | Light | Dark |
| --- | --- | --- | --- | --- |
| label | `text` on `surface` | 4.5:1 | 16.26:1 | 13.87:1 |
| second line and end values | `text-muted` on `surface` | 4.5:1 | 5.89:1 | 6.64:1 |
| chevron | `text-faint` on `surface` | 3:1 | 3.09:1 | 4.42:1 |
| icon in its tinted square | `on-accent-soft` on `accent-soft` | 3:1 | 8.54:1 | 9.49:1 |
| label, selected | `text` on `accent-soft` | 4.5:1 | 14.25:1 | 11.26:1 |
| second line and end values, selected | `text-muted` on `accent-soft` | 4.5:1 | 5.16:1 | 5.39:1 |
| chevron, selected | `text-muted` on `accent-soft` | 3:1 | 5.16:1 | 5.39:1 |
| list heading and the line under it | `text-muted` on `canvas` | 4.5:1 | 5.50:1 | 7.33:1 |
| swipe action, neutral | `text` on `text/8` over `surface` | 4.5:1 | 13.91:1 | 11.29:1 |
| swipe action, accent | `on-accent-solid` on `accent-solid` | 4.5:1 | 5.62:1 | 4.80:1 |
| swipe action, danger | `on-danger-solid` on `danger-solid` | 4.5:1 | 5.21:1 | 4.82:1 |

Ratios are computed from @lingxia/view/theme and rounded down.

## Guidance

- Do: For a dense screen set `density` on the ItemList, or wrap the screen in a DensityProvider with `compact` so every list and table in it follows.
- Do: Use `onClick` with `hasChevron` for a row that opens another page.
- Do: Keep the label short and put detail in `description`.
- Do: At large text, keep `endContent` to a short value or badge: it keeps its whole width and the label and description wrap beside it.
- Do: Group rows in an ItemList, so the rows are divided and read with their position.
- Do: Keep swipe actions to the two or three a person repeats down a list, the destructive one last; offer the same actions on the page the row opens.
- Do: Use `compact` where the list is long and read with a mouse; keep `comfortable` on a phone.
- Do: Mark the current choice with `isSelected` and pass `isSelected={false}` on the other rows of the same list.
- Don't: Put a button or other control in `endContent` of a row with `onClick`; the View refuses a button inside a button.
- Don't: Use a row for a single action; that is a Button.
- Don't: Rely on the end value alone on a pressable row; it is not read, so repeat it in `description` when it matters.

## Not yet

| Missing | Needs | Meanwhile |
| --- | --- | --- |
| One stop for a pressable row with swipe actions. | a hover and pressed wash that stays opaque on a dragged `<button>`, or a way to hide the row's slide from assistive tech while keeping its actions | The slide and the button inside it are two stops with the same name: the first offers the actions, the second opens the row. Not yet heard on a screen reader. |

Related: DropdownMenu, Card.
