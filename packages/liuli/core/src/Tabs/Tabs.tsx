import { cx, reveal, useBox, useId, type Child } from '@lingxia/view';
import { segmentedControlRecipe as s } from '../SegmentedControl/SegmentedControl.recipe';
import { tabsRecipe as r } from './Tabs.recipe';

export type TabsVariant = 'underline' | 'segmented';
/** `equal` shares the row; `content` makes each tab as wide as its label, and the row scrolls when they do not fit. */
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

interface FittedListProps {
  id: string;
  label: string;
  items: readonly TabItem[];
  index: number;
  onChange: (value: string) => void;
  testId?: string;
}

// A fitted tab's side padding, both sides, in points: what its measured label leaves out.
const tabPadding = 24;

/**
 * Tabs as wide as their labels, in a row that scrolls. A button cannot be measured, so each
 * label is; the indicator sits under the chosen tab once every tab before it has a width.
 */
function FittedList({ id, label, items, index, onChange, testId }: FittedListProps) {
  const boxes = items.map(item => useBox(`tab-${item.value}`));
  const widths = boxes.map(box => (box.value ? box.value.width + tabPadding : undefined));
  const before = widths.slice(0, index);
  const width = widths[index];
  const isMeasured = width !== undefined && before.every(each => each !== undefined);
  return (
    <view className={r.frame.fitted}>
      <scroll className={r.scroller.base}>
        <view role="tablist" aria-label={label} data-testid={testId} className={r.list.fitted}>
          {isMeasured && (
            <view aria-hidden className={r.track.base}>
              <view dir="ltr" className={r.track.inner}>
                <view className={cx(r.indicator.base, r.indicator.fitted)} style={{ width, translate: [before.reduce<number>((sum, each) => sum + (each ?? 0), 0), 0] }} />
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
                className={r.tab.fitted}
              >
                <text box={boxes[at]} className={cx(r.label.base, r.label.whole, r.label.underline, isSelected ? r.label.selected : r.label.unselected)}>{item.label}</text>
              </button>
            );
          })}
        </view>
      </scroll>
    </view>
  );
}

/** Tabs over one panel: one Tab stop for the row, arrows move between tabs and choose on arrival. */
export function Tabs({ label, items, value, onChange, variant = 'underline', tabWidth = 'equal', children, className, 'data-testid': testId }: TabsProps) {
  const id = useId();
  const index = Math.max(0, items.findIndex(item => item.value === value));
  const chosen = items[index];
  const segmented = variant === 'segmented';
  return (
    <view className={cx(r.root.base, className)}>
      {!segmented && tabWidth === 'content'
        ? <FittedList id={id} label={label} items={items} index={index} onChange={onChange} testId={testId} />
        : (
          <view className={cx(r.frame.base, segmented ? s.root.base : r.frame.underline)}>
            <view role="tablist" aria-label={label} data-testid={testId} className={r.list.base}>
              {/* One box a tab wide, moved by its own width per tab. It moves in a left-to-right box that
                  right-to-left mirrors, so the same translate slides it toward the end either way. */}
              <view aria-hidden className={r.track.base}>
                <view dir="ltr" className={r.track.inner}>
                  <view
                    className={cx(r.indicator.base, segmented ? s.thumb.base : r.indicator.underline)}
                    style={{ width: `${100 / Math.max(1, items.length)}%`, translate: [`${index * 100}%`, 0] }}
                  />
                </view>
              </view>
              {items.map(item => {
                const isSelected = item.value === chosen?.value;
                return (
                  <button
                    key={item.value}
                    role="tab"
                    id={`${id}-${item.value}`}
                    aria-selected={isSelected}
                    data-state={isSelected ? 'active' : 'inactive'}
                    disabled={item.isDisabled}
                    data-testid={testId && `${testId}-${item.value}`}
                    onClick={() => { if (!isSelected) onChange(item.value); }}
                    className={cx(r.tab.base, segmented ? cx(s.segment.base, s.segment.size.md) : r.tab.underline)}
                  >
                    <text className={cx(r.label.base, r.label.cut, segmented ? s.label.size.md : r.label.underline, isSelected ? r.label.selected : r.label.unselected)}>{item.label}</text>
                  </button>
                );
              })}
            </view>
          </view>
        )}
      {children !== undefined && chosen && (
        <view role="tabpanel" aria-label={chosen.label} data-testid={testId && `${testId}-panel`} className={r.panel.base}>
          {children}
        </view>
      )}
    </view>
  );
}
