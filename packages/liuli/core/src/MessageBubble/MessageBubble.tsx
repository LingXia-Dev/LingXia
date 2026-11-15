import { cx, type Child } from '@lingxia/view';
import { Glyph } from '../internal/Glyph';
import { messageBubbleRecipe as r } from './MessageBubble.recipe';

/** `end` is the user\'s own message; `start` is someone else\'s. */
export type MessageSide = 'start' | 'end';
export type MessageStatus = 'sending' | 'sent' | 'failed';

export interface MessageBubbleProps {
  /** The message. Long words and addresses break to fit. */
  text?: string;
  /** In place of `text`: a Markdown, a Thumbnail, a file row. */
  children?: Child;
  /** @default 'start' */
  side?: MessageSide;
  /** Who wrote it, shown over the first bubble of their run. */
  author?: string;
  /** Their Avatar, at the bubble\'s start. For the other bubbles of a run pass `hasAvatarSpace` to keep the edge. */
  avatar?: Child;
  /** Keeps the avatar\'s width empty. @default false */
  hasAvatarSpace?: boolean;
  /** A line under the bubble, as the app writes it: the time, "Sending", "Not sent". */
  meta?: string;
  /** `failed` adds an error mark to `meta` and an edge to the bubble; say what failed in `meta`. */
  status?: MessageStatus;
  /** Beside `meta`, for a failed message: a Button that sends it again. */
  action?: Child;
  /** Whether the same person\'s bubble comes right before this one: tightens the corner toward it. @default false */
  isJoinedBefore?: boolean;
  /** Whether the same person\'s bubble comes right after. @default false */
  isJoinedAfter?: boolean;
  className?: string;
  'data-testid'?: string;
}

/** One message of a conversation. */
export function MessageBubble({
  text, children, side = 'start', author, avatar, hasAvatarSpace = false, meta, status, action,
  isJoinedBefore = false, isJoinedAfter = false, className, 'data-testid': testId,
}: MessageBubbleProps) {
  const isFailed = status === 'failed';
  return (
    <view data-testid={testId} className={cx(r.root.base, r.root.side[side], className)}>
      {(avatar !== undefined || hasAvatarSpace) && <view className={r.avatar.base}>{avatar}</view>}
      <view className={cx(r.column.base, r.column.side[side])}>
        {author !== undefined && <text className={r.author.base}>{author}</text>}
        <view className={cx(r.bubble.base, r.bubble.side[side], isJoinedBefore && r.bubble.joinBefore[side], isJoinedAfter && r.bubble.joinAfter[side], isFailed && r.bubble.failed)}>
          {children ?? <text className={r.text.base}>{text}</text>}
        </view>
        {(meta !== undefined || action !== undefined) && (
          <view className={r.meta.base}>
            {isFailed && <Glyph name="error" className={r.statusIcon.base} />}
            {meta !== undefined && <text className={cx(r.metaText.base, isFailed && r.metaText.failed)}>{meta}</text>}
            {action}
          </view>
        )}
      </view>
    </view>
  );
}
