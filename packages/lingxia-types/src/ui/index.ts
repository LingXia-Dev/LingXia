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
 * Popup size value.
 *
 * - number: absolute size, must be > 0
 * - `${number}%`: percentage size, must be > 0% and <= 100%
 */
export type PopupSurfaceSizeValue = number | `${number}%`;

export interface PopupSurfaceSize {
  /** Width for popup surface. */
  width?: PopupSurfaceSizeValue;
  /** Height for popup surface. */
  height?: PopupSurfaceSizeValue;
}

export type PopupSurfaceOptions = SurfaceTargetOptions & {
  kind: 'popup';
  position?: 'center' | 'bottom' | 'left' | 'right' | 'top';
  size?: PopupSurfaceSize;
};

export interface WindowSurfaceSize {
  /** Window width, must be a positive number. */
  width?: number;
  /** Window height, must be a positive number. */
  height?: number;
}

export type WindowSurfaceOptions = SurfaceTargetOptions & {
  kind: 'window';
  size?: WindowSurfaceSize;
};

export type SurfaceOpenOptions = PopupSurfaceOptions | WindowSurfaceOptions;

export interface CapsuleRect {
  width?: number;
  height?: number;
  top?: number;
  right?: number;
  bottom?: number;
  left?: number;
}
