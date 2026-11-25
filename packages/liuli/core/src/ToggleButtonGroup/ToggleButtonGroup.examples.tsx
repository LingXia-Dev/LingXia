import { useLocal } from '@lingxia/view';
import { unused } from '../example';
import { ToggleButtonGroup, type ToggleButtonGroupProps } from './ToggleButtonGroup';

const notes = [
  { title: 'Q3 plan', pinned: true, starred: true, files: false },
  { title: 'Reading list', pinned: false, starred: true, files: true },
  { title: 'Interview notes', pinned: true, starred: false, files: true },
  { title: 'Groceries', pinned: false, starred: false, files: false },
];

/** Any number on: filters that each narrow the list, read as toggle buttons. */
export function Filters() {
  const on = useLocal<readonly string[]>('on', ['starred']);
  const shown = notes.filter(note => (!on.value.includes('pinned') || note.pinned) && (!on.value.includes('starred') || note.starred) && (!on.value.includes('files') || note.files));
  return (
    <view className="gap-3">
      <ToggleButtonGroup
        label="Filters"
        selectionMode="multiple"
        size="sm"
        items={[
          { value: 'pinned', label: 'Pinned', icon: 'bookmark' },
          { value: 'starred', label: 'Starred', icon: 'star' },
          { value: 'files', label: 'With files', icon: 'attach' },
        ]}
        bind={on}
        data-testid="filters"
      />
      <text className="text-footnote text-muted" data-testid="filters-count">{shown.length === 1 ? '1 note' : `${shown.length} notes`}</text>
    </view>
  );
}

/** One on at a time, icon-only, in a toolbar: read as a radio group, each toggle named by its label. */
export function Layout() {
  const layout = useLocal('layout', 'grid');
  return (
    <view className="gap-3">
      <ToggleButtonGroup
        label="Layout"
        selectionMode="single"
        items={[
          { value: 'list', label: 'List', icon: 'list', isLabelHidden: true },
          { value: 'grid', label: 'Grid', icon: 'grid', isLabelHidden: true },
          { value: 'rows', label: 'Rows', icon: 'rows', isLabelHidden: true },
        ]}
        bind={layout}
        data-testid="layout"
      />
      <text className="text-footnote text-muted" data-testid="layout-shown">{`Showing notes as a ${layout.value}`}</text>
    </view>
  );
}

/** One on at a time, with labels. */
export function Single() {
  const pen = useLocal('pen', 'medium');
  return (
    <ToggleButtonGroup
      label="Pen"
      selectionMode="single"
      items={[
        { value: 'fine', label: 'Fine' },
        { value: 'medium', label: 'Medium' },
        { value: 'broad', label: 'Broad' },
      ]}
      bind={pen}
      data-testid="pen"
    />
  );
}

export function Sizes() {
  const items = [
    { value: 'list', label: 'List', icon: 'list', isLabelHidden: true },
    { value: 'grid', label: 'Grid', icon: 'grid', isLabelHidden: true },
  ] as const;
  return (
    <view className="gap-3">
      <ToggleButtonGroup label="Small" selectionMode="single" size="sm" items={items} value="grid" onChange={unused} />
      <ToggleButtonGroup label="Medium" selectionMode="single" items={items} value="grid" onChange={unused} />
      <ToggleButtonGroup label="Large" selectionMode="single" size="lg" items={items} value="grid" onChange={unused} />
    </view>
  );
}

export function StateDisabled() {
  return (
    <ToggleButtonGroup
      label="Filters"
      selectionMode="multiple"
      size="sm"
      items={[{ value: 'pinned', label: 'Pinned' }, { value: 'starred', label: 'Starred' }]}
      value={['starred']}
      onChange={unused}
      isDisabled
    />
  );
}

/** The home tile. */
export function Specimen() {
  return (
    <ToggleButtonGroup
      label="Layout"
      selectionMode="single"
      size="sm"
      items={[
        { value: 'list', label: 'List', icon: 'list', isLabelHidden: true },
        { value: 'grid', label: 'Grid', icon: 'grid', isLabelHidden: true },
        { value: 'rows', label: 'Rows', icon: 'rows', isLabelHidden: true },
      ]}
      value="grid"
      onChange={unused}
    />
  );
}

export function playground() {
  return { items: [{ value: 'fine', label: 'Fine' }, { value: 'broad', label: 'Broad' }] } satisfies Partial<ToggleButtonGroupProps>;
}
