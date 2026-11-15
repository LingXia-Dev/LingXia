import { Button } from '../Button/Button';
import { Divider } from './Divider';

/** Between two blocks of a card that belong apart. */
export function Between() {
  return (
    <view className="gap-4">
      <view className="gap-1">
        <text className="text-body font-medium text-text">Delivery</text>
        <text className="text-footnote text-muted">Tuesday 17 November, between 9 and 12</text>
      </view>
      <Divider data-testid="line" />
      <view className="gap-1">
        <text className="text-body font-medium text-text">Payment</text>
        <text className="text-footnote text-muted">Card ending 4421</text>
      </view>
    </view>
  );
}

/** A word on the line, between two ways to do the same thing. */
export function Labelled() {
  return (
    <view className="gap-4">
      <Button label="Continue with email" variant="primary" width="fill" />
      <Divider label="or" data-testid="or" />
      <Button label="Use a passkey" width="fill" />
    </view>
  );
}

/** Upright, between items in a row. */
export function Vertical() {
  return (
    <view className="flex-row items-center gap-3 h-6">
      <text className="text-footnote text-muted">12 notes</text>
      <Divider orientation="vertical" data-testid="upright" />
      <text className="text-footnote text-muted">3 shared</text>
      <Divider orientation="vertical" />
      <text className="text-footnote text-muted">Edited today</text>
    </view>
  );
}

/** The home tile. */
export function Specimen() {
  return (
    <view className="w-28 gap-2">
      <text className="text-footnote text-muted">Above</text>
      <Divider />
      <text className="text-footnote text-muted">Below</text>
    </view>
  );
}
