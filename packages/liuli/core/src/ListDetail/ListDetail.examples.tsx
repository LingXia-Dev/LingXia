import { useLocal } from '@lingxia/view';
import { EmptyState } from '../EmptyState/EmptyState';
import { unused } from '../example';
import { Item, ItemList } from '../Item/Item';
import { ListDetail } from './ListDetail';

const orders = [
  { id: 'ord-101', title: 'Order #101', summary: 'Express delivery · Paid' },
  { id: 'ord-102', title: 'Order #102', summary: 'Warehouse pickup · Processing' },
  { id: 'ord-103', title: 'Order #103', summary: 'Standard shipping · Shipped' },
] as const;

/** Master-detail orders list: two-pane on regular (snapping to vertical hinge), drill-down with Back interception on compact. */
export function OrdersMasterDetail() {
  const selected = useLocal<string | null>('selected-order', null);
  const current = orders.find(o => o.id === selected.value) ?? null;

  return (
    <view className="h-[360px] overflow-hidden rounded-container border border-text/12">
      <ListDetail bind={selected} listLabel="Orders" data-testid="orders-list-detail">
        <ListDetail.List>
          <scroll className="grow">
            <ItemList>
              {orders.map(order => (
                <Item
                  key={order.id}
                  label={order.title}
                  description={order.summary}
                  isSelected={selected.value === order.id}
                  onClick={() => selected.set(order.id)}
                  data-testid={`order-${order.id}`}
                />
              ))}
            </ItemList>
          </scroll>
        </ListDetail.List>
        <ListDetail.Detail
          title={current?.title}
          placeholder={<EmptyState title="Select an order" description="Choose an item from the list to inspect its details." />}
        >
          {current && (
            <view className="p-4 gap-2" data-testid="order-detail-body">
              <text className="text-title font-semibold text-text">{current.title}</text>
              <text className="text-body text-muted">{current.summary}</text>
            </view>
          )}
        </ListDetail.Detail>
      </ListDetail>
    </view>
  );
}

/** Tile specimen showing a master list beside its selected detail record. */
export function Specimen() {
  return (
    <view className="w-full flex-row overflow-hidden rounded-control border border-text/12 bg-canvas">
      <view className="w-[96px] p-1 border-e border-text/8">
        <ItemList>
          <Item label="#101" density="compact" isSelected onClick={unused} />
          <Item label="#102" density="compact" isSelected={false} onClick={unused} />
        </ItemList>
      </view>
      <view className="grow shrink basis-0 p-2.5 gap-1">
        <text className="text-footnote font-semibold text-text">Order #101</text>
        <text className="text-caption text-muted">Paid</text>
      </view>
    </view>
  );
}
