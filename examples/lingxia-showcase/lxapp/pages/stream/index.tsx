import { t, List, useLocal, useStream, type ReadonlyData, type Stream, type MessageKey } from '@lingxia/view';
import '../../theme/index.css';
import { Chart } from '../../shared/chart';
import type { ChartData } from '../../shared/stream-types';
import { EmptyState, ErrorState, Icon } from '../../shared/ui';
import type { View } from './$types';

const hints = ['stream.hint.streaming', 'stream.hint.data', 'stream.hint.bridge'];

function Bubble({ id, content, user, chart }: { id: string; content: string; user: boolean; chart?: ReadonlyData<ChartData> | null }) {
  return (
    <view className={user ? 'flex-row justify-end px-4 py-1.5' : 'flex-row justify-start px-4 py-1.5 gap-2 items-start'}>
      {!user && (
        <view className="w-7 h-7 shrink-0 rounded-full bg-linear-to-br from-chart-4 to-accent items-center justify-center">
          <Icon name="bolt" className="w-3.5 h-3.5 text-on-accent" />
        </view>
      )}
      <view
        className={
          user
            ? 'shrink min-w-0 max-w-4/5 px-4 py-2.5 rounded-3xl rounded-br-md bg-accent text-on-accent'
            : `${chart ? 'grow basis-0' : 'shrink'} min-w-0 max-w-[90%] px-4 py-2.5 rounded-3xl rounded-bl-md border border-line bg-surface shadow-sm`
        }
      >
        <text
          data-testid={id}
          className={content ? 'text-sm leading-relaxed break-words' : 'text-sm leading-relaxed break-words italic text-muted'}
        >
          {content || '...'}
        </text>
        {chart && <Chart data={chart} />}
      </view>
    </view>
  );
}

export function render({ data, actions }: View) {
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
    tokens.set('');
    artifact.set(null);
    stream.start({ text: draft.value.trim(), chartKind: 'auto' });
    draft.set('');
  };
  const messages = data.messages;
  const keys = messages.map(message => `stream-message-${message.id}`);
  if (pending) keys.push('stream-live');
  // Repeat the tail anchor only when conversation content changes, not on user scrolling.
  const anchor = { index: Math.max(0, keys.length - 1), align: 'end' as const, seq: tailRevision.value + messages.length };
  const empty = !messages.length && !pending;
  return (
    <page data-testid="stream-page" className="bg-canvas text-text">
      {messages.length > 0 && !pending && (
        <view className="flex-row justify-end px-4 pt-3">
          <button data-testid="stream-clear" onClick={() => actions.onClear()} className="px-3 py-1 rounded-full bg-surface text-xs text-accent shadow-sm">
            {t('stream.clear')}
          </button>
        </view>
      )}
      {empty ? (
        <scroll data-testid="page-scroll" className="grow basis-0">
          <view className="min-h-full px-8 py-4 items-center justify-center gap-3">
            <EmptyState
              testId="stream-empty"
              icon="chat"
              title={t('stream.streamDemo')}
              message={t('stream.asyncGeneratorStreamingText')}
            />
            <view className="w-full gap-2">
              {hints.map(hint => (
                <text key={hint} data-testid={`stream-hint-${hint}`} className="px-4 py-2.5 rounded-xl bg-accent-soft text-sm text-accent">
                  {t(hint as MessageKey)}
                </text>
              ))}
            </view>
          </view>
        </scroll>
      ) : (
        <List
          data-testid="page-scroll"
          className="grow basis-0 min-h-0 pt-3 pb-4"
          items={keys}
          getKey={key => key}
          anchor={anchor}
          renderItem={(_, index) =>
            index < messages.length ? (
              <Bubble id={keys[index]} content={messages[index].content} user={messages[index].role === 'user'} chart={messages[index].chart} />
            ) : (
              <Bubble id={keys[index]} content={tokens.value} user={false} chart={artifact.value} />
            )
          }
        />
      )}
      <view className="px-4">
        <ErrorState testId="stream-error" error={stream.error} />
      </view>
      <view className="flex-row items-end gap-2 px-3 py-3 border-t border-line bg-surface">
        <textarea
          data-testid="stream-input"
          data-controlled-value={draft.value}
          aria-label={t('stream.message')}
          bind={draft}
          enterKeyHint="send"
          onSubmit={send}
          disabled={busy}
          placeholder={t('stream.message2')}
          className="grow basis-0 min-w-0 h-auto min-h-10 max-h-30 px-3.5 py-2.5 rounded-2xl bg-surface-sunken text-sm leading-relaxed placeholder:text-muted"
        />
        {busy ? (
          <button
            data-testid="stream-stop"
            aria-label={t('stream.stopResponse')}
            onClick={() => stream.cancel()}
            className="w-9 h-9 rounded-full bg-text/90 items-center justify-center"
          >
            <view className="w-3 h-3 rounded-sm bg-surface" />
          </button>
        ) : (
          <button
            data-testid="stream-send"
            aria-label={t('stream.sendMessage')}
            disabled={!draft.value.trim()}
            onClick={send}
            className="w-9 h-9 rounded-full bg-accent-solid items-center justify-center disabled:opacity-30"
          >
            <Icon name="send" className="w-4 h-4 text-on-accent-solid" />
          </button>
        )}
      </view>
    </page>
  );
}
