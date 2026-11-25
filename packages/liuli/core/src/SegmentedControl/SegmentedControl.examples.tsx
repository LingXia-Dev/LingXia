import { useLocal } from '@lingxia/view';
import { Held, unused } from '../example';
import { SegmentedControl, type SegmentedControlProps } from './SegmentedControl';

/** A unit for the numbers below it, bound to a View-local cell. */
export function Units() {
  const unit = useLocal('unit', 'km');
  const distance = unit.value === 'km' ? '12.4 km' : '7.7 mi';
  return (
    <view className="gap-3">
      <SegmentedControl
        label="Units"
        items={[{ value: 'km', label: 'Kilometres' }, { value: 'mi', label: 'Miles' }]}
        bind={unit}
        data-testid="units"
      />
      <view className="gap-0.5">
        <text className="text-footnote text-muted">This week</text>
        <text className="text-title font-semibold text-text tabular-nums" data-testid="distance">{distance}</text>
      </view>
    </view>
  );
}

const themes = [
  { value: 'system', label: 'System' },
  { value: 'light', label: 'Light' },
  { value: 'dark', label: 'Dark' },
];

/** A setting row: the control sits at the end, sized to its segments. */
export function Appearance() {
  const theme = useLocal('theme', 'system');
  return (
    <view className="flex-row items-center gap-4">
      <text className="grow basis-0 text-body text-text">Appearance</text>
      <SegmentedControl label="Appearance" items={themes} value={theme.value} onChange={next => theme.set(next)} size="sm" className="w-56" data-testid="appearance" />
    </view>
  );
}

const views = [
  { value: 'list', label: 'List', icon: 'list', isLabelHidden: true },
  { value: 'grid', label: 'Grid', icon: 'grid', isLabelHidden: true },
  { value: 'rows', label: 'Compact', icon: 'rows', isLabelHidden: true },
] as const;

const chartKinds = [
  { value: 'chart', label: 'Chart', icon: 'chart' },
  { value: 'table', label: 'Table', icon: 'list' },
] as const;

/** A view switcher: icons alone, each still named by its label, and icons beside labels. */
export function Icons() {
  const view = useLocal('view', 'grid');
  const shown = useLocal('shown', 'chart');
  return (
    <view className="flex-row flex-wrap items-center gap-4">
      <SegmentedControl label="View" items={views} bind={view} size="sm" data-testid="view" />
      <SegmentedControl label="Show as" items={chartKinds} bind={shown} data-testid="show-as" />
    </view>
  );
}

export function Sizes() {
  return (
    <view className="gap-3">
      <Held value="light">{(theme, set) => <SegmentedControl label="Small" items={themes} value={theme} onChange={set} size="sm" />}</Held>
      <Held value="light">{(theme, set) => <SegmentedControl label="Medium" items={themes} value={theme} onChange={set} />}</Held>
    </view>
  );
}

export function StateTwo() {
  return <Held value="list">{(view, set) => <SegmentedControl label="View" items={[{ value: 'list', label: 'List' }, { value: 'grid', label: 'Grid' }]} value={view} onChange={set} />}</Held>;
}

export function StateFive() {
  return (
    <Held value="4">
      {(rating, set) => (
        <SegmentedControl
          label="Rating"
          items={['1', '2', '3', '4', '5'].map(value => ({ value, label: value }))}
          value={rating}
          onChange={set}
          size="sm"
        />
      )}
    </Held>
  );
}

export function StateDisabled() {
  return (
    <Held value="auto">
      {(quality, set) => (
        <SegmentedControl
          label="Quality"
          items={[{ value: 'auto', label: 'Auto' }, { value: 'high', label: 'High' }, { value: 'max', label: 'Max', isDisabled: true }]}
          value={quality}
          onChange={set}
          size="sm"
          data-testid="quality"
        />
      )}
    </Held>
  );
}

/** The home tile: two short segments, so it fits a half tile at 360 pt. */
export function Specimen() {
  return <SegmentedControl label="Layout" items={[{ value: 'list', label: 'List' }, { value: 'grid', label: 'Grid' }]} value="list" onChange={unused} size="sm" className="w-32" />;
}

/** The playground's props that are not plain values. */
export function playground() {
  return { items: [{ value: 'km', label: 'Kilometres' }, { value: 'mi', label: 'Miles' }] } satisfies Partial<SegmentedControlProps>;
}
