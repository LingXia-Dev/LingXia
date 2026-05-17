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

export type SurfaceQueryValue = PageQueryValue;
export type SurfaceQuery = PageQuery;

export type SurfacePageTargetOptions =
  | {
      page: string;
      path?: never;
      url?: never;
      query?: SurfaceQuery;
    }
  | {
      path: string;
      page?: never;
      url?: never;
      query?: SurfaceQuery;
    };

export type SurfaceUrlTargetOptions = {
  url: string;
  page?: never;
  path?: never;
  query?: never;
};

export type SurfaceTargetOptions = SurfacePageTargetOptions | SurfaceUrlTargetOptions;

/**
 * Overlay surface size value.
 *
 * - number: absolute size, must be > 0
 * - `${number}%`: percentage size, must be > 0% and <= 100%
 */
export type OverlaySurfaceSizeValue = number | `${number}%`;

export interface OverlaySurfaceSize {
  /** Width for overlay surface. */
  width?: OverlaySurfaceSizeValue;
  /** Height for overlay surface. */
  height?: OverlaySurfaceSizeValue;
}

/**
 * Overlay surface: a webview composited on top of the host activity's
 * content. Cross-platform. Covers the screen (or a fraction of it) until
 * closed; coexists with native media preview at the same z-tier — the
 * later-added overlay or preview wins compositing order.
 */
export type OverlaySurfaceOptions = SurfaceTargetOptions & {
  kind: 'overlay';
  position?: 'center' | 'bottom' | 'left' | 'right' | 'top';
  size?: OverlaySurfaceSize;
};

export interface WindowSurfaceSize {
  /** Window width, must be a positive number. */
  width?: number;
  /** Window height, must be a positive number. */
  height?: number;
}

/**
 * Window-kind surfaces are macOS-only. Android, iOS, and Harmony reject
 * `kind: 'window'` at open() and surface a `surface_open_failed` error;
 * use `OverlaySurfaceOptions` for cross-platform code.
 */
export type WindowSurfaceOptions = SurfaceTargetOptions & {
  kind: 'window';
  size?: WindowSurfaceSize;
};

export type SurfaceOpenOptions = OverlaySurfaceOptions | WindowSurfaceOptions;

export interface CapsuleRect {
  width?: number;
  height?: number;
  top?: number;
  right?: number;
  bottom?: number;
  left?: number;
}
