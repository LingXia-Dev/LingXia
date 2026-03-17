# Native Components Design and API Contract

This document defines the long-term contract for LingXia native components in `@lingxia/components`.

## Design Philosophy

1. Single source of truth for business state: `Page({})` logic + `setData`.
2. Clear event ownership:
- `bindX` / `catchX`: logic-layer callbacks (`Page({})` functions).
- `onX`: view-layer callbacks (React/Vue/local custom element handlers).
3. Cross-platform consistency:
- Native event must reach logic and view through stable contracts.
- Event payload shape is normalized across Android / Apple / Harmony / Desktop.
4. Single implementation for native component behavior:
- The custom element (`lx-*`) is the only place that owns rendering behavior, native bridge integration, and component state machine.
- React/Vue wrappers are adapters, not alternate implementations.
5. Minimal logging in hot paths; only keep logs needed for diagnosis.

## Contract Summary

| Concern | Rule |
| --- | --- |
| Logic callback entry | `bindX` / `catchX` only |
| View callback entry | `onX` only |
| Logic event shape | WeChat-style event object (`type/detail/currentTarget/target/timeStamp`) |
| Dataset source | element `data-*` attributes |
| Binding resolution | case-insensitive event name matching |
| Cross-platform scope | Android / iOS / macOS / Harmony / desktop fallback |
| React/Vue wrappers | Thin adapters only: prop mapping, event bridging, ref/type ergonomics |

## Event Routing Contract

## Logic path (`bindX` / `catchX`)

1. Component collects bindings from element attributes.
2. Native runtime receives `pageFuncBindings` and resolves function by event name.
3. Native dispatches to Rust runtime.
4. Rust invokes `Page({})` function with a normalized event object.

Rules:
- Binding value must be a non-empty function name string.
- Event name matching is case-insensitive.

## View path (`onX`)

1. Native runtime emits `component.event` back to WebView bridge.
2. `LingXiaBridge.nativeComponents.register(id, handler)` receives the event.
3. Custom element dispatches corresponding DOM/CustomEvent.
4. React/Vue wrappers call view handler props/listeners.

Rules:
- `onX` never calls `Page({})` directly.
- `onX` is local to the current view runtime.

## Framework Wrapper Contract

React/Vue wrappers exist for framework ergonomics only. They must stay thin.

Allowed responsibilities:
- Register the underlying custom element if needed.
- Map framework-friendly props to native element attributes/properties.
- Bridge DOM/CustomEvent into framework callbacks/listeners.
- Expose framework-friendly `ref`, typing, `class`, `style`, and children/slot passthrough.

Disallowed responsibilities:
- Re-implement native component rendering or layout logic.
- Maintain a second component state machine separate from the custom element.
- Add framework-only business rules that change runtime behavior across React/Vue/native.
- Depend on wrapper-local synchronization hacks unless required for framework interoperability.

Implementation rule:
- When behavior changes, fix the custom element first.
- Only change React/Vue wrappers when the issue is strictly about framework integration.

Rationale:
- One behavioral implementation keeps Android / Apple / Harmony / desktop behavior aligned.
- Thin wrappers reduce bug surface and avoid fixing the same problem twice in React and Vue.

## Event Object Contract

All logic callbacks receive one event object:

```ts
interface NativeComponentPageEvent<TDetail = unknown> {
  type: string;
  detail: TDetail;
  target: {
    id: string;
    dataset: Record<string, unknown>;
  };
  currentTarget: {
    id: string;
    dataset: Record<string, unknown>;
  };
  timeStamp: number;
}
```

Requirements:
- `type`: normalized lower-case event name.
- `detail`: event-specific payload.
- `target/currentTarget.dataset`: derived from `data-*` attributes.

## Interface/API Contract

### Shared Native Payload Props

| Field | Type | Required | Notes |
| --- | --- | --- | --- |
| `dataset` | `Record<string, unknown>` | Yes | Runtime object form |
| `datasetJson` | `string` | Yes | JSON fallback for platform bridge differences |
| `pageFuncBindings` | `Record<string, string>` | Yes | event -> page-function map |
| `pageFuncBindingsJson` | `string` | Yes | JSON fallback for platform bridge differences |

Rules:
- `dataset` and `pageFuncBindings` are always sent (empty object allowed).
- Empty `pageFuncBindings` explicitly clears previous bindings on native side.

## Component API Reference

Component-specific interface definitions are maintained in:
- [Component API Reference](./component-api-reference.md)

## Cross-platform Requirements Checklist

- Android / iOS / macOS / Harmony all dispatch `bindX` via Rust page-function entry.
- Android / iOS / macOS / Harmony all emit `component.event` for `onX` view callbacks.
- Desktop web fallback follows same payload shape and naming rules.

## Compatibility Notes

- `bindX` is the only supported way to call `Page({})` functions from component events.
- `onX` is the only supported way for local view handlers.
- Mixing semantics is unsupported and may be removed.
