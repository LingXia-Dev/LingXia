import { type Child } from '@lingxia/view';
import { liuliText } from '../i18n/text';
import { Button } from '../Button/Button';
import { Field, type FieldOwnProps } from '../Field/Field';
import { IconButton } from '../IconButton/IconButton';
import { listInputRecipe as r } from './ListInput.recipe';
import { dataOf } from '../internal/data';

export type ListInputProps = FieldOwnProps & {
  /** One stable key per row, in order: an id, never the row's place. The app owns the rows. */
  rowKeys: readonly string[];
  /** A row's controls, by its place from 0. Give each control its own label, hidden: "Phone 2". */
  renderRow: (index: number) => Child;
  /** Adds a row at the end. */
  onAdd: () => void;
  /** The words on the add button: "Add a phone number". Defaults to "Add" in the page's language. */
  addLabel?: string;
  /** Takes the row at this place away. */
  onRemove: (index: number) => void;
  /** Each remove button, named with what it removes: `index => \`Remove phone ${index + 1}\``. Defaults to "Remove item 2" in the page's language. */
  removeLabel?: (index: number) => string;
  /** The add button is disabled once there are this many rows. */
  max?: number;
  /** Rows cannot be removed at or below this many. @default 0 */
  min?: number;
  className?: string;
  /** On the list; the add button gets `<testid>-add`, each remove button `<testid>-remove-<key>`. */
  'data-testid'?: string;
};

/** A field made of rows the user adds and removes: phone numbers, links, line items. */
export function ListInput(props: ListInputProps) {
  const { rowKeys, renderRow, onAdd, addLabel = liuliText('@liuli.add'), onRemove, removeLabel = (index: number) => liuliText('@liuli.removeItem', { number: index + 1 }), max, min = 0, className, 'data-testid': testId, ...field } = props;
  const { isDisabled = false } = field;
  const isFull = max !== undefined && rowKeys.length >= max;
  return (
    <Field {...field} className={className}>
      {control => (
        // With no rows there is no list, so the field's box carries its test id and data-* props.
        <view className={r.root.base} {...(rowKeys.length === 0 ? { 'data-testid': testId, ...dataOf(field) } : {})}>
          {rowKeys.length > 0 && (
            <view role="list" aria-label={control['aria-label']} aria-describedby={control['aria-describedby']} data-testid={testId} {...dataOf(field)} className={r.list.base}>
              {rowKeys.map((key, index) => (
                <view key={key} role="listitem" className={r.row.base}>
                  <view className={r.controls.base}>{renderRow(index)}</view>
                  <IconButton
                    label={removeLabel(index)}
                    icon="close"
                    variant="ghost"
                    isDisabled={isDisabled || rowKeys.length <= min}
                    onClick={() => onRemove(index)}
                    data-testid={testId && `${testId}-remove-${key}`}
                  />
                </view>
              ))}
            </view>
          )}
          <view className={r.add.base}>
            <Button label={addLabel} startIcon="add" size="sm" width="content" isDisabled={isDisabled || isFull} onClick={onAdd} data-testid={testId && `${testId}-add`} />
          </view>
        </view>
      )}
    </Field>
  );
}
