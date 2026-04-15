/**
 * UI feedback, navigation, and surface control APIs.
 */

import type { EventEmitter } from '../app';

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

export interface NavigateToOptions {
  url: string;
}

export interface NavigateToResult {
  eventEmitter: EventEmitter;
}

export interface NavigateBackOptions {
  delta: number;
}

export interface RedirectToOptions {
  url: string;
}

export interface SwitchTabOptions {
  url: string;
}

export interface ReLaunchOptions {
  url: string;
}

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

export interface ShowPopupOptions {
  url: string;
  widthRatio?: number;
  heightRatio?: number;
  position?: 'center' | 'bottom' | 'left' | 'right';
}

export interface ShowPopupResult {
  eventEmitter: EventEmitter;
}

export interface CapsuleRect {
  width?: number;
  height?: number;
  top?: number;
  right?: number;
  bottom?: number;
  left?: number;
}
