<!-- Generated from the doc file by scripts/generate.mjs; do not edit. -->

# BottomSheet

A modal panel on the bottom edge that drags between detents and off to close. Status: preview.

```ts
import { BottomSheet } from '@liulidesign/core/BottomSheet';
```

BottomSheet holds a short task or a choice that belongs to the page under it: where to move a note, the comments on it. It is a dialog on the bottom edge, in reach of a thumb. A `fit` sheet is as tall as its content; an `expandable` one opens halfway and drags up to nearly the full height. Dragging it off screen closes it.

## Examples

### A choice, as tall as its list

```tsx
import { unused } from '../example';
import { useLocal } from '@lingxia/view';
import { Button } from '@liulidesign/core/Button';
import { IconButton } from '@liulidesign/core/IconButton';
import { Item, ItemList } from '@liulidesign/core/Item';
import { BottomSheet } from '@liulidesign/core/BottomSheet';

const moveTargets = ['Inbox', 'Travel', 'Work', 'Recipes'];

/** Choosing where a note goes: a sheet as tall as its list, dragged down or tapped outside to cancel. */
export function MoveTo() {
  const open = useLocal('open', false);
  const folder = useLocal('folder', 'Inbox');
  return (
    <view className="gap-3">
      <Button label="Move note" width="content" onClick={() => open.set(true)} data-testid="move-open" />
      <text className="text-footnote text-muted" data-testid="folder">{`In ${folder.value}`}</text>
      <BottomSheet isOpen={open.value} onOpenChange={next => open.set(next)} label="Move to"
        headerAction={<IconButton icon="add" label="New folder" size="sm" onClick={unused} data-testid="move-new-folder" />}
        data-testid="move"
      >
        <ItemList label="Folders">
          {moveTargets.map(name => (
            <Item key={name} label={name} onClick={() => { folder.set(name); open.set(false); }} data-testid={`folder-${name}`} />
          ))}
        </ItemList>
      </BottomSheet>
    </view>
  );
}
```

### A long list, opening halfway

```tsx
import { useLocal } from '@lingxia/view';
import { Button } from '@liulidesign/core/Button';
import { BottomSheet } from '@liulidesign/core/BottomSheet';

const comments = [
  ['Ana', 'Can we move the review to Thursday?'],
  ['Kai', 'Thursday works. I will update the plan.'],
  ['Mira', 'Added the budget numbers under Q3.'],
  ['Ana', 'Thanks. The hiring section still needs dates.'],
  ['Kai', 'I will add them after the sync.'],
  ['Mira', 'Linked the research notes as well.'],
] as const;

/** A long list: the sheet opens halfway and drags up to nearly the full height. */
export function Comments() {
  const open = useLocal('open', false);
  return (
    <view className="gap-3">
      <Button label="Show comments" width="content" onClick={() => open.set(true)} data-testid="comments-open" />
      <BottomSheet
        isOpen={open.value}
        onOpenChange={next => open.set(next)}
        label="Comments"
        hasCloseButton
        detents="expandable"
        data-testid="comments"
      >
        {comments.map(([who, text], index) => (
          <view key={`c${index}`} className="gap-0.5" data-testid={`comment-${index}`}>
            <text className="text-footnote font-semibold text-text">{who}</text>
            <text className="text-body text-text">{text}</text>
          </view>
        ))}
      </BottomSheet>
    </view>
  );
}
```

### A question that must be answered

```tsx
import { useLocal } from '@lingxia/view';
import { Button } from '@liulidesign/core/Button';
import { BottomSheet } from '@liulidesign/core/BottomSheet';

/** A step that must be answered: no drag off, no tap outside. */
export function Required() {
  const open = useLocal('open', false);
  const answer = useLocal('answer', '');
  return (
    <view className="gap-3">
      <Button label="Leave notebook" variant="destructive" width="content" onClick={() => open.set(true)} data-testid="leave-open" />
      {answer.value !== '' && <text className="text-footnote text-muted" data-testid="answer">{answer.value}</text>}
      <BottomSheet
        isOpen={open.value}
        onOpenChange={next => open.set(next)}
        label="Leave this notebook?"
        description="You lose access to its notes until someone invites you again."
        dismissOn="none"
        data-testid="leave"
        actions={
          <>
            <Button label="Stay" onClick={() => { answer.set('Stayed'); open.set(false); }} data-testid="stay" />
            <Button label="Leave" variant="destructive" onClick={() => { answer.set('Left'); open.set(false); }} data-testid="leave-confirm" />
          </>
        }
      />
    </view>
  );
}
```

### A form that scrolls between the title and the actions

```tsx
import { useLocal } from '@lingxia/view';
import { Button } from '@liulidesign/core/Button';
import { TextInput } from '@liulidesign/core/TextInput';
import { BottomSheet } from '@liulidesign/core/BottomSheet';

/** A form taller than the sheet: the fields scroll between the title and the actions, which stay put. */
export function Form() {
  const open = useLocal('open', false);
  const name = useLocal('name', 'Game server');
  const external = useLocal('external', '25565');
  const address = useLocal('address', '192.168.1.24');
  const internal = useLocal('internal', '25565');
  const note = useLocal('note', '');
  const allowed = useLocal('allowed', '');
  const protocol = useLocal('protocol', 'TCP');
  return (
    <view className="gap-3">
      <Button label="New port rule" width="content" onClick={() => open.set(true)} data-testid="rule-open" />
      <BottomSheet
        isOpen={open.value}
        onOpenChange={next => open.set(next)}
        label="New port rule"
        hasCloseButton
        actions={
          <>
            <Button label="Cancel" onClick={() => open.set(false)} data-testid="rule-cancel" />
            <Button label="Save" variant="primary" onClick={() => open.set(false)} data-testid="rule-save" />
          </>
        }
        data-testid="rule"
      >
        <TextInput bind={name} label="Name" data-testid="rule-name" />
        <TextInput bind={protocol} label="Protocol" description="TCP, UDP or both." autoCapitalize="characters" data-testid="rule-protocol" />
        <TextInput bind={external} label="External port" inputMode="numeric" data-testid="rule-external" />
        <TextInput bind={address} label="Device address" inputMode="decimal" description="The device on your network that takes the traffic." data-testid="rule-address" />
        <TextInput bind={internal} label="Device port" inputMode="numeric" data-testid="rule-internal" />
        <TextInput bind={allowed} label="Allowed from" placeholder="Any address" isRequired={false} description="Leave empty to accept traffic from anywhere." data-testid="rule-allowed" />
        <TextInput bind={note} label="Note" placeholder="What it is for" data-testid="rule-note" />
      </BottomSheet>
    </view>
  );
}
```

## Props

| Prop | Type | Default | Description |
| --- | --- | --- | --- |
| `isOpen` (required) | `boolean` |  | Whether it is shown. A closed sheet renders nothing, so local state inside starts over. |
| `onOpenChange` (required) | `(isOpen: boolean) => void` |  | Called with `false` on a close request: Escape, back, the backdrop or a drag off screen per `dismissOn`, the close button. |
| `label` (required) | `string` |  | The title, and the sheet's accessible name. |
| `isLabelHidden` | `boolean` | `false` | Keeps `label` as the name without drawing it as a title. |
| `description` | `string` |  | One or two sentences under the title, read as the sheet's description. |
| `children` | `Child` |  | The body; it scrolls when the sheet reaches its height. |
| `headerAction` | `Child` |  | Beside the title, before the close button: one small control that acts on the whole sheet, such as Refresh. |
| `actions` | `Child` |  |  |
| `hasCloseButton` | `boolean` | `false` | Shows a close button. Without it Escape, a click or tap outside, Back or a drag closes the sheet, so a mouse and keyboard on a wide window still have a way out. |
| `closeLabel` | `string` |  | Names the close button. Defaults to "Close" in the page's language. |
| `dismissOn` | `DialogDismissOn` | `'any'` | Which requests close it; with `none` it cannot be dragged off either. |
| `detents` | `BottomSheetDetents` | `'fit'` |  |
| `initialDetent` | `"full" \| "half"` | `'half'` | Where an `expandable` sheet opens: `full` when its working field sits at its bottom (a composer, a form), which `half` would hide. |
| `className` | `string` |  | Classes for the panel. |
| `data-testid` | `string` |  |  |

## Anatomy

| Part | Element | Required | Description |
| --- | --- | --- | --- |
| panel | `<dialog>` | yes | The dialog on the bottom edge, rounded at the top; it follows the finger between its detents. |
| grabber | `<view>` | yes | The handle at the top edge; decoration, hidden from assistive tech. |
| header | `<view>` | no | The title, and the close button with `hasCloseButton`. |
| description | `<text>` | no | One or two sentences under the title, read as the description. |
| body | `<scroll>` | no | `children`, scrolling when the sheet reaches its height; a drag in it moves the sheet first. A hairline shows at an edge while content is scrolled under it. |
| headerAction | `<view>` | no | `headerAction`: one small control beside the title that acts on the whole panel, such as Refresh. |
| footer | `<view>` | no | `actions`: on a phone they share the width equally, the main one last; wider, they keep their own widths at the end, wrapping when narrow. |

## What an app may override

- `className`: Classes for the panel.
- The recipe, `bottomSheetRecipe` from `@liulidesign/core/BottomSheet/recipe`: the classes of each part, to style an element of your own to match. The component does not read a changed copy.
- Scale tokens it reads, set in the app's stylesheet under `@theme`: `--text-body`, `--text-title`, `--shadow-modal`, `--radius-dialog`.
- Colour roles it paints with, set by the theme pack: `focus`, `surface-overlay`, `text`, `text-muted`.

## Accessibility

- Pattern: dialog.
- Name: label.
- Description: description.
- States: modal (from isOpen).
- Escape: onOpenChange(false), unless dismissOn is none.
- Up, Down: With the sheet focused, the next detent up or down..
- Screen readers announce the sheet as adjustable: increment and decrement step it between detents.
- Android back, the screen reader dismiss gesture and a drag off screen are close requests, like Escape.
- Without a close button, a mouse closes it by clicking outside and a keyboard by Escape; with `dismissOn` other than `any` and no `actions`, set `hasCloseButton`.
- The sheet pads its bottom by the safe area, so its last control clears the home indicator.

| Contrast | Pair | Minimum | Light | Dark |
| --- | --- | --- | --- | --- |
| title | `text` on `surface-overlay` | 4.5:1 | 16.26:1 | 10.11:1 |
| description | `text-muted` on `surface-overlay` | 4.5:1 | 5.89:1 | 4.84:1 |

Ratios are computed from @lingxia/view/theme and rounded down.

## Guidance

- Do: Use it for a choice or a short task about what is under it, on touch layouts.
- Do: Use `expandable` for a list that can grow long; `fit` for a few rows or a question.
- Do: Open an `expandable` sheet with `initialDetent="full"` when its working field sits at its bottom, a composer or a form: the half detent would hide it.
- Do: Keep the main action in reach at the bottom.
- Don't: Open a sheet from a sheet; finish or close the first.
- Don't: Use `dismissOn="none"` unless the step must be answered.
- Don't: Put a whole page in a sheet; navigate to the page.

Related: Dialog, DropdownMenu.
