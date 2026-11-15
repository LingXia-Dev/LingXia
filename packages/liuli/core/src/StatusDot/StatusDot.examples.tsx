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
      <StatusDot label="In progress" tone="accent" />
      <StatusDot label="Draft" />
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
