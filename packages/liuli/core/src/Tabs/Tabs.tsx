import { cx, useId, type Child } from '@lingxia/view';
import { segmentedControlRecipe as s } from '../SegmentedControl/SegmentedControl.recipe';
import { tabsRecipe as r } from './Tabs.recipe';

export type TabsVariant = 'underline' | 'segmented';

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
  /** The tabs, in order; they share the row equally. */
  items: readonly TabItem[];
  /** The chosen tab's value. */
  value: string;
  /** Runs with the tab chosen by a tap or by arrowing onto it. */
  onChange: (value: string) => void;
  /** `underline` switches between sections of a page; `segmented` switches one view between modes. @default 'underline' */
  variant?: TabsVariant;
  /** The chosen tab's panel. */
  children?: Child;
  className?: string;
  /** On the tab list; each tab gets `<testid>-<value>`, the panel `<testid>-panel`. */
  'data-testid'?: string;
}

/** Tabs over one panel: one Tab stop for the row, arrows move between tabs and choose on arrival. */
export function Tabs({ label, items, value, onChange, variant = 'underline', children, className, 'data-testid': testId }: TabsProps) {
  const id = useId();
  const index = Math.max(0, items.findIndex(item => item.value === value));
  const chosen = items[index];
  const segmented = variant === 'segmented';
  return (
    <view className={cx(r.root.base, className)}>
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
              <text className={cx(r.label.base, segmented ? s.label.size.md : r.label.underline, isSelected ? r.label.selected : r.label.unselected)}>{item.label}</text>
            </button>
          );
        })}
      </view>
      </view>
      {children !== undefined && chosen && (
        <view role="tabpanel" aria-label={chosen.label} data-testid={testId && `${testId}-panel`} className={r.panel.base}>
          {children}
        </view>
      )}
    </view>
  );
}
