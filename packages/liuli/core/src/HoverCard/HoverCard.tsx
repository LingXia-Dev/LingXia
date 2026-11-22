import { cx, useId, type Child } from '@lingxia/view';
import { LayerContext } from '../internal/layer';
import { Trigger } from '../internal/trigger';
import { hoverCardRecipe as r } from './HoverCard.recipe';
import { dataOf } from '../internal/data';

export type HoverCardPlacement = 'bottom' | 'top' | 'end' | 'start';

export interface HoverCardProps {
  /** The trigger: the words the card is about, as a `<text>`, or a Button. Text is wrapped in a plain button named by its words. */
  children: Child;
  /** The card: a few lines about what the trigger names. Nothing that must be reached to finish a task. */
  content: Child;
  /** The card's accessible name: "About Ana Lima". */
  label: string;
  /** Which side of the trigger the card opens on. Beside the trigger it flips for room. @default 'bottom' */
  placement?: HoverCardPlacement;
  /** On a wrapped trigger; the card gets `<testid>-card`. */
  'data-testid'?: string;
}

const areas = { bottom: 'bottom span-inline-end', top: 'top span-inline-end', end: 'inline-end', start: 'inline-start' } as const satisfies Record<HoverCardPlacement, string>;

/**
 * A preview of what a word names, shown while a pointer rests on it or keyboard focus is on it,
 * and on a tap where there is no pointer. It goes away by itself.
 */
export function HoverCard({ children, content, label, placement = 'bottom', 'data-testid': testId, ...attributes }: HoverCardProps) {
  const id = useId();
  return (
    <>
      <Trigger wiring={{ interestFor: id, popoverTarget: id }} className={r.trigger.base} data-testid={testId} {...dataOf(attributes)}>{children}</Trigger>
      <LayerContext.Provider value="overlay">
        <view
          id={id}
          popover="hint"
          role="dialog"
          aria-label={label}
          data-testid={testId && `${testId}-card`}
          style={{ positionArea: areas[placement] }}
          className={cx(r.card.base, r.card.offset[placement])}
        >
          {content}
        </view>
      </LayerContext.Provider>
    </>
  );
}
