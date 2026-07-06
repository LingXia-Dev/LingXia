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

/**
 * Target page for `navigateTo` / `redirectTo` / `switchTab` / `reLaunch`.
 *
 * Pass **exactly one** of `page` or `path` — there is **no `url` field**:
 * - `page` — a configured page **name** from `lingxia.yaml` / `lxapp.json`
 *   (e.g. `"pullToRefresh"`), resolved to its route by the page registry.
 * - `path` — the full page **route**, e.g. `"/pages/pulltorefresh/index"`.
 *
 * Both are discoverable with `lxdev lxapp pages`, which lists every page's
 * `name` and `path`; `lxdev lxapp nav to|relaunch|redirect|switch-tab <name>`
 * drives navigation by name when automating.
 */
export type PageTargetOptions =
  | {
      /**
       * Configured page **name** from `lingxia.yaml` / `lxapp.json`
       * (e.g. `"pullToRefresh"`). Mutually exclusive with `path`.
       */
      page: string;
      path?: never;
      query?: PageQuery;
    }
  | {
      /**
       * Full page **route**, e.g. `"/pages/pulltorefresh/index"`.
       * Mutually exclusive with `page`.
       */
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

export interface CapsuleRect {
  width?: number;
  height?: number;
  top?: number;
  right?: number;
  bottom?: number;
  left?: number;
}
