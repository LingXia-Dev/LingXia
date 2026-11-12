import { cx, useId, type Child } from '@lingxia/view';
import { describedBy } from '../internal/describedBy';
import { glyphBox, glyphs } from '../internal/glyphs';
import { itemRecipe as r } from './Item.recipe';

/** A row is pressable (`onClick`) or holds its own controls in `endContent`, never both. */
export type ItemAction =
  | {
      /** Makes the whole row one button. Its `endContent` is then drawn only, never pressed. */
      onClick: () => void;
      /** A chevron at the end, for a row that opens another page. @default false */
      hasChevron?: boolean;
      /** @default false */
      isDisabled?: boolean;
      /** Shown in place of `description` while disabled. */
      disabledMessage?: string;
    }
  | { onClick?: never; hasChevron?: never; isDisabled?: never; disabledMessage?: never };

export type ItemTone = 'default' | 'danger';

export type ItemProps = ItemAction & {
  /** The row's main text, and its accessible name when it is pressable. */
  label: string;
  /** `danger` draws the label in the danger role, for a row that deletes or ends something. @default 'default' */
  tone?: ItemTone;
  /** A second line under the label; read as the row's description when it is pressable. */
  description?: string;
  /** Before the text: an icon or an avatar. Hidden from assistive tech on a pressable row. */
  startContent?: Child;
  /** After the text: a value, a badge, or, on a row without `onClick`, a control such as a Button. */
  endContent?: Child;
  className?: string;
  'data-testid'?: string;
};

/** One row of a list: a label, an optional second line, and content at either end. */
export function Item(props: ItemProps) {
  const { label, tone = 'default', description, startContent, endContent, className, 'data-testid': testId } = props;
  const descriptionId = `${useId()}-description`;
  const isDisabled = props.isDisabled ?? false;
  const help = isDisabled && props.disabledMessage !== undefined ? props.disabledMessage : description;
  const pressable = props.onClick !== undefined;
  const content = (
    <>
      {startContent !== undefined && <view aria-hidden={pressable || undefined} className={r.start.base}>{startContent}</view>}
      <view className={r.text.base}>
        <text className={cx(r.label.base, r.label.tone[tone])}>{label}</text>
        {help !== undefined && <text id={pressable ? descriptionId : undefined} className={r.description.base}>{help}</text>}
      </view>
      {endContent !== undefined && <view aria-hidden={pressable || undefined} className={r.end.base}>{endContent}</view>}
      {props.hasChevron && <vector aria-hidden paths={glyphs.chevronEnd} viewBox={glyphBox} className={r.chevron.base} />}
    </>
  );
  if (!pressable) return <view data-testid={testId} className={cx(r.root.base, className)}>{content}</view>;
  return (
    <button
      aria-label={label}
      aria-describedby={describedBy(help !== undefined && descriptionId)}
      disabled={isDisabled}
      onClick={props.onClick}
      data-testid={testId}
      className={cx(r.root.base, r.root.pressable, className)}
    >
      {content}
    </button>
  );
}
