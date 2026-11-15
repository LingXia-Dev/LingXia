import { Avatar } from '../Avatar/Avatar';
import { StatusDot } from '../StatusDot/StatusDot';
import { HoverCard } from './HoverCard';

/** A name in a line of activity: resting on it shows who that is. */
export function Person() {
  return (
    <view className="flex-row flex-wrap items-baseline gap-1">
      <HoverCard label="Ana Lima" cardLabel="About Ana Lima" className="text-body font-medium" data-testid="ana">
        <view className="flex-row items-center gap-3">
          <Avatar name="Ana Lima" isDecorative />
          <view className="gap-0.5">
            <text className="text-body font-medium text-text">Ana Lima</text>
            <text className="text-footnote text-muted">Design lead · Lisbon</text>
          </view>
        </view>
        <StatusDot label="Online" tone="success" size="sm" />
      </HoverCard>
      <text className="text-body text-text">commented on Trip to the coast</text>
    </view>
  );
}

/** A term explained where it is used, opening beside it. */
export function Term() {
  return (
    <view className="flex-row flex-wrap items-baseline gap-1">
      <text className="text-footnote text-muted">Billed per</text>
      <HoverCard label="seat" placement="end" className="text-footnote" data-testid="seat">
        <text className="text-footnote text-text">A seat is one person who can edit. People who only read are free.</text>
      </HoverCard>
    </view>
  );
}

/** The home tile. */
export function Specimen() {
  return (
    <HoverCard label="Ana Lima" className="text-footnote font-medium">
      <text className="text-footnote text-text">Design lead</text>
    </HoverCard>
  );
}
