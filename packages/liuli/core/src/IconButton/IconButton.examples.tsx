import { useLocal } from '@lingxia/view';
import { Said, unused } from '../example';
import { Item } from '../Item/Item';
import { IconButton } from './IconButton';

/** The label is never drawn and always read: name what the press does. */
export function Toolbar() {
  const liked = useLocal('liked', 0);
  return (
    <Said>
      {say => (
        <view className="flex-row items-center gap-1">
          <IconButton label="Like" icon="heart" onClick={() => liked.set(liked.value + 1)} data-testid="like" />
          <IconButton label="Share" icon="share" onClick={() => say('Shared')} data-testid="share" />
          <IconButton label="Bookmark" icon="bookmark" onClick={() => say('Bookmarked')} />
          <IconButton label="More actions" icon="more" onClick={() => say('Opened More actions')} />
          <text role="status" data-testid="likes" className="ps-2 text-footnote text-muted tabular-nums">{`${liked.value} likes`}</text>
        </view>
      )}
    </Said>
  );
}

export function Variants() {
  return (
    <Said>
      {say => (
        <view className="flex-row items-center gap-3">
          <IconButton label="Add" icon="add" variant="primary" onClick={() => say('Added')} data-testid="add" />
          <IconButton label="Edit" icon="edit" variant="secondary" onClick={() => say('Opened Edit')} />
          <IconButton label="Settings" icon="settings" onClick={() => say('Opened Settings')} data-testid="quiet" />
          <IconButton label="Delete" icon="delete" variant="destructive" onClick={() => say('Deleted')} />
        </view>
      )}
    </Said>
  );
}

export function Sizes() {
  return (
    <Said>
      {say => (
        <view className="flex-row items-center gap-3">
          <IconButton label="Search, small" icon="search" size="sm" variant="secondary" onClick={() => say('Searched, small')} data-testid="small" />
          <IconButton label="Search, medium" icon="search" variant="secondary" onClick={() => say('Searched, medium')} data-testid="medium" />
          <IconButton label="Search, large" icon="search" size="lg" variant="secondary" onClick={() => say('Searched, large')} data-testid="large" />
        </view>
      )}
    </Said>
  );
}

export function StateDefault() {
  return <Said>{say => <IconButton label="Refresh" icon="refresh" variant="secondary" onClick={() => say('Refreshed')} />}</Said>;
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
      <IconButton label="Like" icon="heart" variant="secondary" onClick={unused} />
      <IconButton label="Add" icon="add" variant="primary" onClick={unused} />
    </view>
  );
}
