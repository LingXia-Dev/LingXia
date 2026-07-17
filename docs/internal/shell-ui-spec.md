# LingXia Shell UI Specification: Surface Model and Platform Projections

> Status: v1.0 (final) · Platforms: macOS / Windows / iOS / Android / Harmony
>
> Scope: the normative UI and runtime contract for the LingXia host shell. This
> document supersedes the former drafts `shell-ui-spec.md` (v0.19) and
> `shell-sidebar-activators-plan.md`; where they disagreed, the shipped
> implementation decided. The public, app-author-facing story lives in the docs
> skill (*Surfaces (adaptive UI)*); this document is the internal contract
> behind it.

This specification uses MUST / MUST NOT / SHOULD / MAY. Except for content
explicitly marked in Appendix A (current state and gaps), the body describes
the target state, not necessarily the current implementation.

JS and Rust API signatures are intentionally **out of scope**: the generated
declarations in `@lingxia/types` are the authoritative JS surface, and the
`lingxia` crate facade is the authoritative Rust surface. This document defines
only the semantics — identity, state machines, arbitration, errors, and layout
tokens — that both surfaces MUST share.

---

## 0. Scope and goals

This specification covers:

- the surface declaration model in `lingxia.yaml`;
- surface identity, relationships, lifecycle, and permissions;
- shell size classes and surface content size classes;
- the desktop sidebar, main area, asides, floats, tray, and window controls;
- deterministic degradation in compact form;
- the runtime open/context/shell-writer semantics;
- shell session persistence and cross-platform consistency.

It does not define the visual design of lxapp pages, and it does not allow an
app to declare two separate shell UIs under `macos:` / `android:` blocks.
Platforms MAY keep system-control differences, but MUST NOT change the states,
relationships, or lifecycle semantics defined here.

### 0.1 Core principles

1. **One model, many projections.** Mobile is not a separate UI; it is the same
   surface graph projected into a narrow container.
2. **Content and relationship are separate.** Content decides *what* is shown,
   role decides *how it relates* to the main content, and the shell derives the
   presentation.
3. **Single owner.** Shell chrome has exactly one writer; one appId has at most
   one live lxapp instance per main window.
4. **Hide preserves, close destroys.** `hide` keeps state; only `close`
   destroys. All platforms follow the same rule.
5. **User assets belong to the user.** Pins and user sessions cannot be
   silently rewritten by an app.

---

## 1. Terminology and architecture boundaries

| Term | Definition |
|---|---|
| **surface** | A shell-managed content instance plus its relationships, state, and presentation |
| **content key** | The logical key that declares and looks up content; not the runtime instance id |
| **runtime id** | A read-only id the shell assigns to a live surface, used by handles and events |
| **main area** | The content region of the main window hosting the currently selected main |
| **sidebar** | The desktop left navigation region: pins, main tabs, activators |
| **aside** | Companion content beside a main; docked when wide, overlaying when narrow |
| **slot** | A container in the aside region grouped by rendering engine: lxapp, browser, native |
| **float** | A floating surface that does not participate in main/aside layout |
| **lxapp tab** | A top-level sidebar tab representing a main lxapp |
| **web tab** | A top-level sidebar tab representing a main browser tab |
| **tabbar item** | A child row under an expanded lxapp tab, sourced from that lxapp's mobile tabbar |
| **pin** | A user-saved quick entry for an lxapp or website |
| **activator** | An app-declared persistent shell entry owned by the single runtime writer |
| **home lxapp** | The host's primary lxapp named by `app.homeAppId`; its identity is independent of `launch` and of whether it currently has a visible surface |

### 1.1 Content

The shell supports four content kinds:

| Content | Declaration/lookup key | Instance policy | Purpose |
|---|---|---|---|
| `lxapp` | appId | Singleton per appId | A complete lxapp, as main, aside, or float |
| `page` | owner appId + page name | Multi-instance by default; `instanceKey` reuses | The caller's own page, only as float or standalone window |
| `url` | Normalized first URL | Main may duplicate; API asides reuse by default | A main tab or aside tab in the built-in browser |
| `native` | capability name | Singleton per capability | A host-registered native capability, e.g. the terminal |

The content key is used for declaration lookup and reuse policy; the runtime id
is used to address the actual instance. Implementations MUST NOT re-wrap the
runtime id into a second open-by-id syntax.

### 1.2 Role and presentation

There are exactly three roles:

| Role | Semantics | Desktop presentation | Compact presentation |
|---|---|---|---|
| `main` | A switchable first-class destination | Sidebar tab + main area | Full screen |
| `aside` | Companion content beside the current main | Docked slot or temporary overlay | Full-screen overlay above main |
| `float` | Short-lived content outside the layout | Tray popover or overlay | Bottom sheet / popover |

`window`, `panel`, `sheet`, and `sidebar` are presentations, not roles. A
standalone window is created only by opening a page as a window; it never
enters the main window's surface graph.

### 1.3 Invariants

- **One appId has at most one live lxapp instance per main window**, in exactly
  one of `main`, `aside`, `float`. It MUST NOT appear in main and aside at the
  same time.
- The lxapp instance here means the shell presentation. On an AppService host
  the home Logic is a host-scoped writer: it starts with the host and lives
  until process exit; closing the home surface destroys only its
  presentation/View, not the home Logic.
- Opening an already-open lxapp under a different role MUST fail with
  `E_SURFACE_CONFLICT`; the caller closes first, then reopens. The shell never
  silently clones or moves an instance.
- `page` content MUST NOT be an aside. For a companion panel, use a separate
  lxapp, a native capability, or a URL; auxiliary UI internal to an app belongs
  in the app's own layout.
- Page navigation stays per **page instance**: the same route may enter the
  navigation stack multiple times with different queries. Opening a page
  creates a new page instance by default; page names are not global singletons.
- One native capability is a singleton in the shell; different native
  capabilities may coexist in the native slot.
- The URL duplication policy only affects browser-tab reuse; it does not change
  content identity after navigation.

### 1.4 Crate boundaries

- `lingxia-shell` is the platform-neutral semantic owner: typed activator/pin
  state, validation, versioned stores, declaration generations, stable-id
  routing, and the combined pin limit.
- `lingxia-surface` is the generic presentation graph: main, aside, slot,
  focus, visibility, and layout plans. It knows nothing about pins, bookmarks,
  activators, or product behavior such as the terminal.
- The top-level `lingxia` crate coordinates the two domains: a shell activation
  intent is projected into the surface graph or a native host capability.
- Logic only parses the JS declaration and owns generation-scoped callbacks.
  Platform SDKs only render resolved snapshots and report stable ids; they MUST
  NOT reinterpret target semantics.

---

## 2. Surface state and arbitration

### 2.1 Lifecycle

Surface states:

```text
created -> visible <-> hidden -> closed
```

- `show()`: display an existing instance; idempotent when already visible.
- `hide()`: hide but keep the View, Logic, scroll, form, and session state;
  idempotent when already hidden.
- `close()`: destroy the instance and release its runtime id; repeated close is
  idempotent.
- After `closed`, `show()`, `hide()`, or messaging MUST fail with
  `E_SURFACE_CLOSED`.
- A hidden surface may receive messages, but MUST NOT keep occupying visible
  layout space.

Main is special:

- `show()` on a main is equivalent to selecting its tab;
- when the user switches tabs, the shell MAY turn the previous main hidden; but
  a main handle does not support explicit `hide()` — such a call MUST fail with
  `E_NOT_SUPPORTED`, guaranteeing there is always a selected main;
- `close()` removes the tab and destroys the instance; closing the selected tab
  selects an adjacent tab; closing the last tab closes the main window.

Child surfaces inside an aside slot:

- `show()` selects that child and shows its slot;
- `hide()` on the active child selects the most recently used other child; with
  no other child, the whole slot hides;
- `close()` destroys only that child; closing the last child closes the slot.

### 2.2 Singletons, reuse, and conflicts

| Content | Default behavior |
|---|---|
| lxapp | Singleton per appId; reopening under the same role focuses, a different role conflicts |
| page | New page instance per open; the same `instanceKey` reuses that instance |
| URL main | Delegated to the browser; duplicate URLs allowed |
| URL aside | API-opened tabs reuse by normalized first URL; explicit duplication in browser UI may create new instances |
| native | Singleton per capability name; same role focuses, different role conflicts |

URL normalization MUST at least unify scheme/host case, default ports, and the
empty path; query and fragment participate in the key. Navigation or redirects
never rewrite the first-URL key. All platforms MUST share one normalization
implementation.

When an existing instance is reused, new `query` / `params` do not re-trigger
the launch lifecycle and do not overwrite the original parameters; callers
SHOULD pass follow-up data via messaging. For an independent page data context,
omit `instanceKey` or use a new key.

### 2.3 Errors

| Error | Meaning |
|---|---|
| `E_INVALID_ARG` | Invalid field, combination, URL, or size |
| `E_DENIED` | Caller lacks permission |
| `E_NOT_FOUND` | lxapp, page, native capability, or declaration does not exist |
| `E_NOT_SUPPORTED` | Platform, capability, or current window mode does not support the operation |
| `E_SURFACE_CONFLICT` | The same logical content is already live under an incompatible role/presentation |
| `E_SURFACE_CLOSED` | Operation on a destroyed handle |

### 2.4 Open pipeline

All platforms MUST process an open in the same order:

1. validate caller permission, content key, URL scheme, platforms, and
   capability;
2. merge defaults with priority `runtime spec > YAML declaration > capability
   metadata > shell default`;
3. run singleton/reuse/conflict arbitration;
4. create or focus the instance and assign a runtime id;
5. hand off to adaptive admission to compute presentation and size;
6. return a handle (URL mains return none — the browser UI owns those tabs).

Platform skins consume this result only; they MUST NOT alter permission,
reuse, or lifecycle semantics.

---

## 3. Adaptive layout

### 3.1 Two size-class scopes

Shell and content use the same width breakpoints, but with different scopes:

| Size class | Available width |
|---|---|
| `compact` | `< 600 dp/pt` |
| `medium` | `600–840 dp/pt` |
| `expanded` | `> 840 dp/pt` |

- **Shell size class**: computed from the full client-area width of the main
  window; drives sidebar and aside arbitration.
- **Content size class**: computed per surface from its actual viewport width;
  exposed to content via the surface-context subscription.
- The two MUST NOT be conflated; a narrow aside inside an expanded shell can
  legitimately receive a compact content size class.
- Breakpoints use 24 dp/pt hysteresis: upgrading requires crossing
  `boundary + 24`, downgrading requires dropping below `boundary - 24`.

### 3.2 Degradation matrix

| Region | expanded | medium | compact |
|---|---|---|---|
| sidebar | full | icon rail | hidden |
| main | main area | main area | full screen |
| aside | up to 3 visible slots | up to 1 visible slot | full-screen overlay |
| float | popover / overlay | popover / overlay | bottom sheet / popover |
| standalone window | supported | supported | rejected |

### 3.3 Sizing and admission

Default desktop size tokens:

| Token | Default |
|---|---:|
| Expanded sidebar width | 184 dp/pt |
| Icon rail width | platform-native (clears system chrome) |
| Main minimum width | 360 dp/pt |
| Left/right aside minimum / default width | 240 / 320 dp/pt |
| Top/bottom aside minimum / default height | 180 / 280 dp/pt |

The expanded sidebar defaults to 184 on both desktop platforms; macOS MAY keep
it user-resizable. Expanded content geometry — not compact-rail width — is the
cross-platform parity boundary.

Arbitration order is fixed:

1. allocate the sidebar per shell size class;
2. reserve the main minimum width;
3. grant aside requested sizes in most-recently-used order, clamped between the
   minimum and 45% of the container;
4. admit at most 3 visible slots in expanded, at most 1 in medium;
5. slots that do not fit stay alive hidden; when the user explicitly opens an
   aside that does not fit, it overlays the main and hides again on return.

Size classes are an admission ceiling, not a guarantee that three panels are
crammed in the moment the window crosses 840.

---

## 4. Desktop shell

### 4.1 Layout

```text
┌─────────┬──────────────────────────┬─────────┐
│ sidebar │        main area         │  aside  │
│         │                          │         │
│  pins   │      lxapp/browser       │  slots  │
│  tabs   │                          │         │
│         ├──────────────────────────┤         │
│ activ.  │       bottom aside       │         │
└─────────┴──────────────────────────┴─────────┘
```

- Left/right asides are full height; top/bottom asides span only the main
  width and never cross the left/right slots.
- With multiple slots on one edge, the later-opened one sits further outside;
  no drag-to-reorder or drag-to-another-edge.
- Main and asides share the content layer, above the sidebar base layer;
  content-layer regions MUST have clear boundaries.

### 4.2 Main tabs

- **Sidebar tabs represent main surfaces only.** Asides never enter the
  sidebar: opening an aside lxapp MUST NOT append a sidebar entry — its
  switching belongs to its slot's header tabs (§4.6), structurally identical to
  the browser aside's title tabs. A sidebar list of "open asides" is abolished
  behavior.
- While the main window has tabs, exactly one tab MUST be selected.
- lxapp tabs and web tabs interleave in one scrollable list; pins and
  activators stay fixed, outside the scroll region.
- Closing the selected tab selects an adjacent tab; closing the last tab closes
  the main window. Whether the process continues is decided by the tray and the
  platform app lifecycle.
- Web tabs come from browser new-tab/navigation, URL opens, pins, and browser
  aside promotion.

#### Uniform spacing

- Top-level lxapp/web tab rows use a **36 dp/pt** height baseline; the net gap
  between any adjacent pair is a uniform **4 dp/pt**.
- Type transitions add no extra margins, blank bands, or separators; the
  4 dp/pt is measured on the visible background or hover hit-area outline and
  MUST NOT double-count row margins.
- The icon rail keeps the same 4 dp/pt vertical rhythm.
- Larger text or accessibility font sizes grow row height only, never the gap.

### 4.3 lxapp tabs and the tabbar

- An expanded lxapp tab shows that app's tabbar items; configuration, selected
  state, badges, red dots, icons, and colors are same-sourced with the mobile
  tabbar.
- Net gap from the group header to the first item: **2 dp/pt**; between items:
  **1 dp/pt**; item row height baseline: **30 dp/pt**.
- From the last item back to the next top-level tab the gap returns to
  **4 dp/pt**. Children are tighter to express attribution but MUST NOT shrink
  into mis-tap territory.
- `hideTabBar()` hides the expanded region and disables the chevron;
  `showTabBar()` clears the API-hidden state and expands. The user chevron only
  changes `userCollapsed` while API-visible; it MUST NOT override the
  API-hidden state.
- **Only explicit API calls map to collapse/expand.** The mobile implicit
  behavior "navigating to a non-tab page auto-hides the tabbar" does not
  propagate to desktop: the sidebar is a persistent navigation region, so
  drilling into a detail page keeps the group expanded and merely clears item
  selection (see two-level selection below) — otherwise every navigation would
  bounce the group and lose the waypoint.
- Desktop MUST fully support `setTabBarBadge`, `removeTabBarBadge`,
  `showTabBarRedDot`, `hideTabBarRedDot`, `setTabBarItem`, `setTabBarStyle`.
  While collapsed, badges/red dots aggregate onto the parent lxapp tab.
- **Mapping of the four style keys onto the sidebar** (one-to-one with mobile
  semantics; unset keys fall back to a neutral theme):

  | tabbar style | Mobile | Desktop sidebar |
  |---|---|---|
  | `color` | Unselected item text | Unselected item title color |
  | `selectedColor` | Selected item text | Selected item title color + left-edge accent bar |
  | `backgroundColor` | Bar background | Expanded group (items container) background |
  | `borderStyle` | Bar border | Attribution line base color |

  Colors apply to text and structural elements only; icons switch via
  `iconPath`/`selectedIconPath` pairs exactly as on mobile, with no tinting.
- macOS and Windows MUST match in structure, spacing, selection, and
  separation; only system-control differences are allowed.

lxapp and web tabs are distinguished by identity cues only: lxapp tabs have a
persistent chevron and a rounded app tile, web tabs use a slightly smaller bare
favicon. Extra background grouping that breaks the uniform rhythm is not
allowed.

#### Selection semantics (two independent levels)

- **lxapp tab selection**: while the main shows that lxapp — whatever internal
  page or tabbar item it is on — the lxapp tab (group header) MUST stay
  highlighted, so a collapsed group still tells the user which app they are in.
- **tabbar item selection**: after `switchTab` enters a tab page, that item
  highlights; when the current page is **not** a tabbar page (e.g. a plain
  `navigateTo` page), no item may be selected — only the lxapp tab highlights.
- The two levels are independent and simultaneously visible, with clearly
  distinct styling (group-header highlight ≠ item highlight).
- The expanded items region carries a vertical attribution line on its left,
  visually binding children to the group header; the thin-line treatment is the
  baseline for both platforms.
- **Styling adapts to the tabbar config**: the attribution line's base color
  follows `borderStyle` (black/white); the selected item shows a left-edge
  accent bar colored by `selectedColor`; selected item text/icon colors are
  same-sourced. All of it is runtime-mutable via `setTabBarStyle`; the shell
  injects no accent of its own and uses neutral system colors when unset.

### 4.4 Pins

Pins are the user's quick entries for lxapps and websites.

- Pins sit above the tab list in a **fixed 4-column × 2-row grid**: at most
  **8 pins**, counting lxapp and web pins together.
- Tile size **36 × 36**, gap **5** both axes (grid width `4·36 + 3·5 = 159`),
  centered in the sidebar content area. Incomplete rows stay aligned to the
  first slot — tiles never redistribute when a pin is added. The grid never
  scrolls: eight is the high-frequency set; bookmarks and ordinary navigation
  hold the long tail.
- Pins are user-owned shell state. There is no pin/unpin app API, JS or Rust.
- Users pin/unpin through the context menu (right-click or a keyboard-
  equivalent entry); every mutation path — native page menu, address bar,
  context menu, bookmark manager, lxapp pin menu — MUST go through the shared
  shell operation, which enforces the limit in shared Rust code (not per
  platform, not at render time).
- Exceeding the limit returns the typed `LimitReached { max: 8 }` result;
  platform chrome shows a localized message instead of silently logging or
  truncating. Stored and visible state MUST agree — a successfully stored pin
  is never render-truncated.
- One ordered, mixed pin list is persisted so user order survives across lxapp
  and web targets; renderers MUST NOT force lxapps before websites.
- Clicking an unopened pin opens a main tab; if already main, it selects it. If
  the lxapp is live as aside/float, the click focuses the existing instance —
  changing role requires close-then-reopen.

### 4.5 Activators

An activator is an app-declared persistent shell entry at the bottom of the
sidebar: it either activates dynamic content or invokes Logic. It is not the
whole sidebar, and it is not a shortcut that requires a YAML surface.

Declaration model (owned by the single runtime writer, §7.2):

- Three target kinds: an **lxapp** (by appId), a **native capability** (by
  name, e.g. `terminal`), or an **action** (a Logic callback).
- Every entry carries an explicit **stable id** used for updates, activation
  routing, and persistence; target values are never overloaded as keys.
- Every entry declares its own icon (resolved against the home lxapp bundle);
  hosts do not infer target metadata icons or render fallback glyphs.
- The declaration is a **full-generation atomic replace**: the shell validates
  the complete generation before touching handlers, persistence, or chrome — a
  bad item leaves the previous generation intact. Single-item patches may
  update label/icon/disabled state. Removing or clearing entries are atomic
  transformations of the same generation, not separate mutation protocols.
- Action callback registration is generation-scoped: replacing or removing an
  item unregisters its previous callback.
- There are no app-controlled layout knobs: no weight, no arbitrary colors.
  Row allocation, hover, active, and disabled styling are shell-owned. Density
  is a future shell-level user preference, not an app configuration.
- An lxapp or native activator does not require a matching YAML `surfaces:`
  entry. An lxapp target only needs to be resolvable (bundled, installed, or
  runtime-provided); a native target requires the corresponding host
  capability (e.g. `capabilities.terminal: true`). Actions have no YAML
  dependency.

Activation behavior:

- **lxapp target**: already main → focus; already aside → toggle visibility;
  not open → resolve and present as an adaptive aside.
- **native target**: verify the capability, then toggle it under its host-owned
  default presentation (terminal: bottom aside).
- **action target**: invoke the currently registered callback; never shown as
  selected/active. Mouse, keyboard, accessibility, shortcut, and automation
  activation are one semantic; each activation invokes the callback once.
- A disabled activator stays visible but cannot activate.
- `active` is derived by the shell from the presentation graph for lxapp/native
  targets; it is never app-written for them.

Expanded footer geometry:

- Outer horizontal inset aligns with top-level sidebar rows: **8**.
- Cell height **30**; cell and row gap **4**; minimum cell width **72**.
- Entries flow left-to-right in declaration order, wrapping only when the next
  cell cannot get its minimum width — two short labels share a row rather than
  stacking as two full-width rows.
- At most **5** visible rows; overflow scrolls inside the footer rather than
  squeezing the tab list.
- Titles are single-line, tail-truncated, with the full label as tooltip and
  accessibility text. Each platform measures text with native font metrics —
  no ASCII/wide-character width heuristics. Row breaks MAY differ where native
  fonts genuinely differ; padding, minimums, state treatment, overflow, and
  order MUST NOT.
- Inactive background is transparent; hover uses a quiet shell-owned wash
  (radius 6); active lxapp/native items use a light selected background plus an
  accent marker; disabled items mute icon/text with no hover wash.

Compact rail:

- Icon-only, label as tooltip/accessibility text, same bounded scrolling, same
  active/disabled treatment. The rail reserves the expand control; activators
  MUST NOT overlap it or run off-window. Rail width MAY stay platform-specific
  for system-chrome clearance.
- In compact shells activators do not render at all, but declarations still
  validate and persist, and reappear in wider forms.

### 4.6 Aside slots

The aside region is fixed at three slots, grouped by rendering engine:

| Slot | Content | Multi-content behavior |
|---|---|---|
| lxapp | Different appIds | Header tabs; one instance per appId |
| browser | URL tabs | Title tabs; API URLs reuse by first URL |
| native | Different capabilities | Header tabs; one instance per capability |

- Slot tab switching performs hide/show and preserves content state; only an
  explicit close destroys the current content.
- Multi-session behavior of a native capability is capability-internal state —
  the terminal manages its own sessions; the shell only switches capabilities.
- Header tabs order by open time, no drag reorder; under pressure they drop
  text before icons, then become a scrollable strip — tabs never shrink into
  unrecognizable slivers.
- **A docked slot always shows its tab strip, including at one content.** The
  strip is the slot's management surface (switching and closing live there);
  hiding it at n=1 removes the close affordance and causes a region jump when
  the second content arrives. The three slots are mechanically identical, only
  their content kinds differ.
- **Slot tab visuals are one component**: every slot's header tabs use the same
  tab component and metrics (Chrome-style title tab: flared outline, bottom
  aligned, hover wash, adjacent separators), with icon + title + close button.
  lxapp tabs show the lxapp icon and name, browser tabs the favicon and page
  title. Slots MUST NOT each paint their own style.
- **The slot tab strip carries no create/menu entries** (no "+", no "···"):
  the strip only switches and closes. Content enters a slot elsewhere — the
  open API, activator entries, the sidebar browser "+"; page-level actions live
  in the tab's context menu.
- Maximizing an aside covers only the main area and toggles back on the next
  click; it does not change the role.
- Browser "open in main browser" promotes and closes only the **current URL
  tab**; other browser-aside tabs stay. Closing the last tab closes the slot.

### 4.7 Window chrome

- The top system region belongs to the shell. A browser main may host the
  address bar; an lxapp navbar always lives inside the main content.
- `window: { frameless: true }` affects the main window only. The default
  `controls: shell` gives a persistent shell control strip independent of the
  current main.
- `controls: content` lets the home H5 draw buttons and drag regions; the build
  then requires the only main to be the home lxapp, and the runtime refuses to
  create browser/guest mains.
- `controls: content` uses `app-region: drag/no-drag`; `controls: shell` ships
  its own drag region.
- A page opened as a standalone window uses the platform-standard frame — it
  inherits neither the main window's framelessness nor sidebar/aside/activator
  chrome.

---

## 5. Compact projection

- Main is full screen; the active lxapp's tabbar returns to the bottom.
- Asides overlay the main full screen. System Back, edge-swipe back, and header
  Back hide the **entire active slot** and restore the main; slot tabs are not
  destroyed.
- Header close closes only the current slot tab; closing the last tab closes
  the slot.
- Floats present as bottom sheets in compact; platforms with a native popover
  semantic MAY use popovers.
- Standalone windows are rejected with `E_NOT_SUPPORTED`.
- Sidebar, pins, and activators do not render; apps needing compact quick
  entries provide them in their own UI.

---

## 6. `lingxia.yaml`

### 6.1 Declarations

A surface entry starts with its content key — there is no `id + render` pair:

```yaml
surfaces:
  - lxapp: home
    role: main
    launch: true

  - lxapp: assistant
    role: aside
    edge: right
    size:
      width: 320

  - native: terminal
    role: aside
    edge: bottom
    platforms: [macos, windows]

  - lxapp: quick-panel
    role: float
    tray:
      icon: icons/tray.svg
```

At most one declaration per content key. Declarations provide build-time
availability and runtime defaults; they are not a registration gate for
dynamic content. `lingxia build` compiles them into the internal `ui.json`;
generated files are never hand-written.

### 6.2 Valid combinations

| Content | YAML roles | Runtime presentations |
|---|---|---|
| lxapp | main / aside / float | main / aside / float |
| page | float | float / standalone window |
| URL | main / aside | main browser tab / browser aside |
| native | aside / float | aside / float |

The build MUST validate:

- `edge` only on asides; the aside default edge is right, and native capability
  metadata may override the default;
- `launch` only on a main, with at most one main `launch: true`;
- a float declaration requires `tray:`; runtime-only floats need no tray;
- at most one tray surface per host;
- page declarations belong to the home lxapp; guest pages never enter the host
  YAML;
- a declared page uses the implicit stable instance key `declared:<page-name>`,
  so tray/activator reopens address the same live instance;
- a `platforms` filter excluding the current platform removes the surface and
  all of its entries together;
- a URL surface requires the browser capability; a native surface requires its
  capability;
- a host has at least one main — or is a main-less, tray-float-only app;
- `controls: content` satisfies the single-main constraint of §4.7.

YAML has **no `sidebar:` entry field**. Persistent app entries are exclusively
the activator's job — one entry system, not a declarative one beside an
imperative one.

---

## 7. Runtime semantics

Signatures live in the generated declarations; this section fixes the
semantics every language surface MUST share.

### 7.1 Opening surfaces

- An open spec is keyed by exactly one content key (`lxapp` / `page` / `url` /
  `native`), with an optional role override (`as`) and presentation hints
  (edge, position, size, modality, dismissal).
- Defaults: an lxapp without `as` takes its YAML role, else main. A URL without
  `as` becomes a main browser tab. A native capability without `as` takes its
  YAML or capability-metadata default, else a bottom aside. The aside default
  edge is right.
- Runtime floats default to centered, non-modal, tap-outside dismissal;
  compact ignores position and presents a bottom sheet. A float without a size
  hint uses 480×360 dp/pt clamped to 90% of the container; a standalone window
  defaults to 960×640 dp/pt clamped to the work area.
- Size values are hints, clamped per §3.3. Non-finite, negative, or malformed
  values fail with `E_INVALID_ARG`; insufficient container space degrades per
  admission rules instead of erroring.
- An explicit role override never mutates the declaration; conflicting with a
  live role fails with `E_SURFACE_CONFLICT`.
- Floats with manual dismissal MUST ship their own close affordance; compact
  keeps system Back as the safety exit. Modal floats block underlying input
  and restore prior focus on close.
- Allowed URL schemes are `https:` and host-authorized `file:`; anything else
  fails with `E_INVALID_ARG`. Handing a URL to the system still passes the host
  scheme allowlist.

### 7.2 Handles, messaging, and context

- An open returns a handle bound to the runtime id, carrying role,
  presentation, visibility, and liveness, with show/hide/close and lifecycle
  events per §2.1. URL mains return no handle — the browser UI owns those
  tabs; URL asides return a visibility-only handle.
- lxapp and page surfaces support instance-bound messaging: messages address a
  runtime id, never broadcast by appId or page name; replies return to the
  opener's handle. Native surfaces support messaging only when the capability
  declares it.
- Content can subscribe to its surface context (content size class plus
  viewport dimensions); the subscription fires once immediately with the
  current value, then only on actual change.

### 7.3 Shell writer

Shell chrome always has exactly one writer:

- On an AppService host, the home Logic is the writer (via the `lx.shell`
  namespace); guest calls fail with `E_DENIED`, and host Rust never writes the
  same state in parallel.
- On a native-only host there is no home Logic; host Rust uses the semantically
  equivalent `lingxia::shell()` facade.
- Both facades MUST share the state machines and errors of §2 and this
  section. The writer declares activators (§4.5) and, under
  `frameless + controls: content`, drives window controls
  (minimize/maximize/close/state); in other modes window-control calls fail
  with `E_NOT_SUPPORTED`.
- In compact shells writer declarations still validate and persist but do not
  render.
- Process/app-level capabilities (update, exit, badge, autostart, screenshot)
  stay on `lx.app`; they never migrate into `lx.shell`.

---

## 8. Persistence

Desktop shell persistence:

| Data | Rule |
|---|---|
| Main window | Size and position; clamped to the current available screen on restore |
| Sidebar | Width and full/rail/hidden state |
| Pins | The user's ordered mixed list |
| Main session | Tab content keys, session entry ids, order, and selection |
| lxapp tabs | User collapse state; API-hidden state is rebuilt by the app |
| Activators | Serializable lxapp/native items; action items are rebuilt by the writer |
| Aside geometry | Each slot's edge and size |

- The activator store is versioned and distinguishes an explicit empty
  declaration from "no writer yet". Persisted lxapp/native items render before
  Logic boots; action items never restore before the writer redeclares them
  (their callbacks are process-local). An explicitly declared empty generation
  stays empty across restarts. Both desktop platforms restore the same
  generation.
- Main sessions restore lazily: tab placeholders appear immediately; a live
  surface and runtime id are created on first selection. Failed restores show a
  retry/close placeholder. Session entry ids and runtime ids MUST NOT be
  conflated.
- Asides restore **geometry only, never content**. After restart the runtime
  writer decides whether to reopen; the shell does not resurrect side-effectful
  companion content on its own.

---

## Appendix A: Current state and gaps

As of 2026-07 (post `feat/shell-ui-spec`, PR #126):

| Area | Status |
|---|---|
| Content-key YAML + open specs | Landed. A legacy declared-surface open spec (`{ surface: <name> }`) still ships in the generated types; target is pure content keys with no alias |
| Aside slot model, unified slot tab chrome | Landed and live-verified (dual-tab lxapp slot, shared tab metrics, strip visible at n=1, no "+"/"···") |
| Activators + pins | Landed per §4.4/§4.5; Windows live-verified, macOS compiled via CI; accessibility activation not yet automated |
| Activator footer overflow scrolling (5-row cap) | Specified, not yet implemented |
| Sidebar/tabbar parity | 184 width, 36/4 and 30/2/1 rhythm, two-level selection, style mapping landed on both platforms |
| `hideTabBar`/`showTabBar` ↔ group collapse | Landed |
| Shell persistence | Window frame, sidebar mode/width, group collapse, aside geometry, pins, activator store landed; main-session lazy restore and the aside geometry-only policy still to be verified against §8 |
| `E_SURFACE_CONFLICT` | Error path exists in the logic layer; full runtime enforcement across role conflicts pending |
| Admission | Arbitration module exists; the 45% clamp / slot-cap / overlay-fallback behavior of §3.3 not yet verified end to end |
| Compact projection | Slot-based back/close semantics of §5 pending regression on mobile |
| Frameless window + `controls:` + writer window controls | Not implemented |
| Declared page floats; native floats | Parsed but rejected by the CLI pending runtime support |
| Naming migration (Appendix C ledger) | Pending — `DockedBrowser`, `panel_activator`, `open_panel_lxapp` word roots still present |

## Appendix B: Pending visual decisions

- Final visuals for the aside header, tab strip, and resize handle.
- Corner radius, shadow, and separation tokens for the main/aside content
  layer.

## Appendix C: Naming bindings (normative)

Implementation identifiers MUST use this specification's word roots; across
languages only the case style changes (Rust `snake_case`, TS `camelCase`, YAML
lowercase keys). Synonyms are not allowed.

| Spec term | Root | TS / JS | Rust | Banned synonyms |
|---|---|---|---|---|
| surface / handle | `surface` | `SurfaceHandle` | `Surface`, `SurfaceHandle` | view, panel |
| content key | `lxapp / page / url / native` | spec field names verbatim | `ContentKey` enum, same variants | id, render |
| runtime id | `surface_id` | `handle.id` | `SurfaceId` | — |
| session entry id | `session_entry` | `sessionEntryId` | `SessionEntryId` | never conflated with runtime id |
| role | `role` | `SurfaceRole` | `SurfaceRole` | mode, kind |
| presentation | `presentation` | `SurfacePresentation` | `Presentation` | form, style |
| sidebar | `sidebar` | — | `Sidebar*` | **tabbar** (reserved for the lxapp tabbar), rail (icon-rail state only) |
| main area / main tab | `main` | `MainTab` | `MainArea`, `MainTab` | primary, home tab |
| lxapp tab / web tab | `lxapp_tab / web_tab` | `LxappTab`, `WebTab` | same roots | auxiliary item |
| aside / slot | `aside / slot` | `AsideSlot` | `AsideSlot`, `SlotKind` | panel, dock (dock is a presentation value only) |
| activator | `activator` | `lx.shell.activators`, `ShellActivator`, `ResolvedShellActivator` | `Activator*` | panel activator, launcher |
| pin | `pin` | — | `Pin*`, `MAX_SHELL_PINS` | favorite, shortcut |
| size class | `size_class` | `sizeClass` | `SizeClass` | breakpoint (internal boundary values may use it) |
| admission | `admission` | — | `admission` module/functions | aliases other than arbitrate |
| writer | `writer` | — | `ShellWriter` | owner, master |
| error codes | `E_*` | verbatim | mapped to the same `E_*` wire strings | per-platform error names |

**Pending rename ledger** (migrate as code is touched; no compatibility
aliases):

| Current name | Rename to |
|---|---|
| `WindowsShellTabBarLayout` / `...TabBarItemLayout` | `SidebarLayout` / `SidebarTabLayout` (the tabbar root is reserved for the lxapp tabbar expansion) |
| `WindowsShellAuxiliaryItemLayout` | `WebTabLayout` |
| `panel_activator.rs` / `WindowsPanelPosition` | `activator.rs` / `AsideEdge` |
| `open_panel_lxapp` / `panels_config_json` / `panel_item_for_id` | `open_aside_lxapp` / `activator_config_json` / `activator_item_for_id` |
| `DockedBrowser` | `BrowserSlot` (the presentation value stays `dock`) |
