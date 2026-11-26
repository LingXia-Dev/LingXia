import { cx, type Local } from '@lingxia/view';
import { liuliText } from '../i18n/text';
import { useControlSize } from '../density';
import { DropdownMenu, DropdownMenuCheckboxItem } from '../DropdownMenu/DropdownMenu';
import { IconButton } from '../IconButton/IconButton';
import { Field, type FieldOwnProps } from '../Field/Field';
import { Token } from '../Token/Token';
import type { IconRef } from '../Icon/Icon';
import { multiSelectRecipe as r } from './MultiSelect.recipe';
import { dataOf } from '../internal/data';

export interface MultiSelectOption {
  /** What `value` and `onChange` carry. */
  value: string;
  /** The text on the token and in the list. */
  label: string;
  /** A second, quieter line in the list, read as the option's description. */
  description?: string;
  /** An icon by name from the set, or a definition, before the label in the list; hidden from assistive tech. */
  icon?: IconRef;
  /** @default false */
  isDisabled?: boolean;
}

export type MultiSelectBinding =
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

export type MultiSelectProps = FieldOwnProps & MultiSelectBinding & {
  /** The choices, in order. */
  items: readonly MultiSelectOption[];
  /** Shown in the box while nothing is chosen. */
  placeholder?: string;
  /** The name of the button that opens the list: "Add a label". Defaults to "Add" in the page's language. */
  addLabel?: string;
  /** Each token's remove button, named with what it removes: `label => \`Remove ${label}\``. Defaults to "Remove <label>" in the page's language. */
  removeLabel?: (label: string) => string;
  className?: string;
  /** On the box; the add button gets `<testid>-add`, each option `<testid>-<value>`. */
  'data-testid'?: string;
};

/** Several choices from a list, shown as tokens in the field, with Field\'s label, help text and status. */
export function MultiSelect(props: MultiSelectProps) {
  const {
    label, isLabelHidden, description, status, isRequired, hasRequiredMark, optionalLabel, info, isDisabled = false, disabledMessage, size: ownSize,
    items, placeholder, addLabel = liuliText('@liuli.add'), removeLabel = (label: string) => liuliText('@liuli.removeNamed', { name: label }), className, 'data-testid': testId,
  } = props;
  const size = useControlSize(ownSize);
  const chosen = props.bind ? props.bind.value : props.value;
  // Kept in the options' order, whatever order they were chosen in.
  const set = (next: readonly string[]) => {
    const ordered = items.filter(option => next.includes(option.value)).map(option => option.value);
    if (props.bind) props.bind.set(ordered);
    else props.onChange(ordered);
  };
  const toggle = (value: string, isOn: boolean) => set(isOn ? [...chosen, value] : chosen.filter(each => each !== value));
  const shown = items.filter(option => chosen.includes(option.value));
  return (
    <Field
      label={label}
      isLabelHidden={isLabelHidden}
      description={description}
      status={status}
      isRequired={isRequired}
      hasRequiredMark={hasRequiredMark}
      optionalLabel={optionalLabel}
      info={info}
      isDisabled={isDisabled}
      disabledMessage={disabledMessage}
      size={size}
      className={className}
    >
      {control => (
        <view
          role="group"
          aria-label={control['aria-label']}
          aria-describedby={control['aria-describedby']}
          data-testid={testId} {...dataOf(props)}
          className={cx(r.box.base, r.box.size[size], status && r.box.status[status.type], isDisabled && r.box.disabled)}
        >
          {shown.map(option => (
            <Token
              key={option.value}
              label={option.label}
              size="sm"
              isDisabled={isDisabled || option.isDisabled}
              removeLabel={removeLabel(option.label)}
              onRemove={() => toggle(option.value, false)}
              data-testid={testId && `${testId}-token-${option.value}`}
            />
          ))}
          {shown.length === 0 && placeholder !== undefined && <text aria-hidden className={r.placeholder.base}>{placeholder}</text>}
          <DropdownMenu
            label={addLabel}
            size="sm"
            alignment="end"
            menu={items.map(option => (
              <DropdownMenuCheckboxItem
                key={option.value}
                label={option.label}
                description={option.description}
                icon={option.icon}
                value={chosen.includes(option.value)}
                isDisabled={option.isDisabled}
                onChange={isOn => toggle(option.value, isOn)}
                data-testid={testId && `${testId}-${option.value}`}
              />
            ))}
          >
            <IconButton label={addLabel} icon="add" size="sm" isDisabled={isDisabled} className={r.add.base} data-testid={testId && `${testId}-add`} />
          </DropdownMenu>
        </view>
      )}
    </Field>
  );
}
