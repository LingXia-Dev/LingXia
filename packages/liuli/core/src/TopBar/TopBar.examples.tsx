import { useLocal } from '@lingxia/view';
import { Said, unused } from '../example';
import { IconButton } from '../IconButton/IconButton';
import { Item, ItemList } from '../Item/Item';
import { SearchField } from '../SearchField/SearchField';
import { TopBar } from './TopBar';

const orders = [
  { id: '104', items: '3 items · Paid', when: 'Today' },
  { id: '103', items: '1 item · Shipped', when: 'Yesterday' },
  { id: '102', items: '2 items · Delivered', when: 'Mon' },
  { id: '101', items: '5 items · Delivered', when: '8 Nov' },
];

/** A page's header: a way back, its title, and its actions at the end. */
export function Standard() {
  return (
    <Said>
      {say => (
        <view className="h-[300px] overflow-hidden rounded-container border border-text/12 bg-canvas">
          <TopBar
            startContent={<IconButton icon="chevron-start" label="Back" variant="ghost" onClick={() => say('Went back')} data-testid="orders-back" />}
            heading="Orders"
            endContent={[
              <IconButton key="filter" icon="filter" label="Filter" variant="ghost" onClick={() => say('Opened Filter')} data-testid="orders-filter" />,
              <IconButton key="add" icon="add" label="New order" variant="ghost" onClick={() => say('Created a new order')} data-testid="orders-add" />,
            ]}
            data-testid="orders-topbar"
          />
          <ItemList label="Orders" isFlush>
            {orders.map(order => (
              <Item
                key={order.id}
                label={`Order #${order.id}`}
                description={order.items}
                endContent={<text className="text-footnote text-muted">{order.when}</text>}
                hasChevron
                onClick={() => say(`Opened order #${order.id}`)}
              />
            ))}
          </ItemList>
        </view>
      )}
    </Said>
  );
}

const mail = [
  { from: 'Harbour Lights', subject: 'Your tickets for Saturday', when: '09:41' },
  { from: 'Kai Chen', subject: 'Photos from the coast walk', when: 'Yesterday' },
  { from: 'Trail Club', subject: 'November route and meeting point', when: 'Mon' },
];

/** A search field at the middle of the bar. */
export function WithSearch() {
  const query = useLocal('query', '');
  return (
    <view className="h-[300px] overflow-hidden rounded-container border border-text/12 bg-canvas">
      <TopBar
        heading="Mail"
        centerContent={<SearchField label="Search mail" isLabelHidden placeholder="Search mail" bind={query} className="w-[200px]" data-testid="mail-search" />}
        endContent={<IconButton icon="add" label="New message" variant="ghost" onClick={unused} />}
        data-testid="mail-topbar"
      />
      <ItemList label="Inbox" isFlush>
        {mail.map(message => (
          <Item key={message.from} label={message.from} description={message.subject} endContent={<text className="text-footnote text-muted">{message.when}</text>} />
        ))}
      </ItemList>
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
