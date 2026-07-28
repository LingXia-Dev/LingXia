# LingXia Host Appearance and ShellTheme Specification

> Status: proposal · Date: 2026-07-28
>
> Platforms: Android / iOS / HarmonyOS / macOS / Windows / Runner
>
> Scope: ownership of host appearance, the semantic native-shell color
> palette, configuration in `lingxia.yaml`, persistence, propagation, and
> platform resolution. Page chrome consumes this contract but is specified in
> [`page-chrome-spec.md`](./page-chrome-spec.md).

This document uses MUST / MUST NOT / SHOULD / MAY in their normative sense.
It describes the target architecture, not the current implementation.

---

## 0. Decision summary

Appearance is host state, and `shellTheme` is host configuration.

- One host process has one appearance preference: `system`, `light`, or
  `dark`.
- The host resolves that preference to one effective color scheme: `light` or
  `dark`.
- `shellTheme` is configured once in the host project's `lingxia.yaml`.
- Every bundled or dynamically opened lxapp inherits the same effective color
  scheme and native shell palette.
- An lxapp MUST NOT declare its own `theme`, `shellTheme`, or light/dark native
  chrome palette.
- Missing theme tokens use platform semantic colors. A missing dark token MUST
  NOT fall back to its light value, or vice versa.
- Page WebViews receive the effective color scheme through the platform's
  normal `prefers-color-scheme` mechanism. `shellTheme` tokens are not injected
  into page CSS.
- Tabbar, navigation bar, browser chrome, sidebar, and other host-owned UI
  consume semantic roles. They do not independently own light/dark palettes.

The intended dependency order is:

```text
host appearance preference
          |
          v
effective color scheme + ShellTheme palette
          |
          +-------------------+
          |                   |
          v                   v
native shell chrome      WebView color scheme
          |
          v
tabbar / navigation bar / sidebar / browser chrome
```

---

## 1. Goals and non-goals

### 1.1 Goals

This design MUST:

1. give a product one place to define its native-shell light/dark identity;
2. make multiple lxapps inherit that identity without repeated configuration;
3. preserve platform-native semantic colors when a product does not customize
   a token;
4. keep native chrome and every attached WebView on the same effective color
   scheme;
5. support a persisted user preference without making any child lxapp an
   appearance owner;
6. make Runner light/dark simulation exercise the same host propagation path;
7. keep the semantic token set small enough to remain stable; and
8. let accessibility policy override product colors where the platform
   requires it.

### 1.2 Non-goals

This specification does not define:

- an lxapp page design-token system;
- CSS variables for page content;
- per-lxapp or per-page light/dark palettes;
- tabbar or navigation-bar configuration and mutation APIs;
- typography, spacing, corner radius, material, blur, or animation tokens;
- a cross-platform replacement for native accessibility settings; or
- arbitrary theming of system-owned controls that a platform does not expose.

---

## 2. Terminology and ownership

| Term | Definition |
|---|---|
| **appearance preference** | Persisted host choice: `system`, `light`, or `dark`. |
| **effective color scheme** | Resolved `light` or `dark` value currently applied by the host. |
| **ShellTheme** | Product-owned, host-level semantic native-chrome colors. |
| **semantic token** | A role such as accent or separator, not a component-specific paint instruction. |
| **platform semantic color** | Native adaptive fallback for a semantic role. |
| **component override** | An explicit tabbar/navbar value defined by the page-chrome specification. |
| **simulation override** | Runner-only effective appearance pin used for development and tests. |

Ownership is strict:

| Layer | Owns | MUST NOT own |
|---|---|---|
| Host configuration | `shellTheme` token values | lxapp page colors |
| Host runtime | preference, effective scheme, persistence, propagation | per-page theme state |
| Native platform adapter | platform fallbacks and accessibility adaptation | product-specific palettes outside `shellTheme` |
| Lxapp core | component declarations and runtime overrides | host appearance preference |
| View | page CSS and `prefers-color-scheme` response | native shell tokens or native chrome refresh |

There is one ShellTheme per host process/profile. It is not selected by the
currently active lxapp and does not change when users switch lxapps.

---

## 3. `lingxia.yaml` schema

`shellTheme` is an optional top-level host field:

```yaml
shellTheme:
  light:
    windowBackgroundColor: "#F4F5F7"
    surfaceBackgroundColor: "#FFFFFF"
    foregroundColor: "#111827"
    mutedForegroundColor: "#667085"
    accentColor: "#2865FF"
    separatorColor: "#E5E7EB"
    selectionBackgroundColor: "#EEF3FF"
  dark:
    windowBackgroundColor: "#17191C"
    surfaceBackgroundColor: "#23262B"
    foregroundColor: "#F3F4F6"
    mutedForegroundColor: "#9CA3AF"
    accentColor: "#5B8CFF"
    separatorColor: "#343840"
    selectionBackgroundColor: "#303641"
```

The target data model is:

```ts
type ShellThemeColorScheme = 'light' | 'dark'

interface ShellThemeStyle {
  windowBackgroundColor?: string
  surfaceBackgroundColor?: string
  foregroundColor?: string
  mutedForegroundColor?: string
  accentColor?: string
  separatorColor?: string
  selectionBackgroundColor?: string
}

interface ShellThemeConfig {
  light?: ShellThemeStyle
  dark?: ShellThemeStyle
}
```

Both scheme blocks and every token are optional. A host may customize only its
brand accent:

```yaml
shellTheme:
  light:
    accentColor: "#2865FF"
  dark:
    accentColor: "#5B8CFF"
```

For v1, configured token values MUST use opaque `#RRGGBB` syntax. The build
MUST reject invalid colors and unknown keys. Native translucency, blur, hover,
pressed, disabled, and elevation treatments are derived by the platform and
are intentionally not separately configurable.

The config is compiled into the host's generated runtime configuration. It is
not copied into `lxapp.json`, and dynamically loaded lxapps cannot replace it.

---

## 4. Semantic token contract

The public set is deliberately small:

| Token | Meaning | Typical consumers |
|---|---|---|
| `windowBackgroundColor` | Base behind host surfaces | desktop window backdrop, compact host background |
| `surfaceBackgroundColor` | Host-owned chrome surface | sidebar, tabbar, navigation bar, browser/toolbar surfaces |
| `foregroundColor` | Primary native chrome content | titles, primary icons, controls |
| `mutedForegroundColor` | Secondary or inactive content | unselected tabs, metadata, subdued icons |
| `accentColor` | Interactive/selected emphasis | selected tab, focus/selection indicator, active control |
| `separatorColor` | Structural separation | dividers, attribution lines, bar edges |
| `selectionBackgroundColor` | Selected/active container fill | selected sidebar row, active native item background |

Components MAY derive hover, pressed, disabled, focus, and high-contrast colors
from these roles and platform state. Such derived colors are not new public
tokens.

Component-specific tokens such as `tabBarBackgroundColor`,
`navigationBarBackgroundColor`, or five separate sidebar colors are rejected
at the ShellTheme layer. They multiply configuration and make every consumer a
second theme system. A component that genuinely needs an exception uses the
explicit override contract in `page-chrome-spec.md`.

### 4.1 Default role mapping

Unless a consumer specification says otherwise:

| Consumer property | ShellTheme role |
|---|---|
| Native surface background | `surfaceBackgroundColor` |
| Primary label/icon | `foregroundColor` |
| Secondary/unselected label/icon | `mutedForegroundColor` |
| Selected label/indicator | `accentColor` |
| Divider/border | `separatorColor` |
| Selected row/container | `selectionBackgroundColor` |

The host window outside those surfaces uses `windowBackgroundColor`.

### 4.2 Resolution algorithm

For a semantic role and the current effective scheme, resolution is:

1. platform accessibility override, when required;
2. configured token in `shellTheme.<effective-scheme>`;
3. the platform semantic color for that role.

The opposite scheme is never a fallback. For example, if only
`shellTheme.light.accentColor` is configured, dark mode uses the native dark
accent rather than the configured light accent.

Page-chrome component overrides sit above this algorithm and are defined in
the consumer specification:

```text
runtime component override
        > manifest component override
        > resolved ShellTheme role
        > platform semantic color
```

---

## 5. Host appearance state

The runtime state is:

```ts
type AppearancePreference = 'system' | 'light' | 'dark'
type EffectiveColorScheme = 'light' | 'dark'

interface HostAppearanceState {
  preference: AppearancePreference
  effective: EffectiveColorScheme
  revision: number
}
```

`revision` is monotonic within one host process and increases only when
`preference` or `effective` changes.

Resolution rules:

- `light` always resolves to effective `light`;
- `dark` always resolves to effective `dark`;
- `system` resolves from the current OS/application appearance;
- an OS change only changes `effective` while preference is `system`; and
- startup uses the persisted preference, or `system` when none exists.

The preference belongs to the host user/profile, not a bundle. Opening,
closing, updating, or uninstalling an lxapp MUST NOT reset it.

Public Logic APIs are specified with the page-chrome API surface because they
are added only when native consumers can update correctly. The host runtime
MUST still have an internal appearance state before those APIs are exposed.

---

## 6. Propagation and commit ordering

One appearance transaction MUST update all host-owned consumers without page
JavaScript coordinating them:

1. resolve the new `HostAppearanceState`;
2. resolve the active ShellTheme palette;
3. apply the effective native application/window appearance;
4. refresh host shell chrome;
5. refresh attached page chrome;
6. update every attached WebView's effective color scheme;
7. repaint Runner/device chrome when applicable; and
8. publish one appearance-change notification after the new state is active.

Consumers SHOULD coalesce repaint/layout work into one native frame. A child
lxapp MUST NOT briefly render its own appearance while the surrounding shell
uses another one.

Newly created windows, surfaces, page instances, and WebViews MUST initialize
from the current appearance revision before becoming interactive. Background
or suspended surfaces MUST use the latest revision when resumed.

ShellTheme colors do not cross the View bridge. The WebView receives only the
effective platform color scheme, which drives standard CSS:

```css
@media (prefers-color-scheme: dark) {
  /* page-owned design */
}
```

---

## 7. Persistence, authority, and Runner

Only the trusted host/home application authority may change the persisted
appearance preference. Child lxapps may observe it but MUST NOT persist a new
host preference.

Persistence requirements:

- write the preference atomically;
- treat absence as `system`;
- preserve the preference across host updates;
- keep storage host/profile-scoped; and
- roll back the native preference if persistence fails during a public change
  request.

Runner's simulated appearance is a development override, not persisted product
state. Its precedence is:

```text
Runner simulation override (when pinned)
    > persisted host preference
    > OS appearance for `system`
```

Setting Runner back to `system` releases only the simulation override. It does
not erase the host user's stored preference. Runner controls MUST drive the
same propagation path as a real appearance change; DOM style injection is
forbidden.

---

## 8. Accessibility and platform policy

Platform accessibility policy outranks product theme configuration.

- High-contrast or increased-contrast modes MAY replace configured colors.
- System accent policy MAY replace an accent that the platform reserves.
- Native controls MAY use platform-derived hover, pressed, disabled, focus,
  vibrancy, and material treatments.
- Implementations MUST preserve readable contrast for platform-owned glyphs
  such as status-bar content.
- An unsupported token MUST fall back semantically; it MUST NOT be ignored in
  one scheme and copied from the opposite scheme.

Cross-platform parity means the same role and ownership, not identical pixels.

---

## 9. Implementation sequence

### Phase 1 — model and validation

1. Add the shared ShellTheme config types to the host app configuration crate.
2. Parse `lingxia.yaml`, reject unknown keys/invalid colors, and emit the
   generated runtime config.
3. Add unit tests for partial themes and platform fallback behavior.

### Phase 2 — host appearance service

1. Add preference/effective/revision state.
2. Add atomic host-profile persistence.
3. Subscribe to OS appearance changes only for `system`.
4. Define one platform adapter contract for reading and applying appearance.

### Phase 3 — base shell consumers

1. Resolve ShellTheme into a shared semantic palette.
2. Apply it to window, sidebar, browser, and other base host chrome.
3. Propagate effective scheme to all WebViews.
4. Make newly attached/resumed surfaces initialize from the latest revision.

### Phase 4 — Runner parity

1. Route Runner appearance controls through the host service.
2. Keep device frame, host chrome, and WebViews synchronized.
3. Verify `system`, pinned light, and pinned dark.

Tabbar, navigation bar, and public JS APIs start only after these phases and
follow [`page-chrome-spec.md`](./page-chrome-spec.md).

---

## 10. Verification matrix

### 10.1 Configuration

- no `shellTheme` uses platform semantic defaults;
- a one-token partial theme parses and affects only that token;
- unknown scheme/token names fail the build;
- invalid or non-opaque colors fail the build;
- missing dark values never copy light values; and
- the generated runtime config round-trips without loss.

### 10.2 Appearance state

- first launch defaults to `system`;
- persisted light/dark survives restart;
- system changes update only a `system` preference;
- preference changes produce one monotonic revision;
- persistence failure rolls back; and
- child lxapps cannot mutate host preference.

### 10.3 Platform propagation

On every platform, verify:

- host window/chrome and WebView `prefers-color-scheme` agree;
- all simultaneously attached lxapps agree;
- newly opened and resumed surfaces use the current revision immediately;
- system accent fallback remains native when `accentColor` is omitted;
- accessibility settings retain authority; and
- Runner pins use the production propagation path.

---

## 11. Rejected alternatives

### 11.1 A theme in every lxapp

Rejected because a host containing many lxapps would duplicate product colors,
drift across bundles, and let the active child restyle global chrome.

### 11.2 `light` / `dark` blocks on every component

Rejected because tabbar, navbar, sidebar, and future controls would each become
an independent theme system. Components consume semantic roles instead.

### 11.3 Inject ShellTheme tokens into page CSS

Rejected because page design belongs to the lxapp and may use a completely
different token system. The standard color scheme is the stable boundary.

### 11.4 Copy a configured light value into dark mode

Rejected because it silently converts a partial theme into a fixed light
palette. Missing values remain adaptive through platform semantics.

### 11.5 Expose every native state as a token

Rejected because hover, pressed, focus, disabled, material, and accessibility
states are platform-derived. A large token surface is hard to keep coherent.

---

## 12. Acceptance criteria

The ShellTheme foundation is complete when:

- one host-owned appearance state drives every native window and WebView;
- `shellTheme` exists only in host configuration;
- all seven semantic tokens have validated platform fallbacks;
- partial light/dark configuration behaves independently per scheme;
- no lxapp manifest contains a theme or appearance palette;
- no page JavaScript is required to synchronize native chrome;
- persistence and Runner simulation use the defined precedence; and
- the configuration, state, propagation, and platform verification matrices
  pass.
