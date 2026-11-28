import { List, useLocal, useView } from '@lingxia/view';
import { Avatar } from '@liulidesign/core/Avatar';
import { Composer } from '@liulidesign/core/Composer';
import { ConversationScrollButton, useConversationScroll } from '@liulidesign/core/ConversationScrollButton';
import { Divider } from '@liulidesign/core/Divider';
import { IconButton } from '@liulidesign/core/IconButton';
import { MessageBubble } from '@liulidesign/core/MessageBubble';
import { StatusDot } from '@liulidesign/core/StatusDot';

type Entry =
  | { id: string; kind: 'day'; label: string }
  | { id: string; kind: 'message'; mine: boolean; text: string; time: string; failed?: boolean };

const earlier: Entry[] = [
  { id: 'd1', kind: 'day', label: 'Yesterday' },
  { id: 'm1', kind: 'message', mine: false, text: 'Are we still on for the coast on Saturday?', time: '18:02' },
  { id: 'm2', kind: 'message', mine: false, text: 'I can drive if you bring the tent.', time: '18:02' },
  { id: 'm3', kind: 'message', mine: true, text: 'Yes! The tent is packed.', time: '18:10' },
  { id: 'm4', kind: 'message', mine: true, text: 'Leaving at eight from the east gate?', time: '18:10' },
  { id: 'd2', kind: 'day', label: 'Today' },
  { id: 'm5', kind: 'message', mine: false, text: 'Eight works. The directions are at https://example.com/maps/place/east-gate-car-park-level-two', time: '08:41' },
  { id: 'm6', kind: 'message', mine: true, text: 'Got it. Is the road past the pier open again?', time: '08:44' },
  { id: 'm7', kind: 'message', mine: false, text: 'It opened on Monday.', time: '08:45' },
  { id: 'm8', kind: 'message', mine: true, text: 'Can you see the car?', time: '08:59', failed: true },
];

export function Content() {
  const entries = useLocal<readonly Entry[]>('entries', earlier);
  const draft = useLocal('draft', '');
  const last = useLocal('last', '');
  const list = entries.value;
  const scroll = useConversationScroll(list.length);
  // On a phone the page gives it its height, less the keyboard; beside the gallery's sidebar it sits in a scroll and brings its own.
  const isWide = useView().env.sizeClass === 'regular';
  // A bubble joins the one beside it when the same person wrote both.
  const sameWriter = (a: Entry | undefined, b: Entry) => a !== undefined && a.kind === 'message' && b.kind === 'message' && a.mine === b.mine;
  const send = (text: string) => {
    entries.set([...list, { id: `m${list.length + 1}`, kind: 'message', mine: true, text, time: '09:00' }]);
    draft.set('');
  };
  const retry = (id: string) => entries.set(list.map(entry => (entry.id === id && entry.kind === 'message' ? { ...entry, failed: false, time: '09:00' } : entry)));
  return (
    <view className={isWide ? 'h-[640px] gap-4' : 'grow basis-0 min-h-0 gap-4'}>
      {/* Beside the sidebar the workspace already shows the page's title. */}
      {!isWide && <text role="heading" headingLevel={1} className="text-title font-semibold tracking-tight text-text">Conversation</text>}
      <view className="grow basis-0 min-h-0 overflow-hidden rounded-container border border-line bg-canvas">
        <view className="flex-row items-center gap-3 px-4 py-3 border-b border-line bg-surface">
          <Avatar name="Kai Chen" size="sm" isDecorative />
          <view className="grow basis-0 min-w-0 gap-0.5">
            <text className="text-body font-medium text-text">Kai Chen</text>
            <StatusDot label="Online" tone="success" size="sm" />
          </view>
          <IconButton label="Conversation details" icon="info" size="sm" onClick={() => last.set('Opened the conversation details')} />
        </view>
        <view className="grow basis-0 min-h-0">
        <List
          aria-label="Messages with Kai Chen"
          items={list}
          getKey={entry => entry.id}
          // The newest message is kept in view as each one arrives, unless the reader is further up.
          anchor={scroll.anchor}
          onScrollEdge={scroll.onScrollEdge}
          renderItem={(entry, index) => {
            if (entry.kind === 'day') return <Divider label={entry.label} className="py-2" />;
            const [before, after] = [list[index - 1], list[index + 1]];
            const startsRun = !sameWriter(before, entry);
            const endsRun = after === undefined || !sameWriter(entry, after);
            return (
              <MessageBubble
                side={entry.mine ? 'end' : 'start'}
                text={entry.text}
                avatar={!entry.mine && endsRun ? <Avatar name="Kai Chen" size="sm" isDecorative /> : undefined}
                hasAvatarSpace={!entry.mine}
                isJoinedBefore={!startsRun}
                isJoinedAfter={!endsRun}
                status={entry.failed ? 'failed' : undefined}
                meta={entry.failed ? 'Not sent' : endsRun ? entry.time : undefined}
                action={entry.failed ? <IconButton label="Send again" icon="refresh" size="sm" onClick={() => retry(entry.id)} data-testid={`retry-${entry.id}`} /> : undefined}
                className={endsRun ? 'pb-2' : 'pb-0.5'}
                data-testid={`message-${entry.id}`}
              />
            );
          }}
          className="grow basis-0 min-h-0 px-3 pt-3 overscroll-contain"
        />
        <ConversationScrollButton scroll={scroll} data-testid="latest" />
        </view>
        <view className="px-3 py-2 border-t border-line bg-surface">
          <Composer bind={draft} label="Message to Kai Chen" placeholder="Message Kai" onSubmit={send} startContent={<IconButton label="Attach a file" icon="attach" onClick={() => last.set('Opened the file picker')} />} data-testid="composer" />
        </view>
      </view>
      {last.value !== '' && <text role="status" className="text-footnote text-muted">{last.value}</text>}
    </view>
  );
}
