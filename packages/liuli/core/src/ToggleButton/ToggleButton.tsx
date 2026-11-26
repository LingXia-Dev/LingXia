import { cx, type Local } from '@lingxia/view';
import type { IconRef } from '../Icon/Icon';
import { useToggleFace } from '../internal/toggleFace';
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

/** What a toggle shows beside or instead of its label. */
export type ToggleButtonLook =
  | {
      /** An icon by name from the set, or a definition, before the label. */
      icon?: IconRef;
      /** The icon while on, where the icon itself shows the state: `volume` and `mute`. The fill and edge still show it too. */
      pressedIcon?: IconRef;
      isLabelHidden?: false;
    }
  | {
      icon: IconRef;
      pressedIcon?: IconRef;
      /** Shows the icon alone, in a square, for a toolbar; the label still names it. @default false */
      isLabelHidden: true;
    };

export type ToggleButtonProps = ToggleButtonBinding & ToggleButtonLook & {
  /** The text and the accessible name; it stays the same on and off ("Unread", not "Show unread"/"Hide unread"). */
  label: string;
  /** @default 'md' */
  size?: ToggleButtonSize;
  /** `ghost` for a toolbar beside icon buttons: no edge, the fill alone shows it on. @default 'secondary' */
  variant?: 'secondary' | 'ghost';
  /** A press leaves the focused editor editing, its caret, selection and keyboard kept: for a format bar. @default false */
  isFocusKept?: boolean;
  /** @default false */
  isDisabled?: boolean;
  className?: string;
  'data-testid'?: string;
};

/** A button that turns one option on or off and shows which: a filter, a view option, a toolbar's bold. */
export function ToggleButton({ label, bind, value: own, onChange: ownChange, size = 'md', variant, isFocusKept, isDisabled = false, icon, pressedIcon, isLabelHidden, className, 'data-testid': testId, ...attributes }: ToggleButtonProps) {
  const isPressed = bind ? bind.value : own;
  const onPressedChange = (next: boolean) => bind ? bind.set(next) : ownChange?.(next);
  const trigger = useTrigger();
  const face = useToggleFace({ label, size, variant, isOn: isPressed, icon, pressedIcon, isLabelHidden });
  return (
    <button
      aria-label={label}
      aria-pressed={isPressed}
      aria-describedby={trigger?.describedBy}
      popoverTarget={trigger?.popoverTarget}
      interestFor={trigger?.interestFor}
      data-state={isPressed ? 'on' : 'off'}
      disabled={isDisabled}
      keepsFocus={isFocusKept || undefined}
      {...identityOf(testId, attributes, trigger)}
      onClick={() => onPressedChange(!isPressed)}
      style={face.style}
      className={cx(face.className, className)}
    >
      {face.content}
    </button>
  );
}
registerPressable(ToggleButton);
