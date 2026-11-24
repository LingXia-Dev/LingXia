import { useLocal } from '@lingxia/view';
import { Button } from '../Button/Button';
import { EmptyState } from '../EmptyState/EmptyState';
import { unused } from '../example';
import { Item, ItemList } from '../Item/Item';
import { NavList } from '../NavList/NavList';
import { ListDetail } from './ListDetail';

const orders = [
  { id: 'ord-101', title: 'Order #101', summary: 'Express delivery · Paid' },
  { id: 'ord-102', title: 'Order #102', summary: 'Warehouse pickup · Processing' },
  { id: 'ord-103', title: 'Order #103', summary: 'Standard shipping · Shipped' },
] as const;

/** Orders and the one chosen: side by side on a wide screen, the order over the list on a phone, where Back returns. */
export function Orders() {
  const selected = useLocal<string | null>('selected-order', null);
  const current = orders.find(order => order.id === selected.value) ?? null;
  const shipped = useLocal<readonly string[]>('shipped', []);
  const isShipped = current !== null && shipped.value.includes(current.id);
  return (
    // A window's worth of page: in an app it fills the page itself.
    <view className="h-[360px] overflow-hidden rounded-container border border-text/12">
      <ListDetail bind={selected} listLabel="Orders" data-testid="orders">
        <ListDetail.List>
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
        </ListDetail.List>
        <ListDetail.Detail
          title={current?.title}
          placeholder={<EmptyState title="Choose an order" description="Its delivery and payment show here." />}
          actions={<Button label={isShipped ? 'Shipped' : 'Mark as shipped'} variant="primary" isDisabled={isShipped} onClick={() => current && shipped.set([...shipped.value, current.id])} data-testid="ship" />}
        >
          {current && (
            <view className="gap-2" data-testid="order-detail">
              <text role="heading" headingLevel={1} className="text-title font-semibold text-text">{current.title}</text>
              <text className="text-body text-text">{current.summary}</text>
              <text className="text-footnote text-muted">Placed on 12 November, paid by card.</text>
            </view>
          )}
        </ListDetail.Detail>
      </ListDetail>
    </view>
  );
}

const sections = [
  { id: 'mail', items: [
    { value: 'inbox', label: 'Inbox', endText: '12' },
    { value: 'drafts', label: 'Drafts', endText: '3' },
    { value: 'sent', label: 'Sent' },
  ] },
  { id: 'projects', title: 'Projects', items: [
    { value: 'launch', label: 'Spring launch' },
    { value: 'research', label: 'User research' },
  ] },
] as const;
const titles: Record<string, string> = { inbox: 'Inbox', drafts: 'Drafts', sent: 'Sent', launch: 'Spring launch', research: 'User research' };

/** A mail workspace held by the caller: the mailboxes beside the open one, its reading width capped for prose. */
export function Workspace() {
  const current = useLocal<string | null>('current', 'inbox');
  const title = current.value === null ? undefined : titles[current.value];
  const archived = useLocal<readonly string[]>('archived', []);
  const isArchived = current.value !== null && archived.value.includes(current.value);
  return (
    // A window's worth of page: in an app it fills the page itself.
    <view className="h-[360px] overflow-hidden rounded-container border border-text/12">
      <ListDetail value={current.value} onChange={next => current.set(next)} listLabel="Mail" listWidth={200} contentWidth="prose" data-testid="workspace">
        <ListDetail.List>
          <NavList label="Mailboxes" sections={sections} value={current.value ?? ''} onChange={next => current.set(next)} data-testid="workspace-nav" />
        </ListDetail.List>
        <ListDetail.Detail title={title} actions={<Button label={isArchived ? 'Archived' : 'Archive all'} isDisabled={isArchived} onClick={() => current.value !== null && archived.set([...archived.value, current.value])} data-testid="archive" />}>
          <text role="heading" headingLevel={1} className="text-title font-semibold text-text" data-testid="workspace-title">{title}</text>
          <text className="text-body text-muted">Each pane scrolls on its own. Beside the list the detail keeps a gutter from it; on a phone it takes the screen.</text>
        </ListDetail.Detail>
      </ListDetail>
    </view>
  );
}

/** The tile: a list's rows beside the chosen record, drawn at the tile's size. */
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
