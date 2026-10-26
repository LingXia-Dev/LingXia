# LingXia View language

A LingXia page can render natively instead of in a WebView. Its View is written in a TSX dialect:
JSX over a fixed set of elements, styled with Tailwind utilities, compiled at build time, and laid out,
shaped and painted by the host. This guide is the complete reference for that dialect.

```tsx data={"id":"n1"}
import { useLocal, useRequest, type View } from '@lingxia/ui';

type Actions = { load(query: { id: string }): Promise<{ title: string; body: string }> };

export function render({ data, actions }: View<Actions, { id: string }>) {
  const note = useRequest('note', actions.load, { id: data.id });
  const draft = useLocal('draft', '');
  return (
    <page className="bg-canvas">
      <scroll className="grow">
        <column className="p-4 gap-3">
          <text role="heading" className="text-xl font-semibold">{note.value?.title ?? 'Loading…'}</text>
          <text className="text-sm text-muted whitespace-pre-line">{note.value?.body ?? ''}</text>
          <input bind={draft} accessibilityLabel="Reply" placeholder="Reply" className="px-3 py-2 rounded-lg border border-line" />
          <button disabled={!draft.value} onClick={() => draft.set('')} className="px-4 py-2 rounded-lg bg-accent text-on-accent active:opacity-80">
            Send
          </button>
        </column>
      </scroll>
    </page>
  );
}
```

## Mental model

- **View and Logic.** A page has a View (`index.tsx`, this language) and optionally Logic (`index.ts`,
  `Page({ … })`). Logic owns data, network, storage and `lx.*`. The View renders `view.data` and calls
  Logic through `view.actions`. The View has no `lx`, `fetch`, timers or DOM.
- **No DOM.** Elements are not HTML elements. There is no `document`, no CSS file at runtime, no
  selector engine, no event bubbling. A tag you do not find in [elements](reference/elements.md) does not exist.
- **Candidate, then commit.** Every render produces a candidate frame. The host lays it out and, if
  anything fails, keeps the last good frame and discards the candidate with its state changes. Resources
  (`useRequest`, `useStream`, `useChannel`, `useLeaveGuard`) start only once a frame commits.
- **Tailwind at build time.** The official Tailwind compiler runs when you build. Each utility is lowered
  to native style or refused with a diagnostic; nothing CSS-shaped runs on the device.
- **The host owns chrome.** Navigation bar, tab bar, status bar, safe areas, keyboard, pull-to-refresh,
  the IME and accessibility services belong to the host. The View lays out inside them.
- **Append-only contract.** Element, prop and style ids only ever grow. A page compiled against a newer
  contract refuses to start on an older host (`UI_COMPAT`) instead of rendering wrongly.

## Platform status

Native rendering ships on **Android**. On other platforms a page marked `"renderer": "gpu"` does not
render natively yet; keep WebView pages there.

## How to read this guide

Each chapter opens with a complete page that compiles; every example in the guide is compiled and
rendered by the test suite. Blocks marked `tsx error UI_…` show code that is refused with that code.

| Chapter | Covers |
| --- | --- |
| [Pages and components](guide/pages-and-components.md) | page config, `render`, components, props, keys |
| [Layout](guide/layout.md) | flex, grid, stack, sticky, fixed, insets, keyboard, units |
| [Styling](guide/styling.md) | utilities, arbitrary values, variants, themes, `style` |
| [Text](guide/text.md) | whitespace, fonts, numerics, rich text, links, selection |
| [Images and vectors](guide/images-and-vectors.md) | images, fit, caching, vectors, icons |
| [Inputs and forms](guide/inputs-and-forms.md) | editors, toggles, radios, sliders, keyboards, pickers |
| [Lists and scrolling](guide/lists-and-scrolling.md) | `<List>`, `<scroll>`, anchors, reach events, overscroll |
| [State and data](guide/state-and-data.md) | `useLocal`, `useRequest`, `useStream`, `useChannel`, `useLeaveGuard` |
| [Interaction](guide/interaction.md) | buttons, press states, gestures, hit testing |
| [Accessibility](guide/accessibility.md) | labels, roles, headings, live regions, hidden content |
| [RTL and i18n](guide/rtl-and-i18n.md) | direction, logical properties, `rtl:`, display language |
| [Motion](guide/motion.md) | transitions, keyframe animations, reduced motion |
| [Diagnostics](guide/diagnostics.md) | build and runtime errors |
| [Limits and compatibility](guide/limits-and-compatibility.md) | bounds, host compatibility |
| [Coming from React](guide/coming-from-react.md) | what changes for React and DOM authors |

The [reference](reference/) is generated from the schema: [elements](reference/elements.md),
[styles](reference/styles.md), [utilities](reference/utilities.md), [variants](reference/variants.md),
[limits](reference/limits.md), [errors](reference/errors.md), [environment](reference/environment.md),
[semantics](reference/semantics.md).
