import { Item, ItemList } from '../Item/Item';
import { StatusDot } from './StatusDot';

/** Services and how each stands: the word says it, the dot lets the eye find it. */
export function Services() {
  return (
    <ItemList label="Services">
      <Item label="Sync" endContent={<StatusDot label="Running" tone="success" size="sm" data-testid="sync" />} />
      <Item label="Search" endContent={<StatusDot label="Slow" tone="warning" size="sm" />} />
      <Item label="Exports" endContent={<StatusDot label="Down" tone="danger" size="sm" />} />
      <Item label="Backups" endContent={<StatusDot label="Paused" size="sm" data-testid="backups" />} />
    </ItemList>
  );
}

/** The five tones. */
export function Tones() {
  return (
    <view className="gap-2">
      <StatusDot label="Published" tone="success" />
      <StatusDot label="Needs review" tone="warning" />
      <StatusDot label="Rejected" tone="danger" />
      <StatusDot label="Scheduled" tone="info" data-testid="info" />
      <StatusDot label="In progress" tone="accent" />
      <StatusDot label="Draft" />
    </view>
  );
}

/** A hue says which, not how: the calendars an event can be on. */
export function Hues() {
  return (
    <view className="flex-row flex-wrap gap-x-5 gap-y-2">
      <StatusDot label="Work" hue={1} data-testid="hue-1" />
      <StatusDot label="Family" hue={2} />
      <StatusDot label="Travel" hue={3} />
      <StatusDot label="Garden" hue={4} />
      <StatusDot label="Birthdays" hue={5} />
      <StatusDot label="Choir" hue={6} />
      <StatusDot label="Swimming" hue={7} />
      <StatusDot label="Holidays" hue={8} />
    </view>
  );
}

/** Without its words, where they are already beside it. */
export function DotOnly() {
  return (
    <view className="flex-row items-center gap-2">
      <StatusDot label="Online" tone="success" isLabelHidden data-testid="bare" />
      <text className="text-body font-medium text-text">Ana Lima</text>
      <text className="text-footnote text-muted">Online</text>
    </view>
  );
}

/** The home tile. */
export function Specimen() {
  return <StatusDot label="Running" tone="success" />;
}
