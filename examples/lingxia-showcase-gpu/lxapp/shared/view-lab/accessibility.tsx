// Accessibility demos of the View catalogue: each is the body of one demo.
import { List, useLocal } from '@lingxia/view';
import { Readout, useSay } from './demo';
import { labels, readouts } from './text';

// Two folders, each holding two files; a folder's press opens and closes it.
export function TreeDemo() {
  const open = useLocal<readonly string[]>('lab-tree-open', []);
  const t = useSay();
  const [documents, invoices, letters, pictures, holidays] = t(labels.treeItems).split('|');
  const toggle = (name: string) => () => open.set(open.value.includes(name) ? open.value.filter(other => other !== name) : [...open.value, name]);
  const row = 'px-3 py-2 rounded-md text-sm text-start active:bg-surface-sunken focus-visible:ring-2 focus-visible:ring-focus';
  const folder = (id: string, name: string, files: readonly [string, string][]) => (
    <>
      <button role="treeitem" aria-level={1} aria-expanded={open.value.includes(id)} data-testid={`lab-tree-${id}`} onClick={toggle(id)} className={row}>
        {`${open.value.includes(id) ? '▾' : '▸'} ${name}`}
      </button>
      {open.value.includes(id) ? files.map(([file, label]) => (
        <button key={file} role="treeitem" aria-level={2} data-testid={`lab-tree-${file}`} onClick={() => {}} className={`${row} ps-8`}>{label}</button>
      )) : null}
    </>
  );
  return (
    <>
      <view role="tree" aria-label={t(labels.treeName)} data-testid="lab-tree-list" className="gap-0.5">
        {folder('documents', documents, [['invoices', invoices], ['letters', letters]])}
        {folder('pictures', pictures, [['holidays', holidays]])}
      </view>
      <Readout testId="lab-tree-state" state={`open=${open.value.join(',') || 'none'}`}>{t(readouts.tree(open.value.join(', ') || 'none'))}</Readout>
    </>
  );
}

// A short list of messages that a screen reader meets by name.
export function NamedListDemo() {
  const t = useSay();
  const rows = t(labels.namedRows).split('|');
  return (
    <List aria-label={t(labels.namedList)} data-testid="lab-named-list" className="h-28 rounded-lg border border-line" items={rows} getKey={row => row}
      renderItem={row => <text className="px-3 py-2 text-sm">{row}</text>} />
  );
}

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
  tree: TreeDemo,
  named: NamedListDemo,
  days: DaysDemo,
};
