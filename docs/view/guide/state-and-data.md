# State and data

```tsx data={"userId":"u1"}
import { useLocal, useRequest, useStream, type Stream, type View } from '@lingxia/ui';

type Profile = { name: string; plan: string };
type Chunk = { token: string };
type Actions = {
  profile(input: { id: string }): Promise<Profile>;
  rename(input: { id: string; name: string }): Promise<void>;
  summarize(input: { id: string }): Stream<Chunk, { tokens: number }>;
};

export function render({ data, actions }: View<Actions, { userId: string }>) {
  const profile = useRequest('profile', actions.profile, { id: data.userId });
  const draft = useLocal('draft', '');
  const summary = useLocal('summary', '');
  const stream = useStream('summary', actions.summarize, { onData: chunk => summary.set(summary.value + chunk.token) });
  const save = async () => {
    await actions.rename({ id: data.userId, name: draft.value });
    await profile.reload();
  };
  return (
    <page className="p-4 gap-3">
      {profile.error && <text role="alert" className="text-sm text-red-700">{profile.error.message}</text>}
      <text className={profile.stale ? 'text-lg opacity-50' : 'text-lg'}>{profile.value?.name ?? (profile.pending ? 'Loading…' : '')}</text>
      <input bind={draft} accessibilityLabel="New name" className="px-3 py-2 rounded-lg border" />
      <button disabled={!draft.value} onClick={save}>Rename</button>
      <button onClick={() => { summary.set(''); stream.start({ id: data.userId }); }} disabled={stream.status === 'streaming'}>Summarize</button>
      <text className="text-sm whitespace-pre-wrap">{summary.value}</text>
    </page>
  );
}
```

The View holds no data of its own beyond `useLocal` cells. Everything else comes from Logic: as
`view.data`, as the result of an action, or as messages on a stream or channel.

## `view.data`

`view.data` is the Logic page's data (`Page({ data })`, updated with `this.setData`). It is deeply
read-only, and every update re-renders the page. Type it with the second parameter of `View`.

## Actions

`view.actions` holds the Logic page's methods. Call one from an event handler (calling during render is refused with `UI_RENDER`) with at most one
JSON payload; it returns a `Promise` of the method's JSON result. An `async *method()` generator is a streaming action and returns a
`Stream`. A failed action rejects with an error carrying `code` and `message`. An action promise that
rejects inside an event handler without a `catch` is reported in the page log.

Logic methods for the first example:

```ts
Page({
  data: { userId: 'u1' },
  async profile({ id }: { id: string }) { return loadProfile(id); },
  async rename({ id, name }: { id: string; name: string }) { await saveName(id, name); },
  async *summarize({ id }: { id: string }) {
    let tokens = 0;
    for await (const token of summarizeProfile(id)) { tokens++; yield { token }; }
    return { tokens };
  },
});
```

## `useLocal(name, initial)`

A cell of JSON state that belongs to the component instance ([Pages and components](pages-and-components.md)).

- `.value` is deeply read-only. `.set(next)` replaces it and re-renders; it is refused during render
  (`UI_RENDER`) and on a cell whose component has unmounted (`UI_LOCAL`).
- Values are JSON: strings, numbers, booleans, `null`, arrays and plain objects. Object fields that
  are `undefined` are dropped. Functions, `Date`, `Map` and class instances are refused.
- A `set` made while a render is being prepared applies when that render commits; if the frame fails,
  the change is discarded with it.
- Custom hooks namespace their cells by name: ``useLocal(`${name}/draft`, '')``.

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
`cancel()` stops it. `onData` receives each yielded value; `result` is the generator's return value.
`status` is `idle`, `streaming`, `completed`, `canceled` or `failed` (with `error`). Unmounting cancels
the stream, and late messages from a replaced stream are ignored.

## `useChannel(name, action, { params, onData? })`

A two-way channel to a Logic method `method(params, channel)`. It opens after commit and reopens when
`params` changes. `send(payload)` sends to Logic; `onData` receives what Logic sends. `connecting`,
`connected` and `error` report its state. `close()` closes it and keeps it closed across re-renders;
`connect()` opens it again. Unmounting closes it.

```tsx
import { useChannel, useLocal, type ChannelEndpoint, type View } from '@lingxia/ui';

type Tick = { symbol: string; price: number };
type Actions = { ticker(params: { symbol: string }, channel: ChannelEndpoint<Tick, { symbol: string }>): void };

export function render({ actions }: View<Actions>) {
  const last = useLocal<Tick | null>('last', null);
  const ticker = useChannel('ticker', actions.ticker, { params: { symbol: 'AAPL' }, onData: tick => last.set(tick) });
  return (
    <page className="p-4 gap-2">
      <text className="text-3xl font-bold tabular-nums">{last.value ? last.value.price.toFixed(2) : '---'}</text>
      <text className="text-xs">{ticker.connected ? 'Live' : ticker.connecting ? 'Connecting…' : 'Offline'}</text>
      <button onClick={() => ticker.send({ symbol: 'MSFT' })} disabled={!ticker.connected}>Watch MSFT</button>
    </page>
  );
}
```

## `useLeaveGuard(name, { enabled })`

While `enabled`, a back gesture or a tap on home does not leave the page. Instead `pending` becomes
`'back'` or `'home'`; show a confirmation and call `answer(true)` to leave or `answer(false)` to stay.
`request(reason)` asks the same question from your own button. Nested guards answer newest first, and
every guard must accept.

```tsx
import { useLeaveGuard, useLocal } from '@lingxia/ui';

export function render() {
  const saved = useLocal('saved', '');
  const draft = useLocal('draft', '');
  const guard = useLeaveGuard('draft', { enabled: draft.value !== saved.value });
  return (
    <page className="p-4 gap-3">
      <textarea bind={draft} accessibilityLabel="Draft" />
      <button onClick={() => saved.set(draft.value)}>Save</button>
      {guard.pending && (
        <column role="dialog" accessibilityLabel="Discard changes?" className="gap-2 p-4 rounded-xl bg-neutral-100">
          <text>Discard your changes?</text>
          <row className="gap-2">
            <button onClick={() => guard.answer(false)}>Keep editing</button>
            <button onClick={() => guard.answer(true)}>Discard</button>
          </row>
        </column>
      )}
    </page>
  );
}
```

## Errors and stale data

Every resource reports failures as `{ code, message }` (`error`), never by throwing into render. Show
`error.message` and offer `reload()`/`start()`/`connect()`. While a reload runs, `value` keeps the old
result and `stale` is `true`, so dim it rather than blanking the screen.

## What the View cannot do

The View runs in an isolate without host capabilities: no `lx`, `fetch`, `XMLHttpRequest`, `WebSocket`,
`setTimeout`, `setInterval`, `requestAnimationFrame` or storage. Polling, timing, network, persistence
and locale formatting (`Intl`) live in Logic; send their results through `data`, an action, a stream or
a channel. A render that runs too long is stopped (`UI_TIMEOUT`) and the page must reload.
