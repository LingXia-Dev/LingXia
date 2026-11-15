<!-- Generated from the doc file by scripts/generate.mjs; do not edit. -->

# Composer

Where a message is written and sent. Status: preview.

```ts
import { Composer } from '@liulidesign/core/Composer';
```

Composer is a field that starts one line tall and grows with the draft to five lines, then scrolls, beside a send button that wakes when there is something to send. The keyboard's send key sends too; with a hardware keyboard Shift and Return add a line. The app sends the text and empties the draft. It sits at the bottom of a conversation, where the page keeps it above the keyboard.

## Examples

### Write and send

```tsx
import { useLocal } from '@lingxia/view';
import { MessageBubble } from '@liulidesign/core/MessageBubble';
import { Composer } from '@liulidesign/core/Composer';

/** Write and send: the button wakes when there is something to send, and the draft empties once it is sent. */
export function Reply() {
  const draft = useLocal('draft', '');
  const sent = useLocal<readonly string[]>('sent', ['See you at eight.']);
  return (
    <view className="gap-3">
      <view className="gap-1">
        {sent.value.map((text, index) => <MessageBubble key={`sent-${index}`} side="end" text={text} />)}
      </view>
      <Composer
        bind={draft}
        label="Message"
        placeholder="Message Kai"
        sendLabel="Send"
        onSubmit={text => { sent.set([...sent.value, text]); draft.set(''); }}
        data-testid="reply"
      />
    </view>
  );
}
```

### With a way to attach, and a long draft

```tsx
import { useLocal } from '@lingxia/view';
import { IconButton } from '@liulidesign/core/IconButton';
import { Composer } from '@liulidesign/core/Composer';

/** With a way to attach, and a long draft: the field grows to five lines, then scrolls. */
export function WithAttach() {
  const draft = useLocal('long', 'Leaving at eight from the east gate.\nBring the tent and the stove.\nI have the maps and water for two days.\nKai has the first-aid kit.\nBack on Sunday before dark.\nCall if the road is closed.');
  return (
    <Composer
      bind={draft}
      label="Message"
      sendLabel="Send"
      onSubmit={() => {}}
      startContent={<IconButton label="Attach a file" icon="attach" />}
      data-testid="long"
    />
  );
}
```

## Props

| Prop | Type | Default | Description |
| --- | --- | --- | --- |
| `bind` | `Local<string>` |  | A View-local cell holding the draft; use it or `value` with `onChange`. |
| `value` | `string` |  | The draft; pass `onChange` with it. |
| `onChange` | `((value: string) => void)` |  |  |
| `label` (required) | `string` |  | The field\'s accessible name: "Message", "Reply to Kai". Not drawn. |
| `placeholder` | `string` |  | A hint shown while the draft is empty. |
| `onSubmit` (required) | `(text: string) => void` |  | Runs with the draft, trimmed, from the send button or the keyboard\'s send key. The app sends it and empties the draft. |
| `sendLabel` (required) | `string` |  | The send button\'s name: "Send". |
| `isSending` | `boolean` | `false` | The send button shows a spinner and takes no press; the draft stays editable. |
| `startContent` | `Child` |  | Before the field: an IconButton that attaches a file or a picture. |
| `isDisabled` | `boolean` | `false` |  |
| `disabledMessage` | `string` |  | Why it is disabled, shown under the field: "Only members can reply." |
| `className` | `string` |  |  |
| `data-testid` | `string` |  | On the field; the send button gets `<testid>-send`. |

## Anatomy

| Part | Element | Required | Description |
| --- | --- | --- | --- |
| root | `<view>` | yes | The row, and under it the reason while disabled. |
| row | `<view>` | yes | What sits beside the field stays on its last line. |
| start | `<view>` | no | Before the field: a button that attaches. |
| field | `<TextArea>` | yes | A TextArea named by `label`, one to five lines. |
| send | `<IconButton>` | yes | An IconButton named by `sendLabel`; disabled while the draft is empty, loading while sending. |

## Accessibility

- Pattern: none.
- Name: label.
- Description: none.
- States: undefined (from undefined); undefined (from undefined).
- Return: Sends, with a hardware keyboard.
- Shift and Return: Adds a line.
- Tab: From the field to the send button.
- Press target: md 40 px.
- The field is named by `label` and the button by `sendLabel`; neither name is drawn.
- The send button is unavailable while there is nothing to send, and reports busy while sending.
- A draft of only spaces is not sent.

| Contrast | Pair | Minimum | Light | Dark |
| --- | --- | --- | --- | --- |

Ratios are computed from @lingxia/view/theme and rounded down.

## Guidance

- Do: Empty the draft when the message has been taken, and keep it when sending fails.
- Do: Say in `placeholder` who or what it goes to: "Message Kai".
- Don't: Use it for a long text with a title; that is a form with a TextArea.
- Don't: Disable it without saying why; give `disabledMessage`.

## Not yet

| Missing | Needs | Meanwhile |
| --- | --- | --- |
| undefined | undefined | undefined |

Related: MessageBubble, TextArea, IconButton.
