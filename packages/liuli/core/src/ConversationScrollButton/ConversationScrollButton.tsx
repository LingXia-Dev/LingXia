import { cx, useLocal, type ScrollEdgeEvent } from '@lingxia/view';
import { liuliText } from '../i18n/text';
import { Glyph } from '../internal/Glyph';
import { conversationScrollButtonRecipe as r } from './ConversationScrollButton.recipe';
import { dataOf } from '../internal/data';

/** A conversation's place in its List: pass `anchor` and `onScrollEdge` to the List, the whole to a ConversationScrollButton. */
export interface ConversationScroll {
  anchor: { index: number; align: 'end'; seq: number };
  onScrollEdge: (event: ScrollEdgeEvent) => void;
  /** The newest message is in view. */
  isAtEnd: boolean;
  /** Messages that arrived while the reader was further up. */
  unseen: number;
  /** Scrolls to the newest message. */
  jumpToLatest: () => void;
}

// Each jump moves the anchor's seq past every count a conversation reaches.
const jumpStep = 1_000_000;

/**
 * Keeps a conversation on its newest message as messages arrive while the reader is there, and holds
 * still while they read further up, counting what arrives meanwhile. `count` is the List's length.
 */
export function useConversationScroll(count: number): ConversationScroll {
  const atEnd = useLocal(true);
  const seen = useLocal(count);
  const jumps = useLocal(0);
  const isAtEnd = atEnd.value;
  return {
    // At the end every new message moves the anchor, so the List follows; further up it stays put, index and all.
    anchor: { index: Math.max(0, (isAtEnd ? count : seen.value) - 1), align: 'end', seq: jumps.value * jumpStep + (isAtEnd ? count : seen.value) },
    onScrollEdge: event => {
      if (event.edge !== 'end') return;
      atEnd.set(event.at);
      seen.set(count);
    },
    isAtEnd,
    unseen: isAtEnd ? 0 : Math.max(0, count - seen.value),
    jumpToLatest: () => { seen.set(count); jumps.set(jumps.value + 1); },
  };
}

export interface ConversationScrollButtonProps {
  /** The conversation's `useConversationScroll`. */
  scroll: ConversationScroll;
  /** The button's name. Defaults to "Jump to the latest message" in the page's language, with the count of new ones. */
  label?: string;
  className?: string;
  'data-testid'?: string;
}

/**
 * Shown while the reader is up the conversation: a button at the end corner over its bottom that goes
 * back to the newest message, with how many arrived meanwhile. Put it after the List in a box that holds both.
 */
export function ConversationScrollButton({ scroll, label, className, 'data-testid': testId, ...attributes }: ConversationScrollButtonProps) {
  if (scroll.isAtEnd) return null;
  const { unseen } = scroll;
  return (
    <button
      aria-label={label ?? liuliText('@liuli.conversation.latest', { count: unseen })}
      onClick={scroll.jumpToLatest}
      data-testid={testId} {...dataOf(attributes)}
      className={cx(r.root.base, className)}
    >
      <Glyph name="arrow-down" className={r.icon.base} />
      {unseen > 0 && <text aria-hidden className={r.count.base}>{unseen > 99 ? '99+' : unseen}</text>}
    </button>
  );
}
