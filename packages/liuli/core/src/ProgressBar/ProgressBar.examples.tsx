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
      <text className="text-sm font-medium text-text">Storage</text>
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
