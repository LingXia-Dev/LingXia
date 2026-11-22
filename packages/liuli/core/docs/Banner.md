<!-- Generated from the doc file by scripts/generate.mjs; do not edit. -->

# Banner

A message in the page about the page or the account, that stays until resolved. Status: preview.

```ts
import { Banner } from '@liulidesign/core/Banner';
```

Banner tells people about something that affects what they are looking at: sync stopped, a card expiring, a notebook locked. It sits in the page's flow, above what it is about, with at most one action and an optional dismiss button. It is not a toast: it does not time out, and the app removes it when the condition ends or it is dismissed.

## Examples

### A problem with one action, dismissible

```tsx
import { useLocal } from '@lingxia/view';
import { Banner } from '@liulidesign/core/Banner';

/** A problem with one way out, that the person can also put aside. */
export function Payment() {
  const shown = useLocal(true);
  const updated = useLocal(false);
  if (!shown.value) return <text className="text-footnote text-muted">Dismissed. It comes back on the next visit.</text>;
  return (
    <Banner
      tone="warning"
      title="Your card expires this month"
      message="Update it before 30 June to keep syncing across devices."
      action={{ label: updated.value ? 'Card updated' : 'Update card', onClick: () => updated.set(true) }}
      onDismiss={() => shown.set(false)}
      dismissLabel="Dismiss"
      data-testid="payment"
    />
  );
}
```

### Tones

```tsx
import { Banner } from '@liulidesign/core/Banner';

export function Tones() {
  return (
    <view className="gap-3">
      <Banner tone="neutral" title="Read only" message="An admin locked this notebook." />
      <Banner tone="info" title="Sharing is changing" message="From July, shared notebooks need a Team plan." />
      <Banner tone="success" title="All notes synced" />
      <Banner tone="warning" title="Storage almost full" message="You have used 9.6 of 10 GB." />
      <Banner tone="danger" title="Sync stopped" message="We could not reach the server. Your changes are kept on this device." />
    </view>
  );
}
```

### Above the list it is about

```tsx
import { useLocal } from '@lingxia/view';
import { Banner } from '@liulidesign/core/Banner';

/** At the top of a list it is about, before the content. */
export function OverList() {
  const retried = useLocal(false);
  return (
    <view className="gap-3">
      <Banner
        tone="danger"
        title={retried.value ? 'Still offline' : 'Sync stopped'}
        message="Changes are kept on this device until the connection returns."
        action={{ label: 'Retry now', onClick: () => retried.set(true) }}
        data-testid="sync"
      />
      <view className="overflow-hidden rounded-container border border-text/12 bg-surface divide-y divide-text/8">
        {['Q3 plan', 'Reading list', 'Interview notes'].map(note => (
          <view key={note} className="px-4 py-3"><text className="text-body text-text">{note}</text></view>
        ))}
      </view>
    </view>
  );
}
```

### Folded to its title

```tsx
import { Said } from '../example';
import { Banner } from '@liulidesign/core/Banner';

/** Folded to its title until someone wants the rest: for a notice that stays for days. */
export function Collapsible() {
  return (
    <Said>
      {say => (
        <Banner
          tone="warning"
          title="Scheduled maintenance on Sunday"
          message="Sync pauses from 02:00 to 04:00 UTC. Notes you write then are kept on this device and sent when it ends."
          action={{ label: 'See the schedule', onClick: () => say('Opened the schedule') }}
          isCollapsible
          toggleLabel="Details"
          isDefaultOpen={false}
          data-testid="maintenance"
        />
      )}
    </Said>
  );
}
```

## Props

| Prop | Type | Default | Description |
| --- | --- | --- | --- |
| `onDismiss` | `(() => void)` |  | Removes the banner; the app stops rendering it. |
| `dismissLabel` | `string` |  | The dismiss button's name: "Dismiss". Defaults to "Dismiss" in the page's language. |
| `isCollapsible` | `boolean` | `false` | Folds the message and the action under the title, behind a chevron button. |
| `toggleLabel` | `string` |  | The chevron button's name: "Details". Read with whether it is open. Defaults to "Details" in the page's language. |
| `isDefaultOpen` | `boolean` | `true` | Whether it starts unfolded. |
| `title` (required) | `string` |  | The point, in a few words. |
| `message` | `string` |  | What it means or what to do, in a sentence. |
| `tone` | `BannerTone` | `'info'` |  |
| `action` | `{ label: string; onClick: () => void; }` |  | One action that resolves it: "Update card", "Retry". |
| `className` | `string` |  |  |
| `data-testid` | `string` |  |  |

## Anatomy

| Part | Element | Required | Description |
| --- | --- | --- | --- |
| root | `<view>` | yes | The tinted box, in the tone's soft fill with a hairline of the tone. |
| icon | `<vector>` | yes | The tone's glyph, hidden from assistive tech; the tone is never carried by colour alone. |
| text | `<view>` | yes | The column holding title, message and action. |
| title | `<text>` | yes | The point, in a few words. |
| message | `<text>` | no | What it means or what to do, in a sentence. |
| action | `<Button>` | no | One link button that resolves it. |
| dismiss | `<button>` | no | A close button named by `dismissLabel`. |

## What an app may override

- `className`: classes merged after the recipe's, so they win.
- The recipe, `bannerRecipe` from `@liulidesign/core/Banner/recipe`: the classes of each part, to style an element of your own to match. The component does not read a changed copy.
- Scale tokens it reads, set in the app's stylesheet under `@theme`: `--text-footnote`, `--text-body`, `--radius-container`.
- Colour roles it paints with, set by the theme pack: `danger`, `danger-soft`, `focus`, `info`, `info-soft`, `success`, `success-soft`, `text`, `text-muted`, `warning`, `warning-soft`.

## Accessibility

- Pattern: none.
- Name: none; its content is read.
- Description: none.
- Role alert for danger, so it is announced when it appears; role status for the other tones, announced politely.
- Render it only when the condition holds; announcing needs it to appear, not to be there from the start.
- The text stays in the text role on every tint; the icon carries the tone.

| Contrast | Pair | Minimum | Light | Dark |
| --- | --- | --- | --- | --- |
| title and message on neutral | `text` on `text/6` over `canvas` | 4.5:1 | 13.54:1 | 13.46:1 |
| neutral icon | `text-muted` on `text/6` over `canvas` | 3:1 | 4.90:1 | 6.45:1 |
| title and message on info | `text` on `info-soft` | 4.5:1 | 14.25:1 | 11.26:1 |
| title and message on success | `text` on `success-soft` | 4.5:1 | 14.33:1 | 11.76:1 |
| title and message on warning | `text` on `warning-soft` | 4.5:1 | 14.59:1 | 11.36:1 |
| title and message on danger | `text` on `danger-soft` | 4.5:1 | 14.12:1 | 12.22:1 |
| action on neutral | `accent` on `text/6` over `canvas` | 4.5:1 | 4.68:1 | 6.01:1 |
| action on info | `accent` on `info-soft` | 4.5:1 | 4.93:1 | 5.02:1 |
| action on success | `accent` on `success-soft` | 4.5:1 | 4.95:1 | 5.25:1 |
| action on warning | `accent` on `warning-soft` | 4.5:1 | 5.05:1 | 5.07:1 |
| action on danger | `accent` on `danger-soft` | 4.5:1 | 4.88:1 | 5.46:1 |

Ratios are computed from @lingxia/view/theme and rounded down.

## Guidance

- Do: Put it above the content it is about, and say what to do in `message` or `action`.
- Do: Use neutral for a plain notice, and danger only when something has stopped working.
- Do: Let people dismiss a banner that is news; keep one that blocks work until it is resolved.
- Don't: Stack several banners; show the most urgent one.
- Don't: Use a banner to confirm a small action that just worked; the control itself can show that.
- Don't: Give it more than one action.

Related: Badge, Dialog, ProgressBar.
