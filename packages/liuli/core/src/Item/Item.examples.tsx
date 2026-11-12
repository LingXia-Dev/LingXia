import { useLocal } from '@lingxia/view';
import { Button } from '../Button/Button';
import { Item } from './Item';

const notes = [
  { id: 'n1', title: 'Groceries', updated: 'Edited today' },
  { id: 'n2', title: 'Trip to the coast', updated: 'Edited yesterday' },
  { id: 'n3', title: 'Reading list', updated: 'Edited last week' },
] as const;

const folder = [{ d: 'M3 7a2 2 0 0 1 2-2h4l2 2h8a2 2 0 0 1 2 2v8a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z', fill: 'none', stroke: 'currentColor', strokeWidth: 2, strokeLinejoin: 'round' }] as const;

/** Rows that open a page: each is one button named by its title, with the second line as its description. */
export function Navigation() {
  const opened = useLocal('opened', '');
  return (
    <view className="gap-2">
      <view className="rounded-container border border-line bg-surface divide-y divide-line">
        {notes.map(note => (
          <Item
            key={note.id}
            label={note.title}
            description={note.updated}
            hasChevron
            onClick={() => opened.set(note.title)}
            data-testid={`note-${note.id}`}
          />
        ))}
      </view>
      <text data-testid="opened" className="text-sm text-muted">{opened.value}</text>
    </view>
  );
}

/** A value at the end of a read-only row, and an icon at the start. */
export function Values() {
  return (
    <view className="rounded-container border border-line bg-surface divide-y divide-line">
      <Item
        label="Version"
        endContent={<text className="text-sm text-muted">2.4.1</text>}
        data-testid="version"
      />
      <Item
        label="Storage"
        description="Photos and documents"
        startContent={<vector aria-hidden paths={folder} viewBox={[0, 0, 24, 24]} className="size-5" />}
        endContent={<text className="text-sm text-muted">7.2 GB</text>}
      />
    </view>
  );
}

/** A row without `onClick` can hold its own control at the end. */
export function WithControl() {
  const signedIn = useLocal('signedIn', true);
  return (
    <view className="rounded-container border border-line bg-surface">
      <Item
        label="Account"
        description={signedIn.value ? 'Signed in' : 'Signed out'}
        endContent={
          <Button
            label={signedIn.value ? 'Sign out' : 'Sign in'}
            size="sm"
            onClick={() => signedIn.set(!signedIn.value)}
            data-testid="account-action"
          />
        }
        data-testid="account"
      />
    </view>
  );
}

export function Disabled() {
  return (
    <view className="rounded-container border border-line bg-surface">
      <Item
        label="Export history"
        description="Download every change as a file"
        disabledMessage="Available once a change is saved"
        isDisabled
        hasChevron
        onClick={() => {}}
        data-testid="export"
      />
    </view>
  );
}
