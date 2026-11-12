import { useLocal } from '@lingxia/view';
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

export function Sizes() {
  const level = useLocal('level', 60);
  return (
    <view className="gap-4">
      <Slider bind={level} label="Small" size="sm" valueLabel={`${level.value}%`} />
      <Slider bind={level} label="Medium" valueLabel={`${level.value}%`} />
    </view>
  );
}

/** A position in a recording: the label is hidden, the times stay visible. */
export function Position() {
  const at = useLocal('at', 84);
  const time = (seconds: number) => `${Math.floor(seconds / 60)}:${String(seconds % 60).padStart(2, '0')}`;
  return (
    <view className="gap-1">
      <text className="text-body font-medium text-text">Interview with the architect</text>
      <Slider bind={at} label="Position" isLabelHidden valueLabel={`${time(at.value)} of 3:10`} max={190} data-testid="position" />
    </view>
  );
}

export function StateDefault() {
  return <Slider value={40} onChange={() => {}} label="Brightness" />;
}

export function StateDisabled() {
  return <Slider value={40} onChange={() => {}} label="Brightness" description="Set by the system." isDisabled />;
}

/** The home tile. */
export function Specimen() {
  return <Slider value={60} onChange={() => {}} label="Volume" isLabelHidden size="sm" className="w-28" />;
}
