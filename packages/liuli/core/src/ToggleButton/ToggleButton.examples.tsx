import { useLocal } from '@lingxia/view';
import { Held, unused } from '../example';
import { ToggleButton } from './ToggleButton';

const notes = [
  { title: 'Q3 plan', unread: true, starred: true },
  { title: 'Reading list', unread: false, starred: true },
  { title: 'Interview notes', unread: true, starred: false },
  { title: 'Groceries', unread: false, starred: false },
];

/** Filters over a list: each one narrows it on its own. */
export function Filters() {
  const unread = useLocal('unread', false);
  const starred = useLocal('starred', false);
  const shown = notes.filter(note => (!unread.value || note.unread) && (!starred.value || note.starred));
  return (
    <view className="gap-3">
      <view role="group" aria-label="Filters" className="flex-row flex-wrap gap-2">
        <ToggleButton label="Unread" size="sm" bind={unread} data-testid="unread" />
        <ToggleButton label="Starred" size="sm" bind={starred} data-testid="starred" />
      </view>
      <text className="text-footnote text-muted" data-testid="count">{shown.length === 1 ? '1 note' : `${shown.length} notes`}</text>
    </view>
  );
}

/** One view option, beside the thing it changes. */
export function WordWrap() {
  const wrap = useLocal('wrap', true);
  return (
    <view className="gap-3">
      <ToggleButton label="Wrap lines" bind={wrap} className="self-start" data-testid="wrap" />
      <text className={wrap.value ? 'text-body text-text' : 'text-body text-text truncate'} data-testid="sample">
        Every line of this note is long enough to reach past the edge of a phone held upright.
      </text>
    </view>
  );
}

export function Sizes() {
  return (
    <view className="flex-row flex-wrap items-center gap-2">
      <Held value={true}>{(isPressed, set) => <ToggleButton label="Small" size="sm" value={isPressed} onChange={set} />}</Held>
      <Held value={true}>{(isPressed, set) => <ToggleButton label="Medium" value={isPressed} onChange={set} />}</Held>
      <Held value={true}>{(isPressed, set) => <ToggleButton label="Large" size="lg" value={isPressed} onChange={set} />}</Held>
    </view>
  );
}

export function StateOff() {
  return <Held value={false}>{(isPressed, set) => <ToggleButton label="Starred" value={isPressed} onChange={set} className="self-start" />}</Held>;
}

export function StateOn() {
  return <Held value={true}>{(isPressed, set) => <ToggleButton label="Starred" value={isPressed} onChange={set} className="self-start" />}</Held>;
}

export function StateDisabled() {
  return <ToggleButton label="Starred" value={false} isDisabled onChange={unused} className="self-start" />;
}

export function StateDisabledOn() {
  return <ToggleButton label="Starred" value isDisabled onChange={unused} className="self-start" />;
}

/** The home tile. */
export function Specimen() {
  return (
    <view className="flex-row gap-2">
      <ToggleButton label="Bold" size="sm" value onChange={unused} />
      <ToggleButton label="Italic" size="sm" value={false} onChange={unused} />
    </view>
  );
}
