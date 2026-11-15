import { useLocal } from '@lingxia/view';
import { Button } from '../Button/Button';
import { ProgressBar, Spinner } from './ProgressBar';

/** The app words the amount; Liuli formats no numbers. */
export function Determinate() {
  const sent = useLocal('sent', 3);
  return (
    <view className="gap-3">
      <ProgressBar label="Uploading photos" value={sent.value} max={8} valueLabel={`${sent.value} of 8`} data-testid="upload" />
      <Button
        label="Send one more"
        size="sm"
        width="content"
        isDisabled={sent.value === 8}
        onClick={() => sent.set(sent.value + 1)}
        data-testid="more"
      />
    </view>
  );
}

export function Indeterminate() {
  return <ProgressBar label="Preparing the export" data-testid="export" />;
}

/** A thin bar under a heading that already says what is happening. */
export function Compact() {
  return (
    <view className="gap-2">
      <text className="text-footnote font-medium text-text">Storage</text>
      <ProgressBar label="Storage used" isLabelHidden size="sm" value={72} valueLabel="7.2 of 10 GB" data-testid="storage" />
    </view>
  );
}

export function Spinners() {
  return (
    <view className="flex-row items-center gap-4">
      <Spinner label="Loading" size="sm" />
      <Spinner label="Loading messages" data-testid="spinner" />
      <Spinner label="Loading" size="lg" />
    </view>
  );
}

/** The home tile. */
export function Specimen() {
  return (
    <view className="flex-row items-center gap-3">
      <ProgressBar label="Uploading" isLabelHidden value={62} className="w-20" />
      <Spinner label="Loading" size="sm" />
    </view>
  );
}

export function SizesBar() {
  return (
    <view className="gap-4">
      <ProgressBar label="Small" size="sm" value={40} />
      <ProgressBar label="Medium" value={40} />
    </view>
  );
}

export function StateStarted() {
  return <ProgressBar label="Sync" value={20} />;
}

export function StateHalf() {
  return <ProgressBar label="Sync" value={50} />;
}

export function StateDone() {
  return <ProgressBar label="Sync" value={100} />;
}

export function StateUnknown() {
  return <ProgressBar label="Sync" />;
}

/** Meters: the fill says how the amount stands, and the words say it too. */
export function Tones() {
  return (
    <view className="gap-4">
      <ProgressBar label="Monthly goal" value={100} valueLabel="Reached" tone="success" data-testid="tone-goal" />
      <ProgressBar label="Storage" value={86} valueLabel="43 of 50 GB, nearly full" tone="warning" data-testid="tone-storage" />
      <ProgressBar label="Data plan" value={100} valueLabel="Over by 1.2 GB" tone="danger" data-testid="tone-plan" />
    </view>
  );
}
