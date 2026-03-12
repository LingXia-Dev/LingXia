import type { CSSProperties } from 'vue';
import type { LxVideoAttributes } from '../video.js';
import type { LxNavigatorEvent, NavigatorOpenType, NavigatorTarget } from '../navigator.js';

export interface LxVideoProps extends Omit<LxVideoAttributes, 'ref' | 'className' | 'style'> {
  class?: string;
}

export interface LxPickerProps {
  columns?: string[][] | [string[], Record<string, string[]>];
  mode?: 'date' | 'time';
  start?: string;
  end?: string;
  fields?: 'year' | 'month' | 'day' | 'range';
  modelValue?: string | string[];
  placeholder?: string;
  class?: string;
  style?: CSSProperties;
  disabled?: boolean;
  cancelText?: string;
  cancelTextColor?: string;
  cancelButtonColor?: string;
  confirmText?: string;
  confirmTextColor?: string;
  confirmButtonColor?: string;
  onChange?: (event: Event) => void;
  onNativeScroll?: (event: Event) => void;
  bindChange?: string;
  bindScroll?: string;
  catchChange?: string;
  catchScroll?: string;
}

export interface LxNavigatorProps {
  url?: string;
  openType?: NavigatorOpenType;
  target?: NavigatorTarget;
  delta?: number;
  appId?: string;
  path?: string;
  phoneNumber?: string;
  hoverClass?: string;
  hoverStopPropagation?: boolean;
  hoverStartTime?: number;
  hoverStayTime?: number;
  class?: string;
  style?: CSSProperties;
}

export type { LxNavigatorEvent };
