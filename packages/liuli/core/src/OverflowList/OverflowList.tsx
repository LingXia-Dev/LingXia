import { cx, useLocal, type BoxSize, type Child } from '@lingxia/view';
import { dataOf } from '../internal/data';
import { overflowListRecipe as r } from './OverflowList.recipe';

export type OverflowListCollapse = 'end' | 'start';

export interface OverflowListProps<Item> {
  /** Everything the line could show, in reading order. */
  items: readonly Item[];
  /** One item as it stands on the line: a Token, a Button, a link. */
  renderItem: (item: Item, index: number) => Child;
  /** What stands for the items that do not fit, given them in order: a "+3" token, a More menu. Not drawn when all fit. */
  renderOverflow: (hidden: readonly Item[]) => Child;
  /** An item's stable key, so its measured width outlives a change of the list. By default its place in `items`. */
  keyOf?: (item: Item, index: number) => string;
  /** The end the items fold away from: `start` keeps the last ones, as a path keeps where you are. @default 'end' */
  collapseFrom?: OverflowListCollapse;
  /** The fewest items shown, even when they do not fit. @default 0 */
  minVisible?: number;
  /** The line's name for assistive tech, when it is a group of its own. */
  label?: string;
  className?: string;
  'data-testid'?: string;
}

interface Measures {
  /** Each measured item's width, by key. */
  widths: Readonly<Record<string, number>>;
  /** The overflow element's width. */
  overflow: number;
  /** The line's natural width with every item and the overflow element on it: the gaps are what it holds beyond them. */
  natural: number;
  /** The line's own width. */
  width: number;
}

const unmeasured: Measures = { widths: {}, overflow: -1, natural: -1, width: -1 };

/**
 * How many items fit before the overflow element: everything when all fit without it; else as many
 * as leave room for it, at least `least`. Null until every width it needs is known.
 */
function fitting(widths: readonly (number | undefined)[], m: Measures, least: number): number | null {
  if (m.width < 0 || m.natural < 0 || m.overflow < 0 || widths.some(width => width === undefined)) return null;
  const known = widths as number[];
  const own = known.reduce((sum, width) => sum + width, 0);
  const gap = known.length > 0 ? Math.max(0, (m.natural - own - m.overflow) / known.length) : 0;
  const total = own + gap * Math.max(0, known.length - 1);
  if (total <= m.width + 0.5) return known.length;
  let used = m.overflow;
  let count = 0;
  for (const width of known) {
    if (used + gap + width > m.width + 0.5) break;
    used += gap + width;
    count += 1;
  }
  return Math.max(least, count);
}

/** A line of items that keeps to one row: what does not fit folds into one element that stands for the rest. */
export function OverflowList<Item>({
  items, renderItem, renderOverflow, keyOf, collapseFrom = 'end', minVisible = 0, label, className, 'data-testid': testId, ...attributes
}: OverflowListProps<Item>) {
  const measures = useLocal<Measures>('measures', unmeasured);
  const m = measures.value;
  const keys = items.map((item, index) => (keyOf ? keyOf(item, index) : `#${index}`));
  // Folding from the start counts from the end, so the same arithmetic serves both.
  const order = collapseFrom === 'end' ? keys : [...keys].reverse();
  const count = fitting(order.map(key => m.widths[key]), m, Math.min(minVisible, items.length));
  const isMeasuring = count === null;
  const kept = new Set(isMeasuring ? keys : order.slice(0, count));
  const hidden = items.filter((_, index) => !kept.has(keys[index]));
  // Measuring, the overflow element stands for every item, its widest likely form.
  const overflowFor = isMeasuring ? items : hidden;
  // Several boxes report in one frame, so each change is laid on the latest measures.
  const set = (next: (latest: Measures) => Partial<Measures>) => measures.set(latest => {
    const merged = { ...latest, ...next(latest) };
    return JSON.stringify(merged) === JSON.stringify(latest) ? latest : merged;
  });
  const measureItem = (key: string) => (box: BoxSize) => set(latest => ({ widths: { ...latest.widths, [key]: box.width } }));
  // While every item and the overflow element are on the line, its natural width holds the gaps too.
  const measureLine = (box: BoxSize) => set(() => (isMeasuring ? { width: box.width, natural: box.maxContentWidth } : { width: box.width }));
  // Read where it stands: before the items when they fold from the start.
  const overflow = overflowFor.length > 0 && (
    <view
      key="overflow"
      aria-hidden={isMeasuring || undefined}
      onLayout={box => { if (isMeasuring) set(() => ({ overflow: box.width })); }}
      className={cx(r.overflow.base, isMeasuring && r.overflow.measuring)}
    >
      {renderOverflow(overflowFor)}
    </view>
  );
  return (
    <view
      role={label === undefined ? undefined : 'group'}
      aria-label={label}
      onLayout={measureLine}
      data-testid={testId}
      {...dataOf(attributes)}
      className={cx(r.root.base, className)}
    >
      {collapseFrom === 'start' && overflow}
      {items.map((item, index) => kept.has(keys[index]) && (
        <view key={keys[index]} onLayout={measureItem(keys[index])} className={r.item.base}>{renderItem(item, index)}</view>
      ))}
      {collapseFrom === 'end' && overflow}
    </view>
  );
}
