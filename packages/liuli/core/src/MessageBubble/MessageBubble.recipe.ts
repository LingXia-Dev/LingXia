import type { MessageSide } from './MessageBubble';

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
  reasoningSpinner: 'vector',
  reasoningLabel: 'text',
  reasoningChevron: 'vector',
  reasoningBox: 'view',
  reasoningText: 'text',
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
    base: 'max-w-[520px] px-3 py-2 gap-2 rounded-container',
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
    base: 'self-start max-w-[520px]',
    /** The avatar's width (32) and the gap after it (8). */
    afterAvatar: 'ms-10',
  },
  reasoningToggle: {
    base: 'flex-row items-center gap-1.5 self-start min-h-8 -mx-2 px-2 rounded-control text-muted transition-colors duration-fast ease-standard hover:bg-text/4 active:bg-text/7 focus-visible:bg-text/7',
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
    base: 'mt-1 mb-2 ps-3 border-s-2 border-text/12 text-footnote text-muted break-words select-text',
  },
} as const;
