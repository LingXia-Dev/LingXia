# WebView pages

The second path: a View written in React or HTML, rendered in a WebView. Logic, lifecycle,
navigation and the tab bar are the same as for GPU pages: [Pages](../guide.md).

## View

A View is a React component or an HTML module entry. It renders `data`
and calls `actions`; it never mutates `data`.

The page mounts once its first state (the `definePage({ data })` defaults) has
arrived, so `data` is whole from the first render. Declare `definePage` `data`
fields as **required**; use `?:` only for a field filled later.
Never write `actions.foo?.()`. If Logic fails to deliver state, the page shows
a panel naming itself.

| Hook | Returns | Changes |
|---|---|---|
| `useLxPage<typeof page>()` | `{ data, actions }` — this page's Logic state and actions | on every update |
| `useLxHost()` | [host facts](#host-facts) | only when a field changes |
| `useLxLeaveGuard(dirty, onRequest)` | nothing — [guards leaving](#leaving-a-page-with-unsaved-changes) while `dirty` | — |

Any component may call them. Streams and channels: `useLxStream` / `useLxChannel`
([Bridge](../bridge.md)); catalogue text: `useText` ([text in several languages](../guide.md#text-in-several-languages)).
Geometry is CSS, not a hook: [page chrome CSS](#page-chrome-css).

`data` is deeply read-only. In a dev session a write throws.
Keep what the user edits as View state and submit it with an action; Logic's
`page.update` brings the result back:

```tsx
const { data, actions } = useLxPage<typeof page>();
const [draft, setDraft] = useState(data.message);
<input value={draft} onChange={(e) => setDraft(e.target.value)} />
<button onClick={() => actions.updateMessage({ text: draft })}>Save</button>
```

### React

```tsx
// pages/home/index.tsx
import { useLxHost, useLxPage } from '@lingxia/react';
import type page from './index.ts';

export default function HomePage() {
  const { data, actions } = useLxPage<typeof page>();
  const { formFactor } = useLxHost();

  return (
    <div data-form-factor={formFactor}>
      <p>Count: {data.count}</p>
      <p>{data.message}</p>
      <button onClick={() => actions.increment()}>+1</button>
      <button onClick={() => actions.updateMessage({ text: 'World' })}>
        Update
      </button>
    </div>
  );
}
```

### HTML

```ts
// pages/home/entry.ts — index.html loads it with <script type="module">
import { getPage, pageReady, subscribePage } from '@lingxia/html';
import type page from './index.ts';

const render = () => draw(getPage<typeof page>().data);
document.getElementById('inc-btn')?.addEventListener('click', () => getPage<typeof page>().actions.increment());

// Plain HTML has no mount to gate: wait for the first state, then follow it.
void pageReady().then(() => {
  render();
  subscribePage(render);
});
```

`pageReady()` shows a startup fault after 10 seconds and still mounts when
state arrives. Pass `{ timeoutMs }` only when the caller handles rejection.
Read `useLxPage()` during component setup/render; pass its actions into helpers.

### Host facts

```ts
const { sizeClass, canDock, language, formFactor, os, runner } = useLxHost();
```

`getHost()` / `subscribeHost(cb)` in `@lingxia/html`;
`window.LingXiaBridge.host.get()` / `.subscribe(cb)` in a page that bundles
nothing. It never changes while a window is dragged.

- `sizeClass` (`compact` | `regular`) and `canDock` — how much room:
  [Adaptive UI](../adaptive-ui.md). CSS reads the class as `<html data-size-class>`.
- `formFactor` (`mobile` | `desktop`) — which machine. Branch on it for what a
  phone must not show. A narrow desktop window is `desktop`; a tablet or
  unfolded fold is `mobile`.
- `os` — `'ios' | 'macos' | 'android' | 'windows' | 'harmony' | 'unknown'` (Logic's `HostOs`), for
  OS-specific features only. `runner` is `true` in the dev Runner.
- `language` — the product language narrowed to lxapp.json `languages`; it sets
  `<html lang>` and `<html dir>`. See [Product settings](../core.md#product-settings).

`formFactor`, `os`, and `runner` are fixed for a page's life. A Runner phone
frame reports `mobile` while `os` names the desktop; switching form factor
re-serves the page. Logic reads the OS lower-case (`'macos'`, `'ios'`, …) from
`lx.host.info.os`.

## Events

Use framework syntax (`onX` in React). A handler can be an
`actions.*` function or a local View function; either receives what the
component reports, and an `actions.*` handler forwards it to Logic as its
payload. Native component callbacks and their payload shapes:
[Native elements](./native-elements.md#callback-shapes).

## Leaving a page with unsaved changes

A pushed page can hold itself while it has unsaved changes. When the user
then tries to leave — navigation bar back or home, Android/Harmony system
back, the edge swipe — the page stays and your handler is asked. Return
`true` to let the user go (the host then does what they asked), `false` to
stay. Works without Logic.

```tsx
import { useLxLeaveGuard } from '@lingxia/react';

useLxLeaveGuard(dirty, () => confirmDiscard()); // boolean | Promise<boolean>
```

Without a framework, hold while dirty and release when clean:

```ts
import { holdLeaveGuard } from '@lingxia/bridge'; // or @lingxia/html,
                                                  // window.LingXiaBridge.leaveGuard.hold
const release = holdLeaveGuard(({ reason }) => confirmDiscard()); // reason: 'back' | 'home'
release();
```

- Several guards on one page are independent: the page is held while any is
  live, and all must return `true` to leave.
- While a request is being answered, further back/home presses are ignored.
- Not guarded: programmatic navigation (`lx.nav`, `<lx-navigator>`), tab switches, closing the lxapp from the
  capsule, and the root page (nothing to pop).
- The guard belongs to the current document: a reload, or leaving the stack,
  clears it.

## Appearance

Which scheme applies: [Appearance](../guide.md#appearance).
The runtime sets `color-scheme` and `data-theme="light|dark"` on `<html>`. Key
theme CSS off `[data-theme]`, with a `prefers-color-scheme` fallback for first
paint. The theme's roles are CSS variables from the first paint and follow appearance and contrast:
`background: var(--lx-canvas); color: var(--lx-text);` keeps the page one surface with its bars.
A variable the page declares itself wins over the runtime's.

## Page chrome CSS

A `standard` tab bar shortens the View. For an `immersive` one, and for the
capsule, use the CSS variables; never hard-code platform heights:

```css
.page-scroll {
  padding-bottom: var(--lx-page-chrome-bottom-inset);
}

.floating-action {
  bottom: calc(16px + var(--lx-page-chrome-bottom-inset));
}

/* Apply this only to controls in the capsule's top band, not the whole page. */
.page-header {
  padding-inline-end: var(--lx-page-chrome-capsule-inline-end-inset);
}
```

```css
.page-header {
  padding-top: calc(var(--lx-page-chrome-capsule-bottom) + 12px);
}
```

- `--lx-page-chrome-capsule-{top,right,bottom,left,width,height}` are `0px`
  without a capsule. `--lx-page-chrome-top-inset` covers a
  [full-chrome window](../adaptive-ui.md#edge-to-edge-windows).
- In Runner, `env(safe-area-inset-*)` alone does not describe simulated chrome;
  use these variables.
- For JavaScript placement, `window.lxPageChrome.layout` holds the snapshot and
  `lxpagechromechange` reports changes. Logic has no capsule API.

