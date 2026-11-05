# Lists and scrolling

```tsx data={"messages":[{"id":"m1","text":"Hi"},{"id":"m2","text":"Hello there"}],"hasMore":true}
import { List, useLocal, type View } from '@lingxia/view';

type Message = { id: string; text: string };
type Actions = { loadOlder(): Promise<void> };

export function render({ data, actions }: View<Actions, { messages: Message[]; hasMore: boolean }>) {
  const messages = data.messages;
  return (
    <page>
      <List
        items={messages}
        getKey={message => message.id}
        renderItem={message => <Bubble text={message.text} />}
        anchor={{ index: Math.max(0, messages.length - 1), align: 'end', seq: messages.length }}
        onReachStart={() => { if (data.hasMore) actions.loadOlder(); }}
        className="grow px-4 gap-2 overscroll-contain"
      />
    </page>
  );
}

function Bubble({ text }: { text: string }) {
  const expanded = useLocal('expanded', false);
  return (
    <button onClick={() => expanded.set(!expanded.value)} className="self-start rounded-2xl bg-surface-sunken text-text px-3 py-2">
      <text className={expanded.value ? 'text-sm' : 'text-sm line-clamp-3'}>{text}</text>
    </button>
  );
}
```

## `<scroll>`

`<scroll>` scrolls its content along one axis: vertically by default, horizontally with
`overflow-x-auto`. Its children lay out as a column; put a `<row>` inside a horizontal scroll.

```tsx
export function render() {
  const chips = ['All', 'Unread', 'Mentions', 'Files', 'Links', 'Pinned'];
  return (
    <page className="py-4 gap-3">
      <scroll className="overflow-x-auto">
        <row className="gap-2 px-4">
          {chips.map(chip => <button key={chip} onClick={() => {}} className="rounded-full bg-accent px-3 py-1 text-sm text-on-accent">{chip}</button>)}
        </row>
      </scroll>
      <scroll className="grow" onReachEnd={() => {}}>
        <column className="px-4 gap-2">
          {Array.from({ length: 30 }, (_, i) => <text key={String(i)} className="text-sm">Row {i + 1}</text>)}
        </column>
      </scroll>
    </page>
  );
}
```

- A `<scroll>` directly in `<page>` fills the remaining height. Elsewhere give it a height, `max-h-*` or `grow`.
- The scroll position belongs to the scroll's identity and survives re-renders.
- `onReachStart` / `onReachEnd` fire once each time the content arrives at that edge, after the frame that
  shows it, and again only after it has left the edge. Use them to load more.
- In RTL a horizontal scroll starts at its inline start (the right edge) and advances leftward.
- A gesture moves the innermost scroll under the finger on its axis; leftover distance passes to the next
  ancestor on the same axis. `overscroll-contain` or `overscroll-none` keeps it in this scroll.
- `sticky` children stick within the scroll ([Layout](layout.md)).

There is no scroll-to API for `<scroll>`. Use a list `anchor` when you must move the position.

### Snapping

`snap-y` (or `snap-x`) on a `<scroll>` or `<List>` and `snap-start`, `snap-center` or `snap-end` on its
children make it settle on a child: when a finger, wheel or fling stops, the region eases to the nearest
point (at once under reduced motion). Every snap is mandatory; `snap-mandatory` is accepted and
`snap-proximity` is refused.

## Virtual lists

`<List>` renders only the rows near the viewport, so it handles long and growing data. Rows are
created by index as they approach the viewport and dropped when they leave it.

| Prop | Meaning |
| --- | --- |
| `items` | the data, up to 100 000 items |
| `getKey(item, index)` | a unique string per item, at most 256 bytes |
| `renderItem(item, index)` | the row element |
| `getItemHeight(item, index)` | optional fixed row height; rows are measured otherwise |
| `anchor` | where to move the position (below) |
| `onReachStart`, `onReachEnd` | edge arrivals, as on `<scroll>` |

`gap-*` separates rows; padding pads the whole list. Lists scroll vertically only. Rows are their
item's sibling index for `first:`, `last:`, `odd:` and `even:`, counted over the whole list.

### Keys and row state

Keys let a row keep its local state and resources through inserts, removals and reorders, including
while it is scrolled out of view. Removing an item from `items` releases its state. Duplicate keys fail
(`UI_LIST`). All keys of a page together may take 256 KiB.

Hooks may sit directly in `renderItem`: their state follows the item's key.

A long list costs nothing per row on a render that passes the same frozen `items` array: its keys are
read once. `Object.freeze` an array the View builds; Logic data is frozen, but each data update brings new
arrays. A mutable array is keyed again on every render.

### Anchors

`anchor` moves the list when its value changes, then leaves scrolling to the user:

- `anchor="end"` moves to the end once.
- `anchor={{ index, align: 'start' | 'end', seq }}` brings row `index` to the top or bottom of the viewport.
  Change `seq` to move again to the same index.
- Add `duration` (ms) to scroll there smoothly instead of jumping. The scroll runs natively, so rows render
  only as they come into view; a finger or wheel on the list stops it, and reduced motion jumps.

To keep a chat at its newest message, anchor the last index with `seq` set to the item count, as in the
first example: each new message changes `seq` and moves the list once. Jumping far away does not render
the rows in between.

Automation reaches rows that are not drawn: a `data-testid` query finds an off-screen row of a `<List>` (the
first 2000 rows) as attached and visible outside the viewport, as a WebView DOM does, and acting on it scrolls the list there from the rows'
estimated heights. Its text and attributes are readable once it draws.

## Pull to refresh

Pull to refresh belongs to the host. Enable it in the page config and handle it in Logic:

```json
{ "enablePullDownRefresh": true }
```

```ts
Page({
  data: { items: [] as string[] },
  async onPullDownRefresh() {
    this.setData({ items: await load() });
    lx.stopPullDownRefresh();
  },
});
```

The host draws the indicator and moves the page; the View only renders the new data.
