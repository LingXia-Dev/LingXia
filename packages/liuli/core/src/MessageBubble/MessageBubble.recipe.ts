import type { MessageSide, MessageToolCallStatus } from './MessageBubble';

export const messageBubbleParts = {
  root: 'view',
  line: 'view',
  avatar: 'view',
  column: 'view',
  author: 'text',
  bubble: 'view',
  text: 'text',
  meta: 'view',
  metaText: 'text',
  statusIcon: 'vector',
  reasoning: 'view',
  reasoningToggle: 'button',
  reasoningSpinner: 'view',
  reasoningLabel: 'text',
  reasoningChevron: 'vector',
  reasoningBox: 'view',
  reasoningText: 'text',
  tool: 'view',
  toolLine: 'button',
  toolMark: 'vector',
  toolLabel: 'text',
  toolWord: 'text',
  toolMeta: 'text',
  toolDetails: 'view',
  toolAnswers: 'view',
} as const;

/**
 * One message. Mine sits at the end edge in the accent's soft step; theirs at the start on a quiet
 * tint. A bubble takes up to four fifths of the row and never more than 520 px; the corner toward
 * the next bubble of a run is tight, so a run reads as one piece.
 */
export const messageBubbleRecipe = {
  root: {
    base: 'self-stretch gap-1',
    side: { start: 'items-start', end: 'items-end' } satisfies Record<MessageSide, string>,
  },
  /** The avatar and the bubble: the avatar sits level with the bubble's last line, not the meta under it. */
  line: {
    base: 'self-stretch flex-row items-end gap-2',
    side: { start: 'justify-start', end: 'justify-end' } satisfies Record<MessageSide, string>,
  },
  /** Holds the avatar's width for every bubble of a run, so their edges line up. */
  avatar: {
    base: 'w-8 shrink-0',
  },
  column: {
    base: 'shrink min-w-0 max-w-[80%] gap-1',
    side: { start: 'items-start', end: 'items-end' } satisfies Record<MessageSide, string>,
  },
  author: {
    base: 'ps-3 text-caption font-medium text-muted',
  },
  bubble: {
    base: 'max-w-130 px-3 py-2 gap-2 rounded-container',
    side: { start: 'bg-text/6 text-text', end: 'bg-accent-soft text-on-accent-soft' } satisfies Record<MessageSide, string>,
    /** Tight on the side of its own edge, toward the bubble before and after it in a run. */
    joinBefore: { start: 'rounded-ss-checkbox', end: 'rounded-se-checkbox' } satisfies Record<MessageSide, string>,
    joinAfter: { start: 'rounded-es-checkbox', end: 'rounded-ee-checkbox' } satisfies Record<MessageSide, string>,
  },
  /** Long words and addresses break rather than push the bubble past the row. */
  text: {
    base: 'text-body wrap-anywhere',
  },
  meta: {
    base: 'flex-row items-center gap-1 px-1',
    /** Under the bubble, past the avatar's width (32) and the gap after it (8). */
    afterAvatar: 'ms-10',
  },
  metaText: {
    base: 'text-caption text-muted tabular-nums',
    failed: 'text-danger font-medium',
  },
  statusIcon: {
    base: 'size-3.5 text-danger',
  },
  /** MessageReasoning: one quiet line that opens the text under it, behind a rule at the start. */
  reasoning: {
    base: 'self-start max-w-130',
    /** The avatar's width (32) and the gap after it (8). */
    afterAvatar: 'ms-10',
  },
  reasoningToggle: {
    base: 'flex-row items-center gap-1.5 self-start min-h-8 -mx-2 px-2 rounded-control text-muted transition-colors duration-fast ease-standard hover:bg-text/4 active:bg-text/7 focus-visible:ring-2 focus-visible:ring-focus focus-visible:ring-offset-2',
  },
  reasoningSpinner: {
    base: 'size-3.5 shrink-0 motion-safe:animate-spin',
  },
  reasoningLabel: {
    base: 'text-footnote',
  },
  reasoningChevron: {
    base: 'size-4 shrink-0 transition-transform duration-base ease-standard',
    open: '-rotate-180',
  },
  reasoningBox: {
    base: 'overflow-hidden transition-[height] duration-base ease-standard motion-reduce:transition-none',
    closed: 'h-0',
  },
  reasoningText: {
    base: 'mt-1 mb-2 ps-3 border-s-2 border-line text-footnote text-muted break-words select-text',
  },
  /** MessageToolCall: the reasoning's quiet line, with a mark for where the run stands; its details open behind the same rule. */
  tool: {
    // The column's width less the avatar space, so a long step ends in an ellipsis rather than past the edge.
    base: 'self-stretch max-w-130',
    /** The avatar's width (32) and the gap after it (8). */
    afterAvatar: 'ms-10',
  },
  toolLine: {
    base: 'flex-row items-center gap-1.5 self-start max-w-full min-h-8 -mx-2 px-2 text-muted',
    toggle: 'rounded-control transition-colors duration-fast ease-standard hover:bg-text/4 active:bg-text/7 focus-visible:ring-2 focus-visible:ring-focus focus-visible:ring-offset-2',
  },
  /** Done and denied stay quiet; a failure is in the danger colour; a wait is in the accent. */
  toolMark: {
    base: 'size-3.5 shrink-0',
    status: { done: 'text-muted', failed: 'text-danger', denied: 'text-muted', waiting: 'text-accent' } satisfies Record<Exclude<MessageToolCallStatus, 'running'>, string>,
  },
  toolLabel: {
    base: 'shrink min-w-0 truncate text-footnote',
    waiting: 'text-text font-medium',
  },
  /** Where it stands in words, after the label: a failure or a refusal is never told by the mark alone. */
  toolWord: {
    failed: 'shrink-0 text-footnote font-medium text-danger',
    denied: 'shrink-0 text-footnote font-medium',
  },
  toolMeta: {
    base: 'shrink-0 text-caption tabular-nums',
  },
  toolDetails: {
    base: 'mt-1 mb-2 ps-3 border-s-2 border-line gap-2 text-footnote text-muted',
  },
  toolAnswers: {
    base: 'flex-row flex-wrap gap-2 mt-1 mb-2 ps-3.5',
  },
} as const;
