import { cx, useContext } from '@lingxia/view';
import { LayerContext } from '../internal/layer';
import { toggleButtonRecipe as r } from './ToggleButton.recipe';
import { registerPressable, useTrigger } from '../internal/trigger';
import { dataOf } from '../internal/data';

export type ToggleButtonSize = 'sm' | 'md' | 'lg';

export interface ToggleButtonProps {
  /** The text and the accessible name; it stays the same on and off ("Unread", not "Show unread"/"Hide unread"). */
  label: string;
  /** Whether it is on. */
  value: boolean;
  /** Runs with the new state on a press. */
  onChange: (isPressed: boolean) => void;
  /** @default 'md' */
  size?: ToggleButtonSize;
  /** @default false */
  isDisabled?: boolean;
  className?: string;
  'data-testid'?: string;
}

/** A button that turns one option on or off and shows which: a filter, a view option. */
export function ToggleButton({ label, value: isPressed, onChange: onPressedChange, size = 'md', isDisabled = false, className, 'data-testid': testId, ...attributes }: ToggleButtonProps) {
  const trigger = useTrigger();
  const layer = useContext(LayerContext);
  return (
    <button
      aria-label={label}
      aria-pressed={isPressed}
      aria-describedby={trigger?.describedBy}
      popoverTarget={trigger?.popoverTarget}
      interestFor={trigger?.interestFor}
      data-state={isPressed ? 'on' : 'off'}
      disabled={isDisabled}
      data-testid={testId} {...dataOf(attributes)}
      onClick={() => onPressedChange(!isPressed)}
      className={cx(r.root.base, isPressed ? r.root.on : cx(r.root.off, layer === 'overlay' && r.root.overlay), r.root.size[size], className)}
    >
      <text className={cx(r.label.base, r.label.size[size])}>{label}</text>
    </button>
  );
}
registerPressable(ToggleButton);
