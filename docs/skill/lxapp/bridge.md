# Bridge

How View and Logic exchange data: page state, streams, and channels. The page
model itself is in [Lxapp pages](./guide.md).

## Three primitives

| Primitive | Direction | Transport | Use for |
|---|---|---|---|
| **State** (`page.update`) | Logic → View | Diff, batched | Durable page state that outlives a stream or channel: lists, flags, final results |
| **Stream** (`yield` / `stream.send`) | Logic → View | Payload as sent, immediate, ordered | One View-started operation with incremental output: tokens, progress |
| **Channel** (`ch.send`) | both | Payload as sent, immediate | Long-lived sessions: live sync, collaboration |

Never push per-chunk or per-event data with `page.update`. Logic subscribes to
external systems itself and surfaces results through `page.update`; View never
gets subscription APIs.

## State

```ts
// pages/counter/index.ts
export default definePage({
  data: { count: 0, profile: { name: '' } },
  setup: page => ({
    actions: {
      increment() { page.update(d => { d.count += 1; }); },
      rename(name: string) { page.update(d => { d.profile.name = name; }); },
    },
  }),
});
```

- `page.data` is read-only. `page.update` runs the recipe on a copy and sends
  the View only what changed, at any depth.
- `page.set({ saving: false })` assigns whole keys, as an `update` would.
  A helper in `logic/` takes `PageLike<Data>` (`data`, `set`, `update`) from `@lingxia/logic`.
- The call is synchronous; replication is asynchronous. `await page.flush()`
  waits until the View has everything written so far.
- `useLxPage().data` follows it reactively.

## Stream

A View-started operation that yields chunks and ends: `request → events* →
done`.

### Generator form

An `async *` action is a stream; each `yield` is a chunk, `return` ends
it, and `finally` runs on cancel.

```ts
export default definePage({
  data: { messages: [] as Message[], isStreaming: false },
  setup: page => ({
    actions: {
      async *onSend(params: { text: string }) {
        page.update(d => { d.isStreaming = true; });
        let text = '';
        try {
          for await (const chunk of chatStream(params.text)) {
            if (chunk.type === 'token') text += chunk.token;
            yield chunk;
          }
        } finally {
          page.update(d => {
            d.messages.push({ role: 'assistant', content: text });
            d.isStreaming = false;
          });
        }
      },
    },
  }),
});
```

### `useLxStream`

```tsx
import { useLxPage, useLxStream } from '@lingxia/react';
import type { LxStream } from '@lingxia/bridge';

const { actions } = useLxPage<PageData, {
  onSend: (params: { text: string }) => LxStream<ChatChunk, void>;
}>();

const chat = useLxStream<typeof actions.onSend, { text: string }>(actions.onSend, {
  params: () => ({ text: input }),
  manual: true,                 // start with chat.start(); default starts on mount
  initial: { text: '' },
  reduce: (acc, chunk) =>
    chunk.type === 'token' ? { text: acc.text + chunk.token } : acc,
});

// chat.data, chat.result, chat.error, chat.streaming, chat.start(), chat.cancel()
```

Without `reduce`, `data` is the latest chunk. A thrown generator ends the
stream with `chat.error`; `chat.cancel()` settles it with `BRIDGE_CANCELED`.

## Channel

A long-lived, two-way session opened by the View. The handler receives a
`PageChannel` from `@lingxia/logic` as its second parameter:

```ts
actions: {
  syncSession(params: { sessionId: string }, ch: PageChannel) {
    const session = openSession(params.sessionId);
    ch.send({ type: 'init', state: session.state });
    session.onUpdate((update) => ch.send({ type: 'update', update }));
    ch.on('data', (msg) => {
      if (msg.type === 'op') ch.send({ type: 'ack', rev: session.apply(msg.op) });
    });
    ch.on('close', () => session.release());
  },
},
```

Carry several message types over one channel as a discriminated union.

### `useLxChannel`

```tsx
import { useLxChannel } from '@lingxia/react';

const session = useLxChannel(actions.syncSession, {
  params: () => ({ sessionId: 'doc-123' }),
});

useEffect(() => {
  if (session.last?.type === 'update') applyUpdate(session.last.update);
}, [session.last]);

// session.send(msg), session.close(), session.reopen(),
// session.connecting, session.connected, session.error
```

The channel reopens when `params` changes; `{ manual: true }` leaves opening
to `reopen()`. `send` returns `false`, sending nothing, while no channel is
open. A method whose payload is required needs `params` (a type error
otherwise), for `useLxStream` too.

## Errors

All three primitives reject with `LxBridgeError` (`{ code, message?, data? }`);
branch on `code`.

| Code | Meaning |
|---|---|
| `BRIDGE_CANCELED` | stream or request was canceled |
| `BRIDGE_METHOD_NOT_FOUND` | method name doesn't match any Logic handler |
| `BRIDGE_TOPIC_NOT_FOUND` | channel topic not registered |
| `BRIDGE_TIMEOUT` | request timed out |
| `BRIDGE_MESSAGE_TOO_LARGE` | the encoded frame exceeded the 64 KiB native message limit — split the payload, or move the bulk through a file or a stream |
| `BRIDGE_INTERNAL_ERROR` | unexpected error in Logic or Bridge |

Check `chat.error` once `streaming` is false, and `session.error` once
`connected` is false.
