import { useLocal } from '@lingxia/view';
import { Held, unused } from '../example';
import { Item } from '../Item/Item';
import { Slider } from './Slider';

/** The value in words at the end of the label row; the app writes it. */
export function Volume() {
  const volume = useLocal('volume', 40);
  return <Slider bind={volume} label="Volume" valueLabel={`${volume.value}%`} data-testid="volume" />;
}

/** Whole steps: the value snaps to each one. */
export function Stepped() {
  const size = useLocal('size', 3);
  return (
    <Slider
      bind={size}
      label="Text size"
      valueLabel={['Smallest', 'Small', 'Default', 'Large', 'Largest'][size.value - 1]}
      min={1}
      max={5}
      description="Applies to notes and the reader."
      data-testid="text-size"
    />
  );
}

/** Among list rows: the row's label style, the value at the end. The stage's own padding is undone so the rows meet its edges. */
export function InList() {
  const speed = useLocal('speed', 2);
  return (
    <view className="-m-5 divide-y divide-line">
      <Item label="Read aloud" description="Notes are read in the system voice" />
      <Slider bind={speed} label="Speed" valueLabel={['Slow', 'Normal', 'Fast'][speed.value - 1]} min={1} max={3} variant="row" data-testid="speed" />
    </view>
  );
}

/** A position in a recording: the label is hidden, the times stay visible, the loaded part is quieter. */
export function Position() {
  const at = useLocal('at', 84);
  const time = (seconds: number) => `${Math.floor(seconds / 60)}:${String(seconds % 60).padStart(2, '0')}`;
  return (
    <view className="gap-1">
      <text className="text-body font-medium text-text">Interview with the architect</text>
      <Slider bind={at} label="Position" isLabelHidden valueLabel={`${time(at.value)} of 3:10`} max={190} secondaryValue={132} size="sm" data-testid="position" />
    </view>
  );
}

export function Sizes() {
  return (
    <view className="gap-4">
      <Held value={30}>{(level, set) => <Slider value={level} onChange={set} label="Small" size="sm" />}</Held>
      <Held value={60}>{(level, set) => <Slider value={level} onChange={set} label="Medium" />}</Held>
    </view>
  );
}

export function StateDefault() {
  return <Held value={40}>{(level, set) => <Slider value={level} onChange={set} label="Brightness" />}</Held>;
}

export function StateDisabled() {
  return <Slider value={40} onChange={unused} label="Brightness" description="Set by the system." isDisabled />;
}

/** The home tile. */
export function Specimen() {
  return <Slider value={60} onChange={unused} label="Volume" isLabelHidden className="w-28" />;
}

/** A short scale with its points named: ticks under the track, labels at the ends and the middle. */
export function Marks() {
  const size = useLocal('size', 2);
  const names = ['Small', 'Default', 'Large', 'Larger', 'Largest'];
  return (
    <Slider
      bind={size}
      label="Text size"
      valueLabel={names[size.value]}
      min={0}
      max={4}
      marks={[{ value: 0, label: 'Small' }, { value: 1 }, { value: 2 }, { value: 3 }, { value: 4, label: 'Largest' }]}
      data-testid="marked"
    />
  );
}

/** Saving waits for the finger to lift: the label follows the drag, the saved value follows its end. */
export function SaveOnRelease() {
  const level = useLocal('level', 60);
  const saved = useLocal('saved', 60);
  return (
    <view className="gap-2">
      <Slider bind={level} label="Brightness" valueLabel={`${level.value}%`} onChangeEnd={value => saved.set(value)} data-testid="brightness" />
      <text role="status" className="text-footnote text-muted">{`Saved at ${saved.value}%`}</text>
    </view>
  );
}
