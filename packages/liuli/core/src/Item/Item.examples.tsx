import { useLocal } from '@lingxia/view';
import { Said, unused } from '../example';
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
    <Said>
      {say => (
        <ItemList label="Notes" data-testid="notes">
          {notes.map(note => (
            <Item key={note.id} label={note.title} description={note.updated} hasChevron onClick={() => say(`Opened ${note.title}`)} data-testid={`note-${note.id}`} />
          ))}
        </ItemList>
      )}
    </Said>
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

const inbox = [
  { id: 'm1', from: 'Ana Lee', subject: 'Notes from Tuesday' },
  { id: 'm2', from: 'Ben Okafor', subject: 'Invoice for October' },
  { id: 'm3', from: 'Chloé Martin', subject: 'Photos from the coast' },
] as const;

/** Drag a row toward its start to show its actions; the list has a heading and a line under it. */
export function Swipe() {
  const gone = useLocal<readonly string[]>('gone', []);
  const flagged = useLocal<readonly string[]>('flagged', []);
  return (
    <Said>
      {say => (
        <ItemList title="Inbox" description="Drag a message to the side to flag or delete it." data-testid="inbox">
          {inbox.filter(mail => !gone.value.includes(mail.id)).map(mail => (
            <Item
              key={mail.id}
              label={mail.from}
              description={mail.subject}
              endContent={flagged.value.includes(mail.id) ? <text className="text-footnote text-muted">Flagged</text> : undefined}
              hasChevron
              onClick={() => say(`Opened ${mail.subject}`)}
              swipeActions={[
                { key: 'flag', label: 'Flag', icon: 'flag', onClick: () => flagged.set(flagged.value.includes(mail.id) ? flagged.value.filter(id => id !== mail.id) : [...flagged.value, mail.id]) },
                { key: 'delete', label: 'Delete', icon: 'delete', tone: 'danger', onClick: () => gone.set([...gone.value, mail.id]) },
              ]}
              data-testid={`mail-${mail.id}`}
            />
          ))}
        </ItemList>
      )}
    </Said>
  );
}

const files = [
  { name: 'Budget.xlsx', size: '48 KB' }, { name: 'Contract.pdf', size: '1.2 MB' }, { name: 'Logo.svg', size: '6 KB' },
  { name: 'Notes.md', size: '3 KB' }, { name: 'Roadmap.key', size: '14 MB' },
] as const;

/** Compact rows for a long list read with a mouse. */
export function Compact() {
  return (
    <Said>
      {say => (
        <ItemList title="Files" density="compact" data-testid="files">
          {files.map(file => (
            <Item key={file.name} label={file.name} endContent={<text className="text-footnote text-muted tabular-nums">{file.size}</text>} onClick={() => say(`Opened ${file.name}`)} data-testid={`file-${file.name}`} />
          ))}
        </ItemList>
      )}
    </Said>
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
        onClick={unused}
        data-testid="export"
      />
    </view>
  );
}

/** The home tile. */
export function Specimen() {
  return (
    <view className="w-28 overflow-hidden rounded-container border border-text/12 bg-surface">
      <Item label="Alerts" description="On" hasChevron onClick={unused} />
    </view>
  );
}

export function StatePressable() {
  return (
    <Said>
      {say => (
        <view className="overflow-hidden rounded-container border border-text/12 bg-surface">
          <Item label="Wi-Fi" description="Home" hasChevron onClick={() => say('Opened Wi-Fi')} />
        </view>
      )}
    </Said>
  );
}

export function StateSelected() {
  return (
    <Said>
      {say => (
        <view className="overflow-hidden rounded-container border border-text/12 bg-surface">
          <Item label="Wi-Fi" description="Home" hasChevron isSelected onClick={() => say('Opened Wi-Fi')} />
        </view>
      )}
    </Said>
  );
}

export function StateDisabled() {
  return (
    <view className="overflow-hidden rounded-container border border-text/12 bg-surface">
      <Item label="Wi-Fi" description="Off" hasChevron isDisabled onClick={unused} />
    </view>
  );
}

/** A row that deletes or ends something; the danger tone marks it before it is pressed. */
export function Destructive() {
  return (
    <Said>
      {say => (
        <view className="overflow-hidden rounded-container border border-text/12 bg-surface">
          <Item label="Delete account" description="Removes your notes from every device" tone="danger" hasChevron onClick={() => say('Opened Delete account')} data-testid="delete-row" />
        </view>
      )}
    </Said>
  );
}
