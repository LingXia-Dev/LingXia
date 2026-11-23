import { cx, useId, type Child } from '@lingxia/view';
import { LayerContext } from '../internal/layer';
import { Trigger } from '../internal/trigger';
import { popoverRecipe as r } from './Popover.recipe';
import { dataOf } from '../internal/data';

/** `end` and `start` open beside the trigger, where there is room at its side: a toolbar down an edge, a row in a sidebar. */
export type PopoverPlacement = 'bottom' | 'top' | 'end' | 'start';
export type PopoverAlignment = 'start' | 'center' | 'end';

export interface PopoverProps {
  /** The trigger: a Button, an IconButton or any element, which is then wrapped in a plain button. */
  children: Child;
  /** The panel's content: a few lines, a short form, a handful of controls. */
  content: Child;
  /** The panel's accessible name, unless `title` is set. */
  label: string;
  /** A heading at the top of the panel, and its accessible name. */
  title?: string;
  /** Which side of the trigger the panel opens on. Beside the trigger it flips to the other side for room, and falls back to below or above. @default 'bottom' */
  placement?: PopoverPlacement;
  /** Which edge of the trigger the panel lines up with: for `end` and `start`, `start` is the top of the trigger and `end` its bottom. @default 'start' */
  alignment?: PopoverAlignment;
  /** Controls whether it is open; leave it out to let the trigger own it. */
  isOpen?: boolean;
  onOpenChange?: (isOpen: boolean) => void;
  /** Classes for the panel. */
  panelClassName?: string;
  /** On the trigger (the wrapper, or a Button or Link child without a test id of its own), and on the panel as `<testid>-panel`. */
  'data-testid'?: string;
}

// Logical areas, so start and end mirror in right-to-left.
const areas = {
  bottom: { start: 'bottom span-inline-end', center: 'bottom', end: 'bottom span-inline-start' },
  top: { start: 'top span-inline-end', center: 'top', end: 'top span-inline-start' },
  end: { start: 'inline-end span-block-end', center: 'inline-end', end: 'inline-end span-block-start' },
  start: { start: 'inline-start span-block-end', center: 'inline-start', end: 'inline-start span-block-start' },
} as const satisfies Record<PopoverPlacement, Record<PopoverAlignment, string>>;

/**
 * A panel of content that opens beside its trigger, above or below. A tap outside, Escape or back
 * closes it, and focus goes back to the trigger. Not modal: the page stays live.
 */
export function Popover({
  children,
  content,
  label,
  title,
  placement = 'bottom',
  alignment = 'start',
  isOpen,
  onOpenChange,
  panelClassName,
  'data-testid': testId, ...attributes
}: PopoverProps) {
  const id = useId();
  return (
    <>
      <Trigger wiring={{ popoverTarget: id }} data-testid={testId} {...dataOf(attributes)}>{children}</Trigger>
      <LayerContext.Provider value="overlay">
        <view
          id={id}
          popover
          role="dialog"
          aria-label={title ?? label}
          open={isOpen}
          onToggle={onOpenChange}
          data-testid={testId && `${testId}-panel`}
          style={{ positionArea: areas[placement][alignment] }}
          className={cx(r.panel.base, r.panel.offset[placement], panelClassName)}
        >
          {title !== undefined && <text role="heading" headingLevel={2} className={r.title.base}>{title}</text>}
          {content}
        </view>
      </LayerContext.Provider>
    </>
  );
}
