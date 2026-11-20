import { useLocal } from '@lingxia/view';
import { Button } from '../Button/Button';
import { CheckboxList } from '../CheckboxList/CheckboxList';
import { Item, ItemList } from '../Item/Item';
import { RadioGroup } from '../RadioGroup/RadioGroup';
import { Drawer } from './Drawer';

/** Filters for the list behind it: the list stays in view while they are chosen. */
export function Filters() {
  const open = useLocal('open', false);
  const sort = useLocal<string | null>('sort', 'edited');
  const kinds = useLocal<readonly string[]>('kinds', ['notes', 'lists']);
  return (
    <view className="items-start gap-2">
      <Button label="Filters" startIcon="filter" onClick={() => open.set(true)} data-testid="filters-open" />
      <Drawer
        isOpen={open.value}
        onOpenChange={next => open.set(next)}
        label="Filters"
        closeLabel="Close"
        data-testid="filters"
        actions={
          <>
            <Button label="Clear" onClick={() => { kinds.set([]); sort.set('edited'); }} />
            <Button label="Show results" variant="primary" onClick={() => open.set(false)} data-testid="filters-apply" />
          </>
        }
      >
        <RadioGroup bind={sort} label="Sort by" items={[{ value: 'edited', label: 'Last edited' }, { value: 'created', label: 'Created' }, { value: 'title', label: 'Title' }]} />
        <CheckboxList
          label="Show"
          value={kinds.value}
          onChange={next => kinds.set(next)}
          items={[{ value: 'notes', label: 'Notes' }, { value: 'lists', label: 'Lists' }, { value: 'photos', label: 'Photos' }, { value: 'archived', label: 'Archived' }]}
        />
      </Drawer>
    </view>
  );
}

/** From the start edge, for the places of an app. */
export function Navigation() {
  const open = useLocal('nav', false);
  return (
    <view className="items-start">
      <Button label="Menu" startIcon="menu" onClick={() => open.set(true)} data-testid="nav-open" />
      <Drawer isOpen={open.value} onOpenChange={next => open.set(next)} label="Notes" description="ana@example.com" side="start" size="sm" data-testid="nav">
        <ItemList label="Places">
          <Item label="All notes" startIcon="file" onClick={() => open.set(false)} data-testid="nav-all" />
          <Item label="Shared with me" startIcon="users" onClick={() => open.set(false)} />
          <Item label="Archive" startIcon="folder" onClick={() => open.set(false)} />
          <Item label="Settings" startIcon="settings" onClick={() => open.set(false)} />
        </ItemList>
      </Drawer>
    </view>
  );
}

/** The home tile: the panel drawn at rest, in the drawer's own surface. */
export function Specimen() {
  return (
    <view className="w-28 h-16 flex-row justify-end overflow-hidden rounded-control bg-surface-sunken">
      <view className="w-12 h-full rounded-s-control border-s border-text/12 bg-surface-overlay" />
    </view>
  );
}
