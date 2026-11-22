import { createContext, cx, useBox, useContext, type Child } from '@lingxia/view';
import { formLayoutRecipe as r } from './FormLayout.recipe';
import { dataOf } from '../internal/data';

const Columns = createContext(1);

export interface FormLayoutProps {
  /** The fields, in reading order; a `FormSpan` takes a whole line. */
  children: Child;
  /** The most columns the fields may take, where the form is wide enough for them. @default 1 */
  columns?: 1 | 2;
  /** The buttons that submit or leave the form, drawn after the fields. */
  actions?: Child;
  className?: string;
  'data-testid'?: string;
}

// Two columns need this much width: about 264 px a field, and the gap between.
const wideFrom = 560;

/** A form's fields at one rhythm: a column on a phone, two where the form is wide enough. */
export function FormLayout({ children, columns = 1, actions, className, 'data-testid': testId, ...attributes }: FormLayoutProps) {
  const box = useBox('form');
  // Until it is measured the form is one column: right on a phone, and it never jumps narrower.
  const shown = columns === 2 && (box.value?.width ?? 0) >= wideFrom ? 2 : 1;
  return (
    <Columns.Provider value={shown}>
      <view box={box} data-testid={testId} {...dataOf(attributes)} className={cx(r.root.base, className)}>
        <view className={cx(r.fields.base, r.fields.columns[shown])}>{children}</view>
        {actions !== undefined && <view className={r.actions.base}>{actions}</view>}
      </view>
    </Columns.Provider>
  );
}

export interface FormSpanProps {
  children: Child;
  className?: string;
}

/** A field, or anything else, that takes the whole line of a two-column form. */
export function FormSpan({ children, className }: FormSpanProps) {
  const columns = useContext(Columns);
  return <view className={cx(r.span.base, columns === 2 && r.span.wide, className)}>{children}</view>;
}
