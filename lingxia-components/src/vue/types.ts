import type { CSSProperties } from 'vue';
import type { LxVideoAttributes } from '../video.js';
import type { LxNavigatorEvent, NavigatorOpenType, NavigatorTarget } from '../navigator.js';

export interface LxVideoProps extends Omit<LxVideoAttributes, 'ref' | 'className' | 'style'> {
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

export interface LxInputProps {
  id?: string;
  modelValue?: string;
  value?: string;
  defaultValue?: string;
  type?: 'text' | 'number' | 'password' | 'digit';
  password?: boolean;
  placeholder?: string;
  placeholderStyle?: string;
  placeholderClass?: string;
  maxlength?: number;
  cursorSpacing?: number;
  autoFocus?: boolean;
  disabled?: boolean;
  focus?: boolean;
  confirmType?: 'send' | 'search' | 'next' | 'go' | 'done';
  alwaysEmbed?: boolean;
  confirmHold?: boolean;
  cursor?: number;
  cursorColor?: string;
  selectionStart?: number;
  selectionEnd?: number;
  adjustPosition?: boolean;
  holdKeyboard?: boolean;
  class?: string;
  style?: CSSProperties;
  bindInput?: string;
  bindChange?: string;
  bindFocus?: string;
  bindBlur?: string;
  bindConfirm?: string;
  bindKeyboardHeightChange?: string;
  bindNicknameReview?: string;
  catchInput?: string;
  catchChange?: string;
  catchFocus?: string;
  catchBlur?: string;
  catchConfirm?: string;
  catchKeyboardHeightChange?: string;
  catchNicknameReview?: string;
}

export interface LxTextareaProps {
  id?: string;
  modelValue?: string;
  value?: string;
  defaultValue?: string;
  placeholder?: string;
  placeholderStyle?: string;
  placeholderClass?: string;
  maxlength?: number;
  disabled?: boolean;
  autoFocus?: boolean;
  focus?: boolean;
  autoHeight?: boolean;
  cursorSpacing?: number;
  showConfirmBar?: boolean;
  adjustPosition?: boolean;
  holdKeyboard?: boolean;
  disableDefaultPadding?: boolean;
  confirmType?: 'send' | 'search' | 'next' | 'go' | 'done' | 'return';
  confirmHold?: boolean;
  fixed?: boolean;
  adjustKeyboardTo?: 'cursor' | 'bottom';
  cursor?: number;
  selectionStart?: number;
  selectionEnd?: number;
  class?: string;
  style?: CSSProperties;
  bindInput?: string;
  bindChange?: string;
  bindFocus?: string;
  bindBlur?: string;
  bindConfirm?: string;
  bindLineChange?: string;
  bindKeyboardHeightChange?: string;
  catchInput?: string;
  catchChange?: string;
  catchFocus?: string;
  catchBlur?: string;
  catchConfirm?: string;
  catchLineChange?: string;
  catchKeyboardHeightChange?: string;
}

export type { LxNavigatorEvent };
