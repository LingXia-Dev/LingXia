import type { View } from '@lingxia/view';
import { ListDetail } from '@liulidesign/core/ListDetail';
import { EmptyState } from '@liulidesign/core/EmptyState';
import { Item, ItemList } from '@liulidesign/core/Item';
import { AdaptiveStatus } from '../../shared/adaptive-status';
import '../../theme/index.css';

interface OrderItem {
  readonly id: string;
  readonly title: string;
  readonly status: string;
  readonly detail: string;
}

/** ListDetail alone: side by side on a wide screen, the detail over the list with Back on a phone. */
export function render({ data, actions }: View) {
  const items = (data.items as readonly OrderItem[] | undefined) ?? [];
  const selectedId = (data.selectedId as string | null | undefined) ?? null;
  const current = items.find(item => item.id === selectedId) ?? null;

  return (
    <page title="List and detail" data-testid="adaptive-page">
      <AdaptiveStatus />
      <ListDetail
        value={selectedId}
        onChange={id => actions.selectItem({ id })}
        listLabel="Orders"
        data-testid="adaptive-list-detail"
      >
        <ListDetail.List>
          <ItemList>
            {items.map(item => (
              <Item
                key={item.id}
                label={item.title}
                description={item.status}
                isSelected={selectedId === item.id}
                onClick={() => actions.selectItem({ id: item.id })}
                data-testid={`adaptive-item-${item.id}`}
              />
            ))}
          </ItemList>
        </ListDetail.List>
        <ListDetail.Detail
          title={current?.title}
          placeholder={<EmptyState title="Choose an order" description="Its delivery shows here." />}
        >
          {current && (
            <view className="gap-2" data-testid="adaptive-detail-body">
              <text role="heading" headingLevel={1} className="text-base font-semibold text-text">{current.title}</text>
              <text className="text-xs font-mono text-muted">{current.status}</text>
              <text className="text-sm text-text">{current.detail}</text>
            </view>
          )}
        </ListDetail.Detail>
      </ListDetail>
    </page>
  );
}
