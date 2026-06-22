/**
 * UI feedback, navigation, and surface control APIs.
 */

export interface ShowToastOptions {
  title: string;
  icon?: 'success' | 'error' | 'loading' | 'none';
  image?: string;
  duration?: number;
  mask?: boolean;
  position?: 'top' | 'center' | 'bottom';
}

export interface ShowModalOptions {
  title?: string;
  content?: string;
  showCancel?: boolean;
  cancelText?: string;
  cancelColor?: string;
  confirmText?: string;
  confirmColor?: string;
}

export interface ModalResult {
  confirm: boolean;
  cancel: boolean;
}

export interface ShowActionSheetOptions {
  itemList: string[];
  itemColor?: string;
}

export interface ActionSheetResult {
  tapIndex: number;
}

export type PageQueryValue = string | number | boolean | null | undefined;
export type PageQuery = Record<string, PageQueryValue>;

export type PageTargetOptions =
  | {
      page: string;
      path?: never;
      query?: PageQuery;
    }
  | {
      path: string;
      page?: never;
      query?: PageQuery;
    };

export type NavigateToOptions = PageTargetOptions;

export interface NavigateBackOptions {
  delta: number;
}

export type RedirectToOptions = PageTargetOptions;

export type SwitchTabOptions = PageTargetOptions;

export type ReLaunchOptions = PageTargetOptions;

export interface SetNavigationBarTitleOptions {
  title: string;
}

export interface SetNavigationBarColorOptions {
  frontColor: string;
  backgroundColor: string;
}

export interface TabBarRedDotOptions {
  index: number;
}

export interface SetTabBarBadgeOptions {
  index: number;
  text: string;
}

export interface RemoveTabBarBadgeOptions {
  index: number;
}

export interface SetTabBarStyleOptions {
  color?: string;
  selectedColor?: string;
  backgroundColor?: string;
  borderStyle?: string;
}

export interface SetTabBarItemOptions {
  index: number;
  text?: string;
  iconPath?: string;
  selectedIconPath?: string;
}

// ── Adaptive Surface Layout ─────────────────────────────────────────────────
// The form is expressed by the `as` field on `lx.openSurface({ page, as })`; the
// Host arbitrates the realized platform form (split pane on larger screens,
// full-screen drill-in on compact screens).

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

export interface CapsuleRect {
  width?: number;
  height?: number;
  top?: number;
  right?: number;
  bottom?: number;
  left?: number;
}
