import { useLocal } from '@lingxia/view';
import { unused } from '../example';
import { DropdownMenuDivider, DropdownMenuItem } from '../DropdownMenu/DropdownMenu';
import { Item, ItemList } from '../Item/Item';
import { ContextMenu, type ContextMenuProps } from './ContextMenu';

const notes = [
  ['coast', 'Trip to the coast', 'Edited yesterday'],
  ['reading', 'Reading list', 'Edited on Monday'],
  ['garden', 'Ideas for the garden', 'Edited last week'],
] as const;

/** Rows with a menu each: hold a row, or click it with the secondary button. The row itself still opens. */
export function Rows() {
  const last = useLocal('last', '');
  return (
    <view className="gap-2">
      <ItemList label="Notes">
        {notes.map(([id, title, edited]) => (
          <ContextMenu
            key={id}
            label={title}
            data-testid={`note-${id}`}
            menu={
              <>
                <DropdownMenuItem label="Pin to top" onClick={() => last.set(`Pinned ${title}`)} data-testid={`pin-${id}`} />
                <DropdownMenuItem label="Share" onClick={() => last.set(`Shared ${title}`)} />
                <DropdownMenuDivider />
                <DropdownMenuItem label="Delete" variant="destructive" onClick={() => last.set(`Deleted ${title}`)} />
              </>
            }
          >
            <Item label={title} description={edited} hasChevron onClick={() => last.set(`Opened ${title}`)} />
          </ContextMenu>
        ))}
      </ItemList>
      {last.value !== '' && <text role="status" className="text-footnote text-muted">{last.value}</text>}
    </view>
  );
}

/** The same menu, its rows given as data. */
export function Data() {
  const last = useLocal('last', '');
  return (
    <view className="gap-2">
      <ItemList label="Photos">
        <ContextMenu
          label="Dunes at noon"
          data-testid="photo-dunes"
          items={[
            { label: 'Add to album', items: [{ label: 'Holidays', onClick: () => last.set('Added to Holidays'), 'data-testid': 'album-holidays' }, { label: 'Favourites', onClick: () => last.set('Added to Favourites') }] },
            { label: 'Share', onClick: () => last.set('Shared Dunes at noon') },
            { type: 'divider' },
            { label: 'Delete', variant: 'destructive', onClick: () => last.set('Deleted Dunes at noon'), 'data-testid': 'photo-delete' },
          ]}
        >
          <Item label="Dunes at noon" description="Taken in May" hasChevron onClick={() => last.set('Opened Dunes at noon')} />
        </ContextMenu>
      </ItemList>
      {last.value !== '' && <text role="status" className="text-footnote text-muted">{last.value}</text>}
    </view>
  );
}

/** The home tile. */
export function Specimen() {
  return (
    <ContextMenu label="Sample" menu={<DropdownMenuItem label="Copy" onClick={unused} />}>
      <view className="w-28 h-12 items-center justify-center rounded-control bg-surface-sunken">
        <text className="text-footnote text-muted">Hold me</text>
      </view>
    </ContextMenu>
  );
}

/** The playground's props that are not plain values. */
export function playground() {
  return {
    menu: (
      <>
        <DropdownMenuItem label="Pin to top" onClick={unused} />
        <DropdownMenuItem label="Share" onClick={unused} />
        <DropdownMenuDivider />
        <DropdownMenuItem label="Delete" variant="destructive" onClick={unused} />
      </>
    ),
    children: <Item label="Trip to the coast" description="Edited yesterday" hasChevron onClick={unused} />,
  } satisfies Partial<ContextMenuProps>;
}
