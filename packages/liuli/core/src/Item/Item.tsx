import { childrenToArray, cx, useContext, useId, type Child } from '@lingxia/view';
import { describedBy } from '../internal/describedBy';
import { Glyph } from '../internal/Glyph';
import { LayerContext } from '../internal/layer';
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
      /**
       * The current choice among rows that select, such as a sidebar's open page: an accent wash and a
       * heavier label, read as selected. Pass `false` on the other rows so they read as not selected.
       */
      isSelected?: boolean;
    }
  | { onClick?: never; hasChevron?: never; isDisabled?: never; disabledMessage?: never; isSelected?: never };

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
  const isSelected = props.isSelected ?? false;
  const content = (
    <>
      {startContent !== undefined && <view aria-hidden={pressable || undefined} className={r.start.base}>{startContent}</view>}
      <view className={r.text.base}>
        <text className={cx(r.label.base, r.label.tone[tone], isSelected && r.label.selected)}>{label}</text>
        {help !== undefined && <text id={pressable ? descriptionId : undefined} className={r.description.base}>{help}</text>}
      </view>
      {endContent !== undefined && <view aria-hidden={pressable || undefined} className={r.end.base}>{endContent}</view>}
      {props.hasChevron && <Glyph name="chevron-end" className={cx(r.chevron.base, isSelected && r.chevron.selected)} />}
    </>
  );
  if (!pressable) return <view data-testid={testId} className={cx(r.root.base, className)}>{content}</view>;
  return (
    <button
      aria-label={label}
      aria-describedby={describedBy(help !== undefined && descriptionId)}
      aria-selected={props.isSelected}
      disabled={isDisabled}
      onClick={props.onClick}
      data-testid={testId}
      className={cx(r.root.base, r.root.pressable, isSelected ? r.root.selected : r.root.idle, className)}
    >
      {content}
    </button>
  );
}

export interface ItemListProps {
  /** Rows: Items, or row variants of Switch and Slider. */
  children: Child;
  /** Names the list for assistive input, when the heading above it does not. */
  label?: string;
  className?: string;
  'data-testid'?: string;
}

/** Rows in one bordered list, read as a list: each row reports its position. */
export function ItemList({ children, label, className, 'data-testid': testId }: ItemListProps) {
  const layer = useContext(LayerContext);
  // A button keeps its own role, so each row sits in a listitem that carries the position.
  return (
    <view role="list" aria-label={label} data-testid={testId} className={cx(r.list.base, r.list.layer[layer], className)}>
      {childrenToArray(children).map((row, index) => <view key={String(index)} role="listitem" className={r.listitem.base}>{row}</view>)}
    </view>
  );
}
