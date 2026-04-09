# LxApp Development Guide

This guide covers how to write lxapp pages — project layout, the View + Logic architecture, data flow, event handling, and native component integration.

For host app project setup, see [App Project](./app-project.md).
For quick onboarding, see [Getting Started](./getting-started.md).
For a deep dive into `setData`, stream, and channel, see [Bridge Guide](./bridge-guide.md).

---

## Create an LxApp

```bash
lingxia new my-lxapp -t lxapp -y
```

This creates a standalone lxapp project. To create a host app (which contains an embedded home lxapp), use `-t native-app` instead (see [App Project](./app-project.md)).

---

## Project Layout

```text
my-lxapp/
├── lxapp.json
├── lxapp.config.ts
├── package.json
├── pages/
│   └── home/
│       ├── index.tsx   # View  — runs in WebView (React or Vue)
│       ├── index.ts    # Logic — runs in native JS runtime
│       └── index.json  # Page config (navigation bar, style)
├── public/
└── shared/
```

Key files:

- `lxapp.json`: Runtime metadata (`appId`, `appName` or `name`, `version`, `pages`).
- `lxapp.config.ts`: Build config for view tooling, aliases, and static asset directories.
- `pages/<name>/index.tsx` (or `.vue`): View layer — UI rendering in WebView.
- `pages/<name>/index.ts`: Logic layer — page lifecycle and business operations.
- `pages/<name>/index.json`: Page-level config (navigation/title/style and related options).

### Static assets

Use `staticDirs` in `lxapp.config.ts` to declare root-level directories that should be copied into `dist/` as-is for `html`, `react`, and `vue`.

```ts
export default {
  staticDirs: ['public', 'view', 'assets'],
};
```

Rules:

- `public/` and `assets/` are the default static directories. If the project root contains either directory, LingXia copies them to `dist/public/` and `dist/assets/` even when `staticDirs` is omitted.
- Additional directories must be declared explicitly in `staticDirs`.
- Explicit `staticDirs` entries must exist at the project root. LingXia treats missing configured directories as build errors.
- Paths are preserved. For example, `view/info-panel.js` becomes `dist/view/info-panel.js`.
- LingXia does not scan HTML, manifest files, or arbitrary source strings to discover static assets.

### Build

- `lingxia build` builds page assets and runtime artifacts into `dist/`.
- `lingxia build --release --package` produces package archive for publish.

---

## Page Architecture

Every page is split into two layers that communicate through a bridge:

```
┌─────────────────────────┐     setData()      ┌──────────────────────────┐
│       View (WebView)    │ ◄────────────────── │   Logic (Native Runtime) │
│  React/Vue + useLxPage  │ ────────────────────► Page({}) instance        │
│                         │   bridge functions   │                          │
└─────────────────────────┘                     └──────────────────────────┘
```

**View** renders UI. **Logic** owns state and business operations. Logic pushes state to View via `setData()`, and View calls Logic functions through auto-generated bridge bindings.

Recommended reading path:

- This guide: page layout, `Page({})`, `useLxPage`, events, and native components.
- [Bridge Guide](./bridge-guide.md): deeper mechanics of `setData`, stream, and channel.

---

## Logic Layer — `Page({})`

The Logic file exports a `Page({})` call. The `Page` function is provided globally by the runtime — you don't import it.

```ts
// pages/home/index.ts
Page({
  data: {
    count: 0,
    message: "Hello",
  },

  onLoad: function (options) {
    // Called when page is created. `options` contains URL query params.
    console.log("query:", options);
  },

  onShow: function () {
    // Called every time the page becomes visible.
  },

  // Action functions — callable from View
  increment: function () {
    this.setData({ count: this.data.count + 1 });
  },

  updateMessage: function (params) {
    // params is whatever the View passes
    this.setData({ message: params?.text || "" });
  },
});
```

### Key concepts

| API | Description |
| --- | --- |
| `this.data` | Current page state. Read-only — use `setData()` to change. |
| `this.setData(patch)` | Merge `patch` into `data` and replicate to View. Triggers re-render. |
| `onLoad(options)` | Lifecycle — page created. `options` are URL query params. |
| `onShow()` | Lifecycle — page becomes visible (including back-navigation). |
| `lx.*` | Global platform APIs (e.g. `lx.setNavigationBarTitle()`, `lx.createVideoContext()`). |

### Private helpers

Functions starting with `_` are private — they are **not** exposed to the View. Use them for internal logic:

```ts
Page({
  data: { total: 0 },

  _calculateTotal: function (items) {
    return items.reduce((sum, item) => sum + item.price, 0);
  },

  onCheckout: function (params) {
    const total = this._calculateTotal(params?.items || []);
    this.setData({ total });
  },
});
```

---

## View Layer

The View file can be a standard React component, a Vue component, or an HTML module entry. The framework packages connect View to the Logic layer and expose:

- `data` — reactive page state replicated from Logic via `setData()`
- `actions` — public functions exported from `Page({})`

### React

```tsx
// pages/home/index.tsx
import { useLxPage } from '@lingxia/react';

type PageData = {
  count?: number;
  message?: string;
};

type PageActions = {
  increment?: () => void;
  updateMessage?: (params: { text: string }) => void;
};

export default function HomePage() {
  const { data, actions } = useLxPage<PageData, PageActions>();
  const { increment, updateMessage } = actions;

  return (
    <div>
      <p>Count: {data?.count ?? 0}</p>
      <p>{data?.message}</p>
      <button onClick={() => increment?.()}>+1</button>
      <button onClick={() => updateMessage?.({ text: "World" })}>
        Update
      </button>
    </div>
  );
}
```

### Vue

```vue
<!-- pages/home/index.vue -->
<template>
  <div>
    <p>Count: {{ count }}</p>
    <p>{{ message }}</p>
    <button @click="increment?.()">+1</button>
    <button @click="updateMessage?.({ text: 'World' })">Update</button>
  </div>
</template>

<script setup lang="ts">
import { computed } from 'vue';
import { useLxPage } from '@lingxia/vue';

type PageData = {
  count?: number;
  message?: string;
};

type PageActions = {
  increment?: () => void;
  updateMessage?: (params: { text: string }) => void;
};

const { data, actions } = useLxPage<PageData, PageActions>();
const { increment, updateMessage } = actions;

const count = computed(() => data?.count ?? 0);
const message = computed(() => data?.message ?? '');
</script>
```

### HTML

```ts
// pages/home/entry.ts
import { getActions, subscribe } from '@lingxia/html';

type PageData = {
  count?: number;
  message?: string;
};

type PageActions = {
  increment?: () => void;
  updateMessage?: (params: { text: string }) => void;
};

const actions = getActions<PageActions>();
const countEl = document.getElementById('count');
const messageEl = document.getElementById('message');

document.getElementById('inc-btn')?.addEventListener('click', () => {
  actions.increment?.();
});

subscribe((data: PageData) => {
  if (countEl) countEl.textContent = String(data.count ?? 0);
  if (messageEl) messageEl.textContent = data.message ?? '';
});
```

```html
<!-- pages/home/index.html -->
<script type="module" src="./entry.ts"></script>
```

### What `useLxPage()` returns

```ts
const { data, actions } = useLxPage<PageData, PageActions>();
```

- **`data`** — Reactive page state, updated whenever Logic calls `setData()`. In React this triggers a re-render; in Vue it's a `reactive()` object.
- **`actions`** — All public functions from `Page({})` (except lifecycle hooks and `_`-prefixed methods). Each action is a bridge function that calls through to the Logic layer.

Use typed `PageActions` interfaces so View and Logic stay aligned as your page grows.

---

## Data Flow

```
Logic: this.setData({ count: 1 })
  ↓
Bridge: state replication (native → WebView)
  ↓
View: useLxPage().data updates → component re-renders
```

```
View: actions.increment?.()
  ↓
Bridge: function call (WebView → native → Logic JS runtime)
  ↓
Logic: increment() runs, may call this.setData() → cycle repeats
```

State flows **one way**: Logic → View. View never mutates `data` directly. Instead, View calls Logic actions which call `setData()` to update state.

---

## Event Handling

### Two event paths

LingXia components support two event paths:

1. **Logic short path** (native → Rust → Logic JS, 3 hops) — when the handler is a function from `actions`. The CLI auto-generates `pageFuncBindings` so the native layer routes the event directly to Logic, bypassing the WebView.

2. **View DOM path** (native → WebView CustomEvent → handler, 2 hops) — when the handler is a local View function. Events arrive as standard DOM CustomEvents.

As a developer, you don't need to choose between these paths. Use framework-native event syntax (`onX` in React, `@event` in Vue) and the system routes automatically.

### Native component events

Use `@lingxia/elements` for native-backed components. Event handlers use standard React/Vue syntax:

**React:**

```tsx
import { useLxPage, LxInput, LxPicker, LxVideo } from '@lingxia/react';

const { actions } = useLxPage();
const { onInputChange, onPickerConfirm, onPlaying } = actions;

// Input — handler receives unwrapped detail object
<LxInput onInput={onInputChange} />

// Picker — handler receives resolved value directly
<LxPicker
  columns={[['A', 'B', 'C']]}
  onConfirm={(value) => onPickerConfirm?.({ field: 'choice', value })}
/>

// Video — handler receives raw DOM Event
<LxVideo src={url} onPlaying={onPlaying} />
```

**Vue:**

```vue
<script setup lang="ts">
import { useLxPage, LxInput, LxPicker, LxVideo } from '@lingxia/vue';

const { actions } = useLxPage();
const { onInputChange, onPickerConfirm, onPlaying } = actions;
</script>

<LxInput @input="onInputChange" />

<LxPicker
  :columns="[['A', 'B', 'C']]"
  @confirm="(value) => onPickerConfirm?.({ field: 'choice', value })"
/>

<LxVideo :src="url" @playing="onPlaying" />
```

### Callback signatures vary by component

| Component | Callback receives | Example |
| --- | --- | --- |
| `LxInput` / `LxTextarea` | Unwrapped `event.detail` object | `onInput(detail)` → `detail.value` |
| `LxPicker` | Resolved value directly | `onConfirm(value)` → `value` is `string \| string[]` |
| `LxVideo` | Raw DOM Event | `onPlaying(event)` → `event.detail` |

See [Component API Reference](../packages/lingxia-elements/docs/component-api-reference.md) for full event lists.

---

## Action Shapes

From a page author's perspective, public `Page({})` methods come in three useful shapes:

| Logic method shape | Use from View | Typical use |
| --- | --- | --- |
| normal function / async function | `actions.foo(...)` from `useLxPage()` | button actions, navigation, one-shot work |
| async generator | `useLxStream(actions.foo, ...)` | progress, incremental output, chat-style streaming |
| channel-style session | `useLxChannel(actions.foo, ...)` | long-lived bidirectional sessions |

Examples:

- `increment()` and `updateMessage()` stay in the normal `actions` bucket.
- `async *onSend(...)` is a stream action and belongs with `useLxStream()`.
- Session-style logic that stays open over time belongs with `useLxChannel()`.

The runtime inspects the Logic method shape and routes it automatically. Use this guide for page authoring; use [Bridge Guide](./bridge-guide.md) for stream/channel lifecycle, cancellation, and transport details.

---

## Complete Example: Input Page

**Logic** (`pages/input/index.ts`):

```ts
Page({
  data: {
    inputValue: "",
    syncValue: "",
  },

  onLoad: function () {},

  onInputChange: function (detail) {
    // detail is the unwrapped event.detail from LxInput
    if (detail?.value === undefined) return;
    console.log("input changed:", detail.value);
  },

  onSyncInput: function (detail) {
    if (detail?.value === undefined) return;
    // Write back to data → View re-renders with updated value
    this.setData({ syncValue: String(detail.value) });
  },
});
```

**View** (`pages/input/index.tsx`):

```tsx
import { useLxPage, LxInput } from '@lingxia/react';

type PageData = { syncValue?: string };
type PageActions = {
  onInputChange?: (detail: Record<string, unknown>) => void;
  onSyncInput?: (detail: Record<string, unknown>) => void;
};

export default function InputPage() {
  const { data, actions } = useLxPage<PageData, PageActions>();
  const { onInputChange, onSyncInput } = actions;

  return (
    <div>
      <LxInput placeholder="Basic input" onInput={onInputChange} />

      <LxInput
        value={data?.syncValue || ''}
        placeholder="Synced input"
        onInput={onSyncInput}
      />
      <p>Current: {data?.syncValue}</p>
    </div>
  );
}
```

---

## Common Pitfalls

- Mixing view logic and page logic in one file; keep `index.tsx` and `index.ts` roles clear.
- Mutating `data` directly in View instead of calling Logic actions.
- Re-documenting bridge behavior inside page code instead of leaning on [Bridge Guide](./bridge-guide.md) for stream/channel details.
- Forgetting that only public `Page({})` methods become actions; lifecycle hooks and `_`-prefixed helpers are not exposed.

---

## Tips

- **Type your data**: Define a `PageData` type in both Logic and View to catch mismatches early.
- **Keep Logic pure**: Logic has no DOM access. Use `lx.*` APIs for platform operations, `setData()` for state.
- **Avoid heavy View state**: Prefer Logic-managed state via `setData()` over local `useState`/`ref`. This keeps state consistent across the bridge boundary.
- **Private with `_` prefix**: Functions starting with `_` won't be exposed to View. Use them for internal helpers.
- **Page config**: `index.json` controls navigation bar title, background color, and other page-level settings.
