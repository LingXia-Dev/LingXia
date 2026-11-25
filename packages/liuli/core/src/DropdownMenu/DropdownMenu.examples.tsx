import { useLocal } from '@lingxia/view';
import {
  DropdownMenu,
  DropdownMenuCheckboxItem,
  DropdownMenuDivider,
  DropdownMenuGroup,
  DropdownMenuItem,
  DropdownMenuRadioGroup,
  DropdownMenuRadioItem,
  DropdownMenuSubmenu,
  type DropdownMenuProps,
} from './DropdownMenu';
import { Said, unused } from '../example';
import { Button } from '../Button/Button';
import { IconButton } from '../IconButton/IconButton';
import { Link } from '../Link/Link';
import { Kbd } from '../Kbd/Kbd';


/** A labelled trigger; the menu lines up with its start edge. */
export function Basic() {
  const order = useLocal('order', 'Name');
  return (
    <view className="flex-row items-center gap-3">
      <DropdownMenu
        label="Sort by"
        menu={<>
          <DropdownMenuItem label="Name" onClick={() => order.set('Name')} data-testid="sort-name" />
          <DropdownMenuItem label="Last updated" description="Newest first" onClick={() => order.set('Last updated')} data-testid="sort-updated" />
          <DropdownMenuItem label="Size" onClick={() => order.set('Size')} />
        </>}
      >
        <Button label="Sort by" endIcon="chevron-down" data-testid="sort" />
      </DropdownMenu>
      <text data-testid="order" className="text-footnote text-muted">{order.value}</text>
    </view>
  );
}

/** Shortcuts at each row's trailing edge, muted, as an app's Edit menu shows them. */
export function Shortcuts() {
  return (
    <Said>
      {say => (
        <view className="flex-row">
          <DropdownMenu
            label="Edit"
            menu={<>
              <DropdownMenuItem label="Undo" endContent={<Kbd keys={['⌘', 'Z']} />} onClick={() => say('Undid')} data-testid="edit-undo" />
              <DropdownMenuDivider />
              <DropdownMenuItem label="Cut" endContent={<Kbd keys={['⌘', 'X']} />} onClick={() => say('Cut')} />
              <DropdownMenuItem label="Copy" endContent={<Kbd keys={['⌘', 'C']} />} onClick={() => say('Copied')} />
              <DropdownMenuItem label="Paste" endContent={<Kbd keys={['⌘', 'V']} />} onClick={() => say('Pasted')} />
              <DropdownMenuDivider />
              <DropdownMenuItem label="Delete" icon="delete" variant="destructive" endContent={<Kbd keys={['⌫']} />} onClick={() => say('Deleted')} />
            </>}
          >
            <Button label="Edit" endIcon="chevron-down" data-testid="edit" />
          </DropdownMenu>
        </view>
      )}
    </Said>
  );
}

/** In a right-to-left language the row mirrors: the shortcut sits at the left edge. */
export function ShortcutsRtl() {
  return (
    <Said>
      {say => (
        <view dir="rtl" className="flex-row">
          <DropdownMenu
            label="تحرير"
            menu={<>
              <DropdownMenuItem label="تراجع" endContent={<Kbd keys={['⌘', 'Z']} />} onClick={() => say('تم التراجع')} data-testid="rtl-undo" />
              <DropdownMenuItem label="نسخ" endContent={<Kbd keys={['⌘', 'C']} />} onClick={() => say('تم النسخ')} />
              <DropdownMenuItem label="لصق" endContent={<Kbd keys={['⌘', 'V']} />} onClick={() => say('تم اللصق')} />
            </>}
          >
            <Button label="تحرير" endIcon="chevron-down" data-testid="rtl-edit" />
          </DropdownMenu>
        </view>
      )}
    </Said>
  );
}

/** An icon-only "more" menu at the end of a row, aligned to the row's end edge. */
export function More() {
  const removed = useLocal('removed', false);
  return (
    <Said>
      {say => (
        <view className="flex-row items-center gap-3">
          <text className="grow basis-0 text-body font-medium text-text">{removed.value ? 'Removed' : 'Groceries'}</text>
          <DropdownMenu
            label="More actions"
            alignment="end"
            menu={<>
              <DropdownMenuItem label="Rename" onClick={() => say('Chose Rename')} />
              <DropdownMenuItem label="Share" onClick={unused} isDisabled />
              <DropdownMenuDivider />
              <DropdownMenuItem
                label="Delete"
                variant="destructive"
                icon="delete"
                onClick={() => removed.set(true)}
                data-testid="delete"
              />
            </>}
          >
            <IconButton label="More actions" icon="more" data-testid="more" />
          </DropdownMenu>
        </view>
      )}
    </Said>
  );
}

/** Rows that each carry a menu: every menu gets its own id, so no row needs one. */
export function InList() {
  const rows = ['Inbox', 'Archive'];
  return (
    <Said>
      {say => (
        <view className="gap-2">
          {rows.map(row => (
            <view key={row} className="flex-row items-center gap-3">
              <text className="grow basis-0 text-body text-text">{row}</text>
              <DropdownMenu label={`${row} actions`} size="sm" alignment="end" menu={<DropdownMenuItem label="Open" onClick={() => say(`Opened ${row}`)} />}>
                <IconButton label={`${row} actions`} icon="more" size="sm" data-testid={`menu-${row}`} />
              </DropdownMenu>
            </view>
          ))}
        </view>
      )}
    </Said>
  );
}

/** The home tile. */
export function Specimen() {
  return (
    <DropdownMenu label="Sort by" size="sm" menu={<DropdownMenuItem label="Name" onClick={unused} />}>
      <Button label="Sort by" size="sm" endIcon="chevron-down" />
    </DropdownMenu>
  );
}

export function Sizes() {
  return (
    <Said>
      {say => (
        <view className="flex-row flex-wrap items-center gap-2">
          <DropdownMenu label="Small" size="sm" menu={<DropdownMenuItem label="Rename" onClick={() => say('Chose Rename')} />}>
            <Button label="Small" size="sm" endIcon="chevron-down" />
          </DropdownMenu>
          <DropdownMenu label="Medium" menu={<DropdownMenuItem label="Rename" onClick={() => say('Chose Rename')} />}>
            <Button label="Medium" endIcon="chevron-down" />
          </DropdownMenu>
          <DropdownMenu label="Large" size="lg" menu={<DropdownMenuItem label="Rename" onClick={() => say('Chose Rename')} />}>
            <Button label="Large" size="lg" endIcon="chevron-down" />
          </DropdownMenu>
        </view>
      )}
    </Said>
  );
}

export function StateDisabled() {
  return (
    <view className="flex-row">
      <DropdownMenu label="Share" menu={<DropdownMenuItem label="Copy link" onClick={unused} />}>
        <Button label="Share" endIcon="chevron-down" isDisabled />
      </DropdownMenu>
    </view>
  );
}

export function StateEnabled() {
  return (
    <Said>
      {say => (
        <view className="flex-row">
          <DropdownMenu label="Share" menu={<DropdownMenuItem label="Copy link" onClick={() => say('Copied the link')} />}>
            <Button label="Share" endIcon="chevron-down" />
          </DropdownMenu>
        </view>
      )}
    </Said>
  );
}

/** A menu that sets a value: the trigger shows the choice, and the menu checks it. */
export function Value() {
  const appearance = useLocal('appearance', 'System');
  return (
    <view className="flex-row items-center gap-3">
      <text className="grow basis-0 text-body text-text">Appearance</text>
      <DropdownMenu
        label="Appearance"
        size="sm"
        alignment="end"
        menu={
          <DropdownMenuRadioGroup value={appearance.value} onChange={next => appearance.set(next)}>
            {['System', 'Light', 'Dark'].map(option => (
              <DropdownMenuRadioItem key={option} value={option} label={option} data-testid={`appearance-${option}`} />
            ))}
          </DropdownMenuRadioGroup>
        }
      >
        <Button label={appearance.value} variant="ghost" size="sm" endIcon="chevron-down" data-testid="appearance" />
      </DropdownMenu>
    </view>
  );
}

/** Options turned on and off, and one choice among several, in one menu that stays open while they change. */
export function Checkable() {
  const completed = useLocal('completed', true);
  const archived = useLocal('archived', false);
  const grouping = useLocal('grouping', 'Day');
  return (
    <DropdownMenu label="View" menu={<>
      <DropdownMenuCheckboxItem label="Show completed" value={completed.value} onChange={next => completed.set(next)} data-testid="show-completed" />
      <DropdownMenuCheckboxItem label="Show archived" value={archived.value} onChange={next => archived.set(next)} data-testid="show-archived" />
      <DropdownMenuDivider />
      <DropdownMenuRadioGroup label="Group by" value={grouping.value} onChange={next => grouping.set(next)}>
        {['Day', 'Week', 'Month'].map(option => (
          <DropdownMenuRadioItem key={option} value={option} label={`Group by ${option.toLowerCase()}`} data-testid={`group-${option}`} />
        ))}
      </DropdownMenuRadioGroup>
    </>}>
      <Button label="View" endIcon="chevron-down" data-testid="view-menu" />
    </DropdownMenu>
  );
}

const labels = ['Work', 'Personal', 'Travel'] as const;

/** An item that turns a set on and off: mixed while only some of the set is on. */
export function StateMixed() {
  const shown = useLocal<readonly string[]>('labels', ['Work']);
  const isAll = shown.value.length === labels.length;
  return (
    <view className="flex-row">
      <DropdownMenu label="Labels" menu={<>
        <DropdownMenuCheckboxItem label="All labels" value={isAll} isIndeterminate={shown.value.length > 0} onChange={next => shown.set(next ? labels : [])} data-testid="labels-all" />
        <DropdownMenuDivider />
        {labels.map(label => (
          <DropdownMenuCheckboxItem key={label} label={label} value={shown.value.includes(label)}
            onChange={next => shown.set(next ? [...shown.value, label] : shown.value.filter(each => each !== label))} data-testid={`label-${label}`} />
        ))}
      </>}>
        <Button label="Labels" endIcon="chevron-down" data-testid="labels-menu" />
      </DropdownMenu>
    </view>
  );
}

/** Groups under their headings, when a menu holds more than one kind of thing. */
export function Grouped() {
  const sort = useLocal('sort', 'Edited');
  const completed = useLocal('done', true);
  return (
    <DropdownMenu label="View options" menu={<>
      <DropdownMenuGroup label="Sort by">
        <DropdownMenuRadioGroup value={sort.value} onChange={next => sort.set(next)}>
          {['Edited', 'Created', 'Title'].map(option => (
            <DropdownMenuRadioItem key={option} value={option} label={option} data-testid={`sort-${option}`} />
          ))}
        </DropdownMenuRadioGroup>
      </DropdownMenuGroup>
      <DropdownMenuDivider />
      <DropdownMenuGroup label="Show">
        <DropdownMenuCheckboxItem label="Completed" value={completed.value} onChange={next => completed.set(next)} data-testid="show-done" />
      </DropdownMenuGroup>
    </>}>
      <Button label="View options" endIcon="chevron-down" data-testid="options" />
    </DropdownMenu>
  );
}

/** A submenu for a few choices under one name; it opens at the item's side. */
export function Submenu() {
  const moved = useLocal('moved', 'Notes');
  return (
    <Said>
      {say => (
        <view className="items-start gap-2">
          <DropdownMenu label="Note" menu={<>
            <DropdownMenuItem label="Rename" onClick={() => say('Chose Rename')} />
            <DropdownMenuSubmenu label="Move to" data-testid="move">
              {['Notes', 'Travel', 'Recipes', 'Archive'].map(folder => (
                <DropdownMenuItem key={folder} label={folder} onClick={() => moved.set(folder)} data-testid={`move-${folder.toLowerCase()}`} />
              ))}
            </DropdownMenuSubmenu>
            <DropdownMenuDivider />
            <DropdownMenuItem label="Delete" variant="destructive" onClick={() => say('Chose Delete')} />
          </>}>
            <Button label="Note" endIcon="chevron-down" data-testid="note-menu" />
          </DropdownMenu>
          <text role="status" className="text-footnote text-muted">{`In ${moved.value}`}</text>
        </view>
      )}
    </Said>
  );
}


/** Rows that change what is beside the menu keep it open, so a second press needs no reopening; Reset closes it. Reset has no icon, so it keeps the icon's space and its label lines up with the others. */
export function Repeatable() {
  const zoom = useLocal('zoom', 100);
  return (
    <view className="flex-row items-center gap-3">
      <DropdownMenu label="Zoom" items={[
        { id: 'in', label: 'Zoom in', icon: 'add', isRepeatable: true, onClick: () => zoom.set(Math.min(200, zoom.value + 10)), 'data-testid': 'zoom-in' },
        { id: 'out', label: 'Zoom out', icon: 'remove', isRepeatable: true, onClick: () => zoom.set(Math.max(50, zoom.value - 10)), 'data-testid': 'zoom-out' },
        { type: 'divider' },
        { id: 'reset', label: 'Reset to 100%', onClick: () => zoom.set(100), 'data-testid': 'zoom-reset' },
      ]}>
        <Button label="Zoom" endIcon="chevron-down" data-testid="zoom" />
      </DropdownMenu>
      <text role="status" className="text-footnote text-muted tabular-nums">{`${zoom.value}%`}</text>
    </view>
  );
}

/** The rows as data: actions, a submenu, a group under a heading and a destructive action after a divider. */
export function Data() {
  return (
    <Said>
      {say => (
        <DropdownMenu
          label="Note actions"
          items={[
            { label: 'Rename', icon: 'edit', onClick: () => say('Chose Rename'), 'data-testid': 'data-rename' },
            { label: 'Duplicate', icon: 'copy', onClick: () => say('Chose Duplicate') },
            { label: 'Note settings', icon: 'settings', to: { page: 'settings' }, 'data-testid': 'data-settings' },
            {
              label: 'Move to',
              icon: 'folder',
              items: [
                { label: 'Inbox', onClick: () => say('Moved to Inbox'), 'data-testid': 'data-inbox' },
                { label: 'Archive', onClick: () => say('Moved to Archive') },
              ],
              'data-testid': 'data-move',
            },
            { type: 'divider' },
            {
              type: 'group',
              label: 'Share',
              items: [
                { label: 'Copy link', icon: 'link', onClick: () => say('Copied the link') },
                { label: 'Invite someone', icon: 'share', description: 'They can edit it', onClick: unused, isDisabled: true },
              ],
            },
            { type: 'divider' },
            { label: 'Delete', icon: 'delete', variant: 'destructive', onClick: () => say('Chose Delete'), 'data-testid': 'data-delete' },
          ]}
        >
          <Button label="Actions" endIcon="chevron-down" data-testid="data" />
        </DropdownMenu>
      )}
    </Said>
  );
}
/** On a link in a sentence: the choice is made where it is read. */
export function InSentence() {
  const order = useLocal('sentence-order', 'newest');
  return (
    <text className="text-body text-text" data-testid="sentence">
      Showing notes,{' '}
      <DropdownMenu
        label="Order"
        menu={<>
          <DropdownMenuItem label="Newest first" onClick={() => order.set('newest')} />
          <DropdownMenuItem label="Oldest first" onClick={() => order.set('oldest')} />
        </>}
      >
        <Link label={`${order.value} first`} data-testid="order-link" />
      </DropdownMenu>
      .
    </text>
  );
}

/** The playground's props that are not plain values. */
export function playground() {
  return {
    menu: (
      <>
        <DropdownMenuItem label="Name" onClick={unused} />
        <DropdownMenuItem label="Last updated" description="Newest first" onClick={unused} />
        <DropdownMenuItem label="Size" onClick={unused} />
      </>
    ),
    children: <Button label="Sort by" endIcon="chevron-down" />,
  } satisfies Partial<DropdownMenuProps>;
}
