// Accessibility demos of the View catalogue: each is the body of one demo.
import { List, useLocal } from '@lingxia/view';
import { Button } from '../ui';
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
      <button role="treeitem" aria-level={1} aria-expanded={open.value.includes(id)} data-testid={`lab-tree-${id}`} onClick={toggle(id)} className={`${row} flex-row gap-2`}>
        <text aria-hidden>{open.value.includes(id) ? '▾' : '▸'}</text>
        <text>{name}</text>
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
                  className={`grow basis-0 h-9 rounded-md text-sm items-center justify-center focus-visible:ring-2 focus-visible:ring-focus ${chosen ? 'bg-accent-solid text-on-accent-solid' : inside ? 'bg-surface-sunken' : 'text-faint'}`}>
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

export function NamesDemo() {
  const saves = useLocal('saves', 0);
  const t = useSay();
  return (
    <>
      <text data-testid="lab-a11y-heading" role="heading" headingLevel={3} className="text-base font-semibold">{t(labels.bills)}</text>
      <view data-testid="lab-a11y-list" role="list" aria-label="Bills" className="gap-1">
        {t(labels.billRows).split('|').map(bill => (
          <view key={bill} role="listitem" className="flex-row justify-between">
            <text className="text-sm">{bill}</text>
            <vector aria-hidden className="w-4 h-4 text-muted" paths={[{ d: 'M9 18l6-6-6-6', fill: 'none', stroke: 'currentColor', strokeWidth: 1.5 }]} />
          </view>
        ))}
      </view>
      <view className="flex-row items-center gap-3">
        <Button testId="lab-a11y-save" size="sm" onClick={() => saves.set(saves.value + 1)}>{t(labels.save)}</Button>
        <text data-testid="lab-a11y-status" role="status" className="text-sm text-muted">{t(readouts.saved(saves.value))}</text>
      </view>
    </>
  );
}

// Tabs, a toolbar and a tree: one Tab stop each, arrows move between items, tabs activate on arrival.
export function GroupsDemo() {
  const range = useLocal('composite-range', 'week');
  const bold = useLocal('composite-bold', false);
  const open = useLocal('composite-open', true);
  const t = useSay();
  const names = t(labels.ranges).split('|');
  const values = ['day', 'week', 'month'];
  const chip = 'px-3 py-1 rounded-lg text-sm focus-visible:ring-2 focus-visible:ring-focus';
  const node = (label: string, level: number, testId: string, expanded?: boolean) => (
    <button role="treeitem" aria-level={level} aria-expanded={expanded} data-testid={testId}
      onClick={expanded === undefined ? () => {} : () => open.set(!open.value)} className="text-sm text-start py-1 focus-visible:ring-2 focus-visible:ring-focus" style={{ paddingLeft: level * 12 }}>{label}</button>
  );
  return (
    <>
      <view role="tablist" aria-label="Range" className="flex-row gap-2">
        {values.map((value, index) => (
          <button key={value} role="tab" aria-selected={range.value === value} data-testid={`lab-range-${value}`} onClick={() => range.set(value)}
            className={`${chip} ${range.value === value ? 'bg-accent-solid text-on-accent-solid' : 'bg-surface-sunken'}`}>{names[index]}</button>
        ))}
      </view>
      <view role="tabpanel" aria-label="Range view" data-testid="lab-range-panel">
        <text className="text-sm">{t(readouts.showing(range.value))}</text>
      </view>
      <view role="toolbar" aria-label="Format" className="flex-row gap-2">
        <button data-testid="lab-tool-bold" aria-pressed={bold.value} onClick={() => bold.set(!bold.value)} className={`${chip} ${bold.value ? 'bg-accent-soft' : 'bg-surface-sunken'}`}>{t(labels.bold)}</button>
        <button data-testid="lab-tool-copy" onClick={() => {}} className={`${chip} bg-surface-sunken`}>{t(labels.copy)}</button>
      </view>
      <view role="tree" aria-label="Files" className="gap-0">
        {node('src', 1, 'lab-tree-src', open.value)}
        {open.value ? node('main.ts', 2, 'lab-tree-main') : null}
        {node('docs', 1, 'lab-tree-docs')}
      </view>
    </>
  );
}

// A dialog that keeps focus inside it, a button that shows focus only from a key, and a hint
// that opens while a key or a resting pointer is on its trigger.
export function FocusDemo() {
  const open = useLocal('dialog-open', false);
  const choice = useLocal('dialog-choice', 'none');
  const t = useSay();
  const close = (value: string) => { choice.set(value); open.set(false); };
  return (
    <>
      <view className="flex-row flex-wrap gap-2 items-center">
        <Button testId="lab-focus-open" size="sm" onClick={() => open.set(true)}>{t(labels.openDialog)}</Button>
        <button data-testid="lab-focus-visible" onClick={() => {}} className="rounded-xl px-4 py-2 text-sm bg-surface-sunken focus-visible:bg-accent-solid focus-visible:text-on-accent-solid">{t(labels.ringByKey)}</button>
        <button data-testid="lab-interest" interestFor="lab-interest-hint" aria-label="About focus" className="rounded-full w-8 h-8 items-center justify-center bg-surface-sunken focus-visible:bg-accent-solid focus-visible:text-on-accent-solid"><text className="text-sm">?</text></button>
      </view>
      <Readout testId="lab-focus-choice" state={`chosen=${choice.value}`}>{t(readouts.chosen(choice.value))}</Readout>
      <view id="lab-interest-hint" popover="hint" role="tooltip" data-testid="lab-interest-hint" className="max-w-56 px-3 py-2 rounded-lg border border-line bg-surface-raised">
        <text className="text-xs text-text">{t(labels.focusHint)}</text>
      </view>
      <dialog open={open.value} onClose={() => close('escape')} data-testid="lab-focus-dialog" aria-label={t(labels.confirmPayment)} className="w-72 gap-3 p-4 rounded-xl border border-line bg-surface">
        <text role="heading" headingLevel={3} className="text-sm font-semibold">{t(labels.confirmPayment)}</text>
        <view className="flex-row gap-2">
          <Button testId="lab-focus-confirm" onClick={() => close('confirm')}>{t(labels.confirm)}</Button>
          <Button testId="lab-focus-cancel" variant="secondary" onClick={() => close('cancel')}>{t(labels.cancel)}</Button>
        </view>
      </dialog>
    </>
  );
}

// Dev only: a page whose View throws on every render, one whose first state never comes, and one
// whose update fails after it has shown, for the host's recovery panels.
function RecoveryDemo() {
  const t = useSay();
  return (
    <view className="flex-row flex-wrap gap-2">
      <Button testId="lab-recovery-open" size="sm" variant="secondary" to={{ page: 'spec-fault' }}>{t(labels.openFailing)}</Button>
      <Button testId="lab-recovery-stall" size="sm" variant="secondary" to={{ page: 'spec-stall' }}>{t(labels.openStalled)}</Button>
      <Button testId="lab-recovery-break" size="sm" variant="secondary" to={{ page: 'spec-break' }}>{t(labels.openBreaking)}</Button>
    </view>
  );
}

/** The Accessibility group: every demo the text table names, drawn by its body. */
export const accessibilityDemos = {
  names: NamesDemo,
  groups: GroupsDemo,
  focus: FocusDemo,
  tree: TreeDemo,
  named: NamedListDemo,
  days: DaysDemo,
  recovery: RecoveryDemo,
};
