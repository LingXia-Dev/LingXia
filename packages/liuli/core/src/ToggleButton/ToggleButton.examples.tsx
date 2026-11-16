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
        <ToggleButton label="Unread" size="sm" isPressed={unread.value} onPressedChange={next => unread.set(next)} data-testid="unread" />
        <ToggleButton label="Starred" size="sm" isPressed={starred.value} onPressedChange={next => starred.set(next)} data-testid="starred" />
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
      <ToggleButton label="Wrap lines" isPressed={wrap.value} onPressedChange={next => wrap.set(next)} className="self-start" data-testid="wrap" />
      <text className={wrap.value ? 'text-body text-text' : 'text-body text-text truncate'} data-testid="sample">
        Every line of this note is long enough to reach past the edge of a phone held upright.
      </text>
    </view>
  );
}

export function Sizes() {
  return (
    <view className="flex-row flex-wrap items-center gap-2">
      <Held value={true}>{(isPressed, set) => <ToggleButton label="Small" size="sm" isPressed={isPressed} onPressedChange={set} />}</Held>
      <Held value={true}>{(isPressed, set) => <ToggleButton label="Medium" isPressed={isPressed} onPressedChange={set} />}</Held>
      <Held value={true}>{(isPressed, set) => <ToggleButton label="Large" size="lg" isPressed={isPressed} onPressedChange={set} />}</Held>
    </view>
  );
}

export function StateOff() {
  return <Held value={false}>{(isPressed, set) => <ToggleButton label="Starred" isPressed={isPressed} onPressedChange={set} className="self-start" />}</Held>;
}

export function StateOn() {
  return <Held value={true}>{(isPressed, set) => <ToggleButton label="Starred" isPressed={isPressed} onPressedChange={set} className="self-start" />}</Held>;
}

export function StateDisabled() {
  return <ToggleButton label="Starred" isPressed={false} isDisabled onPressedChange={unused} className="self-start" />;
}

export function StateDisabledOn() {
  return <ToggleButton label="Starred" isPressed isDisabled onPressedChange={unused} className="self-start" />;
}

/** The home tile. */
export function Specimen() {
  return (
    <view className="flex-row gap-2">
      <ToggleButton label="Bold" size="sm" isPressed onPressedChange={unused} />
      <ToggleButton label="Italic" size="sm" isPressed={false} onPressedChange={unused} />
    </view>
  );
}
