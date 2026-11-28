<!-- Generated from the doc file by scripts/generate.mjs; do not edit. -->

# ConversationScrollButton

Back to the newest message. Status: preview.

```ts
import { useConversationScroll, ConversationScrollButton } from '@liulidesign/core/ConversationScrollButton';
```

`useConversationScroll(count)` keeps a conversation's List on its newest message as messages arrive while the reader is there, and holds it still while they read further up. ConversationScrollButton shows only while they are up: a round button at the end corner over the bottom of the conversation that goes back to the newest message, with the count of messages that arrived meanwhile on its corner. Pass the hook's `anchor` and `onScrollEdge` to the List, and put the button after the List in the box that holds both.

## Examples

### A conversation that follows new messages, with the button while reading up

```tsx
import { List, useLocal } from '@lingxia/view';
import { Button } from '@liulidesign/core/Button';
import { MessageBubble } from '@liulidesign/core/MessageBubble';
import { ConversationScrollButton, useConversationScroll } from '@liulidesign/core/ConversationScrollButton';

const opening = Array.from({ length: 24 }, (_, index) => ({ id: `m${index + 1}`, mine: index % 3 === 2, text: index % 3 === 2 ? `Noted, item ${index + 1}.` : `Packing list item ${index + 1}: ${['tent', 'stove', 'rain jackets', 'torch'][index % 4]}` }));

/** A conversation longer than its box: scroll up and the button appears; messages that arrive meanwhile are counted on it. */
export function Follow() {
  const messages = useLocal('messages', opening);
  const list = messages.value;
  const scroll = useConversationScroll(list.length);
  const arrive = () => messages.set([...list, { id: `m${list.length + 1}`, mine: false, text: `New message ${list.length + 1}` }]);
  return (
    <view className="gap-3">
      <view className="h-80 overflow-hidden rounded-container border border-line bg-canvas">
        <List
          aria-label="Messages"
          items={list}
          getKey={message => message.id}
          anchor={scroll.anchor}
          onScrollEdge={scroll.onScrollEdge}
          renderItem={message => <MessageBubble side={message.mine ? 'end' : 'start'} text={message.text} className="pb-1" data-testid={`message-${message.id}`} />}
          className="grow basis-0 min-h-0 px-3 pt-3"
          data-testid="messages"
        />
        <ConversationScrollButton scroll={scroll} data-testid="latest" />
      </view>
      <Button label="A message arrives" size="sm" onClick={arrive} className="self-start" data-testid="arrive" />
    </view>
  );
}
```

## Props

| Prop | Type | Default | Description |
| --- | --- | --- | --- |
| `scroll` (required) | `ConversationScroll` |  | The conversation's `useConversationScroll`. |
| `label` | `string` |  | The button's name. Defaults to "Jump to the latest message" in the page's language, with the count of new ones. |
| `className` | `string` |  |  |
| `data-testid` | `string` |  |  |

## ConversationScroll props

| Prop | Type | Default | Description |
| --- | --- | --- | --- |
| `anchor` (required) | `{ index: number; align: "end"; seq: number; }` |  |  |
| `onScrollEdge` (required) | `(event: ScrollEdgeEvent) => void` |  |  |
| `isAtEnd` (required) | `boolean` |  | The newest message is in view. |
| `unseen` (required) | `number` |  | Messages that arrived while the reader was further up. |
| `jumpToLatest` (required) | `() => void` |  | Scrolls to the newest message. |

## Anatomy

| Part | Element | Required | Description |
| --- | --- | --- | --- |
| root | `<button>` | yes | The round button on an opaque raised fill with the floating shadow, over the conversation at its end corner. |
| count | `<text>` | no | How many messages arrived out of view, on the button's corner; 99+ past 99. |

## What an app may override

- `className`: classes merged after the recipe's, so they win.
- The recipe, `conversationScrollButtonRecipe` from `@liulidesign/core/ConversationScrollButton/recipe`: the classes of each part, to style an element of your own to match. The component does not read a changed copy.
- Scale tokens it reads, set in the app's stylesheet under `@theme`: `--text-caption`, `--shadow-raised`.
- Colour roles it paints with, set by the theme pack: `accent`, `focus`, `line`, `line-strong`, `on-accent`, `surface-raised`, `text`.

## Accessibility

- Pattern: none.
- Name: label.
- Description: none.
- Enter or Space: Scrolls to the newest message.
- Press target: md 40 px.
- Its name says how many messages are new, so the count is not only seen.
- It leaves the tree at the newest message, so it is never a stop that does nothing.

| Contrast | Pair | Minimum | Light | Dark |
| --- | --- | --- | --- | --- |
| arrow | `text` on `surface-raised` | 4.5:1 | 16.26:1 | 11.83:1 |
| count | `on-accent` on `accent` | 4.5:1 | 5.62:1 | 7.12:1 |

Ratios are computed from @lingxia/view/theme and rounded down.

## Guidance

- Do: Use the hook with every conversation that grows while it is read, a streamed answer included.
- Don't: Scroll the reader down when a message arrives while they read further up; the count tells them.

## Limits

- It counts messages, not unread ones; a list with read state counts those itself and passes `label`.

Related: MessageBubble, Composer.
