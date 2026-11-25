import { cx, type Local } from '@lingxia/view';
import type { ToggleButtonLook, ToggleButtonSize } from '../ToggleButton/ToggleButton';
import { useToggleFace } from '../internal/toggleFace';
import { toggleButtonGroupRecipe as r } from './ToggleButtonGroup.recipe';
import { dataOf } from '../internal/data';

export type ToggleButtonGroupItem = ToggleButtonLook & {
  /** What `value` and `onChange` carry. */
  value: string;
  /** The toggle's text and accessible name; with `isLabelHidden`, its name alone. */
  label: string;
  /** @default false */
  isDisabled?: boolean;
};

export type ToggleButtonGroupSelection =
  | {
      /** One on at a time, read as a radio group: text alignment, a pen. */
      selectionMode: 'single';
      /** A View-local cell the group reads and writes; use it or `value` with `onChange`. */
      bind: Local<string>;
      value?: never;
      onChange?: never;
    }
  | {
      selectionMode: 'single';
      bind?: never;
      /** The value that is on. */
      value: string;
      /** Runs with the toggle pressed or arrowed onto. */
      onChange: (value: string) => void;
    }
  | {
      /** Each on or off on its own, read as toggle buttons: bold, italic, underline. */
      selectionMode: 'multiple';
      bind: Local<readonly string[]>;
      value?: never;
      onChange?: never;
    }
  | {
      selectionMode: 'multiple';
      bind?: never;
      /** The values that are on. */
      value: readonly string[];
      /** Runs with the values on after a press, in the items' order. */
      onChange: (value: readonly string[]) => void;
    };

export type ToggleButtonGroupProps = ToggleButtonGroupSelection & {
  /** Names the group for assistive tech: "Text style", "Alignment". */
  label: string;
  /** The toggles, in order. */
  items: readonly ToggleButtonGroupItem[];
  /** @default 'md' */
  size?: ToggleButtonSize;
  /** Disables every toggle. @default false */
  isDisabled?: boolean;
  className?: string;
  /** On the group; each toggle gets `<testid>-<value>`. */
  'data-testid'?: string;
};

/**
 * Toggles that belong together, as one keyboard stop: the arrows move between them. With one on at a
 * time it is a radio group and choosing follows the arrows; with any number on, a toolbar of toggle buttons.
 */
export function ToggleButtonGroup(props: ToggleButtonGroupProps) {
  const { label, items, size = 'md', isDisabled = false, className, 'data-testid': testId } = props;
  const isSingle = props.selectionMode === 'single';
  const on: readonly string[] = props.selectionMode === 'single'
    ? [props.bind ? props.bind.value : props.value]
    : props.bind ? props.bind.value : props.value;
  const press = (value: string) => {
    if (props.selectionMode === 'single') {
      if (value === on[0]) return;
      if (props.bind) props.bind.set(value);
      else props.onChange(value);
      return;
    }
    const next = items.map(item => item.value).filter(each => (each === value) !== on.includes(each));
    if (props.bind) props.bind.set(next);
    else props.onChange(next);
  };
  return (
    <view role={isSingle ? 'radiogroup' : 'toolbar'} aria-label={label} data-testid={testId} {...dataOf(props)} className={cx(r.root.base, className)}>
      {items.map(item => (
        <Toggle key={item.value} item={item} size={size} isOn={on.includes(item.value)} isSingle={isSingle} isDisabled={isDisabled || item.isDisabled === true} onPress={() => press(item.value)} testId={testId && `${testId}-${item.value}`} />
      ))}
    </view>
  );
}

function Toggle({ item, size, isOn, isSingle, isDisabled, onPress, testId }: { item: ToggleButtonGroupItem; size: ToggleButtonSize; isOn: boolean; isSingle: boolean; isDisabled: boolean; onPress: () => void; testId: string | undefined }) {
  const face = useToggleFace({ label: item.label, size, isOn, icon: item.icon, pressedIcon: item.pressedIcon, isLabelHidden: item.isLabelHidden });
  return (
    <button
      role={isSingle ? 'radio' : undefined}
      aria-checked={isSingle ? isOn : undefined}
      aria-pressed={isSingle ? undefined : isOn}
      aria-label={item.label}
      data-state={isOn ? 'on' : 'off'}
      disabled={isDisabled}
      data-testid={testId}
      onClick={onPress}
      style={face.style}
      className={face.className}
    >
      {face.content}
    </button>
  );
}
