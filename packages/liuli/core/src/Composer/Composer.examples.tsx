import { useLocal } from '@lingxia/view';
import { IconButton } from '../IconButton/IconButton';
import { MessageBubble } from '../MessageBubble/MessageBubble';
import { Token } from '../Token/Token';
import { Composer } from './Composer';

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

/** With a way to attach, and a draft that fits on one line: attach, text and send in a row. */
export function RowWithAttach() {
  const draft = useLocal('short', 'On my way.');
  const said = useLocal('said', 'Nothing sent yet');
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
      <text role="status" className="text-footnote text-muted">{said.value}</text>
    </view>
  );
}

/** A draft past one line: the text takes the full width, to five lines and then scrolling, and the buttons go under it. */
export function WithAttach() {
  const draft = useLocal('long', 'Leaving at eight from the east gate.\nBring the tent and the stove.\nI have the maps and water for two days.\nKai has the first-aid kit.\nBack on Sunday before dark.\nCall if the road is closed.');
  const said = useLocal('said', 'Nothing sent yet');
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
      <text role="status" className="text-footnote text-muted">{said.value}</text>
    </view>
  );
}

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

export function StateSending() {
  return <Composer value="On my way." onChange={() => {}} label="Message" sendLabel="Send" onSubmit={() => {}} isSending data-testid="sending" />;
}

export function StateDisabled() {
  return <Composer value="" onChange={() => {}} label="Message" placeholder="Message" sendLabel="Send" onSubmit={() => {}} isDisabled disabledMessage="Only members can reply." />;
}

/** The home tile. */
export function Specimen() {
  return <Composer value="" onChange={() => {}} label="Message" placeholder="Message" sendLabel="Send" onSubmit={() => {}} className="w-36" />;
}
