import type { MessageSide } from './MessageBubble';

export const messageBubbleParts = {
  root: 'view',
  avatar: 'view',
  column: 'view',
  author: 'text',
  bubble: 'view',
  text: 'text',
  meta: 'view',
  metaText: 'text',
  statusIcon: 'vector',
} as const;

/**
 * One message. Mine sits at the end edge in the accent's soft step; theirs at the start on a quiet
 * tint. A bubble takes up to four fifths of the row and never more than 520 px; the corner toward
 * the next bubble of a run is tight, so a run reads as one piece.
 */
export const messageBubbleRecipe = {
  root: {
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
    base: 'max-w-[520px] px-3 py-2 gap-2 rounded-2xl',
    side: { start: 'bg-text/6 text-text', end: 'bg-accent-soft text-on-accent-soft' } satisfies Record<MessageSide, string>,
    /** Tight on the side of its own edge, toward the bubble before and after it in a run. */
    joinBefore: { start: 'rounded-ss-md', end: 'rounded-se-md' } satisfies Record<MessageSide, string>,
    joinAfter: { start: 'rounded-es-md', end: 'rounded-ee-md' } satisfies Record<MessageSide, string>,
    failed: 'border border-danger',
  },
  /** Long words and addresses break rather than push the bubble past the row. */
  text: {
    base: 'text-body wrap-anywhere',
  },
  meta: {
    base: 'flex-row items-center gap-1 px-1',
  },
  metaText: {
    base: 'text-caption text-muted tabular-nums',
    failed: 'text-danger font-medium',
  },
  statusIcon: {
    base: 'size-3.5 text-danger',
  },
} as const;
