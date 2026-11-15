import { cx, useId, useLocal } from '@lingxia/view';
import { Glyph } from '../internal/Glyph';
import { tooltipRecipe as r } from './Tooltip.recipe';

export type TooltipPlacement = 'top' | 'bottom' | 'end' | 'start';
export type TooltipSize = 'sm' | 'md';

export interface TooltipProps {
  /** The info button's name: what the note is about ("About storage"). */
  label: string;
  /** The note: one or two short sentences. */
  content: string;
  /** Which side of the button the note opens on. @default 'top' */
  placement?: TooltipPlacement;
  /** @default 'md' */
  size?: TooltipSize;
  /** Controlled open state; pass `onOpenChange` with it. */
  isOpen?: boolean;
  onOpenChange?: (isOpen: boolean) => void;
  className?: string;
  /** On the info button; the note gets `<testid>-content`. */
  'data-testid'?: string;
}

const areas = { top: 'top', bottom: 'bottom', end: 'inline-end', start: 'inline-start' } as const satisfies Record<TooltipPlacement, string>;

/**
 * An info button that opens a short note about what is next to it. It opens on a tap, Enter or
 * Space, not on focus or hover; Escape or a tap elsewhere closes it.
 */
export function Tooltip({ label, content, placement = 'top', size = 'md', isOpen, onOpenChange, className, 'data-testid': testId }: TooltipProps) {
  const id = useId();
  const textId = `${id}-text`;
  const local = useLocal(false);
  const open = isOpen ?? local.value;
  const toggle = (next: boolean) => {
    local.set(next);
    onOpenChange?.(next);
  };
  return (
    <>
      <button
        popoverTarget={id}
        aria-label={label}
        aria-describedby={open ? textId : undefined}
        aria-expanded={open}
        data-state={open ? 'open' : 'closed'}
        data-testid={testId}
        className={cx(r.trigger.base, r.trigger.size[size], className)}
      >
        <Glyph name="info" className={r.icon.size[size]} />
      </button>
      <view
        id={id}
        popover
        role="tooltip"
        open={isOpen}
        onToggle={toggle}
        data-testid={testId && `${testId}-content`}
        style={{ positionArea: areas[placement] }}
        className={cx(r.bubble.base, r.bubble.offset[placement])}
      >
        <text id={textId} className={r.text.base}>{content}</text>
      </view>
    </>
  );
}
