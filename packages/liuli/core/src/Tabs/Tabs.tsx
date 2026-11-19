import { cx, reveal, useBox, useId, useLocal, type Child } from '@lingxia/view';
import { segmentedControlRecipe as s } from '../SegmentedControl/SegmentedControl.recipe';
import { tabsRecipe as r } from './Tabs.recipe';

export type TabsVariant = 'underline' | 'segmented';
/** `equal` shares the row; `content` makes each tab as wide as its label. Either row scrolls when the labels do not fit. */
export type TabsWidth = 'equal' | 'content';

export interface TabItem {
  /** What `value` and `onChange` carry. */
  value: string;
  /** The tab's text and accessible name. */
  label: string;
  /** @default false */
  isDisabled?: boolean;
}

export interface TabsProps {
  /** Names the tab list for assistive tech: "Range", "Sections". */
  label: string;
  /** The tabs, in order. */
  items: readonly TabItem[];
  /** The chosen tab's value. */
  value: string;
  /** Runs with the tab chosen by a tap or by arrowing onto it. */
  onChange: (value: string) => void;
  /** `underline` switches between sections of a page; `segmented` switches one view between modes. @default 'underline' */
  variant?: TabsVariant;
  /** How wide a tab is. `content` suits many tabs or long labels; underline tabs only. @default 'equal' */
  tabWidth?: TabsWidth;
  /** The chosen tab's panel. */
  children?: Child;
  className?: string;
  /** On the tab list; each tab gets `<testid>-<value>`, the panel `<testid>-panel`. */
  'data-testid'?: string;
}

/** Tabs over one panel: one Tab stop for the row, arrows move between tabs and choose on arrival. */
export function Tabs({ label, items, value, onChange, variant = 'underline', tabWidth = 'equal', children, className, 'data-testid': testId }: TabsProps) {
  const id = useId();
  const index = Math.max(0, items.findIndex(item => item.value === value));
  const chosen = items[index];
  const segmented = variant === 'segmented';
  const width: TabsWidth = segmented ? 'equal' : tabWidth;
  // Each tab is measured: a tab never narrower than its label leaves equal shares once one is long.
  const frame = useBox('frame');
  const boxes = items.map(item => useBox(`tab-${item.value}`));
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
          <view role="tablist" aria-label={label} data-testid={testId} className={cx(r.list.base, !segmented && r.list.underline)}>
            {/* One box a tab wide, moved along the row. It moves in a left-to-right box that
                right-to-left mirrors, so the same translate slides it toward the end either way. */}
            {place && (
              <view aria-hidden className={r.track.base}>
                <view dir="ltr" className={r.track.inner}>
                  <view className={cx(r.indicator.base, segmented ? cx(s.thumb.base, s.thumb.row) : r.indicator.underline)} style={place} />
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
                  aria-selected={isSelected}
                  data-state={isSelected ? 'active' : 'inactive'}
                  disabled={item.isDisabled}
                  data-testid={testId && `${testId}-${item.value}`}
                  // A tab chosen at the row's edge is brought fully into view.
                  onClick={() => { if (!isSelected) { onChange(item.value); reveal(tabId); } }}
                  className={cx(r.tab.base, segmented ? cx(s.segment.base, s.segment.size.md, s.segment.row) : cx(r.tab[width], r.tab.underline, r.tab.pad[width]))}
                >
                  <text className={cx(r.label.base, segmented ? s.label.size.md : r.label.underline, isSelected ? r.label.selected : r.label.unselected)}>{item.label}</text>
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
