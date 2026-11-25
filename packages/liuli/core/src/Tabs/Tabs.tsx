import { cx, reveal, useBox, useId, useLocal, type Child, type Local } from '@lingxia/view';
import type { IconRef } from '../Icon/Icon';
import { Glyph } from '../internal/Glyph';
import { segmentedControlRecipe as s } from '../SegmentedControl/SegmentedControl.recipe';
import { tabsRecipe as r } from './Tabs.recipe';
import { dataOf } from '../internal/data';

export type TabsVariant = 'underline' | 'segmented';
/** `equal` shares the row; `content` makes each tab as wide as its label. Either row scrolls when the labels do not fit. */
export type TabsWidth = 'equal' | 'content';

export type TabItem = {
  /** What `value` and `onChange` carry. */
  value: string;
  /** The tab's text and accessible name; with `isLabelHidden`, its name alone. */
  label: string;
  /** A short count after the label, such as unread items: "3". Read with the name ("Inbox, 3"), not announced when it changes. */
  badge?: string;
  /** @default false */
  isDisabled?: boolean;
} & (
  | {
      /** An icon by name from the set, or a definition, before the label. */
      icon?: IconRef;
      isLabelHidden?: false;
    }
  | {
      icon: IconRef;
      /** Shows the icon alone; the label still names the tab. @default false */
      isLabelHidden: true;
    }
);

export type TabsBinding =
  | {
      /** A View-local cell the control reads and writes; use it or `value` with `onChange`. */
      bind: Local<string>;
      value?: never;
      onChange?: never;
    }
  | {
      bind?: never;
      /** The chosen tab's value. */
      value: string;
      /** Runs with the tab chosen by a tap or by arrowing onto it. */
      onChange: (value: string) => void;
    };

export type TabsProps = TabsBinding & {
  /** Names the tab list for assistive tech: "Range", "Sections". */
  label: string;
  /** The tabs, in order. */
  items: readonly TabItem[];
  /** `underline` switches between sections of a page; `segmented` switches one view between modes. @default 'underline' */
  variant?: TabsVariant;
  /** How wide a tab is. `content` suits many tabs or long labels; underline tabs only. @default 'equal' */
  tabWidth?: TabsWidth;
  /** The chosen tab's panel. */
  children?: Child;
  className?: string;
  /** On the tab list; each tab gets `<testid>-<value>`, the panel `<testid>-panel`. */
  'data-testid'?: string;
};

/** Tabs over one panel: one Tab stop for the row, arrows move between tabs and choose on arrival. */
export function Tabs({ label, items, bind, value: own, onChange: ownChange, variant = 'underline', tabWidth = 'equal', children, className, 'data-testid': testId, ...attributes }: TabsProps) {
  const value = bind ? bind.value : own;
  const onChange = (next: string) => bind ? bind.set(next) : ownChange?.(next);
  const id = useId();
  const index = Math.max(0, items.findIndex(item => item.value === value));
  const chosen = items[index];
  const segmented = variant === 'segmented';
  const width: TabsWidth = segmented ? 'equal' : tabWidth;
  // Each tab is measured: a tab never narrower than its label leaves equal shares once one is long.
  const frame = useBox('frame');
  // Named by position: a value is any string, a box name at most 64 characters.
  const boxes = items.map((_, at) => useBox(`tab-${at}`));
  const widths = boxes.map(box => box.value?.width);
  const isMeasured = widths.every(each => each !== undefined);
  const total = widths.reduce<number>((sum, each) => sum + (each ?? 0), 0);
  const isEven = isMeasured && widths.every(each => Math.abs((each ?? 0) - (widths[0] ?? 0)) <= 0.5);
  const room = frame.value?.width;
  // A row wider than its frame fades at each edge it runs under, so a cut label reads as more tabs.
  const edges = useLocal<{ start: boolean; end: boolean }>('edges', { start: true, end: false });
  const under = !edges.value.start && !edges.value.end ? 'both' : !edges.value.start ? 'start' : !edges.value.end ? 'end' : undefined;
  const overflows = room !== undefined && isMeasured && total > room + 0.5;
  const share = 100 / Math.max(1, items.length);
  const place = width === 'equal' && (!isMeasured || isEven)
    ? { width: `${share}%` as const, translate: [`${index * 100}%`, 0] as const }
    : isMeasured
      ? { width: widths[index], translate: [widths.slice(0, index).reduce<number>((sum, each) => sum + (each ?? 0), 0), 0] as const }
      : undefined;
  return (
    <view className={cx(r.root.base, className)}>
      <view box={frame} className={cx(r.frame.base, segmented && cx(s.root.base, s.root.row))}>
        <scroll className={cx(r.scroller.base, overflows && under && r.scroller.more[under])} onScrollEdge={({ edge, at }) => edges.set({ ...edges.value, [edge]: at })}>
          <view role="tablist" aria-label={label} data-testid={testId} {...dataOf(attributes)} className={cx(r.list.base, !segmented && r.list.underline)}>
            {/* One box a tab wide, moved along the row. It moves in a left-to-right box that
                right-to-left mirrors, so the same translate slides it toward the end either way. */}
            {place && (
              <view aria-hidden className={r.track.base}>
                <view dir="ltr" className={r.track.inner}>
                  <view className={cx(r.indicator.base, segmented ? cx(s.thumb.base, s.thumb.row, items.some(item => item.isLabelHidden) && s.thumb.edged) : r.indicator.underline)} style={place} />
                </view>
              </view>
            )}
            {items.map((item, at) => {
              const isSelected = at === index;
              const tabId = `${id}-${item.value}`;
              return (
                <button
                  key={item.value}
                  role="tab"
                  id={tabId}
                  aria-label={item.badge === undefined ? item.label : `${item.label}, ${item.badge}`}
                  aria-selected={isSelected}
                  data-state={isSelected ? 'active' : 'inactive'}
                  disabled={item.isDisabled}
                  data-testid={testId && `${testId}-${item.value}`}
                  // A tab chosen at the row's edge is brought fully into view.
                  onClick={() => { if (!isSelected) { onChange(item.value); reveal(tabId); } }}
                  className={cx(r.tab.base, segmented ? cx(s.segment.base, s.segment.size.md, s.segment.row) : cx(r.tab[width], r.tab.underline, r.tab.pad[width]))}
                >
                  <view aria-hidden className={r.content.base}>
                    {item.icon !== undefined && <Glyph name={item.icon} side={20} className={cx(r.icon.base, isSelected ? r.icon.selected : r.icon.unselected)} />}
                    {!item.isLabelHidden && <text className={cx(r.label.base, segmented ? s.label.size.md : r.label.underline, isSelected ? r.label.selected : r.label.unselected)}>{item.label}</text>}
                    {item.badge !== undefined && (
                      <view className={r.badge.base}>
                        <text className={cx(r.badge.label, isSelected ? r.label.selected : r.label.unselected)}>{item.badge}</text>
                      </view>
                    )}
                  </view>
                  <view aria-hidden box={boxes[at]} className={r.tab.extent} />
                </button>
              );
            })}
          </view>
        </scroll>
      </view>
      {children !== undefined && chosen && (
        <view role="tabpanel" aria-label={chosen.label} data-testid={testId && `${testId}-panel`} className={r.panel.base}>
          {children}
        </view>
      )}
    </view>
  );
}
