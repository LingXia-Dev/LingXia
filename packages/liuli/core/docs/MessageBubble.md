<!-- Generated from the doc file by scripts/generate.mjs; do not edit. -->

# MessageBubble

One message of a conversation. Status: preview.

```ts
import { MessageBubble } from '@liulidesign/core/MessageBubble';
```

MessageBubble draws one message: the user's own at the end edge in the accent's soft step, anyone else's at the start on a quiet tint, with a name and a face over the first of a run. Under it goes a line the app writes, the time or how the message stands, and for one that failed a way to send it again. A conversation is the View's list of these with a Composer under it; the Conversation pattern shows them together.

## Examples

### A short exchange

```tsx
import { Avatar } from '@liulidesign/core/Avatar';
import { MessageBubble } from '@liulidesign/core/MessageBubble';

/** A short exchange: theirs at the start with a name and a face, mine at the end; a run shares its inner corners. */
export function Exchange() {
  return (
    <view className="gap-1">
      <MessageBubble author="Kai Chen" avatar={<Avatar name="Kai Chen" size="sm" isDecorative />} text="Are we still on for the coast on Saturday?" isJoinedAfter data-testid="theirs" />
      <MessageBubble hasAvatarSpace text="I can drive if you bring the tent." isJoinedBefore meta="09:41" />
      <view className="h-2" />
      <MessageBubble side="end" text="Yes! Leaving at eight." isJoinedAfter data-testid="mine" />
      <MessageBubble side="end" text="The directions are at https://example.com/maps/place/a-very-long-address-that-does-not-fit-on-one-line" isJoinedBefore meta="09:42" data-testid="long" />
    </view>
  );
}
```

### Sending, sent and not sent

```tsx
import { useLocal } from '@lingxia/view';
import { Button } from '@liulidesign/core/Button';
import { MessageBubble } from '@liulidesign/core/MessageBubble';

/** How a message of mine stands: on its way, sent, and not sent, with a way to try again. */
export function States() {
  const tries = useLocal('tries', 0);
  return (
    <view className="gap-3">
      <MessageBubble side="end" text="On my way." status="sending" meta="Sending" data-testid="sending" />
      <MessageBubble side="end" text="Parked by the gate." status="sent" meta="Sent · 10:02" />
      <MessageBubble
        side="end"
        text="Can you see the car?"
        status="failed"
        meta={tries.value === 0 ? 'Not sent' : `Not sent, tried ${tries.value + 1} times`}
        action={<Button label="Try again" variant="link" size="sm" onClick={() => tries.set(tries.value + 1)} data-testid="retry" />}
        data-testid="failed"
      />
    </view>
  );
}
```

## Props

| Prop | Type | Default | Description |
| --- | --- | --- | --- |
| `text` | `string` |  | The message. Long words and addresses break to fit. |
| `children` | `Child` |  | In place of `text`: a Markdown, a Thumbnail, a file row. |
| `side` | `MessageSide` | `'start'` |  |
| `author` | `string` |  | Who wrote it, shown over the first bubble of their run. |
| `avatar` | `Child` |  | Their Avatar, at the bubble\'s start. For the other bubbles of a run pass `hasAvatarSpace` to keep the edge. |
| `hasAvatarSpace` | `boolean` | `false` | Keeps the avatar\'s width empty. |
| `meta` | `string` |  | A line under the bubble, as the app writes it: the time, "Sending", "Not sent". |
| `status` | `MessageStatus` |  | `failed` adds an error mark to `meta` and an edge to the bubble; say what failed in `meta`. |
| `action` | `Child` |  | Beside `meta`, for a failed message: a Button that sends it again. |
| `isJoinedBefore` | `boolean` | `false` | Whether the same person\'s bubble comes right before this one: tightens the corner toward it. |
| `isJoinedAfter` | `boolean` | `false` | Whether the same person\'s bubble comes right after. |
| `className` | `string` |  |  |
| `data-testid` | `string` |  |  |

## Anatomy

| Part | Element | Required | Description |
| --- | --- | --- | --- |
| root | `<view>` | yes | The row: it puts the bubble at its side. |
| avatar | `<view>` | no | The writer's Avatar, or its width kept empty down a run. |
| author | `<text>` | no | Who wrote it, over the first bubble of a run. |
| bubble | `<view>` | yes | The message: up to four fifths of the row, 520 px at most. |
| meta | `<view>` | no | The line under the bubble: time, state, and the retry action. |

## What an app may override

- `className`: classes merged after the recipe's, so they win.
- The recipe, `messageBubbleRecipe` from `@liulidesign/core/MessageBubble/recipe`: the classes of each part, to style an element of your own to match. The component does not read a changed copy.
- Scale tokens it reads, set in the app's stylesheet under `@theme`: `--text-caption`, `--text-body`.
- Colour roles it paints with, set by the theme pack: `accent-soft`, `danger`, `on-accent-soft`, `text`, `text-muted`.

## Accessibility

- Pattern: none.
- Name: none; its content is read.
- Description: none.
- Press target: .
- It is read in order: the name, the message, then the line under it. Put the list in the View's list so a screen reader steps message by message.
- A failed message says so in words and with an error mark, not only with its red edge.
- The side tells whose it is to the eye; say it in `author` for the ear, or name the list by who is speaking.

| Contrast | Pair | Minimum | Light | Dark |
| --- | --- | --- | --- | --- |
| my message | `on-accent-soft` on `accent-soft` | 4.5:1 | 8.44:1 | 9.49:1 |
| their message | `text` on `text/6` over `canvas` | 4.5:1 | 13.54:1 | 13.46:1 |
| meta | `text-muted` on `canvas` | 4.5:1 | 5.50:1 | 7.33:1 |
| failure words | `danger` on `canvas` | 4.5:1 | 4.87:1 | 5.98:1 |

Ratios are computed from @lingxia/view/theme and rounded down.

## Guidance

- Do: Write `meta` in full: "Not sent", "Sending", a time.
- Do: Join the bubbles of one person's run and name the run once.
- Don't: Colour bubbles per person; side and name tell them apart.
- Don't: Put a whole form in a bubble; a bubble holds a message, a picture or a file.

## Not yet

| Missing | Needs | Meanwhile |
| --- | --- | --- |
| undefined | undefined | undefined |
| undefined | undefined | undefined |

Related: Composer, Avatar, Markdown.
