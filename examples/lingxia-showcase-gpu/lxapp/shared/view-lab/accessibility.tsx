// Accessibility demos of the View catalogue: each is the body of one demo.
import { useLocal } from '@lingxia/view';
import { Readout, useSay } from './demo';
import { labels, readouts } from './text';

// Five weeks of one month: days of the neighbouring months take no focus, PageUp and PageDown
// turn the month.
export function DaysDemo() {
  const month = useLocal('lab-days-month', 0);
  const day = useLocal('lab-days-day', 14);
  const t = useSay();
  const names = t(labels.months).split('|');
  // Each month starts one weekday later than the one before, so the grid visibly changes.
  const lead = 2 + month.value;
  const cells = Array.from({ length: 35 }, (_, index) => index - lead + 1);
  const turn = (direction: 'previous' | 'next') =>
    month.set(Math.max(0, Math.min(names.length - 1, month.value + (direction === 'next' ? 1 : -1))));
  return (
    <>
      <text data-testid="lab-days-month" className="text-sm font-semibold">{names[month.value]}</text>
      <view role="grid" aria-label={t(labels.daysName)} data-testid="lab-days-grid" onPage={turn} className="gap-1">
        {[0, 1, 2, 3, 4].map(week => (
          <view key={String(week)} role="row" className="flex-row gap-1">
            {cells.slice(week * 7, week * 7 + 7).map(date => {
              const inside = date >= 1 && date <= 30;
              const chosen = inside && date === day.value;
              return (
                <button key={String(date)} role="gridcell" data-testid={`lab-day-${date}`} disabled={!inside} aria-selected={chosen}
                  onClick={() => day.set(date)}
                  className={`grow basis-0 h-9 rounded-md text-sm items-center justify-center focus-visible:ring-2 focus-visible:ring-focus ${chosen ? 'bg-accent text-on-accent' : inside ? 'bg-surface-sunken' : 'text-faint'}`}>
                  {inside ? String(date) : ''}
                </button>
              );
            })}
          </view>
        ))}
      </view>
      <Readout testId="lab-days-state" state={`month=${month.value} day=${day.value}`}>{t(readouts.days(names[month.value], day.value))}</Readout>
    </>
  );
}

/** The Accessibility group: every demo the text table names, drawn by its body. */
export const accessibilityDemos = {
  days: DaysDemo,
};
