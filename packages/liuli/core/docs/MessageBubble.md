<!-- Generated from the doc file by scripts/generate.mjs; do not edit. -->

# MessageBubble

One message of a conversation. Status: preview.

```ts
import { MessageBubble, MessageReasoning, MessageToolCall } from '@liulidesign/core/MessageBubble';
```

MessageBubble draws one message: the user's own at the end edge in the accent's soft step, anyone else's at the start on a quiet tint, with a name and a face over the first of a run. Under it goes a line the app writes, the time or how the message stands, and for one that failed a way to send it again. A conversation is the View's list of these with a Composer under it; the Conversation pattern shows them together. MessageReasoning folds how an assistant got to its answer into one line above the message, and MessageToolCall each step it took with a tool: a line with where the run stands, its details behind it, and Allow and Deny while the step waits on the user.

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

### An assistant's reasoning, folded above its answer

```tsx
import { Avatar } from '@liulidesign/core/Avatar';
import { MessageBubble, MessageReasoning } from '@liulidesign/core/MessageBubble';

/** An assistant's answer with how it got there folded above it; the second is still thinking. */
export function Reasoning() {
  return (
    <view className="gap-1">
      <MessageReasoning
        label="Thought for 8 seconds"
        text="The trip is Saturday and the forecast shows rain after four. The ferry at 14:10 gets in before that; the 16:40 does not. So the earlier ferry, and the tent can wait in the car."
        isDefaultOpen
        hasAvatarSpace
        data-testid="reasoning"
      />
      <MessageBubble avatar={<Avatar name="Assistant" initials="A" size="sm" isDecorative />} author="Assistant" text="Take the 14:10 ferry: it lands before the rain." />
      <view className="h-2" />
      <MessageReasoning label="Thinking" text="Checking the return times for Sunday" isStreaming hasAvatarSpace data-testid="thinking" />
    </view>
  );
}
```

### An assistant's tool steps: done, failed, running, and one waiting to be allowed

```tsx
import { useLocal } from '@lingxia/view';
import { CodeBlock } from '@liulidesign/core/Code';
import { MessageToolCall } from '@liulidesign/core/MessageBubble';

/** An assistant's steps with tools: done, failed and running fold to a line; one that deletes files waits to be allowed, then is done or denied. */
export function ToolCalls() {
  const answer = useLocal<'waiting' | 'allowed' | 'denied'>('waiting');
  return (
    <view className="gap-0.5">
      <MessageToolCall label="Searched the notes for 'packing list'" status="done" meta="3 results" hasAvatarSpace data-testid="searched">
        <text>Packing list (Saturday), Tent checklist, Ferry times</text>
      </MessageToolCall>
      <MessageToolCall label="npm test" status="failed" meta="4.2 s" isDefaultOpen hasAvatarSpace data-testid="tests">
        <CodeBlock label="Test output" code={'FAIL  src/trip.test.ts\n  ferry › lands before the rain\n    expected 14:10, got 16:40'} />
      </MessageToolCall>
      <MessageToolCall label="Reading the forecast for Saturday" status="running" hasAvatarSpace data-testid="forecast" />
      {answer.value === 'waiting'
        ? (
          <MessageToolCall
            label="Delete 3 old drafts in Notes"
            status="waiting"
            approval={{ allowLabel: 'Delete 3 drafts', isDestructive: true, onAllow: () => answer.set('allowed'), onDeny: () => answer.set('denied') }}
            hasAvatarSpace
            data-testid="delete"
          >
            <text>{'Trip ideas (old)\nTent checklist (copy)\nUntitled note'}</text>
          </MessageToolCall>
        )
        : <MessageToolCall label="Delete 3 old drafts in Notes" status={answer.value === 'allowed' ? 'done' : 'denied'} hasAvatarSpace data-testid="delete" />}
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
| `status` | `MessageStatus` |  | `failed` adds an error mark to `meta`; say what failed in `meta`. |
| `action` | `Child` |  | Beside `meta`, for a failed message: a Button that sends it again. |
| `isJoinedBefore` | `boolean` | `false` | Whether the same person\'s bubble comes right before this one: tightens the corner toward it. |
| `isJoinedAfter` | `boolean` | `false` | Whether the same person\'s bubble comes right after. |
| `className` | `string` |  |  |
| `data-testid` | `string` |  |  |

## MessageReasoning props

| Prop | Type | Default | Description |
| --- | --- | --- | --- |
| `label` (required) | `string` |  | The one line that is always shown, and the toggle's name: "Thought for 8 seconds". |
| `text` (required) | `string` |  | The reasoning itself, shown while open. |
| `isStreaming` | `boolean` | `false` | Still being written: a spinner sits before the label and the toggle reports busy. |
| `hasAvatarSpace` | `boolean` | `false` | Keeps an avatar's width empty before it, so it lines up with the bubbles of a run. |
| `isDefaultOpen` | `boolean` | `false` | Open on first render, uncontrolled. |
| `isOpen` | `boolean` |  | Controlled open state; pass `onOpenChange` with it. |
| `onOpenChange` | `((isOpen: boolean) => void)` |  |  |
| `className` | `string` |  |  |
| `data-testid` | `string` |  | On the toggle; the text gets `<testid>-text`. |

## MessageToolCall props

| Prop | Type | Default | Description |
| --- | --- | --- | --- |
| `status` (required) | `"denied" \| "done" \| "failed" \| "running" \| "waiting"` |  |  |
| `approval` | `MessageToolCallApproval` |  | The two answers, shown under the line with the details open while it waits. |
| `label` (required) | `string` |  | What the tool does or did, in one line: "Searched the web for tide times", "npm test". |
| `meta` | `string` |  | A short fact after the label: "2.4 s", "3 results". |
| `children` | `Child` |  | The details, shown while open: its arguments and what it returned, as a CodeBlock, a Markdown or text. Without them the line does not open. |
| `hasAvatarSpace` | `boolean` | `false` | Keeps an avatar's width empty before it, so it lines up with the bubbles of a run. |
| `isDefaultOpen` | `boolean` | `false` | Open on first render, uncontrolled. |
| `isOpen` | `boolean` |  | Controlled open state; pass `onOpenChange` with it. |
| `onOpenChange` | `((isOpen: boolean) => void)` |  |  |
| `className` | `string` |  |  |
| `data-testid` | `string` |  | On the line; the details get `<testid>-details`, the answers `<testid>-allow` and `<testid>-deny`. |

## Anatomy

| Part | Element | Required | Description |
| --- | --- | --- | --- |
| root | `<view>` | yes | The row: it puts the bubble at its side. |
| line | `<view>` | yes | The avatar and the bubble, the avatar level with the bubble's last line. |
| avatar | `<view>` | no | The writer's Avatar, or its width kept empty down a run. |
| author | `<text>` | no | Who wrote it, over the first bubble of a run. |
| bubble | `<view>` | yes | The message: up to four fifths of the row, 520 px at most. |
| meta | `<view>` | no | The line under the bubble: time, state, and the retry action. |
| reasoningToggle | `<button>` | no | MessageReasoning's one line: a button named by `label`, with a spinner while `isStreaming`. |
| reasoningText | `<text>` | no | The reasoning, behind a rule at the start; it can be selected. |
| toolLine | `<button>` | no | MessageToolCall's line: the mark for where it stands, the label, "Failed" or "Denied" in words, the meta; a button when it has details. |
| toolDetails | `<view>` | no | Its details, behind the same rule as the reasoning; open while the step waits. |
| toolAnswers | `<view>` | no | Deny, then Allow, while it waits. |

## What an app may override

- `className`: classes merged after the recipe's, so they win.
- `className` on MessageReasoning: classes merged after the recipe's, so they win.
- `className` on MessageToolCall: classes merged after the recipe's, so they win.
- The recipe, `messageBubbleRecipe` from `@liulidesign/core/MessageBubble/recipe`: the classes of each part, to style an element of your own to match. The component does not read a changed copy.
- Scale tokens it reads, set in the app's stylesheet under `@theme`: `--text-caption`, `--text-footnote`, `--text-body`, `--radius-control`, `--radius-container`, `--radius-checkbox`.
- Colour roles it paints with, set by the theme pack: `accent`, `accent-soft`, `danger`, `focus`, `line`, `on-accent-soft`, `text`, `text-muted`.

## Accessibility

- Pattern: none.
- Name: none; its content is read.
- Description: none.
- States: expanded (from MessageReasoning: isOpen, or isDefaultOpen and the presses since); busy (from MessageReasoning: isStreaming; MessageToolCall: status running); expanded (from MessageToolCall with details: isOpen, or isDefaultOpen and the presses since).
- Enter or Space: Opens and closes a MessageReasoning, or a MessageToolCall that has details.
- Press target: .
- It is read in order: the name, the message, then the line under it. Put the list in the View's list so a screen reader steps message by message.
- A failed message says so in words and with an error mark under the bubble, not with colour alone.
- The side tells whose it is to the eye; say it in `author` for the ear, or name the list by who is speaking.
- A tool step is named by its label and where it stands ("npm test, failed"), so the mark is never the only sign.

| Contrast | Pair | Minimum | Light | Dark |
| --- | --- | --- | --- | --- |
| my message | `on-accent-soft` on `accent-soft` | 4.5:1 | 8.54:1 | 9.49:1 |
| their message | `text` on `text/6` over `canvas` | 4.5:1 | 13.54:1 | 13.46:1 |
| meta | `text-muted` on `canvas` | 4.5:1 | 5.50:1 | 7.33:1 |
| reasoning, its line and its text | `text-muted` on `canvas` | 4.5:1 | 5.50:1 | 7.33:1 |
| failure words | `danger` on `canvas` | 4.5:1 | 4.87:1 | 5.98:1 |
| a waiting tool step's mark | `accent` on `canvas` | 3:1 | 5.25:1 | 6.83:1 |
| a waiting tool step's label | `text` on `canvas` | 4.5:1 | 15.19:1 | 15.31:1 |

Ratios are computed from @lingxia/view/theme and rounded down.

## Guidance

- Do: Write `meta` in full: "Not sent", "Sending", a time.
- Do: Join the bubbles of one person's run and name the run once.
- Do: Label a tool step by what it does, in the user's words: "Searched the web for tide times", not the tool's name.
- Do: Name what allowing does when it cannot be undone: `allowLabel` "Delete 3 files" with `isDestructive`.
- Don't: Colour bubbles per person; side and name tell them apart.
- Don't: Put a whole form in a bubble; a bubble holds a message, a picture or a file.

## Limits

- No reactions, threads or read receipts.
- No typing mark.

Related: Composer, Avatar, Markdown.
