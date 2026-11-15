<!-- Generated from the doc file by scripts/generate.mjs; do not edit. -->

# ButtonGroup

Buttons that belong together, apart or joined edge to edge, as one keyboard stop. Status: preview.

```ts
import { ButtonGroup } from '@liulidesign/core/ButtonGroup';
```

ButtonGroup lays out Buttons and IconButtons that act on the same thing. Apart, they keep a gap and wrap; attached, they share edges and read as one shape. The group gives its buttons a size and a variant, can disable them all, and is a toolbar for assistive tech: one Tab stop, with the arrow keys moving among the buttons.

## Examples

### Actions on one thing

```tsx
import { useLocal } from '@lingxia/view';
import { Button } from '@liulidesign/core/Button';
import { ButtonGroup } from '@liulidesign/core/ButtonGroup';

/** Actions on one thing sit together, the main one last. */
export function Actions() {
  const outcome = useLocal('outcome', 'Draft');
  return (
    <view className="gap-3">
      <ButtonGroup label="Draft actions" data-testid="actions">
        <Button label="Discard" variant="ghost" onClick={() => outcome.set('Discarded')} data-testid="discard" />
        <Button label="Save" onClick={() => outcome.set('Saved')} data-testid="save" />
        <Button label="Publish" variant="primary" onClick={() => outcome.set('Published')} data-testid="publish" />
      </ButtonGroup>
      <text role="status" data-testid="outcome" className="text-footnote text-muted">{outcome.value}</text>
    </view>
  );
}
```

### Attached

```tsx
import { Button } from '@liulidesign/core/Button';
import { ButtonGroup } from '@liulidesign/core/ButtonGroup';

/** Attached, for actions of one kind: the buttons share edges and the group is one shape. */
export function Attached() {
  return (
    <ButtonGroup label="History" isAttached data-testid="history">
      <Button label="Undo" startIcon="undo" data-testid="undo" />
      <Button label="Redo" startIcon="redo" data-testid="redo" />
      <Button label="Reset" data-testid="reset" />
    </ButtonGroup>
  );
}
```

### Attached icon buttons

```tsx
import { IconButton } from '@liulidesign/core/IconButton';
import { ButtonGroup } from '@liulidesign/core/ButtonGroup';

export function IconsAttached() {
  return (
    <ButtonGroup label="Zoom" isAttached variant="secondary" data-testid="zoom">
      <IconButton label="Zoom out" icon="remove" data-testid="zoom-out" />
      <IconButton label="Zoom in" icon="add" />
      <IconButton label="Fit to screen" icon="expand" />
    </ButtonGroup>
  );
}
```

### Sizes

```tsx
import { Button } from '@liulidesign/core/Button';
import { ButtonGroup } from '@liulidesign/core/ButtonGroup';

export function Sizes() {
  return (
    <view className="gap-4">
      <ButtonGroup label="Small" isAttached size="sm" data-testid="small">
        <Button label="Day" data-testid="small-day" />
        <Button label="Week" />
        <Button label="Month" />
      </ButtonGroup>
      <ButtonGroup label="Large" isAttached size="lg">
        <Button label="Day" data-testid="large-day" />
        <Button label="Week" />
        <Button label="Month" />
      </ButtonGroup>
    </view>
  );
}
```

### A toolbar

```tsx
import { Button } from '@liulidesign/core/Button';
import { IconButton } from '@liulidesign/core/IconButton';
import { ButtonGroup } from '@liulidesign/core/ButtonGroup';

/** A toolbar over content: icon buttons in a quiet group, a labelled action apart from it. */
export function Toolbar() {
  return (
    <view className="flex-row items-center justify-between gap-3">
      <ButtonGroup label="Message actions" size="sm" data-testid="message">
        <IconButton label="Reply" icon="message" />
        <IconButton label="Forward" icon="send" />
        <IconButton label="Delete" icon="delete" />
      </ButtonGroup>
      <Button label="Archive" size="sm" startIcon="folder" />
    </view>
  );
}
```

## Props

| Prop | Type | Default | Description |
| --- | --- | --- | --- |
| `label` (required) | `string` |  | What the buttons have in common, read once as the group's name: "Text style". |
| `children` (required) | `Child` |  | Button and IconButton elements. |
| `isAttached` | `boolean` | `false` | Joins the buttons edge to edge, for actions of one kind. Apart, they keep a gap and wrap. |
| `size` | `ButtonSize` |  | The size of every button that sets none. |
| `variant` | `ButtonVariant` |  | The variant of every button that sets none. |
| `isDisabled` | `boolean` | `false` | Disables every button in the group. |
| `className` | `string` |  |  |
| `data-testid` | `string` |  |  |

## Anatomy

| Part | Element | Required | Description |
| --- | --- | --- | --- |
| root | `<view>` | yes | The row, and the toolbar assistive tech names by `label`. In an attached group each Button rounds only the group's two ends. |

## Accessibility

- Pattern: none.
- Name: label.
- Description: none.
- States: disabled (from isDisabled, on every button).
- Tab: Enters the group on one button and leaves it.
- Left, Right: Move focus between the buttons without pressing them.
- Enter or Space: Presses the focused button.
- Role toolbar, named by `label`; each button keeps its own name and state.
- The shared edge of attached secondary buttons is decorative and below 3:1 by design: each button is told apart by its label or icon, not by the line.
- A group is not a choice among values: nothing in it stays selected. For that use SegmentedControl or a ToggleButton.

| Contrast | Pair | Minimum | Light | Dark |
| --- | --- | --- | --- | --- |

Ratios are computed from @lingxia/view/theme and rounded down.

## Guidance

- Do: Group actions on the same object, with the main action last.
- Do: Attach buttons of one kind and equal weight: undo and redo, zoom out and in.
- Do: Name the group for what its buttons share: "Text style", "Message actions".
- Do: Set `size` and `variant` on the group, and on a button only to make one stand out.
- Don't: Attach a primary button to others; let it stand apart.
- Don't: Use a group to choose a value or a view; that is a SegmentedControl or Tabs.
- Don't: Put more buttons in an attached group than fit one line; attached groups do not wrap.

## Not yet

| Missing | Needs | Meanwhile |
| --- | --- | --- |
| An attached group that wraps or scrolls when it does not fit. | reading a laid-out box's size (`useBox`) to fold buttons into a menu | An attached group stays on one line and can be clipped; keep it short. |
| A vertical group. | nothing from the View | Rows only. |

Related: Button, IconButton, SegmentedControl.
