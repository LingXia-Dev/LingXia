import React, { forwardRef, useCallback, useEffect, useRef } from "react";
import { registerTextareaComponent } from "../textarea.js";
import { buildTextareaNativeAttrs } from "../text_component_native_attrs.js";
import {
  assignForwardedRef,
  bindElementEvents,
  getCustomEventDetail,
  unbindElementEvents,
} from "./text_component_shared.js";

export interface LxTextareaEventDetail {
  value?: string;
  cursor?: number;
  lineCount?: number;
  height?: number;
  duration?: number;
  selectionStart?: number;
  selectionEnd?: number;
  heightRpx?: number;
}

export interface LxTextareaProps {
  id?: string;
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
  confirmType?: "send" | "search" | "next" | "go" | "done" | "return";
  confirmHold?: boolean;
  fixed?: boolean;
  adjustKeyboardTo?: "cursor" | "bottom";
  cursor?: number;
  selectionStart?: number;
  selectionEnd?: number;

  onInput?: (detail: LxTextareaEventDetail) => void;
  onChange?: (detail: LxTextareaEventDetail) => void;
  onFocus?: (detail: LxTextareaEventDetail) => void;
  onBlur?: (detail: LxTextareaEventDetail) => void;
  onConfirm?: (detail: LxTextareaEventDetail) => void;
  onLineChange?: (detail: LxTextareaEventDetail) => void;
  onKeyboardHeightChange?: (detail: LxTextareaEventDetail) => void;

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

  className?: string;
  style?: React.CSSProperties;
}

if (typeof window !== "undefined") {
  registerTextareaComponent();
}

export const LxTextarea = forwardRef<HTMLElement, LxTextareaProps>(({
  value,
  defaultValue,
  placeholder,
  placeholderStyle,
  placeholderClass,
  maxlength,
  disabled,
  autoFocus,
  focus,
  autoHeight,
  cursorSpacing,
  showConfirmBar,
  adjustPosition,
  holdKeyboard,
  disableDefaultPadding,
  confirmType,
  confirmHold,
  fixed,
  adjustKeyboardTo,
  cursor,
  selectionStart,
  selectionEnd,
  onInput,
  onChange,
  onFocus,
  onBlur,
  onConfirm,
  onLineChange,
  onKeyboardHeightChange,
  bindInput,
  bindChange,
  bindFocus,
  bindBlur,
  bindConfirm,
  bindLineChange,
  bindKeyboardHeightChange,
  catchInput,
  catchChange,
  catchFocus,
  catchBlur,
  catchConfirm,
  catchLineChange,
  catchKeyboardHeightChange,
  className,
  style,
  id,
  ...rest
}, ref) => {
  const elementRef = useRef<HTMLElement | null>(null);
  const boundElementRef = useRef<HTMLElement | null>(null);
  const handlerRef = useRef({
    onInput,
    onChange,
    onFocus,
    onBlur,
    onConfirm,
    onLineChange,
    onKeyboardHeightChange,
  });
  handlerRef.current = {
    onInput,
    onChange,
    onFocus,
    onBlur,
    onConfirm,
    onLineChange,
    onKeyboardHeightChange,
  };
  const listenerMapRef = useRef<Record<string, EventListenerObject>>({
    input: {
      handleEvent: (event: Event) => handlerRef.current.onInput?.(getCustomEventDetail<LxTextareaEventDetail>(event)),
    },
    change: {
      handleEvent: (event: Event) => handlerRef.current.onChange?.(getCustomEventDetail<LxTextareaEventDetail>(event)),
    },
    focus: {
      handleEvent: (event: Event) => handlerRef.current.onFocus?.(getCustomEventDetail<LxTextareaEventDetail>(event)),
    },
    blur: {
      handleEvent: (event: Event) => handlerRef.current.onBlur?.(getCustomEventDetail<LxTextareaEventDetail>(event)),
    },
    confirm: {
      handleEvent: (event: Event) => handlerRef.current.onConfirm?.(getCustomEventDetail<LxTextareaEventDetail>(event)),
    },
    linechange: {
      handleEvent: (event: Event) => handlerRef.current.onLineChange?.(getCustomEventDetail<LxTextareaEventDetail>(event)),
    },
    keyboardheightchange: {
      handleEvent: (event: Event) => handlerRef.current.onKeyboardHeightChange?.(getCustomEventDetail<LxTextareaEventDetail>(event)),
    },
  });
  const elementRefCallback = useCallback((element: HTMLElement | null) => {
    boundElementRef.current = bindElementEvents(boundElementRef.current, element, listenerMapRef.current);
    elementRef.current = element;
    assignForwardedRef(ref, element);
  }, [ref]);

  useEffect(() => () => {
    unbindElementEvents(boundElementRef.current, listenerMapRef.current);
    boundElementRef.current = null;
    elementRef.current = null;
  }, []);

  React.useEffect(() => {
    const el = elementRef.current;
    if (!el) return;
    const setAttr = (name: string, next: string | null) => {
      if (next === null) {
        el.removeAttribute(name);
        return;
      }
      el.setAttribute(name, next);
    };
    setAttr("value", value !== undefined ? String(value) : null);
    setAttr("focus", focus !== undefined ? (focus ? "true" : "false") : null);
    setAttr("maxlength", maxlength !== undefined ? String(maxlength) : null);
    setAttr("placeholder-style", placeholderStyle ? String(placeholderStyle) : null);
    setAttr("auto-height", autoHeight ? "true" : null);
  }, [value, focus, maxlength, placeholderStyle, autoHeight]);

  const textareaProps = buildTextareaNativeAttrs({
    id,
    value,
    defaultValue,
    placeholder,
    placeholderStyle,
    placeholderClass,
    maxlength,
    disabled,
    autoFocus,
    focus,
    autoHeight,
    cursorSpacing,
    showConfirmBar,
    adjustPosition,
    holdKeyboard,
    disableDefaultPadding,
    confirmType,
    confirmHold,
    fixed,
    adjustKeyboardTo,
    cursor,
    selectionStart,
    selectionEnd,
    bindInput,
    bindChange,
    bindFocus,
    bindBlur,
    bindConfirm,
    bindLineChange,
    bindKeyboardHeightChange,
    catchInput,
    catchChange,
    catchFocus,
    catchBlur,
    catchConfirm,
    catchLineChange,
    catchKeyboardHeightChange,
  }, rest as Record<string, unknown>, boundElementRef.current !== null);

  return React.createElement("lx-textarea", {
    ref: elementRefCallback,
    className,
    style,
    ...textareaProps,
  });
});

LxTextarea.displayName = "LxTextarea";
