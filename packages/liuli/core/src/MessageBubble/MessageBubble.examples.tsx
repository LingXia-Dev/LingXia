import { useLocal } from '@lingxia/view';
import { Avatar } from '../Avatar/Avatar';
import { Button } from '../Button/Button';
import { MessageBubble } from './MessageBubble';

/** A short exchange: theirs at the start with a name and a face, mine at the end; a run shares its inner corners. */
export function Exchange() {
  return (
    <view className="gap-1">
      <MessageBubble author="Kai Chen" avatar={<Avatar name="Kai Chen" size="sm" isDecorative />} text="Are we still on for the coast on Saturday?" isJoinedAfter data-testid="theirs" />
      <MessageBubble hasAvatarSpace text="I can drive if you bring the tent." isJoinedBefore meta="09:41" />
      <view className="h-2" />
      <MessageBubble side="end" text="Yes! Leaving at eight." isJoinedAfter data-testid="mine" />
      <MessageBubble side="end" text="The directions are at https://example.com/maps/place/a-very-long-address-that-does-not-fit-on-one-line" isJoinedBefore meta="09:42" data-testid="long" />
    </view>
  );
}

/** How a message of mine stands: on its way, sent, and not sent, with a way to try again. */
export function States() {
  const tries = useLocal('tries', 0);
  return (
    <view className="gap-3">
      <MessageBubble side="end" text="On my way." status="sending" meta="Sending" data-testid="sending" />
      <MessageBubble side="end" text="Parked by the gate." status="sent" meta="Sent · 10:02" />
      <MessageBubble
        side="end"
        text="Can you see the car?"
        status="failed"
        meta={tries.value === 0 ? 'Not sent' : `Not sent, tried ${tries.value + 1} times`}
        action={<Button label="Try again" variant="link" size="sm" onClick={() => tries.set(tries.value + 1)} data-testid="retry" />}
        data-testid="failed"
      />
    </view>
  );
}

/** The home tile. */
export function Specimen() {
  return (
    <view className="w-36 gap-1">
      <MessageBubble text="Saturday?" />
      <MessageBubble side="end" text="Yes!" />
    </view>
  );
}
