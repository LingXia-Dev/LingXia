import { cx, useLocal, type Child } from '@lingxia/view';
import { Glyph } from '../internal/Glyph';
import { spinner, spinnerBox } from '../internal/spinner';
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

export interface MessageReasoningProps {
  /** The one line that is always shown, and the toggle's name: "Thought for 8 seconds". */
  label: string;
  /** The reasoning itself, shown while open. */
  text: string;
  /** Still being written: a spinner sits before the label and the toggle reports busy. @default false */
  isStreaming?: boolean;
  /** Keeps an avatar's width empty before it, so it lines up with the bubbles of a run. @default false */
  hasAvatarSpace?: boolean;
  /** Open on first render, uncontrolled. @default false */
  isDefaultOpen?: boolean;
  /** Controlled open state; pass `onOpenChange` with it. */
  isOpen?: boolean;
  onOpenChange?: (isOpen: boolean) => void;
  className?: string;
  /** On the toggle; the text gets `<testid>-text`. */
  'data-testid'?: string;
}

/** How an assistant got to its answer, folded to one quiet line over the message it belongs to. */
export function MessageReasoning({ label, text, isStreaming = false, hasAvatarSpace = false, isDefaultOpen = false, isOpen, onOpenChange, className, 'data-testid': testId }: MessageReasoningProps) {
  const local = useLocal(isDefaultOpen);
  const open = isOpen ?? local.value;
  const toggle = () => {
    if (isOpen === undefined) local.set(!open);
    onOpenChange?.(!open);
  };
  return (
    <view className={cx(r.reasoning.base, hasAvatarSpace && r.reasoning.afterAvatar, className)}>
      <button aria-label={label} aria-expanded={open} aria-busy={isStreaming || undefined} onClick={toggle} data-testid={testId} className={r.reasoningToggle.base}>
        {isStreaming && <vector aria-hidden paths={spinner} viewBox={spinnerBox} className={r.reasoningSpinner.base} />}
        <text className={r.reasoningLabel.base}>{label}</text>
        <Glyph name="chevron-down" className={cx(r.reasoningChevron.base, open && r.reasoningChevron.open)} />
      </button>
      {/* Kept in the tree so the box can close around it; closed, it is clipped to nothing and hidden. */}
      <view aria-hidden={!open || undefined} className={cx(r.reasoningBox.base, !open && r.reasoningBox.closed)}>
        <text data-testid={testId && `${testId}-text`} className={r.reasoningText.base}>{text}</text>
      </view>
    </view>
  );
}
