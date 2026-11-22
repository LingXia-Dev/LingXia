import { useLocal } from '@lingxia/view';
import { Button } from '../Button/Button';
import { ProgressBar, Spinner, type TransferProgress } from './ProgressBar';

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
      <Spinner label="Loading" isLabelHidden size="sm" />
      <Spinner label="Loading messages" isLabelHidden data-testid="spinner" />
      <Spinner label="Loading" isLabelHidden size="lg" />
    </view>
  );
}

/** A whole panel that is loading says what, under the ring. */
export function LoadingMessage() {
  return (
    <view className="items-center py-6">
      <Spinner label="Loading your notes" size="lg" data-testid="loading-notes" />
    </view>
  );
}

/** The home tile. */
export function Specimen() {
  return (
    <view className="flex-row items-center gap-3">
      <ProgressBar label="Uploading" isLabelHidden value={62} className="w-20" />
      <Spinner label="Loading" isLabelHidden size="sm" />
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

/** Over a picture that is still loading, and beside quiet text. */
export function SpinnerTones() {
  return (
    <view className="gap-3">
      <view className="h-36 items-center justify-center rounded-container bg-media">
        <Spinner label="Loading the video" isLabelHidden size="xl" tone="on-media" data-testid="on-media" />
      </view>
      <view className="flex-row items-center gap-2">
        <Spinner label="Checking for updates" isLabelHidden size="sm" tone="muted" data-testid="quiet" />
        <text className="text-footnote text-muted">Checking for updates</text>
      </view>
    </view>
  );
}

// What a download's stream yields, in order: nothing yet, a size not known, then a known size from empty to full.
const yields: readonly (TransferProgress | null)[] = [
  null,
  { loaded: 1_200_000 },
  { loaded: 0, total: 10_000_000, fraction: 0 },
  { loaded: 3_100_000, total: 10_000_000, fraction: 0.31 },
  { loaded: 10_000_000, total: 10_000_000, fraction: 1 },
];
// The app words the amount; megabytes to one place, so the figures keep their width.
const megabytes = (bytes: number) => (bytes / 1_000_000).toFixed(1);

/** A transfer's progress as it streams: `progress={download.data}`, unknown until a size is known. */
export function Transfer() {
  const step = useLocal('step', 3);
  const value = yields[step.value];
  const words = value?.total === undefined ? undefined : `${megabytes(value.loaded)} of ${megabytes(value.total)} MB`;
  return (
    <view className="gap-3">
      <ProgressBar label="Downloading the map" progress={value} valueLabel={words} data-testid="download" />
      <Button
        label="Next value"
        size="sm"
        width="content"
        onClick={() => step.set((step.value + 1) % yields.length)}
        data-testid="next-value"
      />
    </view>
  );
}
