import { cx, useId, type Child } from '@lingxia/view';
import { LayerContext } from '../internal/layer';
import { hoverCardRecipe as r } from './HoverCard.recipe';

export type HoverCardPlacement = 'bottom' | 'top' | 'end' | 'start';

export interface HoverCardProps {
  /** The trigger\'s words and accessible name: a person, a term, a reference. */
  label: string;
  /** The card: a few lines about what the trigger names. Nothing that must be reached to finish a task. */
  children: Child;
  /** Names the card for assistive tech when the trigger\'s words do not: "About Ana Lima". */
  cardLabel?: string;
  /** Which side of the trigger the card opens on. Beside the trigger it flips for room. @default 'bottom' */
  placement?: HoverCardPlacement;
  /** Classes for the trigger\'s text, to match the text around it. */
  className?: string;
  /** On the trigger; the card gets `<testid>-card`. */
  'data-testid'?: string;
}

const areas = { bottom: 'bottom span-inline-end', top: 'top span-inline-end', end: 'inline-end', start: 'inline-start' } as const satisfies Record<HoverCardPlacement, string>;

/**
 * A preview of what a word names, shown while a pointer rests on it or keyboard focus is on it,
 * and on a tap where there is no pointer. It goes away by itself.
 */
export function HoverCard({ label, children, cardLabel, placement = 'bottom', className, 'data-testid': testId }: HoverCardProps) {
  const id = useId();
  return (
    <>
      <button interestFor={id} popoverTarget={id} aria-label={label} data-testid={testId} className={r.trigger.base}>
        <text className={cx(r.label.base, className)}>{label}</text>
      </button>
      <LayerContext.Provider value="overlay">
        <view
          id={id}
          popover="hint"
          aria-label={cardLabel ?? label}
          data-testid={testId && `${testId}-card`}
          style={{ positionArea: areas[placement] }}
          className={cx(r.card.base, r.card.offset[placement])}
        >
          {children}
        </view>
      </LayerContext.Provider>
    </>
  );
}
