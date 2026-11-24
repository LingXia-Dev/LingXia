<!-- Generated from the doc file by scripts/generate.mjs; do not edit. -->

# ListDetail

A list and the detail of the row chosen in it: side by side on a wide screen, one over the other on a phone. Status: preview.

```ts
import { ListDetailList, ListDetailDetail, ListDetail } from '@liulidesign/core/ListDetail';
```

ListDetail lays out a list and the record chosen in it from one tree, driven by one value: the chosen row's id. On a wide screen the list sits at the start, `listWidth` wide, and the detail beside it, capped so lines stay readable; at a fold that crosses the page the list takes the start leaf and the detail the other. On a phone the list fills the screen; choosing a row opens its detail over it, under a way back, and Back returns to the list where it was scrolled. Each pane scrolls on its own, and the detail starts at its top for each new choice. A deep link that sets the value opens the record directly.

## Examples

### Orders, with an action on the chosen one

```tsx
import { useLocal } from '@lingxia/view';
import { Button } from '@liulidesign/core/Button';
import { EmptyState } from '@liulidesign/core/EmptyState';
import { Item, ItemList } from '@liulidesign/core/Item';
import { ListDetail } from '@liulidesign/core/ListDetail';

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
```

### A mail workspace held by the caller

```tsx
import { useLocal } from '@lingxia/view';
import { Button } from '@liulidesign/core/Button';
import { NavList } from '@liulidesign/core/NavList';
import { ListDetail } from '@liulidesign/core/ListDetail';

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
```

## Props

| Prop | Type | Default | Description |
| --- | --- | --- | --- |
| `bind` | `Local<string \| null>` |  | A View-local cell holding the chosen row's id, `null` while none is. |
| `value` | `string \| null` |  | The chosen row's id, held by the caller: in Logic state, or from a deep link. `null` while none is. |
| `onChange` | `((value: string \| null) => void)` |  | Runs with `null` when Back or the back button leaves the detail on a phone. |
| `listLabel` (required) | `string` |  | Names the list pane: what it lists, "Orders". It is also the back button's label on a phone. |
| `listWidth` | `number` | `360` | The list's width in pt beside the detail, when no fold crosses the page. |
| `contentWidth` | `ListDetailContentWidth` | `'wide'` | How wide the detail may grow; past it the surplus falls on the far side. |
| `children` | `Child` |  | `<ListDetail.List>` and `<ListDetail.Detail>`. |
| `className` | `string` |  |  |
| `data-testid` | `string` |  | On the root; the panes get `<testid>-list` and `<testid>-detail`, the back button `<testid>-back`, the actions `<testid>-actions`. |

## ListDetailList props

| Prop | Type | Default | Description |
| --- | --- | --- | --- |
| `children` | `Child` |  | The list, usually an ItemList of rows; it scrolls on its own. |
| `className` | `string` |  |  |

## ListDetailDetail props

| Prop | Type | Default | Description |
| --- | --- | --- | --- |
| `title` | `string` |  | The chosen record's title, as the detail pane's name; the detail shows its own heading. |
| `placeholder` | `Child` |  | Shown beside the list while nothing is chosen, on a wide screen: what choosing a row will show. |
| `actions` | `Child` |  | Actions on the chosen record, such as Approve: a row under the detail, outside its scroll, so they stay in view. |
| `children` | `Child` |  | The chosen record's detail; it scrolls on its own and starts at its top for each new choice. |
| `className` | `string` |  |  |

## Anatomy

| Part | Element | Required | Description |
| --- | --- | --- | --- |
| root | `<view>` | yes | Fills its page: the two panes side by side, or the one a phone shows. |
| list | `<view>` | yes | The list pane, a group named by `listLabel`, with a hairline at its end. On a phone it stays mounted while the detail covers it, so its scroll survives. |
| detail | `<view>` | yes | The detail pane, a group named by `title`; on a phone it slides in from the end edge and out again, unless motion is reduced. |
| back | `<view>` | no | Phone: the back button, named by `listLabel`, alone in its row outside the detail's scroll. |
| scroll | `<scroll>` | yes | In each pane, so the two scroll apart. |
| listInner | `<view>` | yes | The list's padding. |
| detailInner | `<view>` | yes | The detail's padding and cap, held to the start of its pane. |
| placeholder | `<view>` | no | Wide screen, nothing chosen: what choosing a row will show. |
| actions | `<view>` | no | With `actions`: a row under the detail's scroll, behind a hairline, that stays in view. |

## What an app may override

- `className`: classes merged after the recipe's, so they win.
- `className` on ListDetailList: classes merged after the recipe's, so they win.
- `className` on ListDetailDetail: classes merged after the recipe's, so they win.
- The recipe, `listDetailRecipe` from `@liulidesign/core/ListDetail/recipe`: the classes of each part, to style an element of your own to match. The component does not read a changed copy.
- It reads no Liuli scale token.
- Colour roles it paints with, set by the theme pack: `canvas`, `text`.

## Accessibility

- Pattern: none.
- Name: listLabel.
- Description: none.
- Each pane is a group named by its label, so a screen reader can move between them; the detail's name follows the chosen record.
- The chosen row says so itself: pass `isSelected` to its Item.
- On a phone Back leaves the detail for the list, as the back button does; it is taken only with `bind`, or `value` with `onChange`.

| Contrast | Pair | Minimum | Light | Dark |
| --- | --- | --- | --- | --- |

Ratios are computed from @lingxia/view/theme and rounded down.

## Guidance

- Do: Hold the choice in Logic with `value` and `onChange` when a deep link can open a record; use `bind` when the page alone knows it.
- Do: Give `<ListDetail.Detail>` a `placeholder`, so a wide screen says what choosing a row will show.
- Do: Put the actions on the chosen record in `actions`, so a long record never hides them.
- Do: Start the detail with the record's own heading: the bar carries only Back, on a phone as on a wide screen, so the title shows once.
- Do: Put it straight in a `<page>`: it sizes the panes to the page, and at a fold to its leaves.
- Do: Use `contentWidth="prose"` for reading and `wide` for tables or examples.
- Don't: Build a phone page and a wide page apart just to switch between one pane and two.
- Don't: Put it inside a `<scroll>`, or a scroll inside its panes; each pane scrolls itself.

## Not yet

| Missing | Needs | Meanwhile |
| --- | --- | --- |
| Dragging the hairline to resize the list | Pointer coordinates on an element (`onPointer`) | A fixed `listWidth` |

## Limits

- At a fold, a start leaf narrower than 200 pt still gets a 200 pt list, which then reaches past the crease.

Related: Split, Item, NavList.
