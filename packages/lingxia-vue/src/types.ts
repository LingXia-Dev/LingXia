import type { CSSProperties } from 'vue';
import type { LxMediaSwiperAttributes, LxVideoAttributes } from '@lingxia/elements';
import type {
  LxNavigatorEvent,
  NavigatorEnvVersion,
  NavigatorOpenType,
  NavigatorQuery,
  NavigatorTarget,
} from '@lingxia/elements';

export interface LxVideoProps extends Omit<LxVideoAttributes, 'ref' | 'className' | 'style'> {
  class?: string;
  style?: CSSProperties;
}

export interface LxMediaSwiperProps extends Omit<LxMediaSwiperAttributes, 'ref' | 'className' | 'style'> {
  class?: string;
  style?: CSSProperties;
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
  pageBindings?: Record<string, string>;
}

export interface LxNavigatorProps {
  url?: string;
  page?: string;
  openType?: NavigatorOpenType;
  target?: NavigatorTarget;
  delta?: number;
  query?: NavigatorQuery;
  appId?: string;
  path?: string;
  envVersion?: NavigatorEnvVersion;
  targetVersion?: string;
  phoneNumber?: string;
  hoverClass?: string;
  hoverStopPropagation?: boolean;
  hoverStartTime?: number;
  hoverStayTime?: number;
  class?: string;
  style?: CSSProperties;
}



export type { LxNavigatorEvent };
