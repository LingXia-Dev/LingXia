import { cx } from '@lingxia/view';
import { toggleButtonRecipe as r } from './ToggleButton.recipe';

export type ToggleButtonSize = 'sm' | 'md' | 'lg';

export interface ToggleButtonProps {
  /** The text and the accessible name; it stays the same on and off ("Unread", not "Show unread"/"Hide unread"). */
  label: string;
  /** Whether it is on. */
  isPressed: boolean;
  /** Runs with the new state on a press. */
  onPressedChange: (isPressed: boolean) => void;
  /** @default 'md' */
  size?: ToggleButtonSize;
  /** @default false */
  isDisabled?: boolean;
  className?: string;
  'data-testid'?: string;
}

/** A button that turns one option on or off and shows which: a filter, a view option. */
export function ToggleButton({ label, isPressed, onPressedChange, size = 'md', isDisabled = false, className, 'data-testid': testId }: ToggleButtonProps) {
  return (
    <button
      aria-label={label}
      aria-pressed={isPressed}
      data-state={isPressed ? 'on' : 'off'}
      disabled={isDisabled}
      data-testid={testId}
      onClick={() => onPressedChange(!isPressed)}
      className={cx(r.root.base, isPressed ? r.root.on : r.root.off, r.root.size[size], className)}
    >
      <text className={cx(r.label.base, r.label.size[size])}>{label}</text>
    </button>
  );
}
