import { useLocal } from '@lingxia/view';
import { unused } from '../example';
import { IconButton } from '../IconButton/IconButton';
import { MessageBubble } from '../MessageBubble/MessageBubble';
import { Token } from '../Token/Token';
import { Composer, type ComposerProps } from './Composer';

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
        onSubmit={text => { sent.set([...sent.value, text]); draft.set(''); }}
        data-testid="reply"
      />
    </view>
  );
}

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
        onSubmit={text => { said.set(`Sent "${text}"`); draft.set(''); }}
        startContent={<IconButton label="Attach a file" icon="attach" variant="ghost" onClick={() => said.set('Asked for a file')} />}
        data-testid="short"
      />
      {said.value !== '' && <text role="status" className="text-footnote text-muted">{said.value}</text>}
    </view>
  );
}

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
        onSubmit={text => { said.set(`Sent ${text.split('\n').length} lines`); draft.set(''); }}
        startContent={<IconButton label="Attach a file" icon="attach" variant="ghost" onClick={() => said.set('Asked for a file')} />}
        data-testid="long"
      />
      {said.value !== '' && <text role="status" className="text-footnote text-muted">{said.value}</text>}
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
      onSubmit={() => { draft.set(''); files.set([]); }}
      startContent={<IconButton label="Attach a file" icon="attach" variant="ghost" onClick={() => files.set([...files.value, `File ${files.value.length + 1}.pdf`])} />}
      attachments={files.value.length === 0 ? undefined : files.value.map(name => (
        <Token key={name} label={name} onRemove={() => files.set(files.value.filter(each => each !== name))} />
      ))}
      data-testid="attached"
    />
  );
}

export function StateSending() {
  return <Composer value="On my way." onChange={unused} label="Message" onSubmit={unused} isSending data-testid="sending" />;
}

/** While a reply is being written the send button stops it; the next draft can be written meanwhile. */
export function Stoppable() {
  const draft = useLocal('draft', '');
  const isWriting = useLocal('writing', true);
  return (
    <view className="gap-2">
      <text className="text-footnote text-muted" data-testid="stoppable-status">{isWriting.value ? 'Writing a reply…' : 'Stopped'}</text>
      <Composer bind={draft} label="Message" placeholder="Message" onSubmit={() => isWriting.set(true)}
        isSending={isWriting.value} onStop={() => isWriting.set(false)} data-testid="stoppable" />
    </view>
  );
}

export function StateDisabled() {
  return <Composer value="" onChange={unused} label="Message" placeholder="Message" onSubmit={unused} isDisabled disabledMessage="Only members can reply." />;
}

/** The home tile. */
export function Specimen() {
  return <Composer value="" onChange={unused} label="Message" placeholder="Message" onSubmit={unused} className="w-36" />;
}

/** The playground's props that are not plain values. */
export function playground() {
  return { onSubmit: unused } satisfies Partial<ComposerProps>;
}
