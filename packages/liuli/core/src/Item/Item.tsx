import { childrenToArray, createContext, cx, useBox, useContext, useId, useLocal, type Child } from '@lingxia/view';
import { useDensity } from '../density';
import type { IconRef } from '../Icon/Icon';
import { describedBy } from '../internal/describedBy';
import { Glyph } from '../internal/Glyph';
import { LayerContext } from '../internal/layer';
import { itemRecipe as r } from './Item.recipe';
import { dataOf } from '../internal/data';
import { IconButton } from '../IconButton/IconButton';
import { holdsControl } from '../internal/trigger';
import { warnOnce } from '../internal/warn';

/** A row is pressable (`onClick`) or holds its own controls in `endContent`, never both; a pressable row may add one `endAction`. */
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
/** `compact` is for long lists read with a mouse: rows 40 px tall. */
export type ItemDensity = 'comfortable' | 'compact';
export type ItemSwipeTone = 'neutral' | 'accent' | 'danger';

/**
 * One button at the row's end, its own control beside the row's press: a remove × on a row that
 * opens its device. One, by its shape: more actions belong in `swipeActions` or a menu.
 */
export type ItemEndAction = {
  /** The accessible name: what pressing it does, naming the row ("Remove Emily's iPad"). */
  label: string;
  icon: IconRef;
  /** `destructive` for an action that destroys or ends something at once, as a menu row's. @default 'default' */
  variant?: 'default' | 'destructive';
  /** Shown but not pressable, so the row does not move; say why in `disabledMessage`. @default false */
  isDisabled?: boolean;
  /** Why it is disabled: read as its description while disabled; not drawn. */
  disabledMessage?: string;
  onClick: () => void;
  'data-testid'?: string;
};

export interface ItemSwipeAction {
  /** Identifies the action: its test id is `<row id>-<key>`. */
  key: string;
  /** Drawn under the icon, and what a screen reader offers as the row's action. */
  label: string;
  icon?: IconRef;
  /** @default 'neutral' */
  tone?: ItemSwipeTone;
  /** Runs after the row has closed. */
  onClick: () => void;
}

/** The density an ItemList gives its rows. */
export const ItemDensityContext = createContext<ItemDensity | undefined>(undefined);

export type ItemProps = ItemAction & {
  /** The row's main text, and its accessible name when it is pressable. */
  label: string;
  /** `danger` draws the label in the danger role, for a row that deletes or ends something. @default 'default' */
  tone?: ItemTone;
  /** A second line under the label; read as the row's description when it is pressable. */
  description?: string;
  /** Before the text: an icon or an avatar. Hidden from assistive tech on a pressable row. */
  startContent?: Child;
  /** An icon before the text, by name or as a definition, in a quiet tinted square: for rows that name a place or a kind. Not read. */
  startIcon?: IconRef;
  /** On the label's line, after it: a status Badge beside a name. The name truncates first; this is never squeezed. On a pressable row its text is read after the name, before the second line. */
  labelEnd?: Child;
  /** One icon button after `endContent`, a control of its own: pressing it never presses the row, and it is reached on its own by keyboard and screen reader. */
  endAction?: ItemEndAction;
  /** After the text: a value, a badge, or, on a row without `onClick`, a control such as a Button. On a pressable row its text is read after the second line, as the row's description. */
  endContent?: Child;
  /** The row's height and padding. By default the ItemList's, else `comfortable`. */
  density?: ItemDensity;
  /**
   * Up to three actions behind the row's end, shown by dragging the row toward its start. A
   * screen reader offers them as the row's actions; with a keyboard the arrow keys open the row.
   */
  swipeActions?: readonly ItemSwipeAction[];
  className?: string;
  'data-testid'?: string;
};

const maxSwipeActions = 3;
// Each action's width in points; the row opens by the sum.
const swipeActionWidth = 72;

/** One row of a list: a label, an optional second line, and content at either end. */
export function Item(props: ItemProps) {
  const { label, tone = 'default', description, startContent, startIcon, labelEnd, endContent, endAction, swipeActions, className, 'data-testid': testId } = props;
  const listDensity = useContext(ItemDensityContext);
  const screenDensity = useDensity();
  const density = props.density ?? listDensity ?? screenDensity;
  const layer = useContext(LayerContext);
  const baseId = useId();
  const open = useLocal('open', 0);
  const descriptionId = `${baseId}-description`;
  const valueId = `${baseId}-value`;
  const besideId = `${baseId}-beside`;
  const isDisabled = props.isDisabled ?? false;
  const help = isDisabled && props.disabledMessage !== undefined ? props.disabledMessage : description;
  const pressable = props.onClick !== undefined;
  if (pressable && (holdsControl(labelEnd) || holdsControl(endContent))) warnOnce(`Item "${typeof label === 'string' ? label : ''}": a control in labelEnd or endContent of a pressable row cannot be reached; use endAction for one button, or make the row not pressable`);
  const isSelected = props.isSelected ?? false;
  // The name is the row: left under half the line it is cut to a few letters, so labelEnd takes its own line under it.
  const labelLine = useBox('labelLine');
  const labelEndBox = useBox('labelEnd');
  const stacked = labelLine.value !== null && labelEndBox.value !== null && labelLine.value.width - labelEndBox.value.width - 8 < labelLine.value.width / 2;
  const name = <text className={cx(r.label.base, r.label.tone[tone], isSelected && r.label.selected, labelEnd !== undefined && r.label.beside, stacked && r.label.stacked)}>{label}</text>;
  const content = (
    <>
      {startIcon !== undefined && <view aria-hidden className={r.iconTile.base}><Glyph name={startIcon} className={r.iconTile.icon} /></view>}
      {startContent !== undefined && <view aria-hidden={pressable || undefined} className={r.start.base}>{startContent}</view>}
      <view className={r.text.base}>
        {labelEnd === undefined ? name : (
          <view box={labelLine} className={cx(r.labelLine.base, stacked && r.labelLine.stacked)}>
            {name}
            <view box={labelEndBox} id={pressable ? besideId : undefined} aria-hidden={pressable || undefined} className={r.labelEnd.base}>{labelEnd}</view>
          </view>
        )}
        {help !== undefined && <text id={pressable ? descriptionId : undefined} className={r.description.base}>{help}</text>}
      </view>
      {endContent !== undefined && <view id={pressable ? valueId : undefined} aria-hidden={pressable || undefined} className={r.end.base}>{endContent}</view>}
      {props.hasChevron && <Glyph name="chevron-end" className={cx(r.chevron.base, isSelected && r.chevron.selected)} />}
      {/* Outermost: the chevron belongs to the row's press, the action is a control of its own at the edge. */}
      {endAction !== undefined && (
        <IconButton
          label={endAction.label}
          icon={endAction.icon}
          size="sm"
          edge="end"
          onClick={endAction.onClick}
          isDisabled={endAction.isDisabled}
          disabledMessage={endAction.disabledMessage}
          data-testid={endAction['data-testid']}
          className={cx(r.endAction.base, r.endAction.variant[endAction.variant ?? 'default'])}
        />
      )}
    </>
  );
  const root = cx(r.root.base, r.root.density[density]);
  const actions = swipeActions?.slice(0, maxSwipeActions) ?? [];
  const row = !pressable ? <view data-testid={testId} {...dataOf(props)} className={cx(root, actions.length === 0 && className)}>{content}</view> : (
    <button
      aria-label={label}
      aria-describedby={describedBy(labelEnd !== undefined && besideId, help !== undefined && descriptionId, endContent !== undefined && valueId)}
      aria-selected={props.isSelected}
      disabled={isDisabled}
      onClick={props.onClick}
      data-testid={testId} {...dataOf(props)}
      className={cx(root, r.root.pressable, isSelected ? r.root.selected : r.root.idle, actions.length === 0 && className)}
    >
      {content}
    </button>
  );
  if (actions.length === 0) return row;
  const actionId = (action: ItemSwipeAction) => `${baseId}-${action.key}`;
  return (
    <view className={cx(r.swipe.base, className)}>
      <view className={r.actions.base}>
        {actions.map(action => (
          <button
            key={action.key}
            id={actionId(action)}
            onClick={() => { open.set(0); action.onClick(); }}
            data-testid={testId === undefined ? undefined : `${testId}-${action.key}`}
            className={cx(r.action.base, r.action.tone[action.tone ?? 'neutral'])}
          >
            {action.icon !== undefined && <Glyph name={action.icon} className={r.actionIcon.base} />}
            <text className={r.actionLabel.base}>{action.label}</text>
          </button>
        ))}
      </view>
      {/* The row slides as a whole over its actions, so it carries the list's surface: a pressed row's wash is see-through. */}
      <view
        drag="x"
        dragSnapPoints={[0, -swipeActionWidth * actions.length]}
        bind={open}
        aria-label={label}
        aria-actions={actions.map(actionId).join(' ')}
        data-testid={testId === undefined ? undefined : `${testId}-swipe`}
        className={cx(r.slide.base, r.slide.layer[layer])}
      >
        {row}
      </view>
    </view>
  );
}

export interface ItemListProps {
  /** Rows: Items, or row variants of Switch and Slider. */
  children: Child;
  /** Names the list for assistive input, when neither `title` nor a heading above it does. */
  label?: string;
  /** A heading over the list, in the quiet type of a settings group; it names the list. */
  title?: string;
  /** A line under the list that explains the group. */
  description?: string;
  /** The rows' height and padding; a row's own `density` wins. @default the screen's density, 'comfortable' unless a DensityProvider says otherwise */
  density?: ItemDensity;
  /** Rows on the pane's own surface, with no border, corners or fill of the list's: for a list that fills a bordered pane. @default false */
  isFlush?: boolean;
  className?: string;
  'data-testid'?: string;
}

/** Rows in one bordered list, read as a list: each row reports its position. */
export function ItemList({ children, label, title, description, density, isFlush = false, className, 'data-testid': testId, ...attributes }: ItemListProps) {
  const layer = useContext(LayerContext);
  const hasFrame = title !== undefined || description !== undefined;
  // A button keeps its own role, so each row sits in a listitem that carries the position.
  const list = (
    <view role="list" aria-label={label ?? title} data-testid={testId} {...dataOf(attributes)} className={cx(r.list.base, !isFlush && cx(r.list.framed, r.list.layer[layer]), !hasFrame && className)}>
      {childrenToArray(children).map((row, index) => <view key={String(index)} role="listitem" className={r.listitem.base}>{row}</view>)}
    </view>
  );
  const rows = density === undefined ? list : <ItemDensityContext.Provider value={density}>{list}</ItemDensityContext.Provider>;
  if (!hasFrame) return rows;
  return (
    <view className={cx(r.group.base, className)}>
      {/* The list is named by the same words, so the heading is not read twice. */}
      {title !== undefined && <text aria-hidden={label === undefined || undefined} className={r.title.base}>{title}</text>}
      {rows}
      {description !== undefined && <text className={r.note.base}>{description}</text>}
    </view>
  );
}
