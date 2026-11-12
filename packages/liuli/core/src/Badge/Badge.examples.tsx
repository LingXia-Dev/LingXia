import { Item } from '../Item/Item';
import { Badge } from './Badge';

export function Tones() {
  return (
    <view className="flex-row flex-wrap gap-2">
      <Badge label="Draft" data-testid="neutral" />
      <Badge label="New" tone="accent" data-testid="accent" />
      <Badge label="Paid" tone="success" />
      <Badge label="Due soon" tone="warning" />
      <Badge label="Overdue" tone="danger" data-testid="danger" />
    </view>
  );
}

/** Solid fills stand out; keep one per view. */
export function Solid() {
  return (
    <view className="flex-row flex-wrap gap-2">
      <Badge label="Beta" tone="accent" emphasis="solid" data-testid="solid" />
      <Badge label="Live" tone="success" emphasis="solid" />
      <Badge label="Paused" tone="warning" emphasis="solid" />
      <Badge label="Failed" tone="danger" emphasis="solid" />
    </view>
  );
}

const dot = [{ d: 'M12 8a4 4 0 1 0 0 8a4 4 0 1 0 0-8z', fill: 'currentColor' }] as const;

export function Sizes() {
  return (
    <view className="flex-row flex-wrap items-center gap-2">
      <Badge label="Small" size="sm" data-testid="small" />
      <Badge label="Online" tone="success" startIcon={<vector paths={dot} viewBox={[0, 0, 24, 24]} className="size-full" />} />
    </view>
  );
}

/** In a pressable row the end content is not read, so the status is repeated in the description. */
export function InARow() {
  return (
    <view className="rounded-container border border-line bg-surface">
      <Item
        label="Invoice 2041"
        description="Overdue, due 3 May"
        endContent={<Badge label="Overdue" tone="danger" size="sm" />}
        hasChevron
        onClick={() => {}}
        data-testid="invoice"
      />
    </view>
  );
}
