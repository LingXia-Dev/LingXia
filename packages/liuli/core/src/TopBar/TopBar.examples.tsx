import { useLocal } from '@lingxia/view';
import { Said, unused } from '../example';
import { IconButton } from '../IconButton/IconButton';
import { SearchField } from '../SearchField/SearchField';
import { TopBar } from './TopBar';

/** A page's header: a way back, its title, and its actions at the end. */
export function Standard() {
  return (
    <Said>
      {say => (
        <view className="overflow-hidden rounded-container border border-text/12">
          <TopBar
            startContent={<IconButton icon="chevron-start" label="Back" variant="ghost" onClick={() => say('Went back')} data-testid="orders-back" />}
            heading="Orders"
            endContent={[
              <IconButton key="filter" icon="filter" label="Filter" variant="ghost" onClick={() => say('Opened Filter')} data-testid="orders-filter" />,
              <IconButton key="add" icon="add" label="New order" variant="ghost" onClick={() => say('Created a new order')} data-testid="orders-add" />,
            ]}
            data-testid="orders-topbar"
          />
        </view>
      )}
    </Said>
  );
}

/** A search field at the middle of the bar. */
export function WithSearch() {
  const query = useLocal('query', '');
  return (
    <view className="overflow-hidden rounded-container border border-text/12">
      <TopBar
        heading="Mail"
        centerContent={<SearchField label="Search mail" bind={query} data-testid="mail-search" />}
        endContent={<IconButton icon="add" label="New message" variant="ghost" onClick={unused} />}
        data-testid="mail-topbar"
      />
    </view>
  );
}

/** The tile: a title and an action, drawn at the tile's size. */
export function Specimen() {
  return (
    <view className="w-full overflow-hidden rounded-control border border-text/12 bg-surface">
      <TopBar heading="Orders" endContent={<IconButton icon="add" size="sm" label="New order" isDisabled onClick={unused} />} />
    </view>
  );
}
