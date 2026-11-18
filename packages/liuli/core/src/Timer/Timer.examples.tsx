import { useLocal } from '@lingxia/view';
import { Button } from '../Button/Button';
import { Timer } from './Timer';

/** A countdown the app runs: here each press takes fifteen seconds off. */
export function Countdown() {
  const left = useLocal('left', 125);
  return (
    <view className="items-start gap-3">
      <Timer seconds={left.value} size="lg" label={`${Math.floor(left.value / 60)} minutes ${left.value % 60} seconds left`} data-testid="countdown" />
      <view className="flex-row gap-2">
        <Button label="Skip 15 seconds" onClick={() => left.set(Math.max(0, left.value - 15))} data-testid="skip" />
        <Button label="Reset" variant="ghost" onClick={() => left.set(125)} />
      </view>
    </view>
  );
}

/** In a line of text: how long a code stays valid, how long a call has run. */
export function Inline() {
  return (
    <view className="gap-2">
      <view className="flex-row items-baseline gap-1.5">
        <text className="text-footnote text-muted">Code expires in</text>
        <Timer seconds={272} size="sm" data-testid="expires" />
      </view>
      <view className="flex-row items-baseline gap-1.5">
        <text className="text-body text-text">On a call</text>
        <Timer seconds={4325} data-testid="call" />
      </view>
    </view>
  );
}

/** The home tile. */
export function Specimen() {
  return <Timer seconds={125} size="lg" />;
}
