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
} from './DropdownMenu';
import { Said, unused } from '../example';
import { Button } from '../Button/Button';
import { IconButton } from '../IconButton/IconButton';
import { Link } from '../Link/Link';


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
