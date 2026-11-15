import { cx, useId, type Child } from '@lingxia/view';
import type { ButtonSize, ButtonVariant } from '../Button/Button';
import { buttonRecipe as b } from '../Button/Button.recipe';
import { LayerContext } from '../internal/layer';
import { popoverRecipe as r } from './Popover.recipe';

/** `end` and `start` open beside the trigger, where there is room at its side: a toolbar down an edge, a row in a sidebar. */
export type PopoverPlacement = 'bottom' | 'top' | 'end' | 'start';
export type PopoverAlignment = 'start' | 'center' | 'end';

export interface PopoverProps {
  /** The trigger's text and accessible name; the panel's name too, unless `title` is set. */
  label: string;
  /** A heading at the top of the panel, and its accessible name. */
  title?: string;
  /** The panel's content: a few lines, a short form, a handful of controls. */
  children: Child;
  /** Draws only `icon` on a square trigger; `label` stays the name. @default false */
  isLabelHidden?: boolean;
  /** An icon on the trigger, before the label. */
  icon?: Child;
  /** The trigger's look, from Button. @default 'secondary' */
  variant?: ButtonVariant;
  /** @default 'md' */
  size?: ButtonSize;
  /** Which side of the trigger the panel opens on. Beside the trigger it flips to the other side for room, and falls back to below or above. @default 'bottom' */
  placement?: PopoverPlacement;
  /** Which edge of the trigger the panel lines up with: for `end` and `start`, `start` is the top of the trigger and `end` its bottom. @default 'start' */
  alignment?: PopoverAlignment;
  /** Controls whether it is open; leave it out to let the trigger own it. */
  isOpen?: boolean;
  onOpenChange?: (isOpen: boolean) => void;
  /** @default false */
  isDisabled?: boolean;
  /** Classes for the trigger. */
  className?: string;
  /** Classes for the panel. */
  panelClassName?: string;
  /** On the trigger; the panel gets `<testid>-panel`. */
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
 * A button that opens a panel of content beside it, above or below. A tap outside, Escape or back
 * closes it, and focus goes back to the button. Not modal: the page stays live.
 */
export function Popover({
  label,
  title,
  children,
  isLabelHidden = false,
  icon,
  variant = 'secondary',
  size = 'md',
  placement = 'bottom',
  alignment = 'start',
  isOpen,
  onOpenChange,
  isDisabled = false,
  className,
  panelClassName,
  'data-testid': testId,
}: PopoverProps) {
  const id = useId();
  return (
    <>
      <button
        popoverTarget={id}
        aria-label={label}
        disabled={isDisabled}
        data-testid={testId}
        className={cx(b.root.base, b.root.variant[variant], b.root.disabled[variant], isLabelHidden ? b.root.square[size] : b.root.size[size], className)}
      >
        {icon !== undefined && <view aria-hidden className={b.icon.base}>{icon}</view>}
        {!isLabelHidden && <text className={cx(b.label.base, b.label.weight[variant], b.label.size[size])}>{label}</text>}
      </button>
      <LayerContext.Provider value="overlay">
        <view
          id={id}
          popover
          aria-label={title ?? label}
          open={isOpen}
          onToggle={onOpenChange}
          data-testid={testId && `${testId}-panel`}
          style={{ positionArea: areas[placement][alignment] }}
          className={cx(r.panel.base, r.panel.offset[placement], panelClassName)}
        >
          {title !== undefined && <text role="heading" headingLevel={2} className={r.title.base}>{title}</text>}
          {children}
        </view>
      </LayerContext.Provider>
    </>
  );
}
