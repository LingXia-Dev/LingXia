import { useLocal, type VectorPath } from '@lingxia/view';
import { DropdownMenu, DropdownMenuDivider, DropdownMenuItem } from './DropdownMenu';

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
      <text data-testid="order" className="text-sm text-muted">{order.value}</text>
    </view>
  );
}

/** An icon-only "more" menu at the end of a row, aligned to the row's end edge. */
export function More() {
  const removed = useLocal('removed', false);
  return (
    <view className="flex-row items-center gap-3">
      <text className="grow basis-0 text-base font-medium">{removed.value ? 'Removed' : 'Groceries'}</text>
      <DropdownMenu
        label="More actions"
        isLabelHidden
        icon={<vector aria-hidden paths={dots} viewBox={[0, 0, 24, 24]} className="size-5" />}
        variant="ghost"
        alignment="end"
        data-testid="more"
      >
        <DropdownMenuItem label="Rename" onClick={() => {}} />
        <DropdownMenuItem label="Share" onClick={() => {}} isDisabled />
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
  );
}

/** Rows that each carry a menu: every menu gets its own id, so no row needs one. */
export function InList() {
  const rows = ['Inbox', 'Archive'];
  return (
    <view className="gap-2">
      {rows.map(row => (
        <view key={row} className="flex-row items-center gap-3">
          <text className="grow basis-0 text-base">{row}</text>
          <DropdownMenu label={`${row} actions`} size="sm" data-testid={`menu-${row}`}>
            <DropdownMenuItem label="Open" onClick={() => {}} />
          </DropdownMenu>
        </view>
      ))}
    </view>
  );
}
