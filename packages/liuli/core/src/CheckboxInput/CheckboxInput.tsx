import { cx, type Local } from '@lingxia/view';
import type { FieldOwnProps } from '../Field/Field';
import { glyphBox, glyphs } from '../internal/glyphs';
import { StatusRegion } from '../internal/StatusRegion';
import { checkboxInputRecipe as r } from './CheckboxInput.recipe';

export type CheckboxInputBinding =
  | { bind: Local<boolean>; value?: never; onChange?: never }
  | { bind?: never; value: boolean; onChange: (value: boolean) => void };

export type CheckboxInputProps = Omit<FieldOwnProps, 'isRequired'> & CheckboxInputBinding & {
  className?: string;
  /** On the checkbox. */
  'data-testid'?: string;
};

/**
 * One on/off choice with a label and optional help text. The box, label and help text are one
 * control: a tap anywhere on them toggles it.
 */
export function CheckboxInput(props: CheckboxInputProps) {
  const {
    label, isLabelHidden = false, description, status, isDisabled = false, disabledMessage, size = 'md',
    className, 'data-testid': testId,
  } = props;
  const checked = props.bind ? props.bind.value : props.value;
  const help = isDisabled && disabledMessage ? disabledMessage : description;
  const content = (
    <view className={r.row.base}>
      <view aria-hidden className={cx(r.box.base, r.box.size[size], r.box.state[checked ? 'on' : 'off'], !checked && status?.type === 'error' && r.box.invalid)}>
        {checked && <vector aria-hidden paths={glyphs.check} viewBox={glyphBox} className={r.mark.size[size]} />}
      </view>
      <view className={r.text.base}>
        <text className={cx(r.label.base, r.label.size[size], isLabelHidden && r.label.hidden)}>{label}</text>
        {help !== undefined && <text className={r.description.base}>{help}</text>}
      </view>
    </view>
  );
  return (
    <view className={cx(r.root.base, className)}>
      {props.bind ? (
        <checkbox bind={props.bind} disabled={isDisabled} data-testid={testId} className={r.control.base}>
          {content}
        </checkbox>
      ) : (
        <checkbox value={props.value} onChange={props.onChange} disabled={isDisabled} data-testid={testId} className={r.control.base}>
          {content}
        </checkbox>
      )}
      <StatusRegion status={status} />
    </view>
  );
}
