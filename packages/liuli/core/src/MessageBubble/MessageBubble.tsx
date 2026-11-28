import { cx, useLocal, type Child } from '@lingxia/view';
import { Button } from '../Button/Button';
import { liuliText } from '../i18n/text';
import { Glyph } from '../internal/Glyph';
import { SpinnerMark } from '../internal/spinner';
import { messageBubbleRecipe as r } from './MessageBubble.recipe';
import { dataOf } from '../internal/data';

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
  /** `failed` adds an error mark to `meta`; say what failed in `meta`. */
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
  isJoinedBefore = false, isJoinedAfter = false, className, 'data-testid': testId, ...attributes
}: MessageBubbleProps) {
  const isFailed = status === 'failed';
  const hasAvatar = avatar !== undefined || hasAvatarSpace;
  return (
    <view data-testid={testId} {...dataOf(attributes)} className={cx(r.root.base, r.root.side[side], className)}>
      <view className={cx(r.line.base, r.line.side[side])}>
        {hasAvatar && <view className={r.avatar.base}>{avatar}</view>}
        <view className={cx(r.column.base, r.column.side[side])}>
          {author !== undefined && <text className={r.author.base}>{author}</text>}
          <view className={cx(r.bubble.base, r.bubble.side[side], isJoinedBefore && r.bubble.joinBefore[side], isJoinedAfter && r.bubble.joinAfter[side])}>
            {children ?? <text className={r.text.base}>{text}</text>}
          </view>
        </view>
      </view>
      {(meta !== undefined || action !== undefined) && (
        <view className={cx(r.meta.base, hasAvatar && r.meta.afterAvatar)}>
          {isFailed && <Glyph name="error" className={r.statusIcon.base} />}
          {meta !== undefined && <text className={cx(r.metaText.base, isFailed && r.metaText.failed)}>{meta}</text>}
          {action}
        </view>
      )}
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
export function MessageReasoning({ label, text, isStreaming = false, hasAvatarSpace = false, isDefaultOpen = false, isOpen, onOpenChange, className, 'data-testid': testId, ...attributes }: MessageReasoningProps) {
  const local = useLocal(isDefaultOpen);
  const open = isOpen ?? local.value;
  const toggle = () => {
    if (isOpen === undefined) local.set(!open);
    onOpenChange?.(!open);
  };
  return (
    <view className={cx(r.reasoning.base, hasAvatarSpace && r.reasoning.afterAvatar, className)}>
      <button aria-label={label} aria-expanded={open} aria-busy={isStreaming || undefined} onClick={toggle} data-testid={testId} {...dataOf(attributes)} className={r.reasoningToggle.base}>
        {isStreaming && <SpinnerMark className={r.reasoningSpinner.base} />}
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

/** Where a tool run stands: `waiting` is held until the user allows or denies it; `denied` is one they refused. */
export type MessageToolCallStatus = 'running' | 'done' | 'failed' | 'denied' | 'waiting';

export interface MessageToolCallApproval {
  onAllow: () => void;
  onDeny: () => void;
  /** The allow button's words: "Delete files". Defaults to "Allow" in the page's language. */
  allowLabel?: string;
  /** The deny button's words. Defaults to "Deny" in the page's language. */
  denyLabel?: string;
  /** Allowing removes, spends or sends something that cannot be taken back: the allow button is drawn destructive. @default false */
  isDestructive?: boolean;
}

export type MessageToolCallState =
  | {
      status: 'waiting';
      /** The two answers, shown under the line with the details open while it waits. */
      approval: MessageToolCallApproval;
    }
  | { status: 'running' | 'done' | 'failed' | 'denied'; approval?: never };

export type MessageToolCallProps = MessageToolCallState & {
  /** What the tool does or did, in one line: "Searched the web for tide times", "npm test". */
  label: string;
  /** A short fact after the label: "2.4 s", "3 results". */
  meta?: string;
  /** The details, shown while open: its arguments and what it returned, as a CodeBlock, a Markdown or text. Without them the line does not open. */
  children?: Child;
  /** Keeps an avatar's width empty before it, so it lines up with the bubbles of a run. @default false */
  hasAvatarSpace?: boolean;
  /** Open on first render, uncontrolled. @default false */
  isDefaultOpen?: boolean;
  /** Controlled open state; pass `onOpenChange` with it. */
  isOpen?: boolean;
  onOpenChange?: (isOpen: boolean) => void;
  className?: string;
  /** On the line; the details get `<testid>-details`, the answers `<testid>-allow` and `<testid>-deny`. */
  'data-testid'?: string;
};

const glyphs = { done: 'success', failed: 'error', denied: 'blocked', waiting: 'shield' } as const;

/** One step an assistant took with a tool, folded to a quiet line with its state; one that waits on the user asks to be allowed. */
export function MessageToolCall(props: MessageToolCallProps) {
  const { label, status, approval, meta, children, hasAvatarSpace = false, isDefaultOpen = false, isOpen, onOpenChange, className, 'data-testid': testId } = props;
  const local = useLocal(isDefaultOpen);
  const isWaiting = status === 'waiting';
  const canOpen = children !== undefined && !isWaiting;
  const open = isWaiting || (children !== undefined && (isOpen ?? local.value));
  const toggle = () => {
    if (isOpen === undefined) local.set(!open);
    onOpenChange?.(!open);
  };
  const name = liuliText('@liuli.toolCall.state', { label, status, hasMeta: meta === undefined ? 'no' : 'yes', meta: meta ?? '' });
  // Read once, as the line's name: the words drawn on it are hidden from assistive tech.
  const line = (
    <>
      {status === 'running'
        ? <SpinnerMark className={r.reasoningSpinner.base} />
        : <Glyph name={glyphs[status]} className={cx(r.toolMark.base, r.toolMark.status[status])} />}
      <text aria-hidden className={cx(r.toolLabel.base, isWaiting && r.toolLabel.waiting)}>{label}</text>
      {status === 'failed' && <text aria-hidden className={r.toolWord.failed}>{liuliText('@liuli.toolCall.failed')}</text>}
      {status === 'denied' && <text aria-hidden className={r.toolWord.denied}>{liuliText('@liuli.toolCall.denied')}</text>}
      {meta !== undefined && <text aria-hidden className={r.toolMeta.base}>{meta}</text>}
    </>
  );
  return (
    <view className={cx(r.tool.base, hasAvatarSpace && r.tool.afterAvatar, className)}>
      {canOpen
        ? (
          <button aria-label={name} aria-expanded={open} aria-busy={status === 'running' || undefined} onClick={toggle} data-testid={testId} {...dataOf(props)} className={cx(r.toolLine.base, r.toolLine.toggle)}>
            {line}
            <Glyph name="chevron-down" className={cx(r.reasoningChevron.base, open && r.reasoningChevron.open)} />
          </button>
        )
        : <view role="group" aria-label={name} aria-busy={status === 'running' || undefined} data-testid={testId} {...dataOf(props)} className={r.toolLine.base}>{line}</view>}
      {children !== undefined && (
        // Kept in the tree so the box can close around it; closed, it is clipped to nothing and hidden.
        <view aria-hidden={!open || undefined} className={cx(r.reasoningBox.base, !open && r.reasoningBox.closed)}>
          <view data-testid={testId && `${testId}-details`} className={r.toolDetails.base}>{children}</view>
        </view>
      )}
      {approval !== undefined && (
        <view className={r.toolAnswers.base}>
          <Button size="sm" variant="secondary" label={approval.denyLabel ?? liuliText('@liuli.toolCall.deny')} onClick={approval.onDeny} data-testid={testId && `${testId}-deny`} />
          <Button size="sm" variant={approval.isDestructive ? 'destructive' : 'primary'} label={approval.allowLabel ?? liuliText('@liuli.toolCall.allow')} onClick={approval.onAllow} data-testid={testId && `${testId}-allow`} />
        </view>
      )}
    </view>
  );
}
