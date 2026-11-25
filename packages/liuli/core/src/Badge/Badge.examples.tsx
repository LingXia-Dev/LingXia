import { Said } from '../example';
import { Item } from '../Item/Item';
import { Badge } from './Badge';

export function Tones() {
  return (
    <view className="flex-row flex-wrap gap-2">
      <Badge label="Draft" data-testid="neutral" />
      <Badge label="New" tone="accent" data-testid="accent" />
      <Badge label="Scheduled" tone="info" data-testid="info" />
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
      <Badge label="Update" tone="info" emphasis="solid" />
      <Badge label="Live" tone="success" emphasis="solid" />
      <Badge label="Paused" tone="warning" emphasis="solid" />
      <Badge label="Failed" tone="danger" emphasis="solid" />
    </view>
  );
}

/** Kinds of things, not states: each project keeps its hue wherever it is named. */
export function Hues() {
  return (
    <view className="flex-row flex-wrap gap-2">
      <Badge label="Design" hue={1} data-testid="hue-1" />
      <Badge label="Research" hue={2} />
      <Badge label="Hardware" hue={3} />
      <Badge label="Field trials" hue={4} />
      <Badge label="Marketing" hue={5} />
      <Badge label="Legal" hue={6} />
      <Badge label="Support" hue={7} />
      <Badge label="Archive" hue={8} />
    </view>
  );
}

export function Sizes() {
  return (
    <view className="flex-row flex-wrap items-center gap-2">
      <Badge label="Small" size="sm" data-testid="small" />
      <Badge label="Online" tone="success" startIcon="success" />
    </view>
  );
}

/** In a pressable row the badge is read after the second line, as part of the row's description. */
export function InARow() {
  return (
    <Said>
      {say => (
        <view className="overflow-hidden rounded-container border border-line bg-surface">
          <Item
            label="Invoice 2041"
            description="Due 3 May"
            endContent={<Badge label="Overdue" tone="danger" size="sm" />}
            hasChevron
            onClick={() => say('Opened Invoice 2041')}
            data-testid="invoice"
          />
        </view>
      )}
    </Said>
  );
}

/** The home tile. */
export function Specimen() {
  return (
    <view className="flex-row gap-1.5">
      <Badge label="Paid" tone="success" />
      <Badge label="New" tone="accent" emphasis="solid" />
    </view>
  );
}
