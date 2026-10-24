import { PageContent, contentBounds } from '../../shared/components/page';
import { useLocal, useStream, type View, type Stream, type ReadonlyData } from '@lingxia/ui';
import '../../theme/index.css';
import type { ChatChunk, ChartData, SendMessage, StreamPageData } from '../../shared/stream-types';
import { Chart } from '../../shared/chart.ui';

type Actions = {
  goBack(): Promise<void>;
  onClear(): Promise<void>;
  onSend(params: SendMessage): Stream<ChatChunk, void>;
};

export function render({ data, actions, env }: View<Actions, StreamPageData>) {
  const draft = useLocal('message', '');
  const tokens = useLocal('tokens', '');
  const artifact = useLocal<ChartData | null>('artifact', null);
  const tailRevision = useLocal('tail-revision', 0);
  const stream = useStream('response', actions.onSend, {
    onData(chunk) {
      if (chunk.type === 'token') tokens.set(tokens.value + chunk.token);
      else artifact.set(chunk.chart);
      tailRevision.set(tailRevision.value + 1);
    },
  });
  const pending = stream.status === 'streaming';
  const busy = pending || !!data.isStreaming;
  const send = () => {
    if (busy || !draft.value.trim()) return;
    tokens.set(''); artifact.set(null);
    stream.start({ text: draft.value.trim(), chartKind: 'auto' }); draft.set('');
  };
  const messages = data.messages || [];
  const messageKeys = messages.map(message => `stream-message-${message.id}`);
  if (pending) messageKeys.push('stream-live');
  const messageCount = messageKeys.length;
  // Repeat the tail anchor only when conversation content changes, not on user scrolling.
  const tailAnchor = { index: Math.max(0, messageCount - 1), align: 'end' as const, seq: tailRevision.value + messages.length };
  const { height } = contentBounds(env);
  const bubble = (id: string, content: string, user: boolean, chart?: ReadonlyData<ChartData> | null) => <row key={id} className={user ? 'justify-end px-4 py-1.5' : 'justify-start px-4 py-1.5'}>
    <row className={user ? 'max-w-[78%]' : 'max-w-[90%] items-start gap-2'}>
      {!user ? <column className="w-7 h-7 rounded-full bg-linear-to-br from-violet-500 to-blue-600 items-center justify-center shrink-0"><vector className="w-3.5 h-3.5 text-white" paths={[{ d: 'M12 2a10 10 0 110 20A10 10 0 0112 2zm0 2a8 8 0 100 16A8 8 0 0012 4zm-1 5h2v2h-2V9zm0 4h2v6h-2v-6z', fill: 'currentColor' }]} /></column> : null}
      <column className={user ? 'shrink min-w-0 px-4 py-2.5 rounded-3xl rounded-br-md bg-blue-600 text-white' : 'shrink min-w-0 px-4 py-2.5 rounded-3xl rounded-bl-md bg-surface border border-line-200 text-gray-800 shadow-sm'}>
        <text testId={id} className={content ? "text-sm leading-relaxed break-words" : "text-sm leading-relaxed break-words italic text-gray-400"}>{content || '...'}</text>
        {chart ? <Chart data={chart} /> : null}
      </column>
    </row>
  </row>;
  const empty = messages.length === 0 && !pending;
  return <PageContent testId="stream-page" className="bg-surface-100 text-gray-900">
      <column className="grow basis-0 min-w-0" style={{ height }}>
        <stack className="grow basis-0 min-h-0">
        {empty ? <scroll testId="page-scroll" className="grow basis-0 min-h-0">
          <column className="grow basis-0 shrink-0 min-h-full px-8 py-4 items-center justify-center gap-3 text-center">
            <column className="w-16 h-16 rounded-2xl bg-surface shadow items-center justify-center"><vector className="w-8 h-8 text-blue-600 dark:text-blue-400" paths={[{d:'M8.625 12a.375.375 0 11-.75 0 .375.375 0 01.75 0zm0 0H8.25m4.125 0a.375.375 0 11-.75 0 .375.375 0 01.75 0zm0 0H12m4.125 0a.375.375 0 11-.75 0 .375.375 0 01.75 0zm0 0h-.375M21 12c0 4.556-4.03 8.25-9 8.25a9.764 9.764 0 01-2.555-.337A5.972 5.972 0 015.41 20.97a5.969 5.969 0 01-.474-.065 4.48 4.48 0 00.978-2.025c.09-.457-.133-.901-.467-1.226C3.93 16.178 3 14.189 3 12c0-4.556 4.03-8.25 9-8.25s9 3.694 9 8.25z',fill:'none',stroke:'currentColor',strokeWidth:1.5}]} /></column>
            <column className="gap-1"><text testId="stream-empty" className="text-base font-semibold text-gray-800">Stream Demo</text><text className="text-sm text-gray-500">Async generator streaming — text &amp; chart artifacts via LingXia bridge.</text></column>
            <column className="w-full gap-2 pt-2">{['Tell me about LingXia streaming', 'Show me some data', 'How does the bridge protocol work?'].map(hint => <text key={hint} testId={`stream-hint-${hint}`} className="text-sm text-blue-600 dark:text-blue-400 bg-blue-50 rounded-xl px-4 py-2.5 text-left">{hint}</text>)}</column>
          </column>
        </scroll> : <list testId="page-scroll" className="grow basis-0 min-h-0 pt-12 pb-4" count={messageCount} itemKeys={messageKeys} anchor={tailAnchor} row={index => index < messages.length ? bubble(messageKeys[index], messages[index].content, messages[index].role === 'user', messages[index].chart) : bubble(messageKeys[index], tokens.value, false, artifact.value)} />}
        {messages.length > 0 && !pending ? <row className="justify-end items-start px-4 pt-3"><button testId="stream-clear" onClick={() => actions.onClear()} className="text-xs text-blue-600 dark:text-blue-400 px-3 py-1 bg-surface rounded-full shadow-sm">Clear</button></row> : null}
        </stack>
        {stream.error ? <text testId="stream-error" className="px-4 text-xs text-red-500">{stream.error}</text> : null}
        <row className="bg-surface border-t border-line-200 px-3 py-3 items-end gap-2">
          <row className="grow basis-0 min-w-0 bg-surface-100 rounded-2xl px-3.5 py-2.5 items-end gap-2">
            <textarea accessibilityLabel="Message" testId="stream-input" bind={draft} submitOnEnter onSubmit={send} enabled={!busy} placeholder="Message..." className="grow basis-0 min-w-0 h-auto min-h-[22px] max-h-[120px] bg-transparent p-0 text-sm text-gray-800 placeholder:text-gray-400 leading-relaxed" />
          </row>
          {busy ? <button testId="stream-stop" accessibilityLabel="Stop response" onClick={() => stream.cancel()} className="w-9 h-9 rounded-full bg-surface-800 items-center justify-center"><column className="w-3 h-3 bg-surface rounded-sm" /></button> : <button testId="stream-send" accessibilityLabel="Send message" enabled={!!draft.value.trim()} onClick={send} className={draft.value.trim() ? 'w-9 h-9 rounded-full bg-blue-600 items-center justify-center' : 'w-9 h-9 rounded-full bg-blue-600 items-center justify-center opacity-30'}><vector className="w-4 h-4 text-white" paths={[{d:'M12 4l8 8H14v8h-4v-8H4l8-8z',fill:'currentColor'}]} /></button>}
        </row>
      </column>
  </PageContent>;
}
