import { useLocal } from '@lingxia/view';
import { DropdownMenuDivider, DropdownMenuItem } from '../DropdownMenu/DropdownMenu';
import { Item, ItemList } from '../Item/Item';
import { ContextMenu } from './ContextMenu';

const notes = [
  ['coast', 'Trip to the coast', 'Edited yesterday'],
  ['reading', 'Reading list', 'Edited on Monday'],
  ['garden', 'Ideas for the garden', 'Edited last week'],
] as const;

/** Rows with a menu each: hold a row, or click it with the secondary button. The row itself still opens. */
export function Rows() {
  const last = useLocal('last', 'Nothing yet');
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
      <text role="status" className="text-footnote text-muted">{last.value}</text>
    </view>
  );
}

/** The home tile. */
export function Specimen() {
  return (
    <ContextMenu label="Sample" menu={<DropdownMenuItem label="Copy" onClick={() => {}} />}>
      <view className="w-28 h-12 items-center justify-center rounded-control bg-surface-sunken">
        <text className="text-footnote text-muted">Hold me</text>
      </view>
    </ContextMenu>
  );
}
