import { cx, useLocal, useView } from '@lingxia/view';
import { Avatar } from '../Avatar/Avatar';
import { Badge } from '../Badge/Badge';
import { Button } from '../Button/Button';
import { Composer } from '../Composer/Composer';
import { Icon } from '../Icon/Icon';
import { IconButton } from '../IconButton/IconButton';
import { MessageBubble } from '../MessageBubble/MessageBubble';
import { Split } from './Split';

const chat = [
  { name: 'Mira Patel', text: 'The light on the water right now!' },
  { name: 'Sam Ortiz', text: 'Which harbour is this?' },
  { name: 'Ana Lima', text: 'The north pier, the boats leave at seven.' },
  { name: 'Kai Chen', text: 'Watching from the train, the sound is perfect.' },
];

/** A live stream and its chat: side by side on a wide screen, the chat under the stream on a phone. */
export function LiveStreamSplit() {
  const { env } = useView();
  const draft = useLocal('chat-draft', '');
  const playing = useLocal('playing', true);
  const muted = useLocal('muted', false);
  const large = useLocal('large', false);
  return (
    <view className={cx('overflow-hidden rounded-container border border-text/12', env.sizeClass === 'regular' ? 'h-[400px]' : 'h-[600px]')}>
      <Split
        asideWidth={300}
        asideLabel="Live chat"
        aside={
          <view className="grow min-h-0">
            <view className="flex-row items-center justify-between px-4 pt-3 pb-2">
              <text role="heading" headingLevel={2} className="text-callout font-semibold text-text">Live chat</text>
              <text className="text-caption text-muted tabular-nums">1,284 watching</text>
            </view>
            <scroll className="grow min-h-0">
              <view className="px-4 pb-2 gap-2">
                {chat.map(line => (
                  <MessageBubble key={line.name} author={line.name} avatar={<Avatar name={line.name} size="sm" isDecorative />} text={line.text} />
                ))}
              </view>
            </scroll>
            <view className="px-3 pb-3 pt-2 border-t border-text/8">
              <Composer bind={draft} label="Message" placeholder="Say something" onSubmit={() => draft.set('')} />
            </view>
          </view>
        }
        data-testid="live-split"
      >
        <view className="grow min-h-0 p-4 gap-3">
          <view className="grow min-h-0 rounded-control bg-media-canvas items-center justify-center">
            <Icon name="video" size="lg" className="text-on-media/60" />
            <view className="absolute start-3 top-3">
              <Badge label="Live" tone="danger" emphasis="solid" />
            </view>
          </view>
          <view className="flex-row items-center gap-1">
            <IconButton label={playing.value ? 'Pause' : 'Play'} icon={playing.value ? 'pause' : 'play'} variant="ghost" onClick={() => playing.set(!playing.value)} />
            <IconButton label={muted.value ? 'Unmute' : 'Mute'} icon={muted.value ? 'mute' : 'volume'} variant="ghost" onClick={() => muted.set(!muted.value)} />
            <view className="grow shrink min-w-0 ps-2">
              <text className="text-footnote font-semibold text-text truncate">Evening boats at the north pier</text>
              <text className="text-caption text-muted truncate">Harbour Lights · started 42 minutes ago</text>
            </view>
            <IconButton label={large.value ? 'Exit full screen' : 'Full screen'} icon={large.value ? 'collapse' : 'expand'} variant="ghost" onClick={() => large.set(!large.value)} />
          </view>
        </view>
      </Split>
    </view>
  );
}

const comments = [
  { name: 'Ana Lima', time: '09:12', text: 'Can the legend sit under the map on a phone? It covers the north loop.' },
  { name: 'Kai Chen', time: '10:40', text: 'Agree. And the closed paths need more than colour to tell them apart.' },
  { name: 'Mira Patel', time: 'Yesterday', text: 'The elevation numbers read well at this size.' },
];

/** A document and its comments: beside it on a wide screen, in a sheet the reader opens on a phone. */
export function DocumentSheetSplit() {
  const { env } = useView();
  const commentsOpen = useLocal('comments-open', false);
  const reply = useLocal('reply', '');
  const isCompact = env.sizeClass !== 'regular';
  return (
    <view className="h-[440px] overflow-hidden rounded-container border border-text/12">
      <Split
        compact="sheet"
        bind={commentsOpen}
        asideLabel="Comments"
        asideWidth={300}
        aside={
          <view className="px-4 pt-3 pb-4 gap-4">
            <text role="heading" headingLevel={2} className="text-callout font-semibold text-text">Comments</text>
            {comments.map(comment => (
              <view key={comment.name} className="flex-row gap-3">
                <Avatar name={comment.name} size="sm" isDecorative />
                <view className="grow shrink min-w-0 gap-0.5">
                  <view className="flex-row items-baseline justify-between gap-2">
                    <text className="text-footnote font-semibold text-text">{comment.name}</text>
                    <text className="text-caption text-muted">{comment.time}</text>
                  </view>
                  <text className="text-footnote text-text">{comment.text}</text>
                </view>
              </view>
            ))}
            <Composer bind={reply} label="Reply" placeholder="Add a comment" onSubmit={() => reply.set('')} data-testid="doc-reply" />
          </view>
        }
        data-testid="doc-split"
      >
        <scroll className="grow min-h-0">
          <view className="p-4 gap-3">
            <view className="gap-1">
              <text role="heading" headingLevel={2} className="text-title font-semibold tracking-tight text-text">Trail map redesign</text>
              <text className="text-caption text-muted">Draft · edited 2 hours ago</text>
            </view>
            {isCompact && (
              <Button
                label="Comments (3)"
                variant="secondary"
                width="content"
                startIcon="message"
                onClick={() => commentsOpen.set(true)}
                data-testid="doc-open-comments"
              />
            )}
            <text role="heading" headingLevel={3} className="text-callout font-semibold text-text">Goals</text>
            <text className="text-body text-text">Walkers should find the loop that suits the time they have, and see where a path is closed before they reach it.</text>
            <text role="heading" headingLevel={3} className="text-callout font-semibold text-text">What changes</text>
            <text className="text-body text-text">The map keeps the coast at the top. Each loop gets one colour and a pattern, its length in kilometres and its climb in metres beside the name.</text>
            <text className="text-body text-text">Closed paths are drawn dashed with a short reason, such as a fallen tree or a nesting season.</text>
          </view>
        </scroll>
      </Split>
    </view>
  );
}

/** Tile specimen showing a primary canvas beside its companion aside pane. */
export function Specimen() {
  return (
    <view className="w-full flex-row overflow-hidden rounded-control border border-text/12 bg-canvas">
      <view className="grow shrink basis-0 p-2.5 items-center justify-center bg-surface-sunken">
        <text className="text-caption font-semibold text-text">Canvas</text>
      </view>
      <view className="w-[76px] p-2 gap-1 border-s border-text/8">
        <text className="text-caption font-semibold text-text">Chat</text>
        <text className="text-caption text-muted">Live</text>
      </view>
    </view>
  );
}
