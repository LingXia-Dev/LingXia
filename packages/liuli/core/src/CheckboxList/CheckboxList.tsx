import { cx, useId, type Local } from '@lingxia/view';
import { useControlSize } from '../density';
import { CheckboxInput } from '../CheckboxInput/CheckboxInput';
import type { FieldSize, FieldStatus } from '../Field/Field';
import { fieldRecipe } from '../Field/Field.recipe';
import { describedBy } from '../internal/describedBy';
import { hasMessage, StatusRegion } from '../internal/StatusRegion';
import { checkboxListRecipe as r } from './CheckboxList.recipe';

export interface CheckboxListOption {
  /** What `value` and `onChange` carry for this option. */
  value: string;
  label: string;
  /** Help text under the option's label. */
  description?: string;
  /** @default false */
  isDisabled?: boolean;
}

export type CheckboxListBinding =
  | {
      /** A View-local cell holding the chosen values; use it or `value` with `onChange`. */
      bind: Local<readonly string[]>;
      value?: never;
      onChange?: never;
    }
  | {
      bind?: never;
      /** The chosen options' values. */
      value: readonly string[];
      /** Runs with the chosen values, in the options' order. */
      onChange: (value: readonly string[]) => void;
    };

export type CheckboxListProps = CheckboxListBinding & {
  /** The question the options answer, and the group's accessible name: "Notify me about". */
  label: string;
  /** Hides the visible question; the group keeps it as its name. @default false */
  isLabelHidden?: boolean;
  /** Help text under the question, read as the group's description. */
  description?: string;
  /** A message about the group as a whole: "Choose at least one". */
  status?: FieldStatus;
  /** Marks the question with an asterisk. Validation is the app's. @default false */
  isRequired?: boolean;
  items: readonly CheckboxListOption[];
  /** Adds a first box that chooses or clears every option, named by this text: "Select all". It shows a dash while some are chosen. */
  selectAllLabel?: string;
  /** Disables every option. @default false */
  isDisabled?: boolean;
  /** @default 'md' */
  size?: FieldSize;
  className?: string;
  /** On the group; an option's checkbox is `<id>-<value>`. */
  'data-testid'?: string;
};

/** Several on/off choices under one question, any number of which can be chosen. */
export function CheckboxList(props: CheckboxListProps) {
  const {
    label, isLabelHidden = false, description, status, isRequired = false, items, selectAllLabel, isDisabled = false, size: ownSize,
    className, 'data-testid': testId,
  } = props;
  const size = useControlSize(ownSize);
  const id = useId();
  const [helpId, statusId] = [`${id}-help`, `${id}-status`];
  const chosen = props.bind ? props.bind.value : props.value;
  const toggle = (value: string, isOn: boolean) => {
    // Kept in the options' order, whatever order they were chosen in.
    const next = items.map(option => option.value).filter(each => (each === value ? isOn : chosen.includes(each)));
    if (props.bind) props.bind.set(next);
    else props.onChange(next);
  };
  // Options the list may change; a disabled one keeps its state through "all" and "none".
  const free = items.filter(option => !option.isDisabled).map(option => option.value);
  const on = free.filter(value => chosen.includes(value)).length;
  const setAll = (isOn: boolean) => {
    const next = items.map(option => option.value).filter(each => (free.includes(each) ? isOn : chosen.includes(each)));
    if (props.bind) props.bind.set(next);
    else props.onChange(next);
  };
  return (
    <view
      role="group"
      aria-label={label}
      aria-describedby={describedBy(description !== undefined && helpId, hasMessage(status) && statusId)}
      data-testid={testId}
      className={cx(r.root.base, className)}
    >
      {!isLabelHidden && (
        // The group carries the question as its name, so it is not read a second time.
        <text aria-hidden className={cx(r.label.base, r.label.size[size], isDisabled && r.label.disabled)}>
          {label}
          {isRequired && <span className={fieldRecipe.required.base}>{' *'}</span>}
        </text>
      )}
      {description !== undefined && <text id={helpId} className={r.description.base}>{description}</text>}
      <view className={r.options.size[size]}>
        {selectAllLabel !== undefined && (
          <CheckboxInput
            label={selectAllLabel}
            value={free.length > 0 && on === free.length}
            isIndeterminate={on > 0 && on < free.length}
            onChange={() => setAll(on < free.length)}
            isDisabled={isDisabled || free.length === 0}
            size={size}
            className={r.all.base}
            data-testid={testId && `${testId}-all`}
          />
        )}
        {items.map(option => (
          <CheckboxInput
            key={option.value}
            label={option.label}
            description={option.description}
            value={chosen.includes(option.value)}
            onChange={isOn => toggle(option.value, isOn)}
            isDisabled={isDisabled || option.isDisabled}
            size={size}
            data-testid={testId && `${testId}-${option.value}`}
          />
        ))}
      </view>
      <StatusRegion id={statusId} status={status} />
    </view>
  );
}
