# LingXia Shell UI Specification

The surface model of the host shell and how each kind of host projects it.
Read before changing `crates/lingxia-surface` (the surface graph, arbitration
and the presentation plan), `crates/lingxia-shell` (pins, sidebar actions,
surface menus), the lxapp presentation code in
`crates/lingxia-lxapp/src/lxapp/surface.rs`, the `lx.surface` / `lx.shell`
bindings in `crates/lingxia-logic/src/surface.rs`, or a platform shell. What a
page reads about its own canvas (size class, viewport, insets, hinge) is in
`view-environment-spec.md`; the launch screen is in `launch-screen.md`.

Signatures are not repeated here: `@lingxia/types` is the JS surface and the
`lingxia` crate the Rust one. This document fixes the semantics both share.
MUST, MUST NOT, SHOULD and MAY are normative. Each rule is stated once, for
every host; §10 records which hosts implement it and how they differ.

---

## 1. Model

### 1.1 Principles

1. **One model, many projections.** A phone, a tablet and a desktop show one
   surface graph. Hosts differ only in how they project it.
2. **Content, role and presentation are separate.** Content is what is shown,
   role is how it relates to the main content, and the shell derives the
   presentation from both and from the room it has.
3. **One writer.** Shell chrome has exactly one writer (§12.5). One appId has
   at most one live lxapp instance per main window.
4. **Hide preserves, close destroys**, on every host.
5. **User assets belong to the user.** An app cannot rewrite pins or the
   user's sessions.
6. **One stable root.** The first admitted main is the window's navigation
   root, whatever its content; ordinary tab actions cannot close or move it.

Platforms MAY differ in system controls. They MUST NOT differ in states,
relationships or lifecycle. An app cannot declare a separate shell per
platform.

### 1.2 Terms

| Term | Meaning |
|---|---|
| **surface** | A shell-managed content instance with its role, state and presentation |
| **declaration id** | The stable key of a surface declared in `lingxia.yaml`: the value of its `lxapp` / `url` / `native` field |
| **surface key** | A caller-owned key that selects an additional instance of a declaration that admits several |
| **runtime id** | The id the shell assigns to a live surface (`SurfaceId`). Only the shell and automation see it |
| **main** | A switchable first-class destination; the selected one fills the main area |
| **main switcher** | The shell's list of mains. Desktop shows it as sidebar rows; touch hosts as an app rail |
| **aside** | Companion content beside the selected main: docked when there is room, over the main when there is not |
| **slot** | One container in the aside region. There are two: the side panel and the native slot (§5.1) |
| **side panel** | The slot that holds lxapp and URL asides as tabs |
| **float** | Short-lived content above the layout: a popover, an overlay or a bottom sheet |
| **window** | A page opened as a separate desktop window. It never enters the main window's surface graph |
| **sidebar** | The desktop's left column: pins, the main switcher, sidebar actions. Expanded, or collapsed to an icon rail |
| **app rail** | A touch host's narrow column that switches between mains |
| **lxapp tab bar** | The tabs an lxapp declares in `lxapp.json` `tabBar` |
| **touch tab bar** | The lxapp tab bar on a touch host in `regular`: a floating tab bar or a tab column (§8.3) |
| **status band** | The system status bar painted as one band in the sidebar colour (§8.4) |
| **pin** | A user-saved entry for an lxapp or a website (desktop only) |
| **sidebar action** | An entry the writer declares in the sidebar header or footer; activating it runs a callback |
| **home lxapp** | The lxapp `app.homeAppId` names. Its Logic is the Control app: the shell writer and the only caller of the composition APIs |
| **host form** | `desktop` or `mobile`: what machine this is, read by `isDesktop()` / `isMobile()`. A Runner device frame is `mobile`; a Runner desktop preset is `desktop` |
| **touch host** | A mobile host: a phone, a foldable or a tablet, real or in a Runner device frame |
| **smallest width** | A display's short side in dp/pt. It does not change when the device turns |
| **size class** | `compact` or `regular`, from available width (§4) |
| **hinge** | The fold or crease of a foldable that crosses the window, with its orientation and posture (`flat`, `half-opened`) |

### 1.3 Content

Four provider kinds back a surface. They are `lingxia.yaml` and runtime
details; JS does not pick a provider by kind.

| Content | Identity | Instances | Purpose |
|---|---|---|---|
| `lxapp` | appId | One per appId in a window | A whole lxapp as main, aside or float |
| `page` | owner appId + page name | A new one per open | The caller's own page, as a float or a window only |
| `url` | Normalised first URL | Mains may repeat; asides opened by the API reuse by first URL | A browser tab in the main area or the side panel |
| `native` | Capability name | Set by the declaration; the terminal admits keyed instances | A host provider, such as the terminal or the browser |

JS opens by subject: a declaration (`openDeclared`), another lxapp
(`openApp`), its own page (`openPage`), a URL (`openUrl`), or a host
builtin page (`lx.shell.openBuiltin`). There is no `native` selector: a
native provider is reached only through its declaration. Every handle
carries a `kind` (`page`, `declared`, `app`, `tab`, `builtin`) and the
`realized` placement (`main`, `aside`, `float`, `window`).

### 1.4 Roles and presentations

There are three roles:

| Role | Meaning | Desktop | Touch host in `regular` | `compact` |
|---|---|---|---|---|
| `main` | A first-class destination | Main area, listed in the sidebar | Main area, listed in the app rail | Full screen |
| `aside` | Companion to the selected main | Docked slot, or over the main | Docked side panel, or over the main | Full screen over the main |
| `float` | Outside the layout | Popover or overlay | Popover or overlay | Bottom sheet or popover |

`window`, `sheet`, `panel` and `sidebar` are presentations, not roles.

### 1.5 Invariants

- One appId has at most one live lxapp instance per main window, in exactly
  one of `main`, `aside` and `float`. It MUST NOT be main and aside at once.
- That instance is the shell presentation. The home Logic is host-scoped: it
  starts with the host and lives until the process exits. Closing the home
  surface destroys only its presentation and View.
- Runtime ids stay inside the shell. Logic addresses a live surface by
  identity — appId, declaration id and key, or its own page or URL key — and
  gets back the same handle object for the surface's whole life.
- A declaration's default instance is its declaration id; an additional one is
  `(declaration id, surface key)`. Equal keys reuse one surface. Different keys
  coexist only where the declaration admits several instances. Provider
  workspace or session ids are not surface identity.
- `as` is role, not identity. Reopening the same identity with another `as`
  moves that surface (§3); it never clones it.
- `page` content MUST NOT be an aside. A companion panel is a separate lxapp,
  a native capability or a URL.
- The host owns the window's orientation. A page that declares none follows
  the orientation the host app declares (on Android, its activity's), so
  presenting such a page never rotates the device.
- Page navigation is per page instance: a route may be on the stack more than
  once with different params, and opening a page creates a new instance.
- The URL duplication policy affects browser-tab reuse only. Navigation and
  redirects never change a URL surface's identity.

### 1.6 Crate boundaries

- `lingxia-shell` owns user and writer state: sidebar actions and their
  generations, pins and the combined pin limit, surface menu composition,
  versioned stores and stable-id routing.
- `lingxia-surface` owns the presentation graph: main identity and order, the
  switcher snapshot, asides and slots, focus, visibility, size class, aside
  admission and the presentation plan. It knows nothing of pins, bookmarks,
  sidebar actions or providers such as the terminal.
- The `lingxia` crate joins the two: a sidebar action or a menu intent becomes
  a graph change or a host capability call.
- Logic parses JS declarations and owns generation-scoped callbacks.
- Platform shells render resolved plans and snapshots and report stable ids.
  They MUST NOT reinterpret targets, reuse or lifecycle.

---

## 2. Lifecycle, reuse and errors

### 2.1 Lifecycle

```text
created -> visible <-> hidden -> closed
```

- `show()` displays the instance; it is idempotent.
- `hide()` hides it and keeps the View, Logic, scroll, form and session state;
  it is idempotent. A hidden surface takes no layout space but may receive
  messages.
- `close()` destroys the instance and releases its runtime id; repeating it is
  idempotent. After `closed`, `show()`, `hide()` and messaging fail with
  `E_SURFACE_CLOSED`.

Mains:

- `show()` selects the main.
- The shell hides the previous main when the user switches. A main handle
  cannot be hidden explicitly: `hide()` fails with `E_SURFACE_ROLE_LOCKED`, so
  a main is always selected.
- `close()` on the stable root fails with `E_SURFACE_ROLE_LOCKED`.
- Closing another main removes its switcher item and destroys its provider
  instance; closing the selected one selects an adjacent main. A product host
  never has zero mains.
- The window close button never closes the root. On macOS it hides the window
  and the Dock icon restores it. On Windows it hides the window when a tray
  item exists and quits otherwise.

Children of a slot:

- `show()` selects the child and shows its slot.
- `hide()` on the active child selects the most recently used other child; with
  none, the slot hides.
- `close()` destroys only that child. Closing the last child closes the slot.

### 2.2 Reuse

| Content | Reopening |
|---|---|
| Declared surface | The unkeyed declaration is one instance; `(declaration id, key)` reuses a keyed one where the declaration admits keys |
| lxapp | One per appId in the window. Reopening focuses it (§3.2 for a reopen naming another role) |
| page | Always a new instance |
| URL main | Handed to the browser; duplicates allowed |
| URL aside | An API open reuses the tab with the same normalised first URL; the browser UI may still duplicate |

URL normalisation unifies scheme and host case, default ports and the empty
path. Query and fragment are part of the key. One implementation serves every
platform.

A reused instance keeps its launch parameters. A new `page` or `params` on a
reused lxapp reaches its Logic as App `onShow` (`reason: 'switch_back'`, with
`path` and `query`); its page stack is kept and its launch lifecycle does not
run again. Other follow-up data goes by messaging.

### 2.3 Open pipeline

Every host processes an open in this order:

1. check the caller, the selector, the URL scheme and domain, the platforms
   filter and the capability;
2. merge defaults: runtime options, then the `lingxia.yaml` declaration, then
   capability metadata, then shell defaults;
3. arbitrate reuse, role moves and conflicts (§2.2, §3);
4. create or focus the instance and assign its runtime id;
5. admit it to the layout (§4.3), which decides its presentation and size;
6. return the handle.

Platform shells consume the result. They MUST NOT change permission, reuse or
lifecycle.

### 2.4 Errors

`code` alone tells a caller what to do; `reason` is present where one code
covers several causes. Names are those of `@lingxia/types`.

| Code | `reason` | When |
|---|---|---|
| `E_INVALID_ARG` | — | A malformed option, an unknown `as`, `edge` without an aside, a bad size, a URL that is not `https:` or an allowed `file:`, a reserved `lingxia:` URL, a malformed sidebar-action generation (§12.6) |
| `E_INVALID_ARG` | `tabAlreadyOpen`, `stackFull` | `lx.nav.go` to a tab already on the stack, or onto a stack of ten pages |
| `E_PERMISSION_DENIED` | — | A composition call from an lxapp other than the Control app; a URL whose domain the lxapp's security policy does not allow |
| `E_PERMISSION_DENIED` | `appUnderMaintenance`, `appSuspended` | The operator has taken the target lxapp down |
| `E_NOT_FOUND` | — | No such declaration, page, tab or sidebar action |
| `E_NOT_SUPPORTED` | `placementUnsupported` | The host cannot realize the placement: a window on a host without windows, or moving a live lxapp on a host that cannot (§10) |
| `E_CAPABILITY_MISSING` | — | A `key` on a declaration whose provider has no keyed instances |
| `E_SURFACE_CONFLICT` | `appOpenInOtherRole` | The lxapp is live in the other role and cannot move there: it is on another channel |
| `E_SURFACE_CONFLICT` | — | The lxapp is live only inside another surface's app navigation; `lx.nav.go({ app })` to an lxapp docked as an aside |
| `E_SURFACE_ROLE_LOCKED` | — | Hiding a main; closing the stable root; moving the root out of main |
| `E_SURFACE_CLOSED` | — | Any call on a closed handle |
| `E_INVALID_STATE` | `tabBarNotDeclared` | `lx.app.tabBar.update` in an lxapp without a tab bar |
| `E_INTERNAL` | — | The host failed to open what it accepted |

A refused open changes nothing: a bad `page` or channel is rejected before an
lxapp moves.

---

## 3. Moving a surface between roles

A move keeps the live instance: an lxapp keeps its Logic, page stack and
WebViews and sees one `onHide` / `onShow` pair, never a restart; a URL tab keeps
its tab id and document; a terminal keeps its PTYs, working directory and
processes. The handle the caller holds reports the new `realized` placement.

### 3.1 The moves

**Promote** (aside to main). The surface leaves its slot, is appended to the
main switcher and is selected. Triggers:

- dropping an aside tab on the sidebar, or the tab menu's **Open in Main
  Area** (§5.3), which is also how a touch host promotes;
- a Pin of an lxapp that is live as an aside (§7.3);
- `openApp(appId, { as: 'main' })` or `openDeclared(id, { as: 'main' })`.

**Demote** (main to aside). The surface leaves the switcher, its neighbour is
selected, and it joins the side panel (or, for a native provider, the native
slot). Triggers:

- dragging a sidebar group onto the trailing band of the main area (§7.2);
- a content-provided placement item, such as an lxapp's **Open in Side
  Panel**;
- the built-in **Move to Aside** menu item, which the shell adds to a non-root
  main's menu only in `regular` and only when the content offers no placement
  item of its own (its own item re-hosts the live page first);
- `openApp(appId, { as: 'aside' })` or `openDeclared(id, { as: 'aside' })`.

The stable root never moves; asking fails with `E_SURFACE_ROLE_LOCKED`.
URL-callback and ephemeral browser tabs belong to their opener and cannot be
moved.

### 3.2 A reopen that names a role

- An explicit `as` that differs from the live role moves the surface there.
- A reopen without `as` — `openApp` without it, a sidebar action reopening a
  declaration, a notification or menu target, a Pin — focuses the live surface
  in the role it has. It never puts back the declared role over one the user
  chose by dragging.
- A notification or menu target naming an lxapp docked as an aside shows that
  aside and delivers `page` / `params` as App `onShow`; it does not fail for
  want of the main.
- An lxapp live only inside another surface's app navigation cannot move:
  `E_SURFACE_CONFLICT`. An lxapp live on another channel than the one asked
  for: `E_SURFACE_CONFLICT` with `appOpenInOtherRole`.
- A host that cannot move a live lxapp refuses before any state changes:
  `E_NOT_SUPPORTED` with `placementUnsupported`.

### 3.3 Serialisation

A move holds the lxapp's open guard, so it never interleaves with an open, a
restart or another move of the same app. A drag or menu move runs on the UI
thread and never waits for the guard, since an open holding it may be waiting
on that thread: while the guard is held, the gesture is refused. A refused move
restores the app's region only if no close has cleared it meanwhile.

---

## 4. Size class and the layout decision

One procedure turns a window into a layout. Every host runs it; §7 to §9 say
how each host draws the result.

### 4.1 Constants

| Name | Value | Use |
|---|---:|---|
| Compact boundary | 600 dp/pt | `compact` below, `regular` at or above |
| Hysteresis | 24 dp/pt | A size class changes only once the width is this far past the boundary |
| Sidebar width (budget) | 220 dp/pt | What the budget reserves for an expanded sidebar |
| Rail width | 56 dp/pt | Icon rail and app rail |
| Sidebar budget | 820 dp/pt | Sidebar width plus the compact boundary: the free width an expanded sidebar needs |
| Main minimum | 320 × 240 dp/pt | Size aside admission always leaves the main |
| Aside minimum | 240 dp/pt wide; 180 high at top or bottom | Narrowest docked aside |
| Visible slots | 2 in `regular`, 1 in `compact` | Ceiling on docked slots |
| Portrait ratio | 1.2 | Height over width at which a touch host is clearly portrait |
| Floating tab bar | 60 cell / 60 high / 12 margin dp | Touch tab bar, floating shape |
| Tab column | 80 dp | Touch tab bar, column shape |
| Touch side panel minimum | 320 dp | Narrowest side of a touch host's main / side-panel split |
| Phone tab slots | 5 | Bottom tab bar slots before **More** |

An aside that asks for no size gets its host's default, and grows no further
than its host's maximum (§7.2).

### 4.2 Inputs

- **W**, the available width: the main window's client area on a desktop, the
  app window on a touch host, the simulated screen in a Runner device frame.
- **H**, its height.
- **Smallest width** of the display (§1.2).
- **Host form**: desktop or mobile.
- **Mains**: how many there are, including web tabs and pins on a desktop.
- **Asides**: each slot's edge, requested width, and whether it is collapsed.
- **Sidebar preference**: whether the user collapsed the desktop sidebar.
- **Tab bar**: whether the presented main is on a tab page, and how many tabs it
  shows.
- **Hinge**: position, orientation and posture, when one crosses the window.

### 4.3 Procedure

1. **Size class.** `regular` when W ≥ 600, else `compact`, held across
   the boundary by the hysteresis: a `compact` window becomes `regular` at
   W ≥ 624, a `regular` one becomes `compact` at W < 576. Before the
   first measurement the size class is `compact`.
2. **Host kind.**
   - Desktop host form: the desktop shell (§7), in either size class.
   - Mobile host form with smallest width ≥ 600 and size class `regular`: the
     touch host in `regular` (§8).
   - Any other mobile host: the phone projection (§9). A phone stays here when
     it turns sideways, though its width is then `regular`.
3. **Asides.** The workspace is W less the sidebar or rail. Reserve the main
   minimum from it. Take the slots in most-recently-used order, skipping empty
   and collapsed ones, until the ceiling is reached:
   - in `compact`, the slot is admitted as a full-screen overlay;
   - a left or right slot is admitted when the aside minimum still fits;
   - a top or bottom slot spans the main's width and always fits;
   - a slot that does not fit stays alive and hidden, and a smaller older slot
     is still considered.

   On a desktop an admitted slot gets its requested width, clamped between
   the aside minimum and its host's maximum; on a touch host the side panel
   splits the content area instead (§8.5). When the user explicitly opens an aside that
   was not admitted, it covers the main as an overlay and hides again when the
   user returns to the main. Admission is a ceiling: crossing into `regular`
   does not by itself fill the slots.
4. **Sidebar** (desktop). The free width is W less the widths of the docked,
   expanded left and right slots. The sidebar is expanded when the free width is
   at least the sidebar budget and the user has not collapsed it; otherwise it
   is the icon rail. A `compact` desktop window always shows the rail. With a
   single main whose lxapp has no tab bar there is no sidebar.
5. **App rail** (touch host in `regular`). None with one main; the rail with
   several.
6. **Tab bar shape** (§6.2). On a touch host in `regular`: the floating tab bar
   when the screen's H ≥ 1.2 × W and the main shows at most five tabs, else
   the tab column. On a phone: the bottom tab bar.
7. **Status band** (touch hosts). Present while a side column — the tab column,
   the app rail or an expanded sidebar — is visible.
8. **Hinge.** A docked side panel beside a vertical hinge puts its divider on
   the hinge. A horizontal hinge in the `half-opened` posture reaches the page
   unchanged; the host does not crop the page to one half.

Outputs: the size class; `canDock`, which is true when the size class is
`regular` and false otherwise or before a layout exists; the sidebar or rail
form; each slot's visibility, size and overlay state; the tab bar shape; the
status band; and each page's viewport. What a page reads of them is in
`view-environment-spec.md`. A page's own size class comes from its own
viewport, so a narrow aside in a `regular` window reads `compact`. Crossing
the sidebar budget never changes a size class.

### 4.4 Worked examples

**Phone, 393 × 852 dp.** W = 393: `compact`. Smallest width 393: phone
projection. No sidebar, no rail. The main is full screen with the bottom tab
bar. An aside opens full screen over the main; `canDock` is false. Turned
sideways (W = 852) the size class is `regular` and the projection is still the
phone's.

**Tablet, 1024 × 1366 dp, two mains, no aside.** W = 1024: `regular`.
Smallest width 1024: touch host in `regular`. Two mains: the 56 dp app rail,
leaving a 968 dp main. H / W = 1.33 ≥ 1.2 and the main shows four tabs: the
floating tab bar at the main's leading bottom corner. The rail is visible, so
the status band is on. Turned to 1366 × 1024 with one main and the side panel
open: no rail; H / W = 0.75, so the tab column; the panel docks at half the
width, 683 dp, and the tab column moves to the main's left edge. The main's
pages are 683 − 80 = 603 dp wide and read `regular`.

**Open foldable, 784 × 820 dp, vertical hinge at x = 392, one main, a side
panel open.** W = 784: `regular`. Smallest width 784: touch host in `regular`.
One main: no rail. H / W = 1.05 < 1.2: the tab column, 80 dp. The side panel
docks with its divider on the hinge, so it takes the right leaf (392 dp) and
the main the left; the tab column moves to the main's left edge. The main pane
is 392 − 80 = 312 dp and its pages read `compact`. The tab column is visible,
so the status band is on.

---

## 5. Asides and the side panel

These rules hold wherever a slot is docked. §5.6 says where each host puts it.

### 5.1 Slots

The aside region has two slots:

| Slot | Holds | Several children |
|---|---|---|
| side panel (`panel`) | lxapps and URL tabs, mixed, in open order | One strip; one tab per appId; API URLs reuse by first URL |
| native | Instances of native declarations, such as the terminal | One strip; the default instance plus keyed ones where supported |

The slot APIs also accept `lxapp` and `browser` as names of the side panel.

- Switching tabs hides and shows; only an explicit close destroys content.
- A terminal surface is one workspace that may hold several PTY tabs. Keyed
  terminal surfaces are separate workspaces, shown as separate slot children or
  mains; switching never stops their PTYs.
- A slot's edge is the edge of the child placed most recently. Left and right
  slots are full height; top and bottom slots span the main's width only. With
  two slots on one edge, the later one sits further out. Slots cannot be
  dragged to another edge.

### 5.2 The strip

- **A docked slot always shows its strip, even with one child.** The strip is
  where switching, closing, the tab menu and dragging out live; hiding it would
  remove them and make the region jump when a second child arrives. With one
  child it reads as a plain header — icon, name, "···", ✕ — without chip fill,
  hover wash or divider, and becomes tab chips at the same height when a
  sibling arrives.
- Every slot uses one tab component: a title tab with flared outline, bottom
  aligned, hover wash and separators between tabs, carrying icon, title and
  close. An lxapp shows its icon and name, a URL tab its favicon and page title.
- Tabs keep open order and cannot be reordered (dragging a tab onto the
  sidebar promotes it, §3.1). Under pressure tabs drop their text, then the
  strip scrolls; tabs never shrink to slivers.
- The strip has no create entries and no strip-level menu: no "+", no strip
  "···". Content enters a slot through the open API, sidebar actions or the
  sidebar's browser "+".

### 5.3 The tab menu

Each tab has one menu, reached by its "···" button (space always reserved;
shown on hover, always on a one-child header) and by right-click. In order:
the content's identity header; **Open in Main Area** (§3.1; absent on
URL-callback and ephemeral tabs); the content's own section (a URL tab's page
actions — bookmark, pin, copy link, open externally; an lxapp's More actions);
**Close**. Collapse and maximize are strip controls and stay out of the menu.

### 5.4 Collapse, overlay and maximize

- **Collapse** hides a slot without closing anything. The lxapp child it showed
  gets page `onHide`, and `onShow` when the slot is restored with that child
  still active; Logic and the WebView stay alive.
- A collapsed slot with content leaves a handle at its edge: a slim tongue,
  12 × 56 pt, vertically centred, flush with the window edge, its ends sloped
  8 pt, in the separator colour with the accent on hover and no icon. Its
  tooltip names the content; from two tabs it shows their count. Clicking it,
  or opening or focusing a child, restores the slot.
- A slot with content but no room to dock is never silently absent: an explicit
  open shows it as an overlay over the main, and while hidden it keeps the same
  handle, whose click shows the overlay again. Widening the window docks it. A
  slot whose children were all hidden by their app has no handle, and a handle
  never reveals a hidden child.
- **Maximize** covers the main area and does not change the role. At most one
  slot per window is maximized; the plan marks it `maximized` and `overlay`, so
  hosts draw it as any overlay. It restores on the second toggle, a main
  switch, or the slot collapsing or losing its last child. The content is told
  only its new size. The main and other docked slots keep their frames and get
  no lifecycle event. Focus moves into the maximized slot's page and back to the
  main page on restore. The control sits left of the collapse control; an
  overlay that already covers the main does not offer it.

### 5.5 Content inside a slot

- An lxapp aside shows its page navigation bar below the strip only while Back
  or Home has somewhere to go. At the root of its stack the bar takes no height
  unless Home is offered (a landing page that is neither initial nor a tab);
  `navigationStyle: custom` never shows it. Page titles never move into the
  strip.
- An lxapp with a tab bar can always be an aside. On its tab pages the
  framework draws the tab bar as a 28 pt row of page tabs under the navigation
  bar (§6.2).
- A URL tab active in the side panel has a bar under the strip with history
  navigation and refresh; its title tab, menu and close are the strip's. It
  MUST NOT offer address editing or user-created tabs; a host MAY show the
  current URL read-only when there is room. "Open in main browser" promotes the
  current URL tab only.
- A float an aside lxapp opens anchors to that aside's region, not the main.

### 5.6 Keys and Back

- Back acts on the page stack of the region that has focus: an aside page pops
  its own stack, never the main's.
- The keyboard Back (Backspace on macOS) is taken only when the page left the
  key unhandled, so an editable element keeps deleting text. Windows has no
  keyboard Back: Backspace is not a Back key there, and WebView2 gives the
  shell no unhandled-key path that would spare typing.
- Cmd+Option+\ (Ctrl+Alt+\ on Windows) collapses the side panel or restores it
  (as an overlay when it cannot dock), wherever focus is. Focus moves into the
  aside page when it returns and back to the main when it goes.
- Escape in a maximized aside's page, or in the shell, returns the aside to its
  dock; it never collapses it.
- System Back and edge-swipe Back on a touch host act on the topmost region: an
  overlaid slot is dismissed whole (§9.2) before the main's stack pops.

### 5.7 Where each host puts the slots

| | Desktop | Touch host in `regular` | `compact` |
|---|---|---|---|
| Side panel | Docked at its edge; user-resizable | Docked at the right, sharing the content area through a divider (§8.5) | Full screen over the main |
| Native slot | Docked at its edge | As the side panel | Full screen over the main |
| No room to dock | Over the main pane | Over the main pane | — |
| Collapsed or dismissed | Edge handle | Edge handle | Edge handle |

---

## 6. The lxapp tab bar

### 6.1 One model

An lxapp declares two to ten tabs in `lxapp.json` `tabBar`, each with a page,
label, icon and style. Every host shows the same items, selection, badges and
red dots. `lx.app.tabBar.update` patches items, badges and visibility; colours
are static in `tabBar.style` and a `style` field is rejected
(`E_INVALID_ARG`). An lxapp without a tab bar gets `E_INVALID_STATE` with
`tabBarNotDeclared`.

- `visibility: 'hidden'` hides the bar wherever it is projected; on a desktop
  it hides the expanded rows and disables the chevron. `'auto'` clears that and
  expands. The user's chevron toggles a separate user collapse and never
  overrides an API hide.
- The bar shows only while the presented main is on a tab page. A non-tab page
  hides it on touch hosts; on the desktop the rows stay expanded and no item is
  selected (§6.3), because the sidebar is a persistent navigation region.
- A tab without `text` is never a blank cell or an empty menu row: macOS shows
  the icon alone, named from its page path for tooltip, accessibility and
  pop-up; Windows, whose rows are text only, shows the page's title, else its
  page name.

### 6.2 Projections

| Host | Where the tabs go |
|---|---|
| Desktop, expanded sidebar | Child rows under the lxapp's group header (§7.2) |
| Desktop, icon rail | A page panel that floats out from the lxapp's rail item (§7.2) |
| Touch host in `regular` | The floating tab bar or the tab column (§8.3) |
| Phone (`compact`) | The bottom tab bar, at the platform's bar height: five slots, the fifth becoming **More** beyond five; a tablet's bar shows all declared tabs |
| Inside an aside | A 28 pt row of page tabs under the navigation bar: icon and label, scrolling on overflow, a pop-up menu below 240 pt of width; the current tab in the selected colour over a 2 pt underline, without fill chips |

On a desktop only the active main lxapp's group is expanded; switching mains
collapses the others and reveals the incoming one. Expand and collapse are
instant; only the chevron animates. While collapsed, badges and red dots
aggregate onto the group header.

### 6.3 Two-level selection

- The lxapp's group header (or rail item) stays highlighted while the main
  shows that lxapp, whatever page it is on.
- A tab item is highlighted after a switch to its tab page. On a page that is
  not a tab page no item is selected.
- The two levels are visible at once and styled differently. The expanded rows
  carry a thin vertical attribution line on their left.

### 6.4 Colours

- **Touch hosts.** The bar sits on the lxapp's page. Its colours are the
  static `tabBar.style`, then the lxapp's appearance defaults for unset keys. It
  MUST NOT follow the system or host dark mode on its own: most lxapps have no
  dark page, and a dark bar on a light page is worse than a light bar on a dark
  system. A light-pinned lxapp keeps its authored bar.
- **Desktop.** The rows are host chrome and follow the host `theme`.
  `backgroundColor` is touch-only and never paints the rows, even `#FFFFFF` on a
  light sidebar. Other style keys tint the rows while the host is light; while
  it is dark the host theme wins for every key, also for a light-pinned lxapp.
  Unset keys inherit the shell theme.
- **Inside an aside.** As on the desktop; an undeclared key falls back to the
  host accent or muted foreground.

| Style key | Touch bar | Desktop rows |
|---|---|---|
| `foregroundColor` | Unselected label | Unselected item title |
| `selectedForegroundColor` | Selected label | Selected item title and its left-edge accent bar |
| `backgroundColor` | Bar background | Unused |
| `dividerColor` | Bar divider | Attribution line |

An item's single `iconPath` is a template the host tints with these colours.
A raster icon keeps its own colours.

---

## 7. Desktop shell

macOS and Windows, and a Runner desktop preset. The two platforms MUST match in
structure, spacing, selection and separation; only system controls differ.

### 7.1 Layout

```text
┌─────────┬──────────────────────────┬─────────┐
│ sidebar │        main area         │  aside  │
│  pins   │                          │  slots  │
│  mains  │      lxapp / browser     │         │
│         ├──────────────────────────┤         │
│ actions │       bottom aside       │         │
└─────────┴──────────────────────────┴─────────┘
```

The main and the slots share the content layer, above the sidebar's base
layer, with clear boundaries between regions.

### 7.2 Sidebar and main switcher

macOS and Windows share structure, spacing rules, selection and separation;
their metrics are each platform's own:

| Metric | macOS (pt) | Windows (dp) |
|---|---:|---:|
| Expanded sidebar, default / user-resizable up to | 148 / 400 | 148 / 400 |
| Icon rail | 56 | 56 |
| Top-level row height / gap | 40 / 4 | 36 / 4 |
| Tab item row height; gap below header / between items | 34; 2 / 0 | 28; 1 / 0 |
| Close overlay on the selected item | 26 | 24 |
| Rail divider between pins and mains | 18, with 5 either side | 22 |
| Pin grid columns | 4 | 3 at the default width, 4 when wider |
| Rail page panel width | 188 | 220 |
| Footer action row height / outer inset | 34 / 6 | 30 / 8 |
| Demote drop band | 96, tinted while a group hovers | 72, not tinted |
| Aside default (left / right) / top or bottom | 320 / 320 | 380 / 280 |
| Aside maximum | until the main reaches its minimum (700 before the window is measured) | 700 |
| Float default size, minimum | 360 × 420, clamped to the container | 460 × 560, at least 280 × 220, 24 from the edges |

The sidebar budget (§4.1) reserves 220 whatever the sidebar's actual width.

**Switcher.**

- It lists mains only, in stable graph order, lxapp, web and native mains
  interleaved. Asides never appear in it. A main lxapp MUST NOT gain a tab strip
  inside its content rectangle.
- `SurfaceId` is the only switcher identity. An appId, terminal session id, URL
  or browser workspace tab id is provider metadata.
- While mains exist, exactly one is selected. The root has no close action;
  replacing it is a host configuration matter, not a tab action.
- Each provider supplies its title, icon, capabilities and menu section; the
  shell applies user title overrides and appends lifecycle actions. Browser and
  terminal mains may offer close and rename when not the root. An lxapp's menu
  holds its More actions, not generic close and rename. Close Others / Close
  After follow switcher order and skip the root and items that cannot close.
- Browser workspace tabs are internal to one browser main: opening, navigating
  or closing one never adds, renames or removes a switcher item.
- Pins and sidebar actions stay fixed, outside the scrolling switcher.

**Rows.**

- Top-level rows keep one uniform gap between any two, measured on the visible
  background or hover outline. Changing row type adds no margin, band or
  separator. Larger text grows rows, never the gap.
- Tab item rows are tighter than top-level rows to show attribution, never
  small enough to mis-tap; after the last item the top-level gap returns.
- lxapp rows have a persistent chevron and a rounded app tile; web rows a
  slightly smaller bare favicon. No extra background grouping.
- Hovering the selected item replaces its icon with a rounded close button
  when, and only when, the item can close. Other icons stay; a click
  selects.
- On hover a workspace row shows a vertical ellipsis that opens the same menu
  as right-click. Row controls (ellipsis, close, disclosure) have a circular
  hover wash; menus follow the chrome's light or dark scheme.

**Icon rail.** The rail keeps the switcher's identities, order and gap, and
never shows an lxapp's tab bar in place of mains. With pins and mains both
present, a low-contrast divider sits in their gap. Every
icon-only item exposes its label as tooltip and accessibility text.

**Sidebar controls.** The collapse and expand toggles sit at the top, outside
the scroller. Windows centres a left sidebar's toggles in the caption row; a
right sidebar clears the caption buttons. macOS aligns them with the traffic
lights, and the rail adds a 32 pt row below them (without that clearance in a
frameless window). The collapse button is trailing-aligned with a 24 hit target
and an 18 glyph; the expand button is centred in the rail with rail item
metrics. Both show the hover wash. Switching modes never moves a toggle between
header and footer, and the footer reserves no toggle slot. Cmd/Ctrl+\ toggles
the sidebar.

**Peek.** Hovering the rail's expand control for 300 ms shows the expanded
sidebar as a shadowed overlay. The committed width, the workspace, docked
panels and reported metrics do not change, and the peek is never persisted. It
slides in (no motion under reduced motion). It stays while the pointer is inside
and closes 250 ms after it leaves, at once on Esc, a click outside or the window
losing key status, and before a size-class change or a pin. Choosing a
navigation target closes it; toggles, closing an item, a menu or rename keep it
open. It keeps the rail's header row, so the expand control stays under the
pointer. Clicking that control, or Cmd/Ctrl+\, pins the sidebar expanded. It
does not open from rail items, while a rail page panel shows, or while a button
is held, and opening it closes rail hover panels. In a `compact` window pinning
is unavailable: the control closes the peek and Cmd/Ctrl+\ toggles it.

**Demote band.** While a sidebar group is dragged, a band along the main
area's trailing edge is the drop target for demotion (§3.1). A drag keeps the plain arrow cursor and shows the element's name on a
translucent chip under the pointer — never a move or "not allowed" cursor.

### 7.3 Pins

- Pins are user state. There is no app API to pin or unpin, in JS or Rust.
- Pins sit above the switcher in a grid: at most eight, lxapp and web pins
  together. Tiles are 36 × 36 with a gap of 5, centred; an incomplete row stays
  aligned to the first slot. The grid never scrolls.
- Every path that pins — page menu, address bar, context menu, bookmark
  manager, lxapp menu — goes through one shared operation, which enforces the
  limit. Over it the operation returns `LimitReached { max }` and the host shows
  a localised message. A stored pin is never hidden by rendering.
- One ordered, mixed list is persisted. Hosts MUST NOT put lxapps before
  websites.
- An lxapp pin is a launch intent. Clicking it opens or selects that lxapp as a
  main, which has its own switcher row; removing the pin does not close it. When
  the lxapp is live as an aside, the click promotes it (§3.1); it does not
  inherit a declared aside role. Sidebar actions and `openDeclared` keep the
  declared role.
- A main opened from a pin occupies exactly the root main's rectangle: no inset,
  card, offset or tab strip of its own, and the previous main is hidden.
- A website pin opens or selects a main browser tab.

### 7.4 Sidebar actions on screen

The writer's declaration rules are in §12.6. Placement:

- **Header**: at most two icon-only actions, in declaration order. A bootstrap
  Settings entry takes the first slot when configured; runtime actions beyond
  the free slots are dropped in declaration order with one logged warning per
  distinct set. If the actions do not fit beside the window controls, none
  render; partial truncation by width is not allowed. In the rail, header
  actions lead the icon stack.
- **Footer, expanded**: rows 4 apart (row height and inset per platform,
  §7.2). An action without `value` is a square icon button one row high;
  buttons in a row are 2 apart. An
  action with `value` is a value cell: icon and value on one line,
  tail-truncated. Its minimum width is measured with native font metrics: the
  icon, the padding and the whole value, or two CJK characters and an ellipsis
  when the value is longer. Its icon sits on the buttons' column.
- Declaration order is visual order. Icon buttons wrap only when the next one
  does not fit. A value cell joins the buttons before it when its minimum fits,
  takes the rest of the row and ends it. When its minimum does not fit it starts
  the next row, except after a single icon button, which it keeps beside it and
  truncates further: one icon never stands alone above a value.

  ```text
  [a, B, c, d, E]   (capitals carry a value; c, d and E's minimum overflow)
  │ a │ B value…          │
  │ c │ d                 │
  │ E value…              │
  ```

- At most five footer rows show; beyond that the footer scrolls rather than
  squeezing the switcher. The shell does not reorder to keep value cells
  visible.
- Background is transparent; hover is a quiet wash with radius 6; a disabled
  action mutes icon and text and has no hover.
- **Footer, rail**: icon only, with the same scrolling and disabled treatment.
  The rail keeps room for the expand control; actions never overlap it.
- **Tooltip**: `label`, or `label`, the localised separator and `value`; a
  value equal to its label shows once. Expanded footer tooltips open above the
  action, centred and kept inside the window, after the system delay; the rail
  shows them beside. macOS sets the accessibility label to `label` and the value
  to `value`. Windows custom chrome carries both in the tooltip.
- A warning is logged when two icon-only footer actions share an icon.
- Actions do not render on touch hosts; their declarations still validate and
  reappear if the process returns to a desktop form.

### 7.5 Window chrome

- The top system region belongs to the shell. A browser main may put its
  address bar there; an lxapp's navigation bar lives inside the main content.
- `window: { frameless: true }` affects the main window only. With the default
  `controls: shell` the shell draws a persistent control strip and its own drag
  region, whatever the main.
- `controls: content` lets the home page draw the window buttons and drag
  regions (`app-region: drag / no-drag`). The build then requires the home lxapp
  to be the only main, and the runtime refuses browser and guest mains. Only in
  this mode does the writer drive the window controls (minimize, maximize,
  close, state).
- A page opened as a window uses the platform's standard frame and none of the
  main window's chrome.
- The first launch sizes the main window to 1200 × 800, shrunk to 85 % of the
  work area on smaller screens, never below 480 × 480 unless the work area is
  smaller, and centred on the screen the user is on (Windows: the monitor under
  the cursor, at its DPI). `lingxia.yaml` has no field for it; a host that calls
  the platform's default-size API overrides the size, still fitted to the work
  area. Later launches restore the persisted frame (§13).

### 7.6 A `compact` desktop window

A narrow desktop window is still a desktop shell. The icon rail stays, the main
stays a workspace, and an lxapp's tabs stay in the rail's page panel rather than
moving to the bottom. A browser main keeps its top address bar: the field
flexes and secondary actions MAY move to an overflow menu, but browser chrome
never moves to the bottom or into the rail. Asides are full-screen overlays
over the main pane (§4.3). The sidebar expand control opens the peek instead of
expanding.

---

## 8. Touch host in `regular`

### 8.1 Which hosts

A mobile host whose smallest width is at least 600 — an open foldable or a
tablet, real or in a Runner device frame — in the `regular` size class. Because
the smallest width does not change with rotation, the projection never flips
when the device turns. Pins, sidebar actions and the expanded sidebar do not
render, and the main area carries no top-right capsule. `isMobile()` stays
true.

### 8.2 App rail

With one main there is no column. With several (another lxapp, a web tab), a
56 dp rail lists them and switches between them. It lists apps only: the
presented lxapp's tabs are in the touch tab bar, and asides are not mains.

### 8.3 Touch tab bar

The presented main lxapp's tabs never sit in a sidebar. They form the touch tab
bar, shown only on a tab page of that main, outside the sidebar budget.

- **Floating tab bar**, while the window is clearly portrait (H ≥ 1.2 × W)
  and the bar has at most five tabs: a pill of 60 dp cells (icon over label),
  60 dp high, 12 dp in from the main region's leading and bottom edges, where a
  phone's bar would be. Five cells clear a fold's crease even beside the app
  rail. The page keeps clear of the strip the bar stands in (an immersive page
  runs under it), and the strip takes the page background. The pill is
  translucent, more so when the bar is immersive, but always a plate: labels
  never sit directly on the page. The soft keyboard hides it while it is up.
- **Tab column** otherwise: an 80 dp column at the screen's right edge, its
  cells low, where a holding hand reaches them. A near-square open foldable is
  held like a book, by its sides, so it takes the column too. The column keeps
  its edge in right-to-left languages. With a docked side panel it moves to the
  main's left edge, so the main's tabs stay on the main's outer side. The column
  is opaque: the page does not run under it, and an immersive bar takes the
  navigation-bar colour.

A platform with its own touch tab bar control (iOS `UITabBarController`) uses
it and lets the system place it; the shapes above are what the other hosts and
the Runner draw.

### 8.4 Status band

While a side column (the tab column, the app rail or an expanded sidebar) is
visible, the status bar is one band in the sidebar colour with one contrasting
tint, and the column and the content both start below it; an immersive page
does not bleed under it. A real status bar has one tint, and in `regular` its
clock sits over the column, so the navigation-bar colour cannot reach it.
Without a side column, as in all of `compact`, the active page's navigation-bar
colour extends under the status bar and an immersive page bleeds behind it.

### 8.5 Side panel

The side panel holds lxapp and URL asides. It docks at the right of the main
when the display's smallest width is at least 600, and shares the content area
with it through a divider instead of taking a requested width: half each by
default, each side at least the touch side panel minimum, the ratio persisted.
The divider shows a grip only where there is room to drag it. Beside a vertical
hinge the divider sits on the hinge. Where the panel cannot dock, or when
maximized, it covers the main. Collapsed, it leaves the edge handle of §5.4.
System Back collapses an overlaid side panel before it reaches the main. While
a page is in media full screen, the panel and the tab column hide. Promote and
demote are the menu items of §3.1.

### 8.6 Back and gestures

System Back and edge-swipe Back act on the topmost region (§5.6). There is no
keyboard Back and no keyboard shortcut for the side panel on a touch host.

---

## 9. Phone (`compact`)

Phones, folded foldables, tablet windows narrower than 600, and their Runner
device frames.

### 9.1 Main

The main is full screen. The active lxapp's tab bar is at the bottom (§6.2).
Sidebar, pins, rail and sidebar actions do not render; an app that needs quick
entries draws them itself. With more than one main, a drawer button showing
their count opens the switcher.

### 9.2 Asides

- An aside covers the main full screen. System Back and edge-swipe Back hide the
  whole active slot and restore the main; its tabs are kept.
- lxapp and native slots MAY offer a header Back that does the same. A URL aside
  MUST NOT add one: browser chrome owns navigation and dismissal.
- For an lxapp or native slot, the header close closes the current child;
  closing the last closes the slot.

### 9.3 Browser chrome

- A URL main uses an editable address row above an action row: Back, Forward,
  Reload, New Tab, the tab switcher with its count, and Dismiss when an lxapp
  main can be restored. The desktop address bar and sidebar MUST NOT show behind
  or beside it.
- A URL aside uses one bottom row: Back, Forward, Refresh, the aside tab
  switcher with its count, and Dismiss. It has no address row, no New Tab, no
  overflow menu and no top-left Back. Its Back and Forward buttons move through
  page history; system Back, edge-swipe Back and Dismiss leave the whole slot
  even when history exists, keeping its tabs.
- A browser-only Runner uses the main's two rows without Dismiss, since no lxapp
  main is covered. Its field accepts URLs, not searches.
- Main tabs and aside tabs are separate groups: counts, switchers, activation and
  the successor chosen on close never mix them. A tab is closed from its own
  group's switcher; closing the last hides that group, and hiding or dismissing a
  group closes none of its tabs.

### 9.4 Floats and windows

Floats are bottom sheets; a platform with a native popover MAY use one.
`openPage` with `as: 'window'` fails with `E_NOT_SUPPORTED` and
`placementUnsupported`.

---

## 10. Platform matrix

What each host implements. A cell says "as specified", "differs:" and how,
"not implemented", or "—" where the rule does not apply. Shared-code gaps are
listed in every column they affect. A Runner with a desktop preset is a
desktop host; its Windows or macOS column in §10.2 applies. Every cell comes
from reading the code; none was confirmed by running a host.

### 10.1 Every host

| Rule | Windows | macOS | Windows Runner (frame) | macOS Runner (frame) | iOS | Android | HarmonyOS |
|---|---|---|---|---|---|---|---|
| Size class from the window width (§4.3) | as specified | as specified | as specified | as specified | as specified | as specified | as specified |
| A page's viewport is its own region | as specified | not verified | as specified | not verified | differs: the window, also beside a docked panel | as specified | differs: the window; a docked panel is padding |
| Moving a live lxapp (§3) | as specified; without the shell's move handler, refused | as specified | as specified | differs: no promote in the phone UI | not implemented: refused | differs: promote from the tab menu; no demote in the UI | not implemented: refused |
| Hinge in the page environment | — | — | as specified (simulated) | as specified (simulated) | not implemented | as specified | as specified |
| Rust shell writer (§12.5) | not implemented | not implemented | not implemented | not implemented | not implemented | not implemented | not implemented |

### 10.2 Desktop shell

| Rule | Windows | macOS |
|---|---|---|
| Sidebar budget and single-main rule (§4.3 step 4) | not verified | not verified |
| Collapsed slot handle counts tabs | not implemented | as specified |
| Cmd+Option+\ / Ctrl+Alt+\ | as specified; stands down where AltGr types `\` | as specified |
| Window default 960 × 720 | not verified | as specified |
| `frameless`, `controls`, writer window controls | not implemented | not implemented |
| Persistence: frame, sidebar, pins | as specified | as specified |
| Persistence: group collapse, aside geometry | not implemented | as specified |
| Persistence: main session, lazy restore | not implemented | not implemented |

### 10.3 Touch hosts

| Rule | Windows Runner (frame) | macOS Runner (frame) | iOS | Android | HarmonyOS |
|---|---|---|---|---|---|
| Touch host test: smallest width ≥ 600 in `regular` (§8.1) | differs: window width, so a phone on its side is a touch host | differs: phone frames never; a foldable frame at least 600 wide takes the desktop shell | not implemented | differs: smallest width alone, without the size class | not implemented |
| Floating tab bar and tab column (§8.3) | differs: the floating bar keeps the physical left in right-to-left languages | not implemented | differs: its own tab bar view, not `UITabBarController`; no touch shapes | differs: the portrait test measures the main pane, not the screen | not implemented |
| Soft keyboard hides the floating bar | not implemented | — | — | not implemented | — |
| App rail | as specified | not implemented | not implemented | not implemented | not implemented |
| Status band | as specified | not implemented | not implemented | differs: the tab bar's background colour | not implemented |
| Side panel split (§8.5) | differs: the desktop aside panel | differs: URL asides only, always over the screen | differs: URL asides only | as specified | differs: URL asides only |
| Divider on a vertical hinge | not implemented | not implemented | not implemented | not implemented | not implemented |
| Back collapses an overlaid side panel | not verified | not implemented | not implemented | as specified | as specified |
| Collapsed handle counts tabs | not implemented | as specified | as specified | as specified | as specified |
| Phone drawer for several mains (§9.1) | not implemented | not implemented | not implemented | not implemented | not implemented |
| No top-right capsule in `regular` | not verified | not verified | not verified | not verified | not verified |
| `openPage` window refused | — | — | as specified | as specified | as specified |

---

## 11. `lingxia.yaml`

### 11.1 Declarations

A surface entry starts with its content key:

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

There is at most one declaration per declaration id. Declarations give
build-time availability, provider choice and runtime defaults; they are not a
registration gate for dynamic lxapps, pages or URLs. A native provider is
reachable only through its declaration. `lingxia build` compiles declarations
into the generated `ui.json`.

### 11.2 Valid combinations

| Content | Roles in YAML | Runtime placements |
|---|---|---|
| lxapp | main, aside, float | main, aside, float |
| page | float | float, window |
| URL | main, aside | main, aside |
| native | main, aside | main, aside |

The build checks:

- `edge` only on an aside; the default edge is right, and a native capability
  may set another default;
- `launch` only on a main, on at most one;
- a float declaration has a `tray:`; runtime floats need none; at most one tray
  surface per host;
- page declarations belong to the home lxapp, and a declared page has the stable
  key `declared:<page-name>` so tray and sidebar reopens reach the same instance;
- a `platforms` filter that excludes the current platform removes the surface
  and everything that refers to it;
- a URL surface requires the browser capability; a native surface is `terminal`
  or `browser` and requires its capability;
- a host declares exactly one main, or none and only tray floats; further mains
  are runtime workspaces;
- `controls: content` meets the single-main rule of §7.5.

There is no `sidebar:` field. Sidebar entries come only from
`lx.shell.sidebarActions`.

---

## 12. Runtime API

### 12.1 Calls

"Control app" marks calls only the home lxapp may make; others get
`E_PERMISSION_DENIED`. The namespace is not the privilege.

| Call | Who | Does | Fails with |
|---|---|---|---|
| `lx.surface.openPage(page, options?)` | any lxapp | Its own page as a float or window; returns the handle at once, `opened` settles when shown | `E_INVALID_ARG`, `E_NOT_FOUND`, `E_NOT_SUPPORTED` (`placementUnsupported`) |
| `lx.surface.openUrl(url, options?)` | any lxapp | A browser tab in main (default) or the side panel | `E_INVALID_ARG`, `E_PERMISSION_DENIED` (domain), `E_INTERNAL` |
| `lx.surface.openApp(appId, options?)` | Control app | Opens, focuses or moves another lxapp in main or aside | `E_INVALID_ARG`, `E_PERMISSION_DENIED` (+ `appUnderMaintenance` / `appSuspended`), `E_SURFACE_ROLE_LOCKED`, `E_SURFACE_CONFLICT` (+ `appOpenInOtherRole`), `E_NOT_SUPPORTED` (`placementUnsupported`) |
| `lx.surface.openDeclared(id, options?)` | any lxapp; options Control app | Opens a declaration with its own placement, or a keyed instance or another role | `E_NOT_FOUND`, `E_PERMISSION_DENIED`, `E_SURFACE_ROLE_LOCKED`, `E_CAPABILITY_MISSING` |
| `lx.surface.get(key)` | any lxapp | Its own live page or URL tab by key | — |
| `lx.surface.app(appId)` | Control app | The live lxapp's handle | — |
| `lx.surface.declared(id, { key? })` | Control app | The live declaration's handle | — |
| `lx.shell.openBuiltin('downloads')` | Control app | The browser's downloads page | `E_NOT_FOUND`, `E_PERMISSION_DENIED` |
| `lx.shell.activate()` | Control app | Brings the product window forward without navigating | — |
| `lx.shell.sidebarActions.replace / update / remove` | Control app | §12.6 | `E_INVALID_ARG`, `E_NOT_FOUND` |
| `lx.shell.appDisplay.update(patch)` | Control app | The name and icon the shell shows for the app | `E_INVALID_ARG` |
| `lx.nav.go({ app, page?, params?, channel?, version? })` | any lxapp | App navigation inside the current surface (§12.3) | `E_SURFACE_CONFLICT` |
| `lx.nav.leaveApp()` | any lxapp | Back to the lxapp that opened this one | `E_INTERNAL` |
| `lx.app.tabBar.update(patch)` | any lxapp | Its own tab bar (§6.1) | `E_INVALID_ARG`, `E_INVALID_STATE` (`tabBarNotDeclared`) |

Process-level capabilities — update, exit, badge, autostart, tray,
notifications — are on `lx.host`, never on `lx.shell`.

### 12.2 Opening

- `openDeclared` without a `key` addresses the declaration's default instance. A
  non-empty `key` (1 to 128 UTF-8 bytes) selects or creates an additional
  instance where the provider admits several (the terminal does). Omitting `as`
  uses the declared role; `as` focuses or moves the same instance (§3).
  `{ surface: 'terminal', key: 'project-a', as: 'aside' }` is a separate
  terminal workspace in the native slot that may later move to main without
  losing its PTYs. A declaration that shows an lxapp resolves to that app's
  `AppSurface`.
- `openApp` creates, focuses or moves a dynamic lxapp; it needs no declaration.
  `as` is `main` or `aside`; omitted, a live app keeps its role and a closed one
  opens in main. A float lxapp is declared and opened with `openDeclared`, so
  its tray anchor, dismissal and presentation exist. `page` is a configured page
  name; full routes are rejected. `channel` defaults to `release`; `version`
  installs a newer version first and refuses an older one. `edge` is valid only
  with an aside and is a preference for layouts with room.
- `openPage` takes `as: 'float'` (the default), `'window'`, or an ordered list
  of both, where the first the host can realize wins and `realized` says which.
  `openUrl` takes `as: 'main'` (the default), `'aside'`, or an ordered list.
- A float without a size hint takes its host's default (§7.2); it is centred,
  non-modal and dismissed by a tap outside, and in `compact` it is a bottom
  sheet whatever its position. A window defaults to 960 × 720 dp/pt, clamped
  to the work area. Sizes are hints, clamped by
  admission (§4.3); a non-finite, negative or malformed size fails with
  `E_INVALID_ARG`, while too little room degrades instead of failing.
- `interaction.closeButton` adds the standard circular close control. A float
  with `dismiss: 'manual'` needs it or its own close path. A modal float blocks
  input below and restores focus on close.
- An explicit `as` changes only the live instance, never the declaration or the
  role later opens default to.
- URLs are `https:` or a host-authorized `file:`, and their domain must pass the
  lxapp's security policy. The only public `lingxia:` page is the browser's
  downloads page, opened with `lx.shell.openBuiltin('downloads')`; every other
  `lingxia:` URL, including `lingxia://settings`, fails with `E_INVALID_ARG`.
  Settings is native chrome or browser-local navigation, never a Logic
  destination. Handing a URL to the system still passes the host's scheme
  allowlist.

### 12.3 App navigation and App Surfaces

| Operation | Changes | Switcher | Back | Returns |
|---|---|---|---|---|
| `lx.nav.go({ app, … })` | Pushes an app onto the current lxapp surface's app stack | Keeps the current item | `lx.nav.leaveApp()` pops to the previous app | `Promise<void>` |
| `lx.surface.openApp(appId, { as: 'main' })` | Creates or focuses a dynamic lxapp surface | Its own item | Closing destroys the surface and its app stack | `AppSurface` |
| `lx.surface.openDeclared(id, { as: 'main' })` on an lxapp declaration | Creates or focuses that declared surface | Its own item | Closing destroys it; reopening uses the declaration | The app's `AppSurface` |

A surface opened as main is a parallel workspace with its own runtime id, item,
role and handle. App navigation is sequential inside the selected surface and
never creates or moves a shell item; the app on top of the stack changes, the
surface does not. Navigating to the lxapp already running is a no-op.
Navigating to an lxapp docked as an aside fails with `E_SURFACE_CONFLICT` rather
than moving or cloning it.

### 12.4 Handles and messaging

- A handle is bound to its surface for life and reports `kind`, the live
  `realized` placement (updated when the surface moves or the user drags it),
  `visible` and `alive`, with `show`, `hide`, `close` and the events of §2.1.
  `onShow` / `onHide` carry `source` (`opener`, `page` or `shell`); `onClose`
  carries the close reason.
- One live lxapp has one handle, an `AppSurface`: `openApp`, `lx.surface.app`
  and the `openDeclared` / `lx.surface.declared` of the declaration that shows
  it return the same object. A declaration whose lxapp is not live returns
  `undefined` from `lx.surface.declared`.
- A URL handle covers one tab (`scope: 'tab'`, with `activate()` and `close()`),
  or, when the host does not name the tab, the browser's tab group
  (`scope: 'group'`), whose lifetime the shell owns. A builtin page's handle
  reports identity only.
- Page surfaces have instance-bound messaging: messages address one instance,
  never broadcast by appId or page name, and replies return to the opener's
  handle. A native surface has messaging only when its capability declares it.
- Automation reads each runtime id's owner from `surfaceLayout().identities`.

### 12.5 Shell writer

- On a host with Logic, the Control app is the writer, through `lx.shell` and
  the composition calls of `lx.surface`. Host Rust does not write the same state
  in parallel.
- On a host without Logic, host Rust is the writer through an equivalent
  `lingxia` facade with the same states and errors.
- The writer declares sidebar actions (§12.6); sets the name and icon the shell
  shows for its app (`appDisplay.update`: a name of 1 to 64 characters, an icon
  of at most 1 MiB and 4096 px a side, ranked above the registry and
  `lxapp.json`, kept per service environment); and, under `controls: content`,
  drives the window controls.
- Where they do not render (touch hosts), writer declarations still validate.

### 12.6 Sidebar actions

A sidebar action is a stateless command in the header or footer. The shell runs
its callback and owns no target behaviour; it is not the sidebar and needs no
declared surface.

- Each action has a stable `id` (non-empty, unique across both placements),
  `placement` (`header` or `footer`), `label`, `icon` and `onSelect`, and may be
  `disabled`. A footer action may carry a `value`: the changing fact it shows,
  such as the working brand, store or account. Its presence is the only signal;
  empty means absent.
- `label` is the tooltip and accessibility name and is never drawn.
- `icon` is a bundled path or an `lx://temp`, `lx://usercache` or `lx://userdata`
  file, resolved through the lxapp's sandboxed resource resolver before the
  host sees it. Native absolute paths, `file:` and network URLs, and parent
  traversal are rejected. An SVG is a template the host draws in the shell's
  muted text colour, dimmed while disabled. A raster PNG, JPEG or WebP keeps its
  colours and is centre-cropped to the square slot. Icon-only actions are told
  apart by their icon alone.
- `replace` sets the whole generation atomically: the shell validates it all
  before touching handlers or chrome, and a refused generation leaves the
  previous one showing. It is refused with `E_INVALID_ARG` for a duplicate or
  empty id, a third header action, a header `value`, or a third footer value;
  the message names the ids. `replace([])` clears.
- `update(id, patch)` changes icon, label, value or disabled state, validated
  against the whole patched generation. A present `value` of `undefined`, `null`
  or `''` clears it. An empty patch or unknown field is `E_INVALID_ARG`; an
  unknown id is `E_NOT_FOUND`. `remove(id)` drops one action and its callback.
- All three commit at call time, in call order, and return a promise that
  rejects rather than throws.
- Callbacks are generation-scoped: replacing or removing an action unregisters
  its callback. Actions do not outlive the process, so the writer declares them
  on every Logic launch; the shell never shows entries whose callbacks do not
  exist yet. Within a process an explicit empty generation differs from "no
  writer yet".
- Activation by mouse, keyboard, accessibility, shortcut or automation is one
  semantic and runs the callback once. A disabled action is visible and cannot
  activate. Actions are never selected; whatever the callback opens owns its own
  state.
- Apps control no layout: no weight, colour, size or row. Hover and disabled
  styling are the shell's.
- A callback opens content explicitly: `openDeclared`, `openApp`, or
  `lx.nav.go({ app })`. Hosts never infer a target, open an lxapp, toggle a
  provider or draw a fallback glyph.

### 12.7 Shell routes for trusted pages

Trusted browser pages (the browser web UI) and the Control app use these
routes:

- `shell.pins` returns `{ items, max }`: the ordered `{ kind: 'lxapp' |
  'bookmark', key }` list and the combined limit. Read `max` rather than
  counting rows, since lxapp pins share it.
- `shell.reorderPins({ items })` takes every current item exactly once. Pinning
  past the limit fails with `SHELL_PIN_LIMIT` and `data.max`, so the caller
  localises the message. Automation reorders with the full list from
  `shell.pins`.
- `bookmarks.reorder` orders the bookmark manager only.
  `browser.bookmarks: false` hides bookmark chrome; `lingxia://bookmarks` and
  `bookmarks.list` / `watch` stay available.
- `tabs.recentlyClosed` lists the process's last 25 closed website tabs, newest
  first; `tabs.reopen({ id? })` restores one, the latest when `id` is omitted.
  Private tabs, aside tabs and window surfaces are not recorded.
- A replacement web UI declares `controlProtocolVersion` in its `lxapp.json`,
  equal to the runtime's control protocol version; a missing, older or unknown
  value fails the build.

---

## 13. Persistence

| Data | Rule |
|---|---|
| Main window | Normal size and position and the maximized state, saved at the end of a move or resize, on close and on quit; on restore clamped to the nearest screen's work area, then maximized again on first show |
| Sidebar | Width and expanded / rail state |
| Pins | The user's ordered mixed list |
| Main session | Tab content keys, session entry ids, order and selection |
| lxapp groups | The user's collapse; an API hide is rebuilt by the app |
| Aside geometry | Each slot's edge and size; on a touch host the side panel's divider ratio |
| Sidebar actions | Not persisted |

- Main sessions restore lazily: placeholders appear at once, and the live
  surface and its runtime id are created on first selection. A failed restore
  shows a retry / close placeholder. Session entry ids and runtime ids are never
  confused.
- Asides restore their geometry, never their content. After a restart the
  writer decides what to reopen; the shell does not bring back companion content
  with side effects on its own.

---

## 14. Naming

Code uses these roots; only the case changes between languages (Rust
`snake_case`, TS `camelCase`, YAML lowercase). Synonyms are not used.

| Term | Root | TS / JS | Rust | Not |
|---|---|---|---|---|
| surface, handle | `surface` | `AnySurface`, `AppSurface`, `DeclaredSurface`, `PageSurface`, `TabSurface`, `BuiltinSurface` | `Surface` | view, panel |
| YAML content | `lxapp / page / url / native` | not JS selectors | `SurfaceContent` | render |
| surface key | `key` | `key` | `SurfaceKey` | instanceKey, workspaceId |
| runtime id | `surface_id` | not in Logic; automation `surfaceLayout().identities` | `SurfaceId` | — |
| app navigation | `navigate_to_app / navigate_back_app` | `nav.go({ app })`, `nav.leaveApp` | same roots | navigateToLxApp, openApp |
| session entry id | `session_entry` | `sessionEntryId` | `SessionEntryId` | the runtime id |
| role | `role` | `SurfacePlacement` (with `window`) | `Role` | mode, kind |
| main | `main` | `'main'` | `Role::Main` | primary, home tab |
| aside, slot | `aside / slot` | `'aside'` | `Role::Aside`, `SlotKind` | panel as a role |
| side panel | `panel` | slot name `panel` | `SlotKind::Panel` | lxapp slot, browser slot |
| dock region | `can_dock` | `canDock` | `can_dock` | docked |
| role moves | `promote / demote` | — | `promote_aside_to_main`, `demote_main_to_aside`, `SurfaceMenuBuiltinAction::MoveToAside` | — |
| sidebar | `sidebar` | — | `Sidebar*` | tabbar (the lxapp's), rail (the collapsed form) |
| main switcher | `switcher` | — | `SwitcherForm`, `SurfaceSwitcherSnapshot` | — |
| touch tab bar | `touch_tab_bar` | — | `touch_tab_bar`; shapes `floating`, `tab_rail` (the tab column) | sidebar |
| sidebar action | `sidebar_action` | `lx.shell.sidebarActions`, `ShellSidebarAction` | `SidebarAction*` | activator, launcher |
| pin | `pin` | — | `Pin*`, `MAX_SHELL_PINS` | favorite, shortcut |
| size class | `size_class` | `sizeClass` (`compact` \| `regular`) | `SizeClass` | medium, expanded, breakpoint |
| hinge | `hinge` | `env.hinge` | `HingeRect`, `HingeOrientation`, `DevicePosture` | fold |
| host form | `host_class` | `isMobile()` / `isDesktop()` | `HostClass` | pad as a third class |
| tablet | `pad` | — | `is_pad` | tablet host class |
| admission | `admission` | — | `Policy`, `aside_slots_admitted` | — |
| errors | `E_*` | verbatim | the same `E_*` strings | per-platform names |
