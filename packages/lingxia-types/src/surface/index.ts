/**
 * Surfaces (docked asides, floats, windows, browser tabs, declared surfaces)
 * and the desktop tray — the types behind `lx.openSurface`, `lx.onSurfaceContext`,
 * and `lx.tray`.
 */

// Handles

export type SurfaceCloseReason =
  | 'user'
  | 'programmatic'
  | 'owner_closed'
  | 'app_closed'
  | 'failed'
  /**
   * The SDK reclaimed a long-hidden overlay surface for resource reasons.
   * Treat as a normal close: the page instance is gone; further postMessage /
   * show / hide calls will fail. The opener may immediately reopen if needed.
   */
  | 'reclaimed'
  | 'unknown';

export interface SurfaceClosedEvent {
  id: string;
  kind: 'overlay' | 'window';
  reason: SurfaceCloseReason;
}

/**
 * Detail payload for `onShow` / `onHide` events. `source` identifies which
 * Surface object initiated the visibility change so observers can
 * distinguish self-driven transitions from peer-driven ones (e.g. an opener
 * UI that wants to update its own button state only when the page side
 * toggled visibility).
 */
export interface SurfaceVisibilityEvent {
  id: string;
  kind: 'overlay' | 'window';
  source: 'opener' | 'page';
}

export interface SurfaceHandle {
  readonly id: string;
  /**
   * Show a host-managed surface. Dynamic page/url surfaces return a Promise;
   * host-declared surfaces may complete synchronously.
   */
  show(): void | Promise<void>;
  /**
   * Hide without destroying user-visible state when the platform supports it.
   */
  hide(): void | Promise<void>;
  /**
   * Close or hide the surface depending on how it is managed by the host.
   */
  close(): void | Promise<void>;
}

export interface Surface extends SurfaceHandle {
  readonly kind: 'overlay' | 'window';
  /**
   * Last-known visibility, kept in sync with the native side via show/hide
   * events. False once the surface has been closed. Safe to bind into
   * declarative UI; for event-driven updates subscribe via `onShow`/`onHide`.
   */
  readonly visible: boolean;
  /**
   * True until `close()` fires. After close the surface is detached and the
   * page instance is being torn down; further `show()` / `hide()` calls will
   * reject.
   */
  readonly alive: boolean;
  /**
   * Sends a message to the other side of a page surface.
   *
   * For the opener this targets the opened page. For the opened page this
   * targets the opener. URL surfaces have no page-side receiver.
  */
  postMessage(message: unknown): void;
  onMessage(handler: (message: unknown) => void): () => void;
  onClose(handler: (event: SurfaceClosedEvent) => void): () => void;
  /**
   * Fires when the surface transitions to visible, regardless of whether
   * `show()` was called on this side or on the peer. Returns an unsubscribe
   * function. Only fires on real state changes — calling `show()` on an
   * already-visible surface is a no-op for listeners.
   */
  onShow(handler: (event: SurfaceVisibilityEvent) => void): () => void;
  /**
   * Fires when the surface transitions to hidden, regardless of which side
   * triggered it. Returns an unsubscribe function. Only fires on real state
   * changes.
   */
  onHide(handler: (event: SurfaceVisibilityEvent) => void): () => void;
  close(): Promise<void>;
  /**
   * Toggle the surface to visible without tearing it down. The page instance
   * and its state survive a hide / show round-trip — only close() actually
   * destroys the surface and fires the onClose listener. Idempotent: calling
   * on an already-visible surface resolves without firing `onShow`.
   */
  show(): Promise<void>;
  /**
   * Hide the surface without destroying it. The page instance stays mounted,
   * so a subsequent show() restores the same scroll position, form input,
   * and JS state. Hidden surfaces still receive postMessage but are not
   * visible to the user. Idempotent.
   */
  hide(): Promise<void>;
}

// Layout hints

/**
 * Size hint for an overlay surface (aside / float).
 *
 * - number: absolute px, must be > 0
 * - `${number}%`: percentage of the container, 0 < N ≤ 100
 */
export type OverlaySurfaceSizeValue = number | `${number}%`;

export interface OverlaySurfaceSize {
  /** Width hint. */
  width?: OverlaySurfaceSizeValue;
  /** Height hint. */
  height?: OverlaySurfaceSizeValue;
}

/** Edge an aside docks to; the Host decides the realized form by screen size. */
export type SurfaceEdge = 'left' | 'right' | 'top' | 'bottom';

/** Where a float popup anchors (default `center`). */
export type SurfaceFloatPosition = 'center' | 'top' | 'bottom' | 'left' | 'right';

export interface WindowSurfaceSize {
  /** Initial window width in logical pixels. */
  width?: number;
  /** Initial window height in logical pixels. */
  height?: number;
}

/**
 * The window's adaptive context, delivered to `lx.onSurfaceContext()` so an
 * lxapp can self-adapt (e.g. switch column count by `sizeClass`).
 */
export interface SurfaceContext {
  /** compact (<600) / medium (600–840) / expanded (>840), with hysteresis. */
  sizeClass: 'compact' | 'medium' | 'expanded';
  /** In compact, the bottom region belongs to the app content. */
  bottomOwner: 'app';
}

// Open specs

/**
 * Spec for {@link OpenSurfaceSpec}. A discriminated union keyed by source so a
 * page name and a declared surface id never collide (each is its own string
 * space, separately type-checkable).
 *
 * - `{ page }` — one of this lxapp's own pages, by name, arranged as `as`
 *   (`float` is a popup; `window` is a bare desktop window, which rejects on
 *   mobile). `position` applies to `float`, and `size` is a Host-clamped hint.
 *   They are fixed at open (re-open to change). Your own pages **cannot** be
 *   docked as an `aside` — an aside is external content only (see `{ url }`).
 *   For a side panel of your own, use a declared `surface`, an in-page split
 *   layout, or `role: main` for a switchable destination.
 *
 *   `float` is a popup layered above the main at `position` (like a dialog); it
 *   takes no layout space. The SDK gives it **no chrome of its own — there is no
 *   built-in close button**: the lxapp owns the popup UI and dismisses it by
 *   calling `surface.close()` (or `.hide()`). A float sized to the full container
 *   (`size: { width: '100%', height: '100%' }`) presents immersively on mobile
 *   (system bars hidden) and is likewise chrome-less — draw your own close
 *   affordance. (iOS retains a silent left-edge swipe as a last-resort escape so a
 *   full-screen float can never trap the user; don't rely on it as the primary
 *   dismissal.)
 * - `{ surface }` — a surface declared in `lingxia.yaml` `surfaces:`, by id
 *   (e.g. `'terminal'`, `'ai-assistant'`). Form, position, and startup data come
 *   from the declaration.
 * - `{ url }` — external content in the in-app browser. Without `as` it opens as
 *   a main browser tab (the **self** browser: full chrome **with an editable
 *   address bar**, no handle). With `as: 'aside'` it opens in the **browser
 *   aside** — a docked (large screen) / full-screen (phone) **multi-tab** browser
 *   for external content only (`https://` or `file://`).
 *
 *   The aside is **API-only and has no address input** (its one difference from
 *   the self browser): each `openSurface({ url, as: 'aside' })` call opens a tab;
 *   there is no manual "new tab" affordance and the address is never editable.
 *   Tabs are **deduped by URL** — reopening a URL focuses the existing tab and
 *   returns its handle. The handle is **tab-scoped**: `close()` closes that tab,
 *   and closing the last tab closes the aside. The tab strip shows page
 *   **titles** (never the URL), plus per-tab close, back/forward, refresh, and a
 *   close-aside control.
 *
 *   Presentation is the only large/small difference: on `medium` / `expanded`
 *   the aside **docks** and splits beside the main at `edge` (default `'right'`)
 *   with a horizontal title tab strip; on `compact` (phone / runner) it presents
 *   **full-screen** with a **bottom** browser toolbar (tabs reached via a tab
 *   switcher), dismissed by the host back affordance. `size` is a host-clamped
 *   preferred size (large screen only).
 */
export type OpenPageSurfaceSpec =
  | {
      page: string;
      /**
       * A chrome-less popup above the main: the lxapp draws its own UI and close
       * affordance — there is no SDK-provided close button (see
       * {@link OpenPageSurfaceSpec}).
       */
      as: 'float';
      position?: SurfaceFloatPosition;
      size?: OverlaySurfaceSize;
      query?: Record<string, unknown>;
      edge?: never;
      surface?: never;
      url?: never;
    }
  | {
      page: string;
      as: 'window';
      size?: WindowSurfaceSize;
      query?: Record<string, unknown>;
      edge?: never;
      position?: never;
      surface?: never;
      url?: never;
    };

export interface OpenDeclaredSurfaceSpec {
  surface: string;
  /**
   * Docking edge override for this open. Without it the surface keeps its
   * current placement (initially the `lingxia.yaml` edge); with it the panel
   * opens there — or moves there if already visible.
   */
  edge?: SurfaceEdge;
  page?: never;
  url?: never;
  as?: never;
  position?: never;
  size?: never;
  query?: never;
}

export interface OpenUrlTabSpec {
  url: string;
  as?: never;
  page?: never;
  surface?: never;
  edge?: never;
  position?: never;
  size?: never;
  query?: never;
}

/**
 * Open `url` in the multi-tab browser aside. `url` must be `https://` or
 * `file://` (external content only). Repeated calls add/focus tabs (deduped by
 * URL) in the single aside per window; the returned handle is scoped to that
 * tab. See {@link OpenSurfaceSpec} for the full aside contract.
 */
export interface OpenUrlAsideSpec {
  url: string;
  as: 'aside';
  edge?: SurfaceEdge;
  size?: OverlaySurfaceSize;
  page?: never;
  surface?: never;
  position?: never;
  query?: never;
}

export type OpenSurfaceSpec =
  | OpenPageSurfaceSpec
  | OpenDeclaredSurfaceSpec
  | OpenUrlTabSpec
  | OpenUrlAsideSpec;

// Tray (desktop)

/**
 * Runtime control of the menu-bar (macOS) / system-tray (Windows) status item.
 * The tray is declared in `lingxia.yaml` (`tray:`); these update its dynamic
 * content at runtime.
 *
 * **Desktop only.** Mobile platforms have no tray, so every method here is a
 * no-op there (it never throws) — safe to call from portable code. For an
 * app-icon badge that *is* cross-platform (including mobile), use
 * `lx.app.setBadge`.
 */
export interface TrayMenuItem {
  label: string;
  /** Invoked when this item is clicked. */
  onClick?: () => void;
  enabled?: boolean;
  checked?: boolean;
}

export interface TrayMenuSeparator {
  separator: true;
}

export interface TrayApi {
  /** Replace the status-item icon (a resource path). */
  setIcon(icon: string): void;
  /** Set the text shown beside the icon (macOS). Pass `null`/empty to clear. */
  setTitle(text: string | null): void;
  /** Set the badge — e.g. an unread count. Pass `null`/empty to clear. */
  setBadge(value: string | number | null): void;
  /**
   * Replace the right-click dropdown menu. There is no default menu — provide
   * your own items (e.g. `{ label: 'Quit', onClick: () => lx.app.exit() }`).
   *
   * The menu is a snapshot: to change an item's `checked`/`enabled`/`label`
   * state, call `setMenu` again with the full updated array. There is no
   * per-item mutation API.
   */
  setMenu(items: Array<TrayMenuItem | TrayMenuSeparator>): void;
  /**
   * Handle a left-click on the tray icon yourself. While a handler is
   * registered the click runs only the handler — the tray's configured surface
   * `action` is suppressed, so the click is fully yours (e.g. toggle a state and
   * `setIcon`). Returns an unsubscribe function.
   */
  onClick(handler: () => void): () => void;
  /** Show the tray status item. */
  show(): void;
  /** Hide the tray status item (without removing the app). */
  hide(): void;
}
