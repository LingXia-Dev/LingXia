<!-- Generated from the doc file by scripts/generate.mjs; do not edit. -->

# Composer

Where a message is written and sent. Status: preview.

```ts
import { Composer } from '@liulidesign/core/Composer';
```

Composer is one surface holding the text and its buttons. While the draft fits on one line it is a single row: attach, text, send. Past one line the text takes the full width, growing to five lines and then scrolling, and the buttons sit under it. The send button wakes when there is something to send. The keyboard's send key sends too; with a hardware keyboard Shift and Return add a line. The app sends the text and empties the draft. It sits at the bottom of a conversation, where the page keeps it above the keyboard.

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

### With a way to attach

```tsx
import { useLocal } from '@lingxia/view';
import { IconButton } from '@liulidesign/core/IconButton';
import { Composer } from '@liulidesign/core/Composer';

/** With a way to attach, and a draft that fits on one line: attach, text and send in a row. */
export function RowWithAttach() {
  const draft = useLocal('short', 'On my way.');
  const said = useLocal('said', '');
  return (
    <view className="gap-2">
      <Composer
        bind={draft}
        label="Message"
        placeholder="Message Kai"
        sendLabel="Send"
        onSubmit={text => { said.set(`Sent "${text}"`); draft.set(''); }}
        startContent={<IconButton label="Attach a file" icon="attach" variant="ghost" onClick={() => said.set('Asked for a file')} />}
        data-testid="short"
      />
      {said.value !== '' && <text role="status" className="text-footnote text-muted">{said.value}</text>}
    </view>
  );
}
```

### A draft past one line

```tsx
import { useLocal } from '@lingxia/view';
import { IconButton } from '@liulidesign/core/IconButton';
import { Composer } from '@liulidesign/core/Composer';

/** A draft past one line: the text takes the full width, to five lines and then scrolling, and the buttons go under it. */
export function WithAttach() {
  const draft = useLocal('long', 'Leaving at eight from the east gate.\nBring the tent and the stove.\nI have the maps and water for two days.\nKai has the first-aid kit.\nBack on Sunday before dark.\nCall if the road is closed.');
  const said = useLocal('said', '');
  return (
    <view className="gap-2">
      <Composer
        bind={draft}
        label="Message"
        placeholder="Message Kai"
        sendLabel="Send"
        onSubmit={text => { said.set(`Sent ${text.split('\n').length} lines`); draft.set(''); }}
        startContent={<IconButton label="Attach a file" icon="attach" variant="ghost" onClick={() => said.set('Asked for a file')} />}
        data-testid="long"
      />
      {said.value !== '' && <text role="status" className="text-footnote text-muted">{said.value}</text>}
    </view>
  );
}
```

### With attachments

```tsx
import { useLocal } from '@lingxia/view';
import { IconButton } from '@liulidesign/core/IconButton';
import { Token } from '@liulidesign/core/Token';
import { Composer } from '@liulidesign/core/Composer';

/** What is attached sits inside the composer, over the text. */
export function Attached() {
  const draft = useLocal('note', 'The route and the packing list.');
  const files = useLocal<readonly string[]>('files', ['Route.gpx', 'Packing list.pdf']);
  return (
    <Composer
      bind={draft}
      label="Message"
      placeholder="Message Kai"
      sendLabel="Send"
      onSubmit={() => { draft.set(''); files.set([]); }}
      startContent={<IconButton label="Attach a file" icon="attach" variant="ghost" onClick={() => files.set([...files.value, `File ${files.value.length + 1}.pdf`])} />}
      attachments={files.value.length === 0 ? undefined : files.value.map(name => (
        <Token key={name} label={name} removeLabel={`Remove ${name}`} onRemove={() => files.set(files.value.filter(each => each !== name))} />
      ))}
      data-testid="attached"
    />
  );
}
```

### A stop button while sending

```tsx
import { useLocal } from '@lingxia/view';
import { Composer } from '@liulidesign/core/Composer';

/** While a reply is being written the send button stops it; the next draft can be written meanwhile. */
export function Stoppable() {
  const draft = useLocal('draft', '');
  const isWriting = useLocal('writing', true);
  return (
    <view className="gap-2">
      <text className="text-footnote text-muted" data-testid="stoppable-status">{isWriting.value ? 'Writing a reply…' : 'Stopped'}</text>
      <Composer bind={draft} label="Message" placeholder="Message" sendLabel="Send" onSubmit={() => isWriting.set(true)}
        isSending={isWriting.value} onStop={() => isWriting.set(false)} stopLabel="Stop" data-testid="stoppable" />
    </view>
  );
}
```

## Props

| Prop | Type | Default | Description |
| --- | --- | --- | --- |
| `bind` | `Local<string>` |  | A View-local cell holding the draft; use it or `value` with `onChange`. |
| `value` | `string` |  | The draft; pass `onChange` with it. |
| `onChange` | `((value: string) => void)` |  |  |
| `onStop` | `(() => void)` |  | While sending, the send button becomes a stop button that runs this, as when a reply is being written. |
| `stopLabel` | `string` |  | The stop button's name: "Stop". |
| `label` (required) | `string` |  | The field\'s accessible name: "Message", "Reply to Kai". Not drawn. |
| `placeholder` | `string` |  | A hint shown while the draft is empty. |
| `onSubmit` (required) | `(text: string) => void` |  | Runs with the draft, trimmed, from the send button or the keyboard\'s send key. The app sends it and empties the draft. |
| `sendLabel` (required) | `string` |  | The send button\'s name: "Send". |
| `isSending` | `boolean` | `false` | The send button shows a spinner and takes no press, or becomes the stop button with `onStop`; the draft stays editable. |
| `startContent` | `Child` |  | Before the text, or under it at the start once the draft takes more than a line: an IconButton that attaches a file or a picture. |
| `attachments` | `Child` |  | What is attached, shown inside the composer over the text: Tokens or Thumbnails, each with its own way to remove it. |
| `isDisabled` | `boolean` | `false` |  |
| `disabledMessage` | `string` |  | Why it is disabled, shown under the field: "Only members can reply." |
| `className` | `string` |  |  |
| `data-testid` | `string` |  | On the field; the send button gets `<testid>-send`. |

## Anatomy

| Part | Element | Required | Description |
| --- | --- | --- | --- |
| root | `<view>` | yes | The surface, and under it the reason while disabled. |
| surface | `<view>` | yes | The one bordered box; focus turns all of its border to a 2 px accent edge. |
| attachments | `<view>` | no | What is attached, over the text. |
| start | `<view>` | no | A button that attaches: before the text, or under it at the start. |
| field | `<view>` | yes | The text's place: between the buttons, or the full width over them. |
| editor | `<textarea>` | yes | The text, named by `label`, one to five lines. |
| send | `<IconButton>` | yes | An IconButton named by `sendLabel`; disabled while the draft is empty, loading while sending. |

## What an app may override

- `className`: classes merged after the recipe's, so they win.
- The recipe, `composerRecipe` from `@liulidesign/core/Composer/recipe`: the classes of each part, to style an element of your own to match. The component does not read a changed copy.
- Scale tokens it reads, set in the app's stylesheet under `@theme`: `--text-footnote`, `--text-body`, `--radius-container`.
- Colour roles it paints with, set by the theme pack: `accent`, `line-strong`, `surface`, `text`, `text-faint`, `text-muted`.

## Accessibility

- Pattern: none.
- Name: label.
- Description: none.
- States: disabled (from isDisabled; the send button also while the draft is empty); busy (from isSending, on the send button, unless `onStop` makes it the stop button).
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
- Do: Give `onStop` when what is sent can be stopped, such as a reply being written.
- Don't: Use it for a long text with a title; that is a form with a TextArea.
- Don't: Disable it without saying why; give `disabledMessage`.

## Limits

- It shows what is attached and leaves choosing and uploading files to the app.

Related: MessageBubble, TextArea, IconButton, Token.
