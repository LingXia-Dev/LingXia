<!-- Generated from the doc file by scripts/generate.mjs; do not edit. -->

# ListDetail

Master-detail list and record view: side-by-side on regular screens, single-pane drill-down on compact. Status: preview.

```ts
import { ListDetailList, ListDetailDetail, ListDetail } from '@liulidesign/core/ListDetail';
```

ListDetail pairs a selectable master list with its detail pane from one component tree. On regular screens (`>= 600 dp`) both panes appear side by side, snap to a vertical fold hinge when one crosses the page, and enter `<SizeClassScope value="compact">` so nested cards stay single-column inside narrow panes. On compact screens (`< 600 dp`) choosing an item drills into the detail pane with a back header, keeps the list mounted so scroll position is preserved, and intercepts system Back (`useBackHandler`) without blocking Home.

## Examples

### Master-detail order list

```tsx
import { useLocal } from '@lingxia/view';
import { EmptyState } from '@liulidesign/core/EmptyState';
import { Item, ItemList } from '@liulidesign/core/Item';
import { ListDetail } from '@liulidesign/core/ListDetail';

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
```

## Props

| Prop | Type | Default | Description |
| --- | --- | --- | --- |
| `value` | `string \| null` |  | Controlled selected item id (`null`/`undefined` means no selection). |
| `onChange` | `((value: string \| null) => void)` |  | Called when selection changes (including Back in compact mode). |
| `bind` | `Local<string \| null>` |  | Uncontrolled local selection cell (alternative to `value` + `onChange`). |
| `listLabel` (required) | `string` |  | Accessible label for the list pane (also used as the compact back button label). |
| `listWidth` | `number` | `360` | Default list pane width in `regular` when no vertical hinge is present. |
| `children` | `Child` |  | `<ListDetail.List>` and `<ListDetail.Detail>` children. |
| `className` | `string` |  |  |
| `data-testid` | `string` |  |  |

## ListDetailList props

| Prop | Type | Default | Description |
| --- | --- | --- | --- |
| `children` | `Child` |  | The master list content, typically a `<scroll>` or `<List>` of rows. |
| `className` | `string` |  |  |

## ListDetailDetail props

| Prop | Type | Default | Description |
| --- | --- | --- | --- |
| `title` | `string` |  | Detail header title shown in compact drill-down mode and used as the detail group name. |
| `placeholder` | `Child` |  | Shown on regular screens when no item is selected. |
| `children` | `Child` |  | The selected item's detail content. |
| `className` | `string` |  |  |

## Anatomy

| Part | Element | Required | Description |
| --- | --- | --- | --- |
| root | `<view>` | yes | Fills its container: horizontal two-pane in regular, single active pane in compact. |
| listPane | `<view>` | yes | The master list group named by `listLabel`; hidden rather than unmounted during compact drill-down so scroll offset survives. |
| detailPane | `<view>` | yes | The detail group named by `Detail.title` (or `listLabel`), showing `placeholder` when regular has no selection. |
| compactHeader | `<view>` | no | Compact drill-down bar holding the back button and optional record title. |
| compactTitle | `<text>` | no | Truncated record title beside the compact back button. |

## What an app may override

- `className`: classes merged after the recipe's, so they win.
- `className` on ListDetailList: classes merged after the recipe's, so they win.
- `className` on ListDetailDetail: classes merged after the recipe's, so they win.
- The recipe, `listDetailRecipe` from `@liulidesign/core/ListDetail/recipe`: the classes of each part, to style an element of your own to match. The component does not read a changed copy.
- Scale tokens it reads, set in the app's stylesheet under `@theme`: `--text-body`.
- Colour roles it paints with, set by the theme pack: `canvas`, `text`.

## Accessibility

- Pattern: none.
- Name: listLabel.
- Description: none.
- Both panes expose named `group` landmarks so screen readers can jump between list and detail.
- System Back in compact drill-down clears the selection via `useBackHandler` and returns to the list at its prior scroll position.

| Contrast | Pair | Minimum | Light | Dark |
| --- | --- | --- | --- | --- |
| compact detail title | `text` on `canvas` | 4.5:1 | 15.19:1 | 15.31:1 |

Ratios are computed from @lingxia/view/theme and rounded down.

## Guidance

- Do: Bind selection to Logic (`value` + `onChange`) when deep links (`?id=...`) can open a record directly, or `bind={selected}` for purely local selection.
- Do: Supply a `placeholder` on `<ListDetail.Detail>` so regular screens explain what selecting a row will show.
- Don't: Split a page into separate compact and workspace view files just to switch between drill-down and side-by-side.
- Don't: Wrap `<ListDetail>` in an outer `<scroll>`; let `<ListDetail.List>` and `<ListDetail.Detail>` scroll independently.

Related: Split, SplitView, Item.
