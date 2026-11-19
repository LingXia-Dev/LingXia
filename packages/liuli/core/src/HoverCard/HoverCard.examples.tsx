import { Avatar } from '../Avatar/Avatar';
import { StatusDot } from '../StatusDot/StatusDot';
import { HoverCard } from './HoverCard';
import { Link } from '../Link/Link';

/** A name in a line of activity: resting on it shows who that is. A dotted underline sets a term apart from a link. */
export function Person() {
  return (
    <view className="flex-row flex-wrap items-baseline gap-1">
      <HoverCard
        label="About Ana Lima"
        content={<>
          <view className="flex-row items-center gap-3">
            <Avatar name="Ana Lima" isDecorative />
            <view className="gap-0.5">
              <text className="text-body font-medium text-text">Ana Lima</text>
              <text className="text-footnote text-muted">Design lead · Lisbon</text>
            </view>
          </view>
          <StatusDot label="Online" tone="success" size="sm" />
        </>}
        data-testid="ana"
      >
        <text className="text-body font-medium text-text">Ana Lima</text>
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
      <HoverCard label="Seat" placement="end" content={<text className="text-footnote text-text">A seat is one person who can edit. People who only read are free.</text>} data-testid="seat">
        <text className="text-footnote text-text">seat</text>
      </HoverCard>
    </view>
  );
}

/** The home tile. */
export function Specimen() {
  return (
    <HoverCard label="About Ana Lima" content={<text className="text-footnote text-text">Design lead</text>}>
      <text className="text-footnote font-medium text-text underline decoration-dotted decoration-line-strong">Ana Lima</text>
    </HoverCard>
  );
}

/** On a link in a sentence: a pointer resting on the name, or a press, shows the card. */
export function InSentence() {
  return (
    <text className="text-body text-text">
      Approved by{' '}
      <HoverCard label="Ana Lima" content={<text className="text-footnote text-text">Ana Lima, Finance. Approves purchases over €500.</text>} data-testid="approver">
        <Link label="Ana Lima" data-testid="approver-link" />
      </HoverCard>
      {' '}yesterday.
    </text>
  );
}
