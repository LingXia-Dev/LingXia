import { useLocal } from '@lingxia/view';
import { Item } from '../Item/Item';
import { Tabs } from './Tabs';

const sections = [
  { value: 'overview', label: 'Overview' },
  { value: 'activity', label: 'Activity' },
  { value: 'members', label: 'Members' },
];

const summaries: Record<string, string> = {
  overview: 'Three notebooks, 128 notes, last edited this morning.',
  activity: 'Ana edited "Q3 plan" and two others today.',
  members: 'Ana Lima, Kai Chen and Mira Patel can edit.',
};

/** Sections of one page: the underline slides to the chosen tab. */
export function Sections() {
  const section = useLocal('section', 'overview');
  return (
    <Tabs label="Project" items={sections} value={section.value} onChange={next => section.set(next)} data-testid="project">
      <text className="text-body text-muted">{summaries[section.value]}</text>
    </Tabs>
  );
}

const settings = [
  { value: 'general', label: 'General' }, { value: 'notifications', label: 'Notifications' }, { value: 'privacy', label: 'Privacy and security' },
  { value: 'billing', label: 'Billing' }, { value: 'integrations', label: 'Integrations' }, { value: 'advanced', label: 'Advanced' },
];

/** More tabs than a phone has room for: each is as wide as its label and the row scrolls. */
export function Fitted() {
  const tab = useLocal('tab', 'general');
  return (
    <Tabs label="Settings" items={settings} value={tab.value} onChange={tab.set} tabWidth="content" data-testid="settings">
      <text className="text-body text-muted">{settings.find(item => item.value === tab.value)?.label}</text>
    </Tabs>
  );
}

const ranges = [
  { value: 'day', label: 'Day' },
  { value: 'week', label: 'Week' },
  { value: 'month', label: 'Month' },
];

/** Modes of one view: the thumb slides to the chosen segment. */
export function Segmented() {
  const range = useLocal('range', 'week');
  const totals: Record<string, string> = { day: '42 minutes', week: '5 hours 10 minutes', month: '21 hours' };
  return (
    <Tabs label="Range" items={ranges} value={range.value} onChange={next => range.set(next)} variant="segmented" data-testid="range">
      <view className="gap-0.5">
        <text className="text-footnote text-muted">Time spent writing</text>
        <text className="text-title font-semibold text-text tabular-nums">{totals[range.value]}</text>
      </view>
    </Tabs>
  );
}

const notes = {
  all: ['Q3 plan', 'Reading list', 'Interview notes'],
  pinned: ['Q3 plan'],
  shared: ['Interview notes'],
};

/** A list filtered by a segmented control above it. */
export function Filter() {
  const filter = useLocal('filter', 'all');
  return (
    <Tabs
      label="Show"
      items={[{ value: 'all', label: 'All' }, { value: 'pinned', label: 'Pinned' }, { value: 'shared', label: 'Shared' }]}
      value={filter.value}
      onChange={next => filter.set(next)}
      variant="segmented"
      className="gap-3"
      data-testid="filter"
    >
      <view className="-mx-5 -mb-5 divide-y divide-text/8 border-t border-text/8">
        {notes[filter.value as keyof typeof notes].map(note => <Item key={note} label={note} />)}
      </view>
    </Tabs>
  );
}

export function StateUnderline() {
  return <Tabs label="Sections" items={[{ value: 'notes', label: 'Notes' }, { value: 'files', label: 'Files' }]} value="notes" onChange={() => {}} />;
}

export function StateSegmented() {
  return <Tabs label="Range" items={ranges.slice(0, 2)} value="week" onChange={() => {}} variant="segmented" />;
}

export function StateDisabled() {
  return (
    <Tabs
      label="Sections"
      items={[{ value: 'notes', label: 'Notes' }, { value: 'billing', label: 'Billing', isDisabled: true }]}
      value="notes"
      onChange={() => {}}
    />
  );
}

export function StateSegmentedDisabled() {
  return (
    <Tabs
      label="Range"
      items={[{ value: 'week', label: 'Week' }, { value: 'year', label: 'Year', isDisabled: true }]}
      value="week"
      onChange={() => {}}
      variant="segmented"
    />
  );
}

/** The home tile. */
export function Specimen() {
  return <Tabs label="Range" items={ranges} value="week" onChange={() => {}} className="w-52" />;
}
