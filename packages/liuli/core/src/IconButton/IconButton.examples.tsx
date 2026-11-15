import { useLocal } from '@lingxia/view';
import { Item } from '../Item/Item';
import { IconButton } from './IconButton';

/** The label is never drawn and always read: name what the press does. */
export function Toolbar() {
  const liked = useLocal('liked', 0);
  return (
    <view className="flex-row items-center gap-1">
      <IconButton label="Like" icon="heart" onClick={() => liked.set(liked.value + 1)} data-testid="like" />
      <IconButton label="Share" icon="share" data-testid="share" />
      <IconButton label="Bookmark" icon="bookmark" />
      <IconButton label="More actions" icon="more" />
      <text role="status" data-testid="likes" className="ps-2 text-footnote text-muted tabular-nums">{`${liked.value} likes`}</text>
    </view>
  );
}

export function Variants() {
  return (
    <view className="flex-row items-center gap-3">
      <IconButton label="Add" icon="add" variant="primary" data-testid="add" />
      <IconButton label="Edit" icon="edit" variant="secondary" />
      <IconButton label="Settings" icon="settings" data-testid="quiet" />
      <IconButton label="Delete" icon="delete" variant="destructive" />
    </view>
  );
}

export function Sizes() {
  return (
    <view className="flex-row items-center gap-3">
      <IconButton label="Search, small" icon="search" size="sm" variant="secondary" data-testid="small" />
      <IconButton label="Search, medium" icon="search" variant="secondary" data-testid="medium" />
      <IconButton label="Search, large" icon="search" size="lg" variant="secondary" data-testid="large" />
    </view>
  );
}

export function StateDefault() {
  return <IconButton label="Refresh" icon="refresh" variant="secondary" />;
}

export function StateBusy() {
  return <IconButton label="Refresh" icon="refresh" variant="secondary" isLoading data-testid="busy" />;
}

export function StateDisabled() {
  return <IconButton label="Refresh" icon="refresh" variant="secondary" isDisabled disabledMessage="Nothing new to load" data-testid="disabled" />;
}

/** At the end of a row, an icon button acts on that row alone. */
export function InRows() {
  const removed = useLocal<readonly string[]>('removed', []);
  const files = ['Budget.xlsx', 'Notes.txt', 'Plan.pdf'].filter(name => !removed.value.includes(name));
  return (
    <view role="list" aria-label="Attachments" className="divide-y divide-text/8">
      {files.map(name => (
        <view key={name} role="listitem">
          <Item
            label={name}
            endContent={<IconButton label={`Remove ${name}`} icon="close" size="sm" onClick={() => removed.set([...removed.value, name])} data-testid={`remove-${name}`} />}
          />
        </view>
      ))}
    </view>
  );
}

/** The home tile. */
export function Specimen() {
  return (
    <view className="flex-row items-center gap-2">
      <IconButton label="Like" icon="heart" variant="secondary" />
      <IconButton label="Add" icon="add" variant="primary" />
    </view>
  );
}
