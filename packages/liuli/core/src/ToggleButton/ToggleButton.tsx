import { cx, useContext, type Local } from '@lingxia/view';
import { LayerContext } from '../internal/layer';
import { toggleButtonRecipe as r } from './ToggleButton.recipe';
import { registerPressable, identityOf, useTrigger } from '../internal/trigger';

export type ToggleButtonSize = 'sm' | 'md' | 'lg';

export type ToggleButtonBinding =
  | {
      /** A View-local cell the control reads and writes; use it or `value` with `onChange`. */
      bind: Local<boolean>;
      value?: never;
      onChange?: never;
    }
  | {
      bind?: never;
      /** Whether it is on. */
      value: boolean;
      /** Runs with the new state on a press. */
      onChange: (isPressed: boolean) => void;
    };

export type ToggleButtonProps = ToggleButtonBinding & {
  /** The text and the accessible name; it stays the same on and off ("Unread", not "Show unread"/"Hide unread"). */
  label: string;
  /** @default 'md' */
  size?: ToggleButtonSize;
  /** @default false */
  isDisabled?: boolean;
  className?: string;
  'data-testid'?: string;
};

/** A button that turns one option on or off and shows which: a filter, a view option. */
export function ToggleButton({ label, bind, value: own, onChange: ownChange, size = 'md', isDisabled = false, className, 'data-testid': testId, ...attributes }: ToggleButtonProps) {
  const isPressed = bind ? bind.value : own;
  const onPressedChange = (next: boolean) => bind ? bind.set(next) : ownChange?.(next);
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
      {...identityOf(testId, attributes, trigger)}
      onClick={() => onPressedChange(!isPressed)}
      className={cx(r.root.base, isPressed ? r.root.on : cx(r.root.off, layer === 'overlay' && r.root.overlay), r.root.size[size], className)}
    >
      <text className={cx(r.label.base, r.label.size[size])}>{label}</text>
    </button>
  );
}
registerPressable(ToggleButton);
