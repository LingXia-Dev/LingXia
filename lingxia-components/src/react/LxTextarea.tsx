import React, { forwardRef, useCallback, useRef } from "react";
import { registerTextareaComponent } from "../textarea.js";
import {
  appendBindingAndDatasetAttrs,
  bindEventListeners,
  getCustomEventDetail,
  unbindEventListeners,
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
  const hasMountedRef = useRef(false);
  const initialValueRef = useRef(defaultValue);
  const didInitialNativeSyncRef = useRef(false);

  const propsRef = useRef({ onInput, onChange, onFocus, onBlur, onConfirm, onLineChange, onKeyboardHeightChange });
  propsRef.current = { onInput, onChange, onFocus, onBlur, onConfirm, onLineChange, onKeyboardHeightChange };

  const boundRef = useRef<HTMLElement | null>(null);
  const boundListenersRef = useRef<Record<string, EventListenerObject> | null>(null);

  const textareaRefCallback = useCallback((el: HTMLElement | null) => {
    if (typeof ref === "function") ref(el);
    else if (ref) (ref as React.MutableRefObject<HTMLElement | null>).current = el;

    if (boundRef.current && boundRef.current !== el && boundListenersRef.current) {
      unbindEventListeners(boundRef.current, boundListenersRef.current);
      boundRef.current = null;
      boundListenersRef.current = null;
    }

    if (el && boundRef.current !== el) {
      const listeners: Record<string, EventListenerObject> = {
        input: {
          handleEvent: (event: Event) => {
            propsRef.current.onInput?.(getCustomEventDetail<LxTextareaEventDetail>(event));
          }
        },
        change: {
          handleEvent: (event: Event) => {
            propsRef.current.onChange?.(getCustomEventDetail<LxTextareaEventDetail>(event));
          }
        },
        focus: {
          handleEvent: (event: Event) => {
            propsRef.current.onFocus?.(getCustomEventDetail<LxTextareaEventDetail>(event));
          }
        },
        blur: {
          handleEvent: (event: Event) => {
            propsRef.current.onBlur?.(getCustomEventDetail<LxTextareaEventDetail>(event));
          }
        },
        confirm: {
          handleEvent: (event: Event) => {
            propsRef.current.onConfirm?.(getCustomEventDetail<LxTextareaEventDetail>(event));
          }
        },
        linechange: {
          handleEvent: (event: Event) => {
            propsRef.current.onLineChange?.(getCustomEventDetail<LxTextareaEventDetail>(event));
          }
        },
        keyboardheightchange: {
          handleEvent: (event: Event) => {
            propsRef.current.onKeyboardHeightChange?.(getCustomEventDetail<LxTextareaEventDetail>(event));
          }
        },
      };
      bindEventListeners(el, listeners);
      boundRef.current = el;
      boundListenersRef.current = listeners;
      hasMountedRef.current = true;
      return;
    }

    if (!el) {
      hasMountedRef.current = false;
    }
  }, [ref]);

  // React custom-element prop forwarding can be inconsistent for generic keys.
  // Force attribute sync so native side always receives the expected values.
  React.useEffect(() => {
    const el = boundRef.current;
    if (!el) return;
    const setAttr = (name: string, next: string | null) => {
      if (next === null || next === "") {
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
    const syncNativeProps = (el as unknown as { syncNativeProps?: () => void }).syncNativeProps;
    if (didInitialNativeSyncRef.current && typeof syncNativeProps === "function") {
      syncNativeProps.call(el);
    } else {
      didInitialNativeSyncRef.current = true;
    }
  }, [value, focus, maxlength, placeholderStyle, autoHeight]);

  const textareaProps: Record<string, string> = {};
  if (typeof id === "string" && id.trim().length > 0) {
    textareaProps.id = id.trim();
  }
  if (value !== undefined) {
    textareaProps.value = value;
  } else if (!hasMountedRef.current && initialValueRef.current !== undefined) {
    textareaProps.value = initialValueRef.current;
  }
  if (placeholder) textareaProps.placeholder = placeholder;
  if (placeholderStyle) textareaProps["placeholder-style"] = placeholderStyle;
  if (placeholderClass) textareaProps["placeholder-class"] = placeholderClass;
  if (maxlength !== undefined) textareaProps.maxlength = String(maxlength);
  if (disabled) textareaProps.disabled = "true";
  if (autoFocus) textareaProps["auto-focus"] = "true";
  if (focus !== undefined) textareaProps.focus = focus ? "true" : "false";
  if (autoHeight) textareaProps["auto-height"] = "true";
  if (cursorSpacing !== undefined) textareaProps["cursor-spacing"] = String(cursorSpacing);
  if (showConfirmBar === false) textareaProps["show-confirm-bar"] = "false";
  if (adjustPosition === false) textareaProps["adjust-position"] = "false";
  if (holdKeyboard) textareaProps["hold-keyboard"] = "true";
  if (disableDefaultPadding) textareaProps["disable-default-padding"] = "true";
  if (confirmType) textareaProps["confirm-type"] = confirmType;
  if (confirmHold) textareaProps["confirm-hold"] = "true";
  if (fixed) textareaProps.fixed = "true";
  if (adjustKeyboardTo) textareaProps["adjust-keyboard-to"] = adjustKeyboardTo;
  if (cursor !== undefined) textareaProps.cursor = String(cursor);
  if (selectionStart !== undefined) textareaProps["selection-start"] = String(selectionStart);
  if (selectionEnd !== undefined) textareaProps["selection-end"] = String(selectionEnd);

  if (bindInput) textareaProps.bindinput = bindInput;
  if (bindChange) textareaProps.bindchange = bindChange;
  if (bindFocus) textareaProps.bindfocus = bindFocus;
  if (bindBlur) textareaProps.bindblur = bindBlur;
  if (bindConfirm) textareaProps.bindconfirm = bindConfirm;
  if (bindLineChange) textareaProps.bindlinechange = bindLineChange;
  if (bindKeyboardHeightChange) textareaProps.bindkeyboardheightchange = bindKeyboardHeightChange;
  if (catchInput) textareaProps.catchinput = catchInput;
  if (catchChange) textareaProps.catchchange = catchChange;
  if (catchFocus) textareaProps.catchfocus = catchFocus;
  if (catchBlur) textareaProps.catchblur = catchBlur;
  if (catchConfirm) textareaProps.catchconfirm = catchConfirm;
  if (catchLineChange) textareaProps.catchlinechange = catchLineChange;
  if (catchKeyboardHeightChange) textareaProps.catchkeyboardheightchange = catchKeyboardHeightChange;

  appendBindingAndDatasetAttrs(rest as Record<string, unknown>, textareaProps);

  return React.createElement("lx-textarea", {
    ref: textareaRefCallback,
    className,
    style,
    ...textareaProps,
  });
});

LxTextarea.displayName = "LxTextarea";
