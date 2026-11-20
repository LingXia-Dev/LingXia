<!-- Generated from the doc file by scripts/generate.mjs; do not edit. -->

# Details

A summary row that shows and hides the content under it. Status: preview.

```ts
import { Details, DetailsGroup } from '@liulidesign/core/Details';
```

Details hides secondary content behind a row the person opens when they need it: an answer, advanced settings, the fine print. DetailsGroup stacks rows in one bordered list, and can keep a single row open as an accordion.

## Examples

### One row

```tsx
import { Details } from '@liulidesign/core/Details';

const surface = 'gap-1 rounded-container border border-text/12 bg-surface px-5 pt-4 pb-1';

/** One disclosure under the text it qualifies: the summary opens and closes the content under it. */
export function Single() {
  return (
    <view className={surface}>
      <text className="text-body text-text">Team is €8 a month for each active member.</text>
      <Details label="What counts as an active member?" data-testid="active">
        <text className="text-body text-muted">Anyone who opened a shared notebook in the last 30 days. Guests with view access are free.</text>
      </Details>
    </view>
  );
}
```

### An accordion

```tsx
import { Details, DetailsGroup } from '@liulidesign/core/Details';

/** An accordion: opening one row closes the one that was open. */
export function Accordion() {
  return (
    <DetailsGroup isExclusive defaultValue="refunds" data-testid="faq">
      <Details value="refunds" label="Can I get a refund?" data-testid="refunds">
        <text className="text-body text-muted">Within 14 days of a payment, in full, from Billing.</text>
      </Details>
      <Details value="export" label="How do I export my notes?" data-testid="export">
        <text className="text-body text-muted">Settings, then Export: Markdown files in one archive, with attachments.</text>
      </Details>
      <Details value="delete" label="What happens when I delete my account?" data-testid="delete">
        <text className="text-body text-muted">Your notes are removed from every device after 30 days. Shared notebooks pass to their next member.</text>
      </Details>
    </DetailsGroup>
  );
}
```

### Rows that open on their own

```tsx
import { Details, DetailsGroup } from '@liulidesign/core/Details';

/** Several rows open at once. */
export function Independent() {
  return (
    <DetailsGroup>
      <Details label="Shipping" isDefaultOpen>
        <text className="text-body text-muted">Free over €40. Two to four working days.</text>
      </Details>
      <Details label="Returns" isDefaultOpen>
        <text className="text-body text-muted">30 days, in the original packaging.</text>
      </Details>
    </DetailsGroup>
  );
}
```

### Sections with an icon and a second line

```tsx
import { useLocal } from '@lingxia/view';
import { Details, DetailsGroup } from '@liulidesign/core/Details';

/** Sections of a settings page: an icon and a second line say what is inside; one cannot be opened. */
export function Sections() {
  const open = useLocal<string | null>('open', 'notifications');
  return (
    <DetailsGroup isExclusive label="Settings" value={open.value} onChange={open.set} data-testid="sections">
      <Details value="notifications" label="Notifications" description="Mentions and the weekly digest" startIcon="notifications" data-testid="section-notifications">
        <text className="text-body text-muted">You are told about mentions at once and get a digest on Mondays.</text>
      </Details>
      <Details value="privacy" label="Privacy" description="Who can see your profile" startIcon="lock" data-testid="section-privacy">
        <text className="text-body text-muted">Only members of your workspace can see your profile.</text>
      </Details>
      <Details value="billing" label="Billing" description="Managed by your workspace owner" startIcon="payment" isDisabled data-testid="section-billing">
        <text className="text-body text-muted">Ask the owner for an invoice.</text>
      </Details>
    </DetailsGroup>
  );
}
```

### A field inside a row

```tsx
import { useLocal } from '@lingxia/view';
import { TextInput } from '@liulidesign/core/TextInput';
import { Details } from '@liulidesign/core/Details';

/** A row with a field inside: what is typed is kept while the row is closed, and a closed row's field is out of reach. */
export function WithForm() {
  const name = useLocal('name', '');
  return (
    <Details label="Add a nickname" description={name.value === '' ? 'None yet' : name.value} data-testid="form">
      <TextInput bind={name} label="Nickname" placeholder="Shown beside your name" data-testid="form-name" />
    </Details>
  );
}
```

## Props

| Prop | Type | Default | Description |
| --- | --- | --- | --- |
| `label` (required) | `string` |  | The summary row's text; also what the toggle is named. |
| `description` | `string` |  | A second, quieter line under the label: what is inside. |
| `startIcon` | `"add" \| "add-circle" \| "arrow-down" \| "arrow-end" \| "arrow-start" \| "arrow-up" \| "attach" \| "blocked" \| "bookmark" \| "calendar" \| "camera" \| "cancel" \| "cart" \| "chart" \| "check" \| "checklist" \| "chevron-down" \| "chevron-end" \| "chevron-start" \| "chevron-up" \| "click" \| "clock" \| "close" \| "cloud" \| "collapse" \| "compass" \| "copy" \| "dark" \| "delete" \| "document" \| "download" \| "edit" \| "error" \| "expand" \| "external-link" \| "file" \| "filter" \| "flag" \| "folder" \| "globe" \| "grid" \| "heart" \| "help" \| "hidden" \| "home" \| "image" \| "info" \| "layers" \| "light" \| "like" \| "link" \| "list" \| "location" \| "lock" \| "mail" \| "menu" \| "message" \| "microphone" \| "more" \| "more-vertical" \| "mute" \| "notifications" \| "notifications-off" \| "palette" \| "pause" \| "payment" \| "phone" \| "play" \| "print" \| "redo" \| "refresh" \| "remove" \| "rows" \| "save" \| "search" \| "send" \| "settings" \| "share" \| "sign-in" \| "sign-out" \| "sort" \| "star" \| "stop" \| "success" \| "tag" \| "template" \| "text-input" \| "undo" \| "unlock" \| "upload" \| "user" \| "users" \| "video" \| "visible" \| "volume" \| "warning" \| "wifi"` |  | An icon before the label, by name; not read. |
| `children` (required) | `Child` |  | The body, which exists only while open. |
| `value` | `string` |  | Identifies the row in an exclusive `DetailsGroup`. |
| `isDefaultOpen` | `boolean` | `false` | Open on first render, uncontrolled. |
| `isOpen` | `boolean` |  | Controlled open state; pass `onOpenChange` with it. |
| `onOpenChange` | `((isOpen: boolean) => void)` |  |  |
| `isDisabled` | `boolean` | `false` | Cannot be opened or closed. |
| `className` | `string` |  |  |
| `data-testid` | `string` |  | On the disclosure. |

## Anatomy

| Part | Element | Required | Description |
| --- | --- | --- | --- |
| group | `<view>` | no | DetailsGroup: rows in one bordered list, divided by hairlines. |
| summary | `<button>` | yes | The toggle: a button named by `label` that announces whether it is expanded. |
| icon | `<vector>` | no | `startIcon`, before the label; not read. |
| description | `<text>` | no | A second line under the label, read with it. |
| chevron | `<vector>` | yes | Turns over as the row opens. |
| box | `<view>` | yes | The clip around the body: no height while closed, the body's height while open. |
| body | `<view>` | no | The content; it stays in the page while closed, hidden and out of reach. |

## What an app may override

- `className`: classes merged after the recipe's, so they win.
- The recipe, `detailsRecipe` from `@liulidesign/core/Details/recipe`: the classes of each part, to style an element of your own to match. The component does not read a changed copy.
- Scale tokens it reads, set in the app's stylesheet under `@theme`: `--spacing-control-lg`, `--text-footnote`, `--text-body`, `--radius-control`, `--radius-container`.
- Colour roles it paints with, set by the theme pack: `focus`, `surface`, `surface-overlay`, `text`, `text-muted`.

## Accessibility

- Pattern: button.
- Name: label.
- Description: none.
- States: expanded (from isOpen, isDefaultOpen, or the group's open row).
- Enter, Space: Opens or closes the row..
- Press target: lg 48 px.
- Closed content stays in the page but is hidden and clipped to nothing: focus, a press and a screen reader reach it only once it is open.
- What is typed or chosen inside a row is kept while the row is closed.
- The body grows open and shut; under reduced motion it changes at once. In a group the rows below move with it; what follows a row on its own jumps unless it takes `transition-layout`.
- In an exclusive group, closing the open row leaves none open.

| Contrast | Pair | Minimum | Light | Dark |
| --- | --- | --- | --- | --- |
| summary text | `text` on `surface` | 4.5:1 | 16.26:1 | 13.87:1 |

Ratios are computed from @lingxia/view/theme and rounded down.

## Guidance

- Do: Write the summary as the question or the name of what is inside.
- Do: Use an exclusive group when the rows are alternatives, such as answers in an FAQ.
- Don't: Hide what most people need; show it.
- Don't: Put a form someone must fill in behind a closed row.

## Not yet

| Missing | Needs | Meanwhile |
| --- | --- | --- |
| Liuli on the Details primitive. | a `<Summary>` that can drop or move its start marker, and whose inner row stretches to the button's width | Liuli builds the same button with aria-expanded, so the chevron can sit at the end. |

Related: Item, Card.
