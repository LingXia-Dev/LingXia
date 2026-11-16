import { useLocal, type VectorPath } from '@lingxia/view';
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

const dots: VectorPath[] = [{ d: 'M5 12h.01M12 12h.01M19 12h.01', fill: 'none', stroke: 'currentColor', strokeWidth: 3, strokeLinecap: 'round' }];
const trash: VectorPath[] = [{ d: 'M4 7h16M9 7V4h6v3M6 7l1 13h10l1-13', fill: 'none', stroke: 'currentColor', strokeWidth: 2, strokeLinejoin: 'round' }];

/** A labelled trigger; the menu lines up with its start edge. */
export function Basic() {
  const order = useLocal('order', 'Name');
  return (
    <view className="flex-row items-center gap-3">
      <DropdownMenu label="Sort by" data-testid="sort">
        <DropdownMenuItem label="Name" onClick={() => order.set('Name')} data-testid="sort-name" />
        <DropdownMenuItem label="Last updated" description="Newest first" onClick={() => order.set('Last updated')} data-testid="sort-updated" />
        <DropdownMenuItem label="Size" onClick={() => order.set('Size')} />
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
            isLabelHidden
            icon={<vector aria-hidden paths={dots} viewBox={[0, 0, 24, 24]} className="size-5" />}
            variant="ghost"
            alignment="end"
            data-testid="more"
          >
            <DropdownMenuItem label="Rename" onClick={() => say('Chose Rename')} />
            <DropdownMenuItem label="Share" onClick={unused} isDisabled />
            <DropdownMenuDivider />
            <DropdownMenuItem
              label="Delete"
              variant="destructive"
              icon={<vector aria-hidden paths={trash} viewBox={[0, 0, 24, 24]} className="size-4" />}
              onClick={() => removed.set(true)}
              data-testid="delete"
            />
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
              <DropdownMenu
                label={`${row} actions`}
                isLabelHidden
                icon={<vector aria-hidden paths={dots} viewBox={[0, 0, 24, 24]} className="size-5" />}
                variant="ghost"
                size="sm"
                alignment="end"
                data-testid={`menu-${row}`}
              >
                <DropdownMenuItem label="Open" onClick={() => say(`Opened ${row}`)} />
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
    <DropdownMenu label="Sort by" size="sm">
      <DropdownMenuItem label="Name" onClick={unused} />
    </DropdownMenu>
  );
}

export function Sizes() {
  return (
    <Said>
      {say => (
        <view className="flex-row flex-wrap items-center gap-2">
          <DropdownMenu label="Small" size="sm">
            <DropdownMenuItem label="Rename" onClick={() => say('Chose Rename')} />
          </DropdownMenu>
          <DropdownMenu label="Medium">
            <DropdownMenuItem label="Rename" onClick={() => say('Chose Rename')} />
          </DropdownMenu>
          <DropdownMenu label="Large" size="lg">
            <DropdownMenuItem label="Rename" onClick={() => say('Chose Rename')} />
          </DropdownMenu>
        </view>
      )}
    </Said>
  );
}

export function StateDisabled() {
  return (
    <view className="flex-row">
      <DropdownMenu label="Share" isDisabled>
        <DropdownMenuItem label="Copy link" onClick={unused} />
      </DropdownMenu>
    </view>
  );
}

export function StateEnabled() {
  return (
    <Said>
      {say => (
        <view className="flex-row">
          <DropdownMenu label="Share">
            <DropdownMenuItem label="Copy link" onClick={() => say('Copied the link')} />
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
      <DropdownMenu label="Appearance" value={appearance.value} variant="ghost" size="sm" alignment="end" data-testid="appearance">
        <DropdownMenuRadioGroup value={appearance.value} onChange={next => appearance.set(next)}>
          {['System', 'Light', 'Dark'].map(option => (
            <DropdownMenuRadioItem key={option} value={option} label={option} data-testid={`appearance-${option}`} />
          ))}
        </DropdownMenuRadioGroup>
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
    <DropdownMenu label="View" data-testid="view-menu">
      <DropdownMenuCheckboxItem label="Show completed" isChecked={completed.value} onCheckedChange={next => completed.set(next)} data-testid="show-completed" />
      <DropdownMenuCheckboxItem label="Show archived" isChecked={archived.value} onCheckedChange={next => archived.set(next)} data-testid="show-archived" />
      <DropdownMenuDivider />
      <DropdownMenuRadioGroup label="Group by" value={grouping.value} onChange={next => grouping.set(next)}>
        {['Day', 'Week', 'Month'].map(option => (
          <DropdownMenuRadioItem key={option} value={option} label={`Group by ${option.toLowerCase()}`} data-testid={`group-${option}`} />
        ))}
      </DropdownMenuRadioGroup>
    </DropdownMenu>
  );
}

/** Groups under their headings, when a menu holds more than one kind of thing. */
export function Grouped() {
  const sort = useLocal('sort', 'Edited');
  const completed = useLocal('done', true);
  return (
    <DropdownMenu label="View options" data-testid="options">
      <DropdownMenuGroup label="Sort by">
        <DropdownMenuRadioGroup value={sort.value} onChange={next => sort.set(next)}>
          {['Edited', 'Created', 'Title'].map(option => (
            <DropdownMenuRadioItem key={option} value={option} label={option} data-testid={`sort-${option}`} />
          ))}
        </DropdownMenuRadioGroup>
      </DropdownMenuGroup>
      <DropdownMenuDivider />
      <DropdownMenuGroup label="Show">
        <DropdownMenuCheckboxItem label="Completed" isChecked={completed.value} onCheckedChange={next => completed.set(next)} data-testid="show-done" />
      </DropdownMenuGroup>
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
          <DropdownMenu label="Note" data-testid="note-menu">
            <DropdownMenuItem label="Rename" onClick={() => say('Chose Rename')} />
            <DropdownMenuSubmenu label="Move to" data-testid="move">
              {['Notes', 'Travel', 'Recipes', 'Archive'].map(folder => (
                <DropdownMenuItem key={folder} label={folder} onClick={() => moved.set(folder)} data-testid={`move-${folder.toLowerCase()}`} />
              ))}
            </DropdownMenuSubmenu>
            <DropdownMenuDivider />
            <DropdownMenuItem label="Delete" variant="destructive" onClick={() => say('Chose Delete')} />
          </DropdownMenu>
          <text role="status" className="text-footnote text-muted">{`In ${moved.value}`}</text>
        </view>
      )}
    </Said>
  );
}
