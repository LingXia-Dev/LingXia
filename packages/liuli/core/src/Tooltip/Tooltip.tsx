import { cx, useId, useLocal, type Child } from '@lingxia/view';
import { Trigger } from '../internal/trigger';
import { tooltipRecipe as r } from './Tooltip.recipe';
import { dataOf } from '../internal/data';

export type TooltipPlacement = 'top' | 'bottom' | 'end' | 'start';

export interface TooltipProps {
  /** The trigger, usually an IconButton; any other element is wrapped in a plain button. */
  children: Child;
  /** The hint: the trigger's own name in words, a few at most. It never says what the trigger's label lacks. */
  content: string;
  /** Which side of the trigger the hint opens on. @default 'top' */
  placement?: TooltipPlacement;
  /** On the trigger: the wrapper, or a Button or Link child without a test id of its own. The hint gets `<testid>-content`. */
  'data-testid'?: string;
}

const areas = { top: 'top', bottom: 'bottom', end: 'inline-end', start: 'inline-start' } as const satisfies Record<TooltipPlacement, string>;

/**
 * A short hint shown while a pointer rests on its trigger or keyboard focus is on it, and on a
 * long press on a phone; never on a tap. It goes away by itself and describes the trigger while shown.
 */
export function Tooltip({ children, content, placement = 'top', 'data-testid': testId, ...attributes }: TooltipProps) {
  const id = useId();
  const textId = `${id}-text`;
  const open = useLocal(false);
  return (
    <>
      <Trigger wiring={{ interestFor: id, describedBy: open.value ? textId : undefined }} data-testid={testId} {...dataOf(attributes)}>{children}</Trigger>
      <view
        id={id}
        popover="hint"
        role="tooltip"
        onToggle={open.set}
        data-testid={testId && `${testId}-content`}
        style={{ positionArea: areas[placement] }}
        className={cx(r.bubble.base, r.bubble.offset[placement])}
      >
        <text id={textId} className={r.text.base}>{content}</text>
      </view>
    </>
  );
}
