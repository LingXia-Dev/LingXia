import { useLocal } from '@lingxia/view';
import { Button } from '../Button/Button';
import { Item, ItemList } from './Item';

const notes = [
  { id: 'n1', title: 'Groceries', updated: 'Edited today' },
  { id: 'n2', title: 'Trip to the coast', updated: 'Edited yesterday' },
  { id: 'n3', title: 'Reading list', updated: 'Edited last week' },
] as const;

const folder = [{ d: 'M3 7a2 2 0 0 1 2-2h4l2 2h8a2 2 0 0 1 2 2v8a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z', fill: 'none', stroke: 'currentColor', strokeWidth: 2, strokeLinejoin: 'round' }] as const;

/** Rows that open a page, in a list: each is one button named by its title, read with its position. */
export function Navigation() {
  return (
    <ItemList label="Notes" data-testid="notes">
      {notes.map(note => (
        <Item key={note.id} label={note.title} description={note.updated} hasChevron onClick={() => {}} data-testid={`note-${note.id}`} />
      ))}
    </ItemList>
  );
}

const mailboxes = [
  { id: 'inbox', label: 'Inbox', count: '12' },
  { id: 'drafts', label: 'Drafts', count: '3' },
  { id: 'sent', label: 'Sent', count: '' },
] as const;

/** Rows that choose one of several, as a sidebar does: the chosen row is selected, the others are not. */
export function Selection() {
  const open = useLocal<string>('open', 'inbox');
  return (
    <ItemList label="Mailboxes" data-testid="mailboxes">
      {mailboxes.map(box => (
        <Item
          key={box.id}
          label={box.label}
          endContent={box.count ? <text className="text-footnote text-muted">{box.count}</text> : undefined}
          isSelected={open.value === box.id}
          onClick={() => open.set(box.id)}
          data-testid={`mailbox-${box.id}`}
        />
      ))}
    </ItemList>
  );
}

/** A value at the end of a read-only row, and an icon at the start. */
export function Values() {
  return (
    <ItemList label="About">
      <Item
        label="Version"
        endContent={<text className="text-footnote text-muted">2.4.1</text>}
        data-testid="version"
      />
      <Item
        label="Storage"
        description="Photos and documents"
        startContent={<vector aria-hidden paths={folder} viewBox={[0, 0, 24, 24]} className="size-5" />}
        endContent={<text className="text-footnote text-muted">7.2 GB</text>}
      />
    </ItemList>
  );
}

/** A row without `onClick` can hold its own control at the end. */
export function WithControl() {
  const signedIn = useLocal('signedIn', true);
  return (
    <view className="overflow-hidden rounded-container border border-text/12 bg-surface">
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
    <view className="overflow-hidden rounded-container border border-text/12 bg-surface">
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

/** The home tile. */
export function Specimen() {
  return (
    <view className="w-28 overflow-hidden rounded-container border border-text/12 bg-surface">
      <Item label="Alerts" description="On" hasChevron onClick={() => {}} />
    </view>
  );
}

export function StatePressable() {
  return (
    <view className="overflow-hidden rounded-container border border-text/12 bg-surface">
      <Item label="Wi-Fi" description="Home" hasChevron onClick={() => {}} />
    </view>
  );
}

export function StateSelected() {
  return (
    <view className="overflow-hidden rounded-container border border-text/12 bg-surface">
      <Item label="Wi-Fi" description="Home" hasChevron isSelected onClick={() => {}} />
    </view>
  );
}

export function StateDisabled() {
  return (
    <view className="overflow-hidden rounded-container border border-text/12 bg-surface">
      <Item label="Wi-Fi" description="Off" hasChevron isDisabled onClick={() => {}} />
    </view>
  );
}

/** A row that deletes or ends something; the danger tone marks it before it is pressed. */
export function Destructive() {
  return (
    <view className="overflow-hidden rounded-container border border-text/12 bg-surface">
      <Item label="Delete account" description="Removes your notes from every device" tone="danger" hasChevron onClick={() => {}} data-testid="delete-row" />
    </view>
  );
}
