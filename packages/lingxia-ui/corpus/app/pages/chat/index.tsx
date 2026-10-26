import { List, useLocal, type View } from '@lingxia/ui';

type Message = { id: string; mine: boolean; text: string };

export function render({ data }: View<object, { messages: Message[] }>) {
  const draft = useLocal('draft', '');
  return (
    <page className="bg-canvas">
      <List testId="chat" items={data.messages} getKey={message => message.id} anchor="end" className="grow"
        renderItem={message => (
          <row className={message.mine ? 'justify-end px-4 py-1.5' : 'justify-start px-4 py-1.5'}>
            <text className={message.mine
              ? 'max-w-4/5 px-4 py-2.5 rounded-3xl rounded-br-md bg-accent text-on-accent text-sm whitespace-pre-wrap'
              : 'max-w-4/5 px-4 py-2.5 rounded-3xl rounded-bl-md bg-surface border border-line text-sm whitespace-pre-wrap select-text'}>
              {message.text}
            </text>
          </row>
        )} />
      <row className="p-3 gap-2 items-end bg-surface border-t border-line">
        <textarea bind={draft} rows={1} maxRows={5} enterKeyHint="send" submitOnEnter accessibilityLabel="Message" placeholder="Ask anything" className="grow px-3 py-2 rounded-2xl bg-surface-sunken text-sm" />
        <button accessibilityLabel="Send" disabled={!draft.value} onClick={() => draft.set('')} className="w-10 h-10 rounded-full bg-accent text-on-accent items-center justify-center">↑</button>
      </row>
    </page>
  );
}
