# Bridge API for JS Developers

This guide explains how View and Logic communicate through the bridge — covering `setData`, stream, and channel. It is written for developers writing lxapp pages, not for implementers of the bridge itself.

For a broad introduction to the View/Logic split, see [LxApp Development Guide](./guide.md).

---

## The Bridge Model

Every lxapp page has two layers:

```
┌─────────────────────────────┐                ┌──────────────────────────────┐
│        View (WebView)       │                │    Logic (Native JS Runtime) │
│   React / Vue component     │ ◄──── bridge ──► Page({}) instance            │
└─────────────────────────────┘                └──────────────────────────────┘
```

**Logic** owns all business state and operations. It runs in a native JS runtime, not in the WebView. **View** renders UI and reacts to user input. It runs in the WebView and has no direct access to Logic's data.

The bridge is the only path between them. It carries three categories of data:

| Category | Direction | When to use |
|---|---|---|
| **State** (`setData`) | Logic → View | Durable page state: counters, lists, flags |
| **Stream** (`yield` / `stream.send`) | Logic → View | Incremental output: tokens, progress, chunks |
| **Channel** (`ch.send`) | bidirectional | Long-lived sessions: real-time sync, collaboration |

These three cover every communication pattern. Choosing the right one for a given scenario keeps the architecture clean and performant.

---

## State — `setData`

`setData` is the primary mechanism for Logic to push data to View. It merges a partial object into `this.data` and replicates the updated state to the WebView.

### Logic side

```ts
// pages/counter/index.ts
Page({
  data: {
    count: 0,
    label: 'Start',
  },

  increment() {
    this.setData({ count: this.data.count + 1 });
  },

  reset() {
    this.setData({ count: 0, label: 'Start' });
  },
});
```

Rules:
- `this.data` is read-only. Never mutate it directly — use `setData`.
- `setData` accepts a partial object. Only the listed keys are updated; the rest are unchanged.
- The call is synchronous on the Logic side. Replication to View is asynchronous.

### View side

```tsx
// pages/counter/index.tsx
import { useLxPage } from '@lingxia/react';

type PageData = { count: number; label: string };

export default function Counter() {
  const { data, actions } = useLxPage<
    PageData,
    {
      increment: () => void;
      reset: () => void;
    }
  >();

  return (
    <div>
      <p>{data.label}: {data.count}</p>
      <button onClick={() => actions.increment()}>+1</button>
      <button onClick={() => actions.reset()}>Reset</button>
    </div>
  );
}
```

`useLxPage().data` reflects whatever Logic has replicated. It updates reactively — no polling, no manual subscription.

### How replication works

Under the hood, `setData` produces a JSON Patch diff and delivers it to View via `state.patch` frames. View applies the patch and triggers a re-render. This diff-based approach is efficient for low-frequency state transitions, but it is not designed for high-frequency payloads — for that, use stream.

```
Logic: this.setData({ count: 1 })
  │
  ▼  (JSON Patch diff computed)
Bridge: state.patch { ops: [{ op:"replace", path:"/count", value:1 }] }
  │
  ▼
View: data.count === 1 → re-render
```

### When to use `setData`

- Page state that must persist across navigation and be restorable (e.g., a message list, user profile, form values).
- State that must outlive a stream or channel session (e.g., saving the final output after a stream completes).
- Any data the View needs to render its initial or resting state.

Do **not** use `setData` for per-chunk stream output. The diff cost and delivery cycle make it unsuitable for hot-path data.

---

## Stream

A stream is a one-shot, View-initiated operation where Logic produces a sequence of chunks and terminates. The pattern is `request → events* → done`.

Use streams when:
- Logic performs a long operation and View needs progress updates (file processing, LLM token output, multi-step calculations).
- The output is incremental and the client should start rendering before completion.

### Logic side — generator form

The simplest form — no imports, no special API. Write a standard `async *` generator method on your `Page({})` and the runtime detects it automatically via `Symbol.asyncIterator`. Each `yield` becomes an event frame delivered to View; `return` ends the stream.

```ts
type ChatChunk =
  | { type: 'token'; token: string }
  | { type: 'artifact'; chart: ChartData };

async function* mockChatStream(): AsyncGenerator<ChatChunk, void> {
  yield { type: 'token', token: 'Hello ' };
  yield { type: 'token', token: 'from ' };
  yield { type: 'token', token: 'LingXia.' };
}

Page({
  data: {
    messages: [] as Message[],
    isStreaming: false,
  },

  async *onSend(params: { text: string }) {
    const text = (params?.text ?? '').trim();
    if (!text || this.data.isStreaming) return;

    const userMsg: Message = {
      id: `u${Date.now()}`,
      role: 'user',
      content: text,
    };
    this.setData({
      messages: [...this.data.messages, userMsg],
      isStreaming: true,
    });

    let accumulated = '';
    let chartData: ChartData | undefined;

    try {
      for await (const chunk of mockChatStream()) {
        if (chunk.type === 'token') accumulated += chunk.token;
        if (chunk.type === 'artifact') chartData = chunk.chart;
        yield chunk;
      }
    } finally {
      const assistantMsg: Message = {
        id: `a${Date.now()}`,
        role: 'assistant',
        content: accumulated || '(no response)',
        chart: chartData,
      };
      this.setData({
        messages: [...this.data.messages, assistantMsg],
        isStreaming: false,
      });
    }
  },
});
```

The real chat example optionally probes an app-installed AI extension before falling back to mock data, but that extension is app-specific and not part of LingXia's built-in bridge API.

### Logic side — explicit handle form

Use this when your async source is callback-based rather than an async iterator. You do not import `StreamHandle` — the runtime creates and injects it as the second parameter automatically for methods the build classifies as streams.

```ts
Page({
  async onProcess(params: { fileId: string }, stream: StreamHandle) {
    const job = lx.files.process(params.fileId);

    job.on('progress', (pct) => stream.send({ type: 'progress', pct }));
    job.on('done',     (out) => stream.end(out));
    job.on('error',    (err) => stream.error('PROCESS_FAILED', err.message));
  },
});
```

`StreamHandle` exposes `send` (a chunk), `end` (final value), and `error` (terminate with an error). For the exact signatures read the `StreamHandle` declaration in `@lingxia/types` — that is authoritative; don't re-copy it here.

The explicit handle has **no cancellation callback**: when the View cancels, the runtime resolves the call with `BRIDGE_CANCELED` and drops the handle — your handler is not notified. If you need to clean up on cancel (abort a job, close a file), use the generator form and a `finally` block instead; that is the only form that observes cancellation.

The runtime distinguishes the two forms automatically — you never declare them by hand. At build time the CLI classifies each page action into a `BridgeMode` (`"notify" | "call" | "stream"`) and emits a `__modes` map onto `window.__pageBridge`; a method that returns an `AsyncGenerator` (or takes the injected handle) is tagged `"stream"`, everything else falls through. There is no author-facing metadata field to maintain.

### View side

```tsx
import { useState } from 'react';
import { useLxPage, useLxStream } from '@lingxia/react';
import type { LxStream } from '@lingxia/bridge';

type StreamState = { text: string; chart?: ChartData };

export default function ChatPage() {
  const { data, actions } = useLxPage<
    { messages: Message[] },
    {
      onSend: (params: { text: string }) => LxStream<ChatChunk, void>;
      onClear: () => void;
    }
  >();

  const [inputText, setInputText] = useState('');

  const chat = useLxStream<typeof actions.onSend, StreamState>(
    actions.onSend,
    {
      params: () => ({ text: inputText }),
      manual: true,
      initial: { text: '' },
      reduce: (acc, chunk) => {
        if (chunk.type === 'token') return { ...acc, text: acc.text + chunk.token };
        if (chunk.type === 'artifact') return { ...acc, chart: chunk.chart };
        return acc;
      },
    },
  );

  const handleSend = () => {
    const text = inputText.trim();
    if (!text || chat.streaming) return;
    chat.start();
    setInputText('');
  };

  return (
    <div>
      <MessageList messages={data.messages} />
      {chat.streaming && <StreamingBubble text={chat.data.text} />}
      <input value={inputText} onChange={e => setInputText(e.target.value)} />
      <button onClick={handleSend} disabled={chat.streaming}>Send</button>
      {chat.streaming && <button onClick={() => chat.cancel()}>Stop</button>}
    </div>
  );
}
```

`useLxStream` returns a `LxStreamState` — `data` (accumulated via `reduce`, or the latest chunk), `result` (final value), `error`, `streaming`, plus `start()` and `cancel()`. The exact field types live in `LxStreamState` / `LxStreamOptions` in `@lingxia/react`; that's the authoritative shape — read it rather than trusting a copy here.

The options worth knowing conceptually:
- `manual: true` — stream doesn't start until you call `chat.start()`. With `manual: false` (default), it starts on mount and cancels on unmount.
- `initial` — initial `data` value before the first chunk arrives.
- `reduce` — accumulator function. If omitted, `data` is simply the latest chunk.

### `setData` vs `yield` — which to use during a stream

Both can push data to View, but they are for different things:

| | `setData` | `yield` / `stream.send` |
|---|---|---|
| Transport | JSON Patch diff | Direct payload, no diff |
| Delivery | Batched state cycle | Immediate |
| Use for | State that outlives the stream | Per-chunk hot-path data |

**Rule of thumb**: `yield` every chunk. Use `setData` for state transitions that should persist after the stream ends — saving the final message, clearing a loading flag.

### Data flow

```
View: chat.start() → actions.onSend({ text })
  │
  ▼  (req frame)
Bridge → Logic: invoke async generator
  │
  ▼  generator yields { type:'token', token:'H' }
Bridge: event frame { seq:0, payload:{ type:'token', token:'H' } }
  │
  ▼
View: reduce(acc, chunk) → chat.data.text = 'H' → re-render

  ... more yields ...

  generator returns
  │
  ▼  (res frame, ok:true)
View: chat.streaming = false

  (if View cancels)
View: chat.cancel() → cancel frame
  │
  ▼
Logic: generator.return() → finally block executes
  │
  ▼  (res frame, ok:false, BRIDGE_CANCELED)
View: chat.streaming = false, chat.error set
```

Chunks carry a per-`yield` sequence number, so delivery order is guaranteed regardless of async timing. An unhandled exception in the generator terminates the stream with an error result (surfaced on `chat.error`).

---

## Channel

A channel is a long-lived, bidirectional session between View and Logic. Either side can send messages at any time after the channel is open.

Use channels when:
- The connection must persist for the duration of a user interaction session (collaborative editing, real-time sync, live data feeds).
- Logic needs to push multiple unsolicited updates while the session is active.
- View needs to send multiple commands to Logic over time.

### Logic side

You do not import `ChannelHandle` — the runtime creates and injects it as the second parameter when View opens a channel. The runtime routes `ch.open` frames by topic (derived from the method name) and invokes the handler.

```ts
Page({
  syncSession(params: { sessionId: string }, ch: ChannelHandle) {
    const session = lx.sessions.open(params.sessionId);

    // Logic → View: push updates when they happen
    session.onUpdate(update => ch.send({ type: 'update', update }));
    session.onEvent(event  => ch.send({ type: 'event', event }));

    // Send initial state when channel opens
    ch.send({ type: 'init', state: session.state, rev: session.rev });

    // Receive messages from View
    ch.on('data', (msg) => {
      if (msg.type === 'op') {
        const result = session.apply(msg.op);
        ch.send({ type: 'ack', rev: result.rev });
      }
    });

    // Cleanup when channel closes
    ch.on('close', () => {
      session.release();
    });
  },
});
```

The handler function receives `ChannelHandle` as its second parameter. Use `ch.send()` to push data to View, and `ch.on()` to register listeners for incoming data and close events. This is the same event-listener pattern used throughout LingXia.

`ChannelHandle` (injected by the runtime) exposes `send` (push to View), `close`, and `on('data' | 'close', …)` for receiving from View. For the precise generic signatures read the `ChannelHandle` declaration in `@lingxia/types` — authoritative, not re-listed here.

### View side

```tsx
import { useEffect } from 'react';
import { useLxPage, useLxChannel } from '@lingxia/react';
import type { LxChannel } from '@lingxia/bridge';

export default function EditorPage() {
  const { actions } = useLxPage<
    {},
    { syncSession: (p: { sessionId: string }) => Promise<LxChannel<SessionMessage, SessionCommand>> }
  >();

  const session = useLxChannel(
    actions.syncSession,
    { params: () => ({ sessionId: 'doc-123' }) },
  );

  // Handle incoming messages from Logic
  useEffect(() => {
    if (!session.last) return;
    const msg = session.last;
    if (msg.type === 'init')   applyInitialState(msg.state);
    if (msg.type === 'update') applyUpdate(msg.update);
  }, [session.last]);

  const sendOp = (op: Op) => {
    session.send({ type: 'op', op });
  };

  return (
    <div>
      {session.connecting && <p>Connecting...</p>}
      <Editor onOp={sendOp} />
      <button onClick={() => session.close()}>End session</button>
    </div>
  );
}
```

`useLxChannel` returns a `LxChannelState` — `last` (latest received message), `error`, `connecting`, `connected`, plus `send(payload)`, `close()`, and `reopen()`. The authoritative field types are `LxChannelState` / `LxChannelOptions` in `@lingxia/react`; read those rather than a copy. `connected` flips `true` after the channel acks and `false` after it closes; `reopen()` is useful after an error or with `manual: true`.

The channel re-opens automatically when `params` changes. Pass `{ manual: true }` to control open timing yourself and call `reopen()` manually.

### Push during a channel session: `ch.send`, not `setData`

Within an open channel, Logic-to-View pushes go through `ch.send`. Do not use `setData` for high-frequency in-session events.

`ch.send` delivers directly, without the diff cost or delivery batch of `state.patch`. Use `setData` only for state that must survive the channel — for example, a badge count that reflects an external change that happened while no channel was active.

### Multiplexed message types

One channel carries multiple message types via discriminated union. This avoids opening parallel channels for related concerns.

```ts
// All of these flow through a single channel:
ch.send({ type: 'init',   state, rev });
ch.send({ type: 'update', update });
ch.send({ type: 'ack',    rev });
ch.send({ type: 'error',  reason });
```

On the View side, switch on `msg.type` to route each frame.

### Channel lifecycle

```
View: useLxChannel opens
  │
  ▼  ch.open frame
Bridge → Logic: invoke syncSession(params, ch)
  │
  ▼  ch.ack { ok: true }
View: session.connected = true
  │
  ┌─────────────────────────────────┐
  │  bidirectional data exchange    │
  │  ch.data (both directions)      │
  └─────────────────────────────────┘
  │
  ▼  ch.close (either side)
View: session.connected = false
Logic: ch.on('close') listener fires → cleanup
```

---

## Choosing the Right Primitive

| Scenario | Use |
|---|---|
| Counter, form values, lists | `setData` |
| LLM token streaming | `stream` (generator) |
| File processing with progress | `stream` (explicit handle) |
| Save final output after streaming | `setData` in `finally` block |
| Real-time collaborative editing | `channel` |
| Live sensor data feed | `channel` |
| Device event subscription (internal) | Logic subscribes internally, exposes via `setData` |

**Note on subscriptions**: Logic subscribes to external systems (sensors, push, backend events) internally and surfaces results through `setData`. Subscription APIs are not exposed to View — that would move resource ownership to the wrong layer.

---

## Error Handling

All three primitives surface errors via `LxBridgeError` — `{ code: string | number; message?: string; data?: unknown }`, declared authoritatively in `@lingxia/bridge`. The `code` is the part you branch on; the common values are labeled below.

Common error codes:

| Code | Meaning |
|---|---|
| `BRIDGE_CANCELED` | stream or request was canceled |
| `BRIDGE_METHOD_NOT_FOUND` | method name doesn't match any Logic handler |
| `BRIDGE_TOPIC_NOT_FOUND` | channel topic not registered |
| `BRIDGE_TIMEOUT` | request timed out |
| `BRIDGE_INTERNAL_ERROR` | unexpected error in Logic or Bridge |

For streams, check `chat.error` after `chat.streaming` becomes `false`. For channels, check `session.error` after `session.connected` becomes `false`.

---

## Platform Detection

- **View**: `window.LingXiaBridge.platform` — `isIOS()`, `isMacOS()`, `isDesktop()`, `getOS()`, … (sync; read the global, never import).
- **Logic**: `lx.device.getDeviceInfo()` → `osName` (async).
