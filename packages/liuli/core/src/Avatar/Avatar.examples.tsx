import { Badge } from '../Badge/Badge';
import { hueFor } from '../hue';
import { Avatar, AvatarGroup } from './Avatar';

// Pictures in the gallery package; a person's own photo goes here.
const photo = (n: number) => `public/avatars/avatar-${n}.png`;

/** A photo, and initials where there is none or it fails to load. */
export function People() {
  return (
    <view className="flex-row items-center gap-3">
      <Avatar name="Ana Lima" src={photo(1)} data-testid="ana" />
      <Avatar name="Kai Chen" data-testid="kai" />
      <Avatar name="Mira Patel" src="public/avatars/missing.png" data-testid="mira" />
    </view>
  );
}

export function Sizes() {
  return (
    <view className="flex-row items-center gap-3">
      <Avatar name="Ana Lima" size="xs" />
      <Avatar name="Ana Lima" size="sm" />
      <Avatar name="Ana Lima" size="md" />
      <Avatar name="Ana Lima" size="lg" />
    </view>
  );
}

/** Whether each person is around, as a dot on the avatar and in words for a screen reader. */
export function Presence() {
  return (
    <view className="flex-row items-center gap-4">
      <Avatar name="Ana Lima" src={photo(1)} status="online" statusLabel="Online" data-testid="online" />
      <Avatar name="Kai Chen" status="away" statusLabel="Away" data-testid="away" />
      <Avatar name="Mira Patel" status="busy" statusLabel="Do not disturb" data-testid="busy" />
      <Avatar name="Sam Ortiz" status="offline" statusLabel="Offline" data-testid="offline" />
      <Avatar name="Design team" size="lg" status="online" statusLabel="3 online" />
    </view>
  );
}

/** A count on the avatar: unread messages from this person. */
export function Unread() {
  return <Avatar name="Kai Chen" badge={<Badge label="3" tone="danger" emphasis="solid" size="sm" />} data-testid="unread" />;
}

/** Overlapping, with the rest folded into a count. */
export function Group() {
  return (
    <AvatarGroup label="Editors" max={4} overflowLabel={count => `${count} more editors`} onOverflowClick={() => {}} data-testid="editors">
      <Avatar name="Ana Lima" src={photo(1)} />
      <Avatar name="Kai Chen" />
      <Avatar name="Mira Patel" src={photo(2)} />
      <Avatar name="Tomás Ruiz" />
      <Avatar name="Lena Vogel" />
      <Avatar name="Sam Okafor" />
    </AvatarGroup>
  );
}

/** Beside the name it stands for, so it is decorative. */
export function WithName() {
  return (
    <view className="flex-row items-center gap-3">
      <Avatar name="Kai Chen" isDecorative />
      <view className="gap-0.5">
        <text className="text-body font-medium text-text">Kai Chen</text>
        <text className="text-footnote text-muted">Edited 2 minutes ago</text>
      </view>
    </view>
  );
}

export function StatePicture() {
  return <Avatar name="Ana Lima" src={photo(1)} size="lg" />;
}

export function StateInitials() {
  return <Avatar name="Kai Chen" size="lg" />;
}

export function StateFailed() {
  return <Avatar name="Mira Patel" src="public/avatars/missing.png" size="lg" />;
}

export function StateOverflow() {
  return (
    <AvatarGroup label="Editors" max={1} size="lg" overflowLabel={count => `${count} more editors`}>
      <Avatar name="Ana Lima" src={photo(1)} />
      <Avatar name="Kai Chen" />
      <Avatar name="Mira Patel" />
    </AvatarGroup>
  );
}

/** The home tile. */
export function Specimen() {
  return (
    <AvatarGroup label="Editors" size="sm" surface="sunken">
      <Avatar name="Ana Lima" />
      <Avatar name="Kai Chen" />
      <Avatar name="Mira Patel" />
    </AvatarGroup>
  );
}

/** A rounded square for what is not a person: a team, a workspace. */
/** Each name gets its own hue, the same one every time, so a list of initials is not a column of one colour. */
export function Hues() {
  const people = ['Ana Lima', 'Kai Chen', 'Mira Patel', 'Tomas Reyes', 'Yuki Sato', 'Noor Haddad'];
  return (
    <view className="flex-row flex-wrap gap-3">
      {people.map(name => <Avatar key={name} name={name} hue={hueFor(name)} data-testid={`hue-${name.split(' ')[0].toLowerCase()}`} />)}
    </view>
  );
}

export function Teams() {
  return (
    <view className="gap-3">
      <view className="flex-row items-center gap-3">
        <Avatar name="Design Systems" shape="rounded" isDecorative data-testid="team" />
        <view className="gap-0.5">
          <text className="text-body font-medium text-text">Design Systems</text>
          <text className="text-footnote text-muted">Team · 12 members</text>
        </view>
      </view>
      <view className="flex-row items-center gap-3">
        <Avatar name="Ana Lima" isDecorative />
        <view className="gap-0.5">
          <text className="text-body font-medium text-text">Ana Lima</text>
          <text className="text-footnote text-muted">Owner</text>
        </view>
      </view>
    </view>
  );
}
