import { useLocal } from '@lingxia/view';
import { IconButton } from '../IconButton/IconButton';
import { MessageBubble } from '../MessageBubble/MessageBubble';
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
