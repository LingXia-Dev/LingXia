<!-- Generated from the doc file by scripts/generate.mjs; do not edit. -->

# ActionSheet

A short list of actions about one thing, with Cancel apart from them. Status: preview.

```ts
import { ActionSheet } from '@liulidesign/core/ActionSheet';
```

ActionSheet asks what to do with one thing: a photo, a draft, a message. Its rows are the answers, each a verb, a destructive one in the danger colour; Cancel stands apart under a hairline. On a phone it is a sheet on the bottom edge, in reach of a thumb, that drags off to cancel; on a wide window it is a small panel in the middle. Choosing a row closes the sheet and runs its action. The window feedback layer draws `lx.ui.actionSheet` with it.

## Examples

### Actions on a photo, with icons, the destructive one last

```tsx
import { useLocal } from '@lingxia/view';
import { Button } from '@liulidesign/core/Button';
import { ActionSheet } from '@liulidesign/core/ActionSheet';

/** What to do with a photo: a few actions with icons, the destructive one last, Cancel apart. */
export function Basic() {
  const open = useLocal('open', false);
  const done = useLocal('done', 'Nothing yet');
  const act = (what: string) => () => done.set(what);
  return (
    <view className="gap-3">
      <Button label="Photo options" width="content" onClick={() => open.set(true)} data-testid="photo-open" />
      <text role="status" className="text-footnote text-muted" data-testid="photo-done">{done.value}</text>
      <ActionSheet isOpen={open.value} onOpenChange={next => open.set(next)} label="This photo"
        items={[
          { label: 'Share', icon: 'share', onClick: act('Shared'), 'data-testid': 'photo-share' },
          { label: 'Duplicate', icon: 'copy', onClick: act('Duplicated'), 'data-testid': 'photo-duplicate' },
          { label: 'Add to album', icon: 'folder', onClick: act('Added to an album'), 'data-testid': 'photo-album' },
          { label: 'Delete', icon: 'delete', variant: 'destructive', onClick: act('Deleted'), 'data-testid': 'photo-delete' },
        ]}
        data-testid="photo"
      />
    </view>
  );
}
```

### A question with its consequence under the title

```tsx
import { useLocal } from '@lingxia/view';
import { Button } from '@liulidesign/core/Button';
import { ActionSheet } from '@liulidesign/core/ActionSheet';

/** A question with its consequence spelled out under the title; the rows are its answers. */
export function WithMessage() {
  const open = useLocal('open', false);
  const done = useLocal('done', 'Draft open');
  return (
    <view className="gap-3">
      <Button label="Close draft" width="content" onClick={() => open.set(true)} data-testid="draft-open" />
      <text role="status" className="text-footnote text-muted" data-testid="draft-done">{done.value}</text>
      <ActionSheet isOpen={open.value} onOpenChange={next => open.set(next)} label="Close this draft?"
        description="Unsaved changes are lost if you discard it."
        items={[
          { label: 'Save draft', onClick: () => done.set('Draft saved'), 'data-testid': 'draft-save' },
          { label: 'Discard', variant: 'destructive', onClick: () => done.set('Draft discarded'), 'data-testid': 'draft-discard' },
        ]}
        data-testid="draft"
      />
    </view>
  );
}
```

### No drawn title: the rows speak for themselves

```tsx
import { useLocal } from '@lingxia/view';
import { Button } from '@liulidesign/core/Button';
import { ActionSheet } from '@liulidesign/core/ActionSheet';

/** No title to draw: the sheet keeps it as its name, and the rows speak for themselves. */
export function Untitled() {
  const open = useLocal('open', false);
  const sort = useLocal('sort', 'Newest first');
  return (
    <view className="gap-3">
      <Button label={sort.value} endIcon="chevron-down" width="content" onClick={() => open.set(true)} data-testid="order-open" />
      <ActionSheet isOpen={open.value} onOpenChange={next => open.set(next)} label="Order" isLabelHidden
        items={['Newest first', 'Oldest first', 'By name'].map(name => ({ label: name, onClick: () => sort.set(name), 'data-testid': `order-${name}` }))}
        data-testid="order"
      />
    </view>
  );
}
```

## Props

| Prop | Type | Default | Description |
| --- | --- | --- | --- |
| `isOpen` (required) | `boolean` |  | Whether it is shown. |
| `onOpenChange` (required) | `(isOpen: boolean) => void` |  | Called with `false` on Cancel, Escape, back, a tap outside or a drag off, and when an action is chosen. |
| `label` (required) | `string` |  | The title, and the sheet's accessible name: what the actions act on ("This photo"). |
| `isLabelHidden` | `boolean` | `false` | Keeps `label` as the name without drawing it. |
| `description` | `string` |  | One or two sentences under the title, read as the sheet's description. |
| `items` (required) | `readonly ActionSheetItem[]` |  | The actions, in order. |
| `cancelLabel` | `string` |  | Names the cancel row. Defaults to "Cancel" in the page's language. |
| `data-testid` | `string` |  |  |

## Anatomy

| Part | Element | Required | Description |
| --- | --- | --- | --- |
| panel | `<dialog>` | yes | The dialog: on the bottom edge on a phone, rounded at the top; in the middle when wider. |
| grabber | `<view>` | no | On a phone, the handle at the top edge; decoration, hidden from assistive tech. |
| header | `<view>` | no | The title (`label`, unless `isLabelHidden`) and `description`, in footnote type above the rows. |
| actions | `<view>` | yes | The rows, in order; each a button with an optional icon. With any icon, every row keeps the icon's space, so the labels line up. |
| cancel | `<view>` | yes | Cancel, under a hairline, apart from the actions. |

## What an app may override

- No class props: it takes its look from its props, the tokens and the roles below.
- The recipe, `actionSheetRecipe` from `@liulidesign/core/ActionSheet/recipe`: the classes of each part, to style an element of your own to match. The component does not read a changed copy.
- Scale tokens it reads, set in the app's stylesheet under `@theme`: `--spacing-control-md`, `--spacing-control-lg`, `--text-footnote`, `--text-body`, `--shadow-modal`, `--radius-control`, `--radius-dialog`.
- Colour roles it paints with, set by the theme pack: `danger`, `line`, `surface-overlay`, `text`, `text-muted`.

## Accessibility

- Pattern: dialog.
- Name: label.
- Description: description.
- States: modal (from isOpen); disabled (from items[].isDisabled).
- Tab, Shift+Tab: Moves between the rows and Cancel; focus stays in the sheet..
- Enter, Space: Chooses the focused row: the sheet closes and its action runs..
- Escape: onOpenChange(false): cancels..
- Android back, the screen reader dismiss gesture, a tap outside and, on a phone, a drag off screen all cancel, like Escape.
- The sheet pads its bottom by the safe area, so Cancel clears the home indicator.

| Contrast | Pair | Minimum | Light | Dark |
| --- | --- | --- | --- | --- |
| row label | `text` on `surface-overlay` | 4.5:1 | 16.26:1 | 10.11:1 |
| destructive row label | `danger` on `surface-overlay` | 4.5:1 | 5.21:1 | 5.43:1 |
| description | `text-muted` on `surface-overlay` | 4.5:1 | 5.89:1 | 4.84:1 |

Ratios are computed from @lingxia/view/theme and rounded down.

## Guidance

- Do: Name each row by what it does: "Delete", "Save draft".
- Do: Put the destructive action last among the actions, in the `destructive` variant.
- Do: Say the consequence in `description` when an action cannot be undone.
- Don't: Use it for a long list or a choice of values; use `Select` or a `BottomSheet` with a list.
- Don't: Put the only way forward in it; an action sheet is always safe to cancel.
- Don't: Repeat Cancel as an item; the sheet has one.

Related: BottomSheet, DropdownMenu, AlertDialog.
