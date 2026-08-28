# LingXia Shell UI Specification: Surface Model and Platform Projections

> Status: v1.1 (target) · Platforms: macOS / Windows / iOS / Android / Harmony
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
6. **One stable root.** The first admitted main is the window's navigation
   root. Its content kind does not matter, and ordinary tab actions cannot
   close it.

---

## 1. Terminology and architecture boundaries

| Term | Definition |
|---|---|
| **surface** | A shell-managed content instance plus its relationships, state, and presentation |
| **declaration id** | The stable YAML lookup key of a host-declared surface; currently the value of its `lxapp` / `url` / `native` content field |
| **surface key** | An optional caller-owned reuse key for an additional instance of a declaration that supports multiple instances |
| **runtime id** | A read-only id the shell assigns to a live surface, used by handles and events |
| **main area** | The content region of the main window hosting the currently selected main |
| **sidebar** | The desktop left navigation region: pins, main tabs, and app-owned header/footer actions |
| **aside** | Companion content beside a main; docked when wide, overlaying when narrow |
| **slot** | A container in the aside region grouped by rendering engine: lxapp, browser, native |
| **float** | A floating surface that does not participate in main/aside layout |
| **lxapp tab** | A top-level sidebar tab representing a main lxapp |
| **web tab** | A top-level sidebar tab representing a browser-backed main surface |
| **browser workspace tab** | A provider-internal page tab inside one browser main; it is not a surface switcher item |
| **tabbar item** | A child row under an expanded lxapp tab, sourced from that lxapp's mobile tabbar |
| **pin** | A user-saved quick entry for an lxapp or website |
| **sidebar action** | An app-declared runtime callback entry owned by the single runtime writer and placed in the header or footer |
| **home lxapp** | The host's primary lxapp named by `app.homeAppId`; its identity is independent of `launch` and of whether it currently has a visible surface |

### 1.1 Content

The shell supports four provider content kinds. These are YAML/runtime model
details; JS does not select a provider by its implementation kind:

| Content | Provider identity | Instance policy | Purpose |
|---|---|---|---|
| `lxapp` | appId | Singleton per appId in one window | A complete lxapp, as main, aside, or float |
| `page` | owner appId + page name | New instance per open | The caller's own page, only as float or standalone window |
| `url` | Normalized first URL | Main may duplicate; API asides reuse by default | A main tab or aside tab in the built-in browser |
| `native` | capability name | Declaration-defined; terminal declarations may create keyed workspace instances | A host-registered provider, e.g. the terminal |

JS uses intent-level selectors instead: `surface` for a YAML declaration,
`appId` for a dynamic business lxapp, `page` for the caller's own page, and
`url` for browser content. In particular, JS has no `native` selector: native
is a provider implementation detail behind a declaration. The runtime id is
used to address the resulting live instance. Implementations MUST NOT re-wrap
the runtime id into a second open-by-id syntax.

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
- An explicit `as` may migrate an existing non-root Surface between supported
  roles. Migration preserves provider state and runtime identity; it never
  clones the Surface. The stable root main cannot migrate away from `main`.
- `page` content MUST NOT be an aside. For a companion panel, use a separate
  lxapp, a native capability, or a URL; auxiliary UI internal to an app belongs
  in the app's own layout.
- Page navigation stays per **page instance**: the same route may enter the
  navigation stack multiple times with different queries. Opening a page
  creates a new page instance by default; page names are not global singletons.
- A declaration's default instance is identified by its declaration id. An
  additional instance is identified by `(declaration id, surface key)`. Equal
  keys reuse one Surface; different keys may coexist only when that declaration
  supports multiple instances. `as` is role, not identity: reopening the same
  tuple with a different `as` migrates the same Surface. Provider-private
  workspace/session ids are not Surface identity.
- The URL duplication policy only affects browser-tab reuse; it does not change
  content identity after navigation.

### 1.4 Crate boundaries

- `lingxia-shell` is the platform-neutral semantic owner: typed sidebar-action/pin
  state, validation, versioned stores, declaration generations, stable-id
  routing, and the combined pin limit.
- `lingxia-surface` is the generic presentation graph: main identity/order,
  switcher presentation, aside, slot, focus, visibility, and layout plans. It
  knows nothing about pins, bookmarks, sidebar actions, or product behavior
  such as the terminal.
- The top-level `lingxia` crate coordinates the two domains: a sidebar action
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
- the first admitted main is a stable root and ordinary close MUST reject it;
- closing an eligible non-root main removes its switcher item and destroys its
  provider instance; closing the selected item selects an adjacent remaining
  main. A product Host therefore never enters a zero-main placeholder state.

Child surfaces inside an aside slot:

- `show()` selects that child and shows its slot;
- `hide()` on the active child selects the most recently used other child; with
  no other child, the whole slot hides;
- `close()` destroys only that child; closing the last child closes the slot.

### 2.2 Singletons, reuse, and conflicts

| Content | Default behavior |
|---|---|
| declared Surface | The unkeyed declaration is its default instance; `(declaration id, key)` reuses an additional instance when supported |
| dynamic lxapp Surface | Singleton per appId in one window; reopening the same role focuses it, while an incompatible live role conflicts |
| page | New page instance per open |
| URL main | Delegated to the browser; duplicate URLs allowed |
| URL aside | API-opened tabs reuse by normalized first URL; explicit duplication in browser UI may create new instances |

URL normalization MUST at least unify scheme/host case, default ports, and the
empty path; query and fragment participate in the key. Navigation or redirects
never rewrite the first-URL key. All platforms MUST share one normalization
implementation.

When an existing Surface instance is reused, new startup `query` / `params` do
not re-trigger the launch lifecycle and do not overwrite the original
parameters; callers SHOULD pass follow-up data via messaging. Page opens are
independent by default.

### 2.3 Errors

| Error | Meaning |
|---|---|
| `E_INVALID_ARG` | Invalid field, combination, URL, or size |
| `E_PERMISSION_DENIED` | Caller lacks permission |
| `E_NOT_FOUND` | appId, page, URL target, or declaration does not exist |
| `E_NOT_SUPPORTED` | Platform, capability, or current window mode does not support the operation |
| `E_SURFACE_CONFLICT` | The same logical content is already live under an incompatible role/presentation |
| `E_SURFACE_CLOSED` | Operation on a destroyed handle |

### 2.4 Open pipeline

All platforms MUST process an open in the same order:

1. validate caller permission, selector (`surface` / `appId` / `page` / `url`),
   URL scheme, platforms, and capability;
2. merge defaults with priority `runtime spec > YAML declaration > capability
   metadata > shell default`;
3. run singleton/reuse/conflict arbitration;
4. create or focus the instance and assign a runtime id;
5. hand off to adaptive admission to compute presentation and size;
6. return a handle, except for browser-owned projections: URL mains and compact
   URL asides return none.

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

Window size class and host form are independent inputs. Resizing a desktop
window into the compact class MUST NOT turn its navigation into a mobile
projection. Mobile and phone Runner hosts use the device-compact column because
of their host form, not merely because their width is below 600.

| Region | expanded desktop | medium desktop | compact desktop | device compact |
|---|---|---|---|---|
| sidebar | full | icon rail | icon rail | hidden |
| main | main area | main area | main area | full screen |
| aside | up to 3 visible slots | up to 1 visible slot | full-screen overlay over main | full-screen overlay |
| float | popover / overlay | popover / overlay | popover / overlay | bottom sheet / popover |
| standalone window | supported | supported | supported | rejected |

### 3.3 Sizing and admission

Default desktop size tokens:

| Token | Default |
|---|---:|
| Expanded sidebar width | 148 pt (macOS) / 184 dp (Windows) |
| Icon rail width | platform-native (clears system chrome) |
| Main minimum width | 360 dp/pt |
| Left/right aside minimum / default width | 240 / 320 dp/pt |
| Top/bottom aside minimum / default height | 180 / 280 dp/pt |

The expanded sidebar defaults to 148pt on macOS and 184dp on Windows; both keep
it user-resizable. Expanded content geometry — not the exact width or compact-rail
width — is the cross-platform parity boundary.

Arbitration order is fixed:

1. allocate the sidebar per shell size class;
2. reserve the main minimum width;
3. grant aside requested sizes in most-recently-used order, clamped between the
   minimum and 45% of the container;
4. admit at most 3 visible slots in expanded, at most 1 in medium;
5. slots that do not fit stay alive hidden; when the user explicitly opens an
   aside that does not fit, it overlays the main and hides again on return.

An adaptive aside overlay is still the current host's aside slot: it MUST cover
the main content pane exactly, remain above the selected main, and MUST NOT
claim the main-switcher slot or create another shell window.

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

- **The main switcher projects main surfaces only.** A full desktop shell renders
  it as sidebar tabs; compact/custom shells may use another projection or none.
  The switcher is shell chrome outside the main content rectangle: a main
  lxapp MUST NOT gain a content-area tab strip.
  Asides never enter the
  sidebar: opening an aside lxapp MUST NOT append a sidebar entry — its
  switching belongs to its slot's header tabs (§4.6), structurally identical to
  the browser aside's title tabs. A sidebar list of "open asides" is abolished
  behavior.
- While the main window has tabs, exactly one tab MUST be selected.
- lxapp, browser, and native mains interleave in stable graph order; pins and
  sidebar actions stay fixed, outside the main navigation scroll region.
- The first admitted main is the stable root regardless of content kind. It has
  no close action. Root replacement is a host configuration/lifecycle operation,
  not a user tab action.
- Each content provider supplies its automatic title, icon, capabilities, and
  content-specific menu section. The shell applies user title overrides and
  appends common lifecycle actions. Platform code only renders the resolved
  snapshot and returns revisioned intents.
- Browser and terminal providers may expose close and rename for non-root mains.
  Lxapp presentation lifecycle remains lxapp-owned, so its context menu contains
  lxapp MoreActions rather than generic close/rename.
- Closing the selected eligible main selects an adjacent remaining main. Close
  Others/After operate in global switcher order but skip root and any provider
  item that does not support close.
- Browser workspace tabs are internal to one browser Surface. New-tab,
  navigation, and closing an internal page tab never create, rename, or remove a
  top-level switcher item. Direct URL Runner window lifecycle remains a Runner
  policy, outside this product-host root contract.
- `SurfaceId` is the only switcher identity. An lxapp appId, terminal session id,
  URL, or browser workspace tab id is provider metadata and MUST NOT be used as
  a substitute for the Surface id.
- A collapsed desktop rail preserves the same switcher identities and order; it
  never substitutes the active lxapp's mobile bottom tabbar. Every icon-only
  switcher and footer action exposes its label as tooltip/accessibility text.
- Hovering the current switcher replaces its icon with a **24 dp/pt** rounded
  surface containing a close `x` when and only when that item is closable.
  Clicking this overlay closes the item. Inactive and non-closable switcher
  icons stay intact; their primary click continues to select the item.

#### Uniform spacing

- Top-level lxapp/web tab rows use a **36 dp/pt** height baseline; the net gap
  between any adjacent pair is a uniform **4 dp/pt**.
- Type transitions add no extra margins, blank bands, or separators; the
  4 dp/pt is measured on the visible background or hover hit-area outline and
  MUST NOT double-count row margins.
- The icon rail keeps the same 4 dp/pt vertical rhythm.
- When both Pins and live switchers exist, the collapsed rail centers a
  **22 dp/pt** low-contrast divider in their existing gap. It adds no empty row
  and disappears when either section is empty.
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
- `lx.tabBar.update({ visibility: 'hidden' })` hides the expanded region and
  disables the chevron; `visibility: 'auto'` clears the API-hidden state and
  expands. The user chevron only changes `userCollapsed` while API-visible; it
  MUST NOT override the API-hidden state.
- **Only explicit API calls map to collapse/expand.** The mobile implicit
  behavior "navigating to a non-tab page auto-hides the tabbar" does not
  propagate to desktop: the sidebar is a persistent navigation region, so
  drilling into a detail page keeps the group expanded and merely clears item
  selection (see two-level selection below) — otherwise every navigation would
  bounce the group and lose the waypoint.
- Desktop MUST fully support `lx.tabBar.update()` item, badge, red-dot,
  visibility, and style patches. While collapsed, badges/red dots aggregate
  onto the parent lxapp tab.
- **Mapping of tabbar style keys onto the sidebar** (one-to-one with mobile
  semantics; unset keys inherit the resolved Page Chrome theme):

  | tabbar style | Mobile | Desktop sidebar |
  |---|---|---|
  | `foregroundColor` | Unselected item text | Unselected item title color |
  | `selectedForegroundColor` | Selected item text | Selected item title color + left-edge accent bar |
  | `backgroundColor` | Bar background | Expanded group (items container) background |
  | `dividerColor` | Bar divider | Attribution line base color |

  Colors apply to text and structural elements alike; an item's single
  `iconPath` is a template the host tints, exactly as on mobile.
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
  follows `dividerColor`; the selected item shows a left-edge accent bar
  colored by `selectedForegroundColor`; selected item text/icon colors are
  same-sourced. Only `foregroundColor` and `selectedForegroundColor` are
  runtime-mutable via `lx.tabBar.update()`; background and divider remain
  manifest-owned. The shell injects no accent of its own and inherits the Page
  Chrome theme when fields are unset.

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
- An lxapp Pin is a **workspace launch intent**, not a declared-Surface
  shortcut. Clicking it opens or selects that lxapp as a main workspace and it
  MUST appear as its own row in the sidebar main switcher. The Pin tile remains
  a launch shortcut; the workspace row is a separate, durable control entry for
  selecting and closing the lxapp. On pointer hover, the row MUST expose an
  explicit ellipsis that opens the same provider-backed context menu available
  from right-click; lifecycle controls must not be discoverable only by a hidden
  gesture. Removing the Pin MUST NOT remove a live workspace row. If the same lxapp is live as an aside,
  the host closes that one-region presentation and reopens it as main; the Pin
  does not inherit the app's declared aside default. Sidebar actions and
  `lx.surface.openDeclared(id)` continue to honor the declared role.
- A main opened from a Pin MUST occupy the exact same host content rectangle as
  the stable root main. No edge or pixel of the previously active main may
  remain exposed behind it. A page's native navigation bar may reserve space
  inside that rectangle, but the previous main WebView MUST be hidden and no
  duplicate workspace window may remain visible. The Pin rule restricts entry
  role only; it MUST NOT introduce a Pin-specific inset, clip, card, navigation
  offset, content-area tab strip, or alternate content rectangle. Workspace
  identity and controls belong to the sidebar, not inside the main content.
- A website Pin opens or selects a main browser tab.

### 4.5 Sidebar actions

A sidebar action is an app-declared runtime shell entry in the header or
footer. The shell invokes Logic and owns no built-in target behavior. It is not
the whole sidebar, and it does not require a YAML surface.

Declaration model (owned by the single runtime writer, §7.2):

- Every entry carries an explicit **stable id**, `placement: header | footer`,
  `label`, local lxapp-accessible `icon`, and `onActivate` callback. Stable ids share
  one namespace across placements and route updates and activation; callbacks
  decide what activation does.
- `icon` accepts bundled resources and runtime-managed `lx://temp`,
  `lx://usercache`, or `lx://userdata` files, including icons downloaded before
  registration. Shell core resolves them through the app's standard sandboxed
  resource resolver before native projection. Native absolute paths, network
  URLs, and parent traversal are rejected. The portable asset profile is a
  square, transparent, monochrome SVG or PNG designed for a 16-point visual.
  Hosts may tint it to match shell theme and disabled state.
- Hosts do not infer target metadata, open lxapps, toggle providers, or render
  fallback glyphs. A callback explicitly opens a declared Surface with
  `lx.shell.openDeclared(id, options?)`, a dynamic business app with
  `lx.shell.openApp(appId, { as, ... })`, or performs app navigation with
  `lx.navigateToApp({ appId: ... })`.
- The declaration is a **full-generation atomic replace**: the shell validates
  the complete generation before touching handlers or chrome — a
  bad item leaves the previous generation intact. Single-item patches may
  update label/icon/disabled state. Removing or clearing entries are atomic
  transformations of the same generation, not separate mutation protocols.
- Callback registration is generation-scoped: replacing or removing an item
  unregisters its previous callback.
- Sidebar actions are runtime-scoped because callbacks are not serializable. The
  home Logic writer redeclares them on every launch; the shell does not restore
  stale entries before their callbacks exist.
- There are no app-controlled layout knobs: no weight, no arbitrary colors.
  Row allocation, hover, and disabled styling are shell-owned. Density is a
  future shell-level user preference, not an app configuration.

Activation behavior:

- Invoke the currently registered callback. Mouse, keyboard, accessibility,
  shortcut, and automation activation are one semantic; each activation
  invokes the callback once.
- A disabled sidebar action stays visible but cannot activate.
- Sidebar actions are never selected/active. Any content state created by the
  callback belongs to that content's own UI and APIs.

Header geometry:

- Header accepts at most **2** icon-only actions and preserves declaration
  order. If the complete set cannot fit beside native window controls, none of
  the actions render; partial truncation is forbidden.
- Header actions do not render in the collapsed desktop rail or compact size
  class. Their `label` is still the tooltip and accessibility text.

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
- Background is transparent; hover uses a quiet shell-owned wash (radius 6);
  disabled items mute icon/text with no hover wash.

Compact rail:

- Footer actions become icon-only, with label as tooltip/accessibility text,
  the same bounded scrolling and disabled treatment. The rail reserves the
  expand control; actions
  MUST NOT overlap it or run off-window. Rail width MAY stay platform-specific
  for system-chrome clearance.
- In device-compact projections sidebar actions do not render, but declarations
  still validate and reappear if the same process returns to a desktop form.

### 4.6 Aside slots

The aside region is fixed at three slots, grouped by rendering engine:

| Slot | Content | Multi-content behavior |
|---|---|---|
| lxapp | Different appIds | Header tabs; one instance per appId |
| browser | URL tabs | Title tabs; API URLs reuse by first URL |
| native | Different declaration instances | Header tabs; the default instance plus keyed instances when supported |

- Slot tab switching performs hide/show and preserves content state; only an
  explicit close destroys the current content.
- A terminal Surface is one workspace and may contain multiple provider-owned
  PTY tabs. Multiple keyed terminal Surfaces are separate workspaces and appear
  as separate slot children or main switcher entries; switching Surfaces never
  stops their PTYs.
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
  open API, sidebar action entries, the sidebar browser "+"; page-level actions live
  in the tab's context menu.
- Maximizing an aside covers only the main area and toggles back on the next
  click; it does not change the role.
- Browser "open in main browser" promotes and closes only the **current URL
  tab**; other browser-aside tabs stay. Closing the last tab closes the slot.
- A desktop browser slot offers history navigation, refresh, title tabs, and
  slot dismissal. It MUST NOT offer address editing or user-created tabs; a
  skin MAY expose the current URL as read-only when space permits.

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
  inherits neither the main window's framelessness nor sidebar/aside/action
  chrome.

---

## 5. Compact projections

A compact desktop window remains a desktop shell: its icon rail stays visible,
the main remains a distinct workspace, and an lxapp tabbar remains projected
into the rail rather than returning to the bottom. A browser main likewise
retains its top address toolbar. At narrow widths the address field flexes and
secondary actions MAY collapse into overflow, but desktop browser chrome MUST
NOT move to the bottom or paint inside the sidebar rail.

The following rules apply to device-compact hosts (mobile and phone Runner):

- Main is full screen; the active lxapp's tabbar returns to the bottom.
- A device-compact browser main uses the same provider chrome on Windows and macOS:
  an editable address row above an action row with page Back/Forward, Reload,
  user New Tab, browser-workspace tab switcher/count, and Dismiss when an
  lxapp main can be restored. The desktop top address bar and sidebar MUST NOT
  remain visible behind or beside this projection.
- Asides overlay the main full screen. System Back and edge-swipe Back hide the
  **entire active slot** and restore the main; slot tabs are not destroyed.
- Lxapp and native slots MAY use a compact header Back to perform that slot
  dismissal. Browser asides MUST NOT add the generic header Back: browser
  chrome already owns navigation and dismissal.
- A compact browser aside uses one bottom action row: page Back, page Forward,
  Refresh, aside-tab switcher/count, and Dismiss. It has no address row,
  user-new-tab action, overflow menu, or top-left shell Back.
- The explicit page Back/Forward buttons navigate session history. System Back,
  edge-swipe Back, and Dismiss exit the entire browser-aside slot even when page
  history exists, preserving its tabs for the next show.
- A browser-only Runner uses the same two-row main projection but has no
  Dismiss action because there is no covered lxapp main. The field accepts
  URLs, not search queries. Main/Runner and aside tab counts,
  switchers, activation, and close-successor selection MUST remain isolated;
  neither group may surface a tab from the other group.
- Closing a browser tab happens in its current group's switcher. Closing the
  last tab hides that browser group; hiding or dismissing the group does not
  close its tabs.
- For non-browser slots, header close closes only the current slot tab; closing
  the last tab closes the slot.
- Floats present as bottom sheets in compact; platforms with a native popover
  semantic MAY use popovers.
- Standalone windows are rejected with `E_NOT_SUPPORTED`.
- Sidebar, pins, and sidebar actions do not render; apps needing compact quick
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

At most one declaration per declaration id. Declarations provide build-time
availability, provider selection, and runtime defaults; they are not a
registration gate for dynamic business apps, pages, or URLs. Native providers
are declaration-only and cannot be addressed directly by capability name from
JS. `lingxia build` compiles declarations into the internal `ui.json`;
generated files are never hand-written.

### 6.2 Valid combinations

| Content | YAML roles | Runtime presentations |
|---|---|---|
| lxapp | main / aside / float | main / aside / float |
| page | float | float / standalone window |
| URL | main / aside | declared main ships on macOS; macOS aside is runtime-opened, Windows retains declared aside |
| native | main / aside | macOS main: terminal/browser; terminal aside: macOS/Windows |

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
- a URL surface requires the browser capability; a native surface is the closed
  set `terminal | browser` and requires its matching capability;
- a host declares exactly one main — or is a main-less, tray-float-only app;
  additional switcher mains are runtime workspace Surfaces;
- `controls: content` satisfies the single-main constraint of §4.7.

YAML has **no `sidebar:` entry field**. App-owned sidebar entries come only
from the runtime `lx.shell.sidebarActions` collection.

---

## 7. Runtime semantics

Signatures live in the generated declarations; this section fixes the
semantics every language surface MUST share.

### 7.1 Opening surfaces

- The selector is the method, not a field: `openDeclared` takes a declaration
  id, `openApp` an appId, `openPage` a page name, `openUrl` a URL. Provider
  kinds such as YAML `lxapp` and `native` are not JS selectors. Composition —
  choosing `as`, or opening another lxapp — lives on `lx.shell`; an lxapp's own
  presentations live on `lx.surface`.
- `lx.shell.openDeclared(id, { key?, as? })` opens a YAML declaration. Without
  `key` it addresses the declaration's default instance. A non-empty `key`
  selects or creates an additional instance only when that declaration admits
  multiple instances; currently this is supported by instantiable native
  providers such as terminal. The returned handle binds the resolved runtime
  `SurfaceId`, never the caller key.
- `as` is orthogonal to identity. Omitting it uses the declaration role;
  supplying it focuses or migrates the same `(surface, key?)` instance to that
  supported role. A different `as` never creates a second instance. The stable
  root main rejects any request to migrate away from main.
- Terminal follows the generic declaration rule. For example,
  `{ surface: 'terminal', as: 'main' }` moves/focuses the default terminal in
  the main switcher, while `{ surface: 'terminal', key: 'project-a', as:
  'aside' }` opens/reuses a distinct workspace in the native aside slot. The
  same keyed workspace may later migrate to main without losing PTYs, cwd, or
  running processes.
- `lx.shell.openApp(appId, { as, page?, query?, envVersion?, targetVersion?,
  edge? })` creates or focuses a dynamic business-app Surface and
  does not require a YAML declaration. `as` is required because the caller is
  creating shell composition rather than using declaration defaults; it is
  `main` or `aside`. A float lxapp must be host-declared and opened with
  `{ surface }` so its tray anchor, dismissal policy, and presentation contract
  exist. `page` is the configured page name; full routes are not JS API input.
  `query`, `envVersion`, and `targetVersion` are optional startup inputs, and
  `envVersion` defaults to `release`. `edge` is valid only with `as: 'aside'`:
  `aside` chooses the companion region, while `edge` is its preferred docking
  side on layouts with room. Omit it for the default; compact hosts may
  reproject the same aside.
- A dynamic App Surface is singleton by appId within the window. Reopening the
  same appId under its current role focuses it without restarting its lifecycle
  or replacing startup parameters. A live main/aside role change currently
  fails with `E_SURFACE_CONFLICT`; close it before reopening in the other role.
  It returns a lifecycle handle and, when main, owns an independent switcher item.
- `lx.surface.openPage(page, options?)` opens the caller's own page as a float
  or standalone window. `lx.surface.openUrl(url, options?)` opens browser
  content; a URL without `as` becomes a main browser tab.
- Runtime floats default to centered, non-modal, tap-outside dismissal;
  compact ignores position and presents a bottom sheet. A float without a size
  hint uses 480×360 dp/pt clamped to 90% of the container; a standalone window
  defaults to 960×640 dp/pt clamped to the work area.
- Size values are hints, clamped per §3.3. Non-finite, negative, or malformed
  values fail with `E_INVALID_ARG`; insufficient container space degrades per
  admission rules instead of erroring.
- An explicit `as` changes only the live Surface instance. It never mutates the
  YAML declaration or the default role used by later opens.
- `interaction.closeButton` adds the standard native circular close control.
  Manual floats require it or an app-owned close path. Modal floats block
  underlying input and restore prior focus on close.
- Allowed URL schemes are `https:` and host-authorized `file:`. The only public
  product URLs are exact `lingxia://settings` and `lingxia://downloads`, both
  restricted to the home lxapp and `capabilities.browser`; every other
  `lingxia:` value fails with `E_INVALID_ARG`. Handing a URL to the system still
  passes the host scheme allowlist.

#### 7.1.1 App navigation versus App Surfaces

`navigateToApp` and `openSurface` solve different navigation levels:

| Operation | Changes | Main switcher item | Back behavior | Result |
|---|---|---|---|---|
| `lx.navigateToApp({ appId, ... })` | Pushes an app onto the current lxapp Surface's app-navigation stack | Reuses the current item | `lx.navigateBackApp()` pops to the previous app | `Promise<void>` |
| `lx.shell.openApp(appId, { as: 'main', ... })` | Creates/focuses a dynamic app-owned Surface | Own independent item | Closing destroys that Surface and its app stack | `AppSurface` |
| `lx.shell.openDeclared(id, { as: 'main' })` where the declaration contains an lxapp | Creates/focuses that host-declared Surface | Own declaration-backed item | Closing destroys the live Surface; reopening uses declaration defaults | `DeclaredSurface` |

Therefore a declared lxapp Surface opened as main is still different from
`navigateToApp`: the former is a parallel shell workspace with its own runtime
id, switcher lifecycle, role, and handle; the latter is sequential navigation
inside the already-selected Surface and never creates or moves a shell item.
The active app at the top of a navigation stack may change, but the owning
Surface identity and role do not.

App navigation accepts the same optional startup selectors as a dynamic App
Surface: `page`, `query`, `envVersion`, and `targetVersion`; `page` is the
configured page name, full routes are rejected, and `envVersion` defaults to `release`. If the
target appId is already owned by another live Surface, navigation fails with
`E_SURFACE_CONFLICT` rather than stealing or cloning that instance.

### 7.2 Handles, messaging, and context

- An open returns a handle bound to the runtime id, carrying role,
  presentation, visibility, and liveness, with show/hide/close and lifecycle
  events per §2.1. URL mains return no handle. A medium/expanded URL aside uses
  the generic surface graph and returns a tab-scoped visibility handle; its
  compact projection is owned by browser chrome and returns no handle.
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
  namespace); guest calls fail with `E_PERMISSION_DENIED`, and host Rust never writes the
  same state in parallel.
- On a native-only host there is no home Logic; host Rust uses the semantically
  equivalent `lingxia::shell()` facade.
- Both facades MUST share the state machines and errors of §2 and this
  section. The writer declares sidebar actions (§4.5) and, under
  `frameless + controls: content`, drives window controls
  (minimize/maximize/close/state); in other modes window-control calls fail
  with `E_NOT_SUPPORTED`.
- In compact shells writer declarations still validate for the current runtime
  but do not render.
- Process/app-level capabilities (update, exit, badge, autostart, screenshot)
  stay on `lx.app`; they never migrate into `lx.shell`.

### 7.4 Launch screen (splash)

`splash:` in `lingxia.yaml` (`background`, optional `image`, `mark`,
`minDuration`) drives the launch screen; hosts build none of it by hand. Two
OS-composed beats: the launch frame — `background` with a small centered
image — then the app's first frame, which is the cover (`image`, aspect-fill
over the same `background`), held until the home page first renders. One
shared `background` makes the frame read as the cover's entrance.

- **The cover rides the first app frame, never the launch frame** — launch
  frames render full-bleed bitmaps soft (Harmony) or hide app-drawn content
  until first frame (Android 12+). The SDK renders it fully opaque; the
  frame's own exit is the one transition onto it. Nothing heavier than
  building that frame may run before it: runtime initialization happens
  under the cover, never in front of it.
- **The launch frame carries only `background` plus a small centered image**
  — the one composition it renders sharp and on time. Harmony
  (`startWindowIcon`) and iOS (`UILaunchScreen`) center `mark`, unscaled;
  Android 12+ blanks its icon slot so the mandatory splash beat is a plain
  brand-color frame. `background` is the only required field.
- **Without `image`, hold instead**: Android suspends the first draw so the
  system splash persists until home is ready; Harmony and iOS render a first
  frame reproducing the launch frame exactly.
- Dismissal: the once-per-process home-first-render signal, held by the core
  until `minDuration` (default 600 ms). A 6 s timeout MUST dismiss
  regardless and MUST NOT be configurable. While up, the cover swallows
  input.
- **Runtime hook.** A host MAY implement `HostAddon::select_splash` to
  substitute this launch's cover file. Selection is synchronous, on-disk
  only, and budgeted — the bundled cover wins on overrun; acquisition goes
  through `lingxia::spawn` and MUST NOT block it. The cover store sits
  under app data, exempt from OS and framework eviction; `splash::store`
  writes it atomically and `splash::retain` bounds it. The hook cannot
  choose the background — that is baked into the launch frame at build
  time.
- Generated resource names are the CLI↔SDK contract: Android
  `lingxia_splash_background` / `lingxia_splash_image` /
  `Theme.LingXia.Splash`, Apple `LingXiaSplashBackground` / `LingXiaSplash`
  / `LingXiaSplashMark`, Harmony `$color:lingxia_splash_background` /
  `$media:lingxia_splash` / `$media:lingxia_splash_mark`. Missing resources
  disable the runtime half; a host without `splash:` is unchanged.
- On Apple the runtime half MUST NOT depend on the compiled asset catalog
  (`actool` can fail): the cover and mark also ship as plain bundle
  resources and the color as an Info.plist key. The catalog entries remain
  for `UILaunchScreen`, which has no non-catalog equivalent.
- Desktop shells take no overlay; the ready signal is a no-op there.

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
| Sidebar actions | Not persisted; the home Logic writer redeclares callbacks each launch |
| Aside geometry | Each slot's edge and size |

- Within one process, sidebar actions distinguish an explicit empty declaration from
  "no writer yet" so `replace([])` can clear chrome. No action
  metadata crosses a process restart without its callback.
- Main sessions restore lazily: tab placeholders appear immediately; a live
  surface and runtime id are created on first selection. Failed restores show a
  retry/close placeholder. Session entry ids and runtime ids MUST NOT be
  conflated.
- Asides restore **geometry only, never content**. After restart the runtime
  writer decides whether to reopen; the shell does not resurrect side-effectful
  companion content on its own.

---

## Appendix A: Current state and gaps

As of 2026-08 (PR #202 follow-up design):

| Area | Status |
|---|---|
| Declaration-first JS open specs | Landed in generated types and runtime: `{ surface }`, `{ appId }`, `{ page }`, and `{ url }`; legacy `{ lxapp }` and `{ native }` selectors are rejected |
| Aside slot model, unified slot tab chrome | Landed and live-verified (dual-tab lxapp slot, shared tab metrics, strip visible at n=1, no "+"/"···") |
| Sidebar actions + pins | `lx.shell.sidebarActions` drives header/footer snapshots on Windows/macOS; accessibility activation is not yet automated |
| Sidebar action footer overflow scrolling (5-row cap) | Landed on Windows/macOS |
| Sidebar/tabbar parity | 148 (macOS) / 184 (Windows) width, 36/4 and 30/2/1 rhythm, two-level selection, style mapping landed on both platforms |
| Main surface switcher | Shared ordered/root/capability snapshot plus macOS and Windows projections landed; Windows public-API automation live-verifies switching, keyed reuse, role migration, root protection, and close cleanup |
| `lx.tabBar.update({ visibility })` ↔ group collapse | Landed |
| Shell persistence | Window frame, sidebar mode/width, group collapse, aside geometry, and pins landed; main-session lazy restore and the aside geometry-only policy still to be verified against §8 |
| `E_SURFACE_CONFLICT` | Error path exists; ownership conflicts such as navigating to an appId already hosted by another live Surface still need full enforcement |
| Admission | Arbitration module exists; the 45% clamp / slot-cap / overlay-fallback behavior of §3.3 not yet verified end to end |
| Compact projection | Desktop rail and top browser chrome retention landed on Windows/macOS; browser aside/self chrome, group isolation, and browser-owned back/close semantics aligned with §5 on mobile and Runner |
| Frameless window + `controls:` + writer window controls | Not implemented |
| Declared page floats; native floats | Parsed but rejected by the CLI pending runtime support |
| Launch screen (splash, §7.4) | CLI generation and Android/iOS/Harmony runtime halves implemented; iOS device-verified, Android/Harmony pending; campaign download channel and Windows not implemented |
| Naming migration (Appendix C ledger) | Pending — `DockedBrowser` and `open_panel_lxapp` word roots still present |

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
| YAML provider content | `lxapp / page / url / native` | not exposed as provider selectors | `ContentKey` enum | render |
| open selector | `surface / app_id / page / url` | `surface / appId / page / url` | typed open-spec variants | native, lxapp |
| surface key | `key` | `key` | `SurfaceKey` | instanceKey, workspaceId |
| runtime id | `surface_id` | `handle.id` | `SurfaceId` | — |
| app navigation | `navigate_to_app / navigate_back_app` | `navigateToApp / navigateBackApp` | same roots | navigateToLxApp, openApp |
| session entry id | `session_entry` | `sessionEntryId` | `SessionEntryId` | never conflated with runtime id |
| role | `role` | `SurfaceRole` | `SurfaceRole` | mode, kind |
| presentation | `presentation` | `SurfacePresentation` | `Presentation` | form, style |
| sidebar | `sidebar` | — | `Sidebar*` | **tabbar** (reserved for the lxapp tabbar), rail (icon-rail state only) |
| main area / main tab | `main` | `MainTab` | `MainArea`, `MainTab` | primary, home tab |
| lxapp tab / web tab | `lxapp_tab / web_tab` | `LxappTab`, `WebTab` | same roots | auxiliary item |
| aside / slot | `aside / slot` | `AsideSlot` | `AsideSlot`, `SlotKind` | panel, dock (dock is a presentation value only) |
| sidebar action | `sidebar_action` | `lx.shell.sidebarActions`, `ShellSidebarAction`, `ResolvedShellSidebarAction` | `SidebarAction*` | activator, launcher |
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
| `WindowsPanelPosition` | `AsideEdge` |
| `open_panel_lxapp` / `panels_config_json` / `panel_item_for_id` | `open_aside_lxapp` / `activator_config_json` / `activator_item_for_id` |
| `DockedBrowser` | `BrowserSlot` (the presentation value stays `dock`) |
