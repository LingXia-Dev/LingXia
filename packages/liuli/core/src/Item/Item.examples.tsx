import { useLocal } from '@lingxia/view';
import { Said, unused } from '../example';
import { Avatar } from '../Avatar/Avatar';
import { Badge } from '../Badge/Badge';
import { Button } from '../Button/Button';
import { Section } from '../Section/Section';
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
                { key: 'delete', label: 'Delete', icon: 'delete', variant: 'destructive', onClick: () => gone.set([...gone.value, mail.id]) },
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

/** A row that deletes or ends something; the destructive variant marks it before it is pressed. */
export function Destructive() {
  return (
    <Said>
      {say => (
        <view className="overflow-hidden rounded-container border border-text/12 bg-surface">
          <Item label="Delete account" description="Removes your notes from every device" variant="destructive" hasChevron onClick={() => say('Opened Delete account')} data-testid="delete-row" />
        </view>
      )}
    </Said>
  );
}

/** Rows that go to a page: each row is one link. A press or Enter opens it; Space does not. */
export function Links() {
  return (
    <ItemList label="Inputs" data-testid="links">
      <Item label="TextInput" description="One line of text" hasChevron to={{ page: 'text-input' }} data-testid="link-text-input" />
      <Item label="DateInput" description="A day, typed or picked" hasChevron to={{ page: 'date-input' }} data-testid="link-date-input" />
    </ItemList>
  );
}

const members = [
  { id: 'lena', name: 'Lena', hue: 7, status: 'On shift', tone: 'success', note: 'Until 18:00 today' },
  { id: 'omar', name: 'Omar', hue: 3, status: 'On shift', tone: 'success', note: '4 open tasks' },
  { id: 'alexandra', name: 'Alexandra-Charlotte Montgomery', hue: 5, status: 'Away', tone: 'neutral', note: 'Back on Monday · 3 open tasks' },
] as const;

/** A status beside the name: the name gives way first, and the badge is read after it. */
export function WithStatus() {
  return (
    <Said>
      {say => (
        <ItemList label="Team members" data-testid="members">
          {members.map(member => (
            <Item
              key={member.id}
              label={member.name}
              labelEnd={member.tone === 'success' ? <Badge tone="success" size="sm" label={member.status} /> : <Badge size="sm" label={member.status} />}
              description={member.note}
              startContent={<Avatar name={member.name} hue={member.hue} isDecorative />}
              hasChevron
              onClick={() => say(`Opened ${member.name}`)}
              data-testid={`member-${member.id}`}
            />
          ))}
        </ItemList>
      )}
    </Said>
  );
}

const people = [
  { id: 'oskar', name: 'Oskar Lind', hue: 2, facts: ['oskar@example.com', 'Joined Oct 6, 2025'] },
  { id: 'ines', name: 'Ines Lind', hue: 4, facts: ['ines.l@example.com', 'Invited by Oskar Lind to the spring launch project', 'Joined 23 days ago', 'Active yesterday'] },
] as const;

/** Several facts under a name, one text each, so a fact wraps within itself. */
export function WithFacts() {
  return (
    <Said>
      {say => (
        <ItemList label="People" data-testid="people">
          {people.map(person => (
            <Item
              key={person.id}
              label={person.name}
              description={<>{person.facts.map((fact, index) => <text key={`f${index}`}>{fact}</text>)}</>}
              startContent={<Avatar name={person.name} hue={person.hue} isDecorative />}
              hasChevron
              onClick={() => say(`Opened ${person.name}`)}
              data-testid={`person-${person.id}`}
            />
          ))}
        </ItemList>
      )}
    </Said>
  );
}

const devices = [
  { id: 'tablet', name: "Lena's tablet", note: 'Online · 2 h today' },
  { id: 'phone', name: "Lena's phone", note: 'Offline since 9:40' },
] as const;

/** Downloads under way: stopping one ends it at once, with nothing to confirm, so the action is in danger. */
export function WithStop() {
  return (
    <Said>
      {say => (
        <ItemList label="Downloads" data-testid="downloads">
          <Item label="Coast, November.mov" description="1.2 of 3.4 GB" startIcon="download" endAction={{ label: 'Stop downloading Coast, November.mov', icon: 'close', variant: 'destructive', onClick: () => say('Stopped Coast, November.mov'), 'data-testid': 'download-coast-stop' }} />
        </ItemList>
      )}
    </Said>
  );
}

/** A row that opens its device, with a remove button of its own at the end. */
export function WithRemove() {
  return (
    <Said>
      {say => (
        <ItemList label="Lena's devices" data-testid="devices">
          {devices.map(device => (
            <Item
              key={device.id}
              label={device.name}
              description={device.note}
              startIcon="phone"
              onClick={() => say(`Opened ${device.name}`)}
              endAction={{ label: `Remove ${device.name}`, icon: 'close', onClick: () => say(`Removed ${device.name}`), 'data-testid': `device-${device.id}-remove` }}
              data-testid={`device-${device.id}`}
            />
          ))}
        </ItemList>
      )}
    </Said>
  );
}

/** Rows on a right-to-left page: the avatar starts the row, the status follows the name, and the action ends it on the left. */
export function RightToLeft() {
  return (
    <view dir="rtl" data-testid="rtl-family">
      <ItemList label="חברי הצוות">
        <Item label="לנה" labelEnd={<Badge tone="success" size="sm" label="במשמרת" />} description="עד 18:00 היום" startContent={<Avatar name="Lena" hue={7} isDecorative />} hasChevron onClick={unused} />
        <Item label="הטאבלט של לנה" description="מחובר" startIcon="phone" onClick={unused} endAction={{ label: 'הסרת הטאבלט של לנה', icon: 'close', onClick: unused }} />
        <Item label="אלכסנדרה-שרלוט מונטגומרי" labelEnd={<Badge size="sm" label="לא זמינה" />} description="חוזרת ביום שני · 3 משימות פתוחות" startContent={<Avatar name="Alexandra" hue={5} isDecorative />} hasChevron onClick={unused} />
      </ItemList>
    </view>
  );
}

/** Lists under headings: a list's own title and a Section's level-4 heading share the card's left edge. */
export function TitledLists() {
  return (
    <view className="gap-6" data-testid="titled-lists">
      <Section title="Team members" headingLevel={4}>
        <ItemList label="Team members">
          <Item label="Lena" description="4 open tasks" startContent={<Avatar name="Lena" hue={7} isDecorative />} hasChevron onClick={unused} />
        </ItemList>
      </Section>
      <ItemList title="Unclaimed tasks" description="Tasks wait here until someone on the team picks them up.">
        <Item label="Not picked up" description="2 tasks aren't anyone's yet" startIcon="info" hasChevron onClick={unused} />
        <Item label="Shared" description="Anyone on the team can close these" hasChevron onClick={unused} />
      </ItemList>
    </view>
  );
}
