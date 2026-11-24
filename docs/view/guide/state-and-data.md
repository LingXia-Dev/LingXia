# State and data

```tsx data={"userId":"u1"}
import { useLocal, useRequest, useStream } from '@lingxia/view';
import type { View } from './$types';

export function render({ data, actions }: View) {
  const profile = useRequest('profile', actions.profile, { id: data.userId });
  const draft = useLocal('');
  const summary = useLocal('');
  const stream = useStream('summary', actions.summarize, { onData: chunk => summary.set(summary.value + chunk.token) });
  const save = async () => {
    await actions.rename({ id: data.userId, name: draft.value });
    await profile.reload();
  };
  return (
    <page className="p-4 gap-3">
      {profile.error && <text role="alert" className="text-sm text-danger">{profile.error.message}</text>}
      <text className={profile.stale ? 'text-lg opacity-50' : 'text-lg'}>{profile.value?.name ?? (profile.pending ? 'Loading…' : '')}</text>
      <input bind={draft} aria-label="New name" className="px-3 py-2 rounded-lg border" />
      <button disabled={!draft.value} onClick={save} className="rounded-lg bg-accent-solid px-4 py-2 text-on-accent-solid">Rename</button>
      <button onClick={() => { summary.set(''); stream.start({ id: data.userId }); }} disabled={stream.status === 'streaming'} className="rounded-lg bg-accent-solid px-4 py-2 text-on-accent-solid">Summarize</button>
      <text className="text-sm whitespace-pre-wrap">{summary.value}</text>
    </page>
  );
}
```

The View holds no data of its own beyond `useLocal` cells. Everything else comes from Logic: as
`view.data`, as the result of an action, or as messages on a stream or channel.

## Which one when

| Need | Use |
| --- | --- |
| State Logic reads or keeps | `data` in `definePage`; Logic changes it with `page.update(d => …)`, or `page.set({ key })` for whole keys |
| State only one piece of UI shows (a draft, an open menu) | [`useLocal`](#uselocalinitial) |
| State several pages show (session, cart) | a `defineStore` in Logic, changed with its `set`/`update`; a page shows it with `follow: { key: store.pick(…) }` |
| Wait or repeat | `page.after(ms, fn)` / `page.every(ms, fn)` in Logic; the View has [no timers](#what-the-view-cannot-do) |
| Call Logic from a handler | the [action](#actions) itself (`onClick={actions.refresh}`); add [`useAction`](#useactionaction) to show `pending`/`error` |
| Load when the component first renders, again when params change | [`useRequest`](#userequestname-action-params) |
| Progress or streamed output | an `async *` action, with [`useStream`](#usestreamname-action--ondata-) |
| A long-lived two-way line | a channel action, with [`useChannel`](#usechannelname-action--params-ondata-) |
| Go to another page | `<link to={{ page, params }}>` ([links](text.md#links)); `lx.nav.go` in Logic when it decides |

## `view.data`

`view.data` is the Logic page's data (`definePage({ data })`, changed with `page.update`). It is deeply
read-only, and every update re-renders the page. `import type { View } from './$types'` types it, and
`view.actions`, from the page's Logic.

## Actions

`view.actions` holds the Logic page's `actions`. Call one from an event handler (calling during render is refused with `UI_RENDER`) with at most one
JSON payload; it returns a `Promise` of the action's JSON result. An `async *action()` generator is a streaming action and returns a
`Stream`. A failed action rejects with an error carrying `code` and `message`. An action promise that
rejects inside an event handler without a `catch` is reported in the page log.

An action that takes no payload can be the handler itself, `onClick={actions.refresh}`; one that takes a
payload is wrapped, `onClick={() => actions.open({ id })}`, and passing it bare is a type error.

Logic for the first example:

```ts
import { definePage } from '@lingxia/logic';

const names = new Map<string, string>([['u1', 'Ada']]);

export default definePage({
  data: { userId: 'u1' },
  actions: {
    async profile({ id }: { id: string }) { return { name: names.get(id) ?? '', plan: 'free' }; },
    async rename({ id, name }: { id: string; name: string }) { names.set(id, name); },
    async *summarize({ id }: { id: string }) {
      let tokens = 0;
      for (const token of `${names.get(id)} is on the free plan.`.split(' ')) { tokens++; yield { token: `${token} ` }; }
      return { tokens };
    },
  },
});
```

## `useLocal(initial)`

A cell of JSON state that belongs to the component instance ([Pages and components](pages-and-components.md)).

- `useLocal(initial)` keys the cell by its call order in the component, as React's `useState`: call it
  unconditionally. `useLocal(name, initial)` keys it by name, so it survives reordered calls and hot edits.

- `.value` is deeply read-only. `.set(next)` replaces it and re-renders; it is refused during render
  (`UI_RENDER`) and on a cell whose component has unmounted (`UI_LOCAL`).
- `.focus()` focuses the editor bound to the cell with `bind`, from a handler (refused in render and
  after unmount, as `.set`). See [after a server error](inputs-and-forms.md#after-a-server-error).
- Values are JSON: strings, numbers, booleans, `null`, arrays and plain objects. Object fields that
  are `undefined` are dropped. Functions, `Date`, `Map` and class instances are refused.
- A `set` made while a render is being prepared applies when that render commits. A frame the screen
  only declined keeps the change for the next one; a frame that fails (the render throws, or its
  description is rejected with a `UI_*` error) discards it, and the page keeps its last good state.
- Named cells in custom hooks take a namespace: ``useLocal(`${name}/draft`, '')``.

## `useAction(action)`

One action called from handlers, with its state: `run(payload?)` calls it; `pending` is true while the
latest call is in flight; `error` is its `{ code, message }` failure, or `null`. `run` never rejects,
so it can be the handler itself: `<button onClick={save.run} disabled={save.pending}>`. It resolves
`{ ok: true, value }` (`value` is `null` for an action that returns nothing) or `{ ok: false, error }`:
`if ((await save.run(draft)).ok) draft.set('')`. Several actions that share one busy state compose their hooks:
`const writes = [add, remove]; const busy = writes.some(w => w.pending);`. A second use of one action in a component takes a
name: `useAction('retry', actions.save)`. An action is also a handler by itself (`onClick={actions.pick}`);
then a failure goes to the page log.

## `useRequest(name, action, params?)`

Calls a unary action after the frame that declares it commits, and again whenever `params` (compared as
JSON) changes.

| Field | Meaning |
| --- | --- |
| `pending` | a call is in flight |
| `value` | the last successful result, or `null`; kept while a new call runs |
| `stale` | `value` belongs to an earlier call than the one in flight or failed |
| `error` | `{ code, message }` of the last failure, or `null` |
| `reload()` | call again with the same params; resolves when it settles |

A result that arrives after its params changed, or after the component unmounted, is dropped.

## `useStream(name, action, { onData? })`

Owns one streaming action. `start(...params)` begins a stream, cancelling one already running;
`cancel()` stops it. `data` is the latest yielded value (`null` before the first and after a restart),
so showing progress needs no state of its own. `onData` receives each
yielded value, for side effects such as appending chunks; `result` is the generator's return value.
`status` is `idle`, `streaming`, `completed`, `canceled` or `failed` (with `error`). Unmounting cancels
the stream, and late messages from a replaced stream are ignored.

## `useChannel(name, action, { params, onData? })`

A two-way channel to a Logic method `method(params, channel)`. It opens after commit and reopens when
`params` changes. `send(payload)` sends to Logic; `onData` receives what Logic sends. `connecting`,
`connected` and `error` report its state. `close()` closes it and keeps it closed across re-renders;
`connect()` opens it again. Unmounting closes it.

```tsx
import { useChannel, useLocal } from '@lingxia/view';
import type { View } from './$types';

type Tick = { symbol: string; price: number };

export function render({ actions }: View) {
  const last = useLocal<Tick | null>('last', null);
  const ticker = useChannel('ticker', actions.ticker, { params: { symbol: 'AAPL' }, onData: tick => last.set(tick) });
  return (
    <page className="p-4 gap-2">
      <text className="text-3xl font-bold tabular-nums">{last.value ? last.value.price.toFixed(2) : '---'}</text>
      <text className="text-xs">{ticker.connected ? 'Live' : ticker.connecting ? 'Connecting…' : 'Offline'}</text>
      <button onClick={() => ticker.send({ symbol: 'MSFT' })} disabled={!ticker.connected} className="rounded-lg bg-accent-solid px-4 py-2 text-on-accent-solid">Watch MSFT</button>
    </page>
  );
}
```

Its Logic types the channel with `PageChannel<Send, Receive>`:

```ts
import { definePage, type PageChannel } from '@lingxia/logic';

type Tick = { symbol: string; price: number };

export default definePage({
  actions: {
    ticker(params: { symbol: string }, channel: PageChannel<Tick, { symbol: string }>) {
      let symbol = params.symbol;
      channel.on('data', next => { symbol = next.symbol; });
      channel.send({ symbol, price: 100 });
    },
  },
});
```

## `useLeaveGuard(name, { enabled })`

While `enabled`, a back gesture or a tap on home does not leave the page. Instead `pending` becomes
`'back'` or `'home'`; show a confirmation and call `answer(true)` to leave or `answer(false)` to stay.
`request(reason)` asks the same question from your own button. Nested guards answer newest first, and
every guard must accept.

```tsx
import { useLeaveGuard, useLocal } from '@lingxia/view';

export function render() {
  const saved = useLocal('saved', '');
  const draft = useLocal('draft', '');
  const guard = useLeaveGuard('draft', { enabled: draft.value !== saved.value });
  return (
    <page className="p-4 gap-3">
      <textarea bind={draft} aria-label="Draft" />
      <button onClick={() => saved.set(draft.value)} className="rounded-lg bg-accent-solid px-4 py-2 text-on-accent-solid">Save</button>
      <dialog open={guard.pending !== null} onClose={() => guard.answer(false)} aria-label="Discard changes?" className="gap-2 p-4 rounded-xl bg-surface-overlay">
        <text>Discard your changes?</text>
        <view className="flex-row gap-2">
          <button onClick={() => guard.answer(false)} className="rounded-lg bg-surface-sunken px-4 py-2 text-text">Keep editing</button>
          <button onClick={() => guard.answer(true)} className="rounded-lg bg-accent-solid px-4 py-2 text-on-accent-solid">Discard</button>
        </view>
      </dialog>
    </page>
  );
}
```

A page that leaves the stack is parked: its local state is released and it renders afresh if it is
shown again. Until then automation still reads its last frame, and refuses to act on it
([data-testid](pages-and-components.md#data-testid)).

## Errors and stale data

Every resource reports failures as `{ code, message }` (`error`), never by throwing into render. Show
`error.message` and offer `reload()`/`start()`/`connect()`. While a reload runs, `value` keeps the old
result and `stale` is `true`, so dim it rather than blanking the screen.

## What the View cannot do

The View runs in an isolate without host capabilities: no `lx`, `fetch`, `XMLHttpRequest`, `WebSocket`,
`setTimeout`, `setInterval`, `requestAnimationFrame` or storage. Polling, timing, network, persistence
and locale formatting (`Intl`) live in Logic; send their results through `data`, an action, a stream or
a channel. A render that runs too long is dropped (`UI_OVERRUN`) and runs again; several in a row stop the page (`UI_TIMEOUT`) and it must reload.
