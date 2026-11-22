<!-- Generated from the doc file by scripts/generate.mjs; do not edit. -->

# Token

One chosen thing as a pill. Status: preview.

```ts
import { Token, TokenList } from '@liulidesign/core/Token';
```

Token stands for one thing someone chose or typed: a recipient, a tag, a filter. It can carry a small button that takes it away, and its label can be a button of its own that opens the thing or toggles a filter. TokenList lays tokens out in a row that wraps and names them as a group. MultiSelect uses both for its chosen options.

## Examples

### People, each removable

```tsx
import { useLocal } from '@lingxia/view';
import { Token, TokenList } from '@liulidesign/core/Token';

/** People a note is shared with: each can be taken away. */
export function Recipients() {
  const people = useLocal<readonly string[]>('people', ['Ana Lima', 'Kai Chen', 'Mira Patel']);
  return (
    <view className="gap-2">
      <TokenList label="Shared with" data-testid="shared">
        {people.value.map(name => (
          <Token key={name} label={name} startIcon="user" removeLabel={`Remove ${name}`} onRemove={() => people.set(people.value.filter(each => each !== name))} data-testid={`person-${name.split(' ')[0].toLowerCase()}`} />
        ))}
      </TokenList>
      <text role="status" className="text-footnote text-muted">{`${people.value.length} people`}</text>
    </view>
  );
}
```

### Filters that toggle

```tsx
import { useLocal } from '@lingxia/view';
import { Token, TokenList } from '@liulidesign/core/Token';

/** Filters that toggle: the ones applied are in the accent's soft step and say they are pressed. */
export function Filters() {
  const on = useLocal<readonly string[]>('on', ['Unread']);
  const toggle = (name: string) => on.set(on.value.includes(name) ? on.value.filter(each => each !== name) : [...on.value, name]);
  return (
    <TokenList label="Filters">
      {['Unread', 'Shared', 'With photos', 'Archived'].map(name => (
        <Token key={name} label={name} isSelected={on.value.includes(name)} onClick={() => toggle(name)} data-testid={`filter-${name.toLowerCase().replace(' ', '-')}`} />
      ))}
    </TokenList>
  );
}
```

### Labels, each in its hue

```tsx
import { useLocal } from '@lingxia/view';
import { Token, TokenList } from '@liulidesign/core/Token';

/** Labels keep their hue wherever they appear; removing one takes it off the issue. */
export function Labels() {
  const labels = useLocal('labels', [
    { name: 'bug', hue: 5 as const },
    { name: 'design', hue: 6 as const },
    { name: 'needs triage', hue: 4 as const },
    { name: 'mobile', hue: 2 as const },
  ]);
  return (
    <TokenList label="Labels" data-testid="labels">
      {labels.value.map(each => (
        <Token
          key={each.name}
          label={each.name}
          hue={each.hue}
          size="sm"
          onRemove={() => labels.set(labels.value.filter(other => other.name !== each.name))}
          removeLabel={`Remove ${each.name}`}
          data-testid={`label-${each.name.replace(/ /g, '-')}`}
        />
      ))}
    </TokenList>
  );
}
```

### Sizes

```tsx
import { Held } from '../example';
import { Token } from '@liulidesign/core/Token';

export function Sizes() {
  return (
    <Held value={['sm', 'md'] as readonly string[]}>
      {(left, set) => (
        <view className="flex-row flex-wrap items-center gap-2">
          <Token label="Design" size="sm" />
          <Token label="Design" />
          {left.includes('sm') && <Token label="Design" size="sm" removeLabel="Remove Design" onRemove={() => set(left.filter(size => size !== 'sm'))} />}
          {left.includes('md') && <Token label="Design" removeLabel="Remove Design" onRemove={() => set(left.filter(size => size !== 'md'))} />}
        </view>
      )}
    </Held>
  );
}
```

## Props

| Prop | Type | Default | Description |
| --- | --- | --- | --- |
| `onRemove` | `(() => void)` |  | Takes the token away; the app stops rendering it. |
| `removeLabel` | `string` |  | The remove button's name, with what it removes: "Remove Ana Lima". Defaults to "Remove <label>" in the page's language. |
| `label` (required) | `string` |  | What the token stands for: a person, a tag, a filter. |
| `startIcon` | `"add" \| "add-circle" \| "arrow-down" \| "arrow-end" \| "arrow-start" \| "arrow-up" \| "attach" \| "blocked" \| "bookmark" \| "calendar" \| "camera" \| "cancel" \| "cart" \| "chart" \| "check" \| "checklist" \| "chevron-down" \| "chevron-end" \| "chevron-start" \| "chevron-up" \| "click" \| "clock" \| "close" \| "cloud" \| "collapse" \| "compass" \| "copy" \| "dark" \| "delete" \| "document" \| "download" \| "edit" \| "error" \| "expand" \| "external-link" \| "file" \| "filter" \| "flag" \| "folder" \| "globe" \| "grid" \| "heart" \| "help" \| "hidden" \| "home" \| "image" \| "info" \| "layers" \| "light" \| "like" \| "link" \| "list" \| "location" \| "lock" \| "mail" \| "menu" \| "message" \| "microphone" \| "more" \| "more-vertical" \| "mute" \| "notifications" \| "notifications-off" \| "palette" \| "pause" \| "payment" \| "phone" \| "play" \| "print" \| "redo" \| "refresh" \| "remove" \| "rows" \| "save" \| "search" \| "send" \| "settings" \| "share" \| "sign-in" \| "sign-out" \| "sort" \| "star" \| "stop" \| "success" \| "tag" \| "template" \| "text-input" \| "undo" \| "unlock" \| "upload" \| "user" \| "users" \| "video" \| "visible" \| "volume" \| "warning" \| "wifi"` |  | An icon before the label, by name. |
| `hue` | `Hue` |  | A category's colour, one of the theme's eight hues: the pill takes the hue's soft fill, with a dot (or the icon) in the hue. |
| `onClick` | `(() => void)` |  | Makes the label a button: opens the thing, or toggles a filter. |
| `isSelected` | `boolean` | `false` | Marks a pressable token as on, for a filter that is applied. |
| `size` | `TokenSize` | `'md'` |  |
| `isDisabled` | `boolean` | `false` |  |
| `className` | `string` |  |  |
| `data-testid` | `string` |  | On the token; the remove button gets `<testid>-remove`. |

## Anatomy

| Part | Element | Required | Description |
| --- | --- | --- | --- |
| root | `<view>` | yes | The pill, on a tint of the text colour. |
| icon | `<vector>` | no | An icon before the label; not read. |
| dot | `<view>` | no | With `hue` and no icon: a dot in the hue; not read. |
| label | `<text>` | yes | The name, cut short with an ellipsis when the row is narrow. |
| remove | `<button>` | no | A round button at the end, named by `removeLabel`. |
| list | `<view>` | no | TokenList: a wrapping row, a group named by `label`. |

## What an app may override

- `className`: classes merged after the recipe's, so they win.
- The recipe, `tokenRecipe` from `@liulidesign/core/Token/recipe`: the classes of each part, to style an element of your own to match. The component does not read a changed copy.
- Scale tokens it reads, set in the app's stylesheet under `@theme`: `--text-caption`, `--text-footnote`.
- Colour roles it paints with, set by the theme pack: `accent-soft`, `focus`, `hue-1`, `hue-1-soft`, `hue-2`, `hue-2-soft`, `hue-3`, `hue-3-soft`, `hue-4`, `hue-4-soft`, `hue-5`, `hue-5-soft`, `hue-6`, `hue-6-soft`, `hue-7`, `hue-7-soft`, `hue-8`, `hue-8-soft`, `on-accent-soft`, `text`, `text-muted`.

## Accessibility

- Pattern: none.
- Name: label.
- Description: none.
- States: pressed (from isSelected, on a pressable token); disabled (from isDisabled).
- Tab: To a pressable token, and to each remove button.
- Enter or Space: Presses the token, or removes it.
- Press target: sm 24 px, md 32 px.
- A plain token is read as text. A pressable one is a button; as a filter it reports pressed.
- The remove button is named with what it removes: "Remove Ana Lima".
- A token that opens and can be removed is two buttons side by side.
- The remove button is 24 or 32 px; a finger press that misses goes to the nearest one within reach.

| Contrast | Pair | Minimum | Light | Dark |
| --- | --- | --- | --- | --- |
| label on the pill | `text` on `text/8` over `surface` | 4.5:1 | 13.91:1 | 11.29:1 |
| label, selected | `on-accent-soft` on `accent-soft` | 4.5:1 | 8.54:1 | 9.49:1 |
| remove icon | `text-muted` on `text/8` over `surface` | 3:1 | 5.04:1 | 5.41:1 |
| hue 1, label on its fill | `text` on `hue-1-soft` | 4.5:1 | 14.25:1 | 11.24:1 |
| hue 2, label on its fill | `text` on `hue-2-soft` | 4.5:1 | 14.42:1 | 11.00:1 |
| hue 3, label on its fill | `text` on `hue-3-soft` | 4.5:1 | 14.23:1 | 11.31:1 |
| hue 4, label on its fill | `text` on `hue-4-soft` | 4.5:1 | 14.34:1 | 11.06:1 |
| hue 5, label on its fill | `text` on `hue-5-soft` | 4.5:1 | 14.16:1 | 11.41:1 |
| hue 6, label on its fill | `text` on `hue-6-soft` | 4.5:1 | 14.17:1 | 11.31:1 |
| hue 7, label on its fill | `text` on `hue-7-soft` | 4.5:1 | 14.32:1 | 11.05:1 |
| hue 8, label on its fill | `text` on `hue-8-soft` | 4.5:1 | 14.22:1 | 11.20:1 |
| hue 1, the dot on its fill | `hue-1` on `hue-1-soft` | 3:1 | 3.26:1 | 8.07:1 |
| hue 2, the dot on its fill | `hue-2` on `hue-2-soft` | 3:1 | 4.65:1 | 9.71:1 |
| hue 3, the dot on its fill | `hue-3` on `hue-3-soft` | 3:1 | 6.72:1 | 4.32:1 |
| hue 4, the dot on its fill | `hue-4` on `hue-4-soft` | 3:1 | 4.29:1 | 8.37:1 |
| hue 5, the dot on its fill | `hue-5` on `hue-5-soft` | 3:1 | 7.22:1 | 6.73:1 |
| hue 6, the dot on its fill | `hue-6` on `hue-6-soft` | 3:1 | 5.60:1 | 4.20:1 |
| hue 7, the dot on its fill | `hue-7` on `hue-7-soft` | 3:1 | 6.47:1 | 5.92:1 |
| hue 8, the dot on its fill | `hue-8` on `hue-8-soft` | 3:1 | 3.25:1 | 4.40:1 |

Ratios are computed from @lingxia/view/theme and rounded down.

## Guidance

- Do: Use tokens for things people chose or made: recipients, tags, applied filters.
- Do: Say what is removed in `removeLabel`.
- Don't: Use a token for a status; that is a Badge.
- Don't: Use one for an action; that is a Button.

## Limits

- Tokens are not reordered by dragging.
- Backspace in a field does not remove the last token; MultiSelect removes by each token's button.

Related: MultiSelect, Badge, ToggleButton.
