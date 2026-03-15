import React, { forwardRef, useCallback, useEffect, useRef } from "react";
import { registerInputComponent } from "../input.js";
import {
  appendBindingAndDatasetAttrs,
  bindEventListeners,
  getCustomEventDetail,
  unbindEventListeners,
} from "./text_component_shared.js";

export interface LxInputEventDetail {
  value?: string;
  cursor?: number;
  keyCode?: number;
  height?: number;
  duration?: number;
  encryptedValue?: string;
  encryptError?: string;
  pass?: boolean;
  timeout?: boolean;
}

export interface LxInputProps {
  id?: string;
  value?: string;
  defaultValue?: string;
  type?: "text" | "number" | "password" | "digit";
  password?: boolean;
  placeholder?: string;
  placeholderStyle?: string;
  placeholderClass?: string;
  maxlength?: number;
  cursorSpacing?: number;
  autoFocus?: boolean;
  disabled?: boolean;
  focus?: boolean;
  confirmType?: "send" | "search" | "next" | "go" | "done";
  alwaysEmbed?: boolean;
  confirmHold?: boolean;
  cursor?: number;
  cursorColor?: string;
  selectionStart?: number;
  selectionEnd?: number;
  adjustPosition?: boolean;
  holdKeyboard?: boolean;

  onInput?: (detail: LxInputEventDetail) => void;
  onChange?: (detail: LxInputEventDetail) => void;
  onFocus?: (detail: LxInputEventDetail) => void;
  onBlur?: (detail: LxInputEventDetail) => void;
  onConfirm?: (detail: LxInputEventDetail) => void;
  onKeyboardHeightChange?: (detail: LxInputEventDetail) => void;
  onNicknameReview?: (detail: LxInputEventDetail) => void;

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

  className?: string;
  style?: React.CSSProperties;
}

if (typeof window !== "undefined") {
  registerInputComponent();
}

export const LxInput = forwardRef<HTMLElement, LxInputProps>(({
  value,
  defaultValue,
  type,
  password,
  placeholder,
  placeholderStyle,
  placeholderClass,
  maxlength,
  cursorSpacing,
  autoFocus,
  disabled,
  focus,
  confirmType,
  alwaysEmbed,
  confirmHold,
  cursor,
  cursorColor,
  selectionStart,
  selectionEnd,
  adjustPosition,
  holdKeyboard,
  onInput,
  onChange,
  onFocus,
  onBlur,
  onConfirm,
  onKeyboardHeightChange,
  onNicknameReview,
  bindInput,
  bindChange,
  bindFocus,
  bindBlur,
  bindConfirm,
  bindKeyboardHeightChange,
  bindNicknameReview,
  catchInput,
  catchChange,
  catchFocus,
  catchBlur,
  catchConfirm,
  catchKeyboardHeightChange,
  catchNicknameReview,
  className,
  style,
  id,
  ...rest
}, ref) => {
  const hasMountedRef = useRef(false);
  const initialValueRef = useRef(defaultValue);
  const didInitialNativeSyncRef = useRef(false);

  const propsRef = useRef({
    onInput,
    onChange,
    onFocus,
    onBlur,
    onConfirm,
    onKeyboardHeightChange,
    onNicknameReview,
  });
  propsRef.current = {
    onInput,
    onChange,
    onFocus,
    onBlur,
    onConfirm,
    onKeyboardHeightChange,
    onNicknameReview,
  };

  const boundRef = useRef<HTMLElement | null>(null);
  const boundListenersRef = useRef<Record<string, EventListenerObject> | null>(null);

  const inputRefCallback = useCallback((el: HTMLElement | null) => {
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
            propsRef.current.onInput?.(getCustomEventDetail<LxInputEventDetail>(event));
          }
        },
        change: {
          handleEvent: (event: Event) => {
            propsRef.current.onChange?.(getCustomEventDetail<LxInputEventDetail>(event));
          }
        },
        focus: {
          handleEvent: (event: Event) => {
            propsRef.current.onFocus?.(getCustomEventDetail<LxInputEventDetail>(event));
          }
        },
        blur: {
          handleEvent: (event: Event) => {
            propsRef.current.onBlur?.(getCustomEventDetail<LxInputEventDetail>(event));
          }
        },
        confirm: {
          handleEvent: (event: Event) => {
            propsRef.current.onConfirm?.(getCustomEventDetail<LxInputEventDetail>(event));
          }
        },
        keyboardheightchange: {
          handleEvent: (event: Event) => {
            propsRef.current.onKeyboardHeightChange?.(getCustomEventDetail<LxInputEventDetail>(event));
          }
        },
        nicknamereview: {
          handleEvent: (event: Event) => {
            propsRef.current.onNicknameReview?.(getCustomEventDetail<LxInputEventDetail>(event));
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
  useEffect(() => {
    const el = boundRef.current;
    if (!el) return;
    const setAttr = (name: string, next: string | null) => {
      if (next === null || next === "") {
        el.removeAttribute(name);
        return;
      }
      el.setAttribute(name, next);
    };

    if (type && String(type).trim().length > 0) {
      el.setAttribute("type", String(type).trim().toLowerCase());
    } else {
      el.removeAttribute("type");
    }
    if (password) {
      el.setAttribute("password", "true");
    } else {
      el.removeAttribute("password");
    }
    setAttr("value", value !== undefined ? String(value) : null);
    setAttr("focus", focus !== undefined ? (focus ? "true" : "false") : null);
    setAttr("maxlength", maxlength !== undefined ? String(maxlength) : null);
    setAttr("placeholder-style", placeholderStyle ? String(placeholderStyle) : null);
    const syncNativeProps = (el as unknown as { syncNativeProps?: () => void }).syncNativeProps;
    if (didInitialNativeSyncRef.current && typeof syncNativeProps === "function") {
      syncNativeProps.call(el);
    } else {
      didInitialNativeSyncRef.current = true;
    }
  }, [type, password, value, focus, maxlength, placeholderStyle]);

  const inputProps: Record<string, string> = {};
  if (typeof id === "string" && id.trim().length > 0) {
    inputProps.id = id.trim();
  }
  if (value !== undefined) {
    inputProps.value = value;
  } else if (!hasMountedRef.current && initialValueRef.current !== undefined) {
    inputProps.value = initialValueRef.current;
  }
  if (type) inputProps.type = type;
  if (password) inputProps.password = "true";
  if (placeholder) inputProps.placeholder = placeholder;
  if (placeholderStyle) inputProps["placeholder-style"] = placeholderStyle;
  if (placeholderClass) inputProps["placeholder-class"] = placeholderClass;
  if (maxlength !== undefined) inputProps.maxlength = String(maxlength);
  if (cursorSpacing !== undefined) inputProps["cursor-spacing"] = String(cursorSpacing);
  if (autoFocus) inputProps["auto-focus"] = "true";
  if (disabled) inputProps.disabled = "true";
  if (focus !== undefined) inputProps.focus = focus ? "true" : "false";
  if (confirmType) inputProps["confirm-type"] = confirmType;
  if (alwaysEmbed) inputProps["always-embed"] = "true";
  if (confirmHold) inputProps["confirm-hold"] = "true";
  if (cursor !== undefined) inputProps.cursor = String(cursor);
  if (cursorColor) inputProps["cursor-color"] = cursorColor;
  if (selectionStart !== undefined) inputProps["selection-start"] = String(selectionStart);
  if (selectionEnd !== undefined) inputProps["selection-end"] = String(selectionEnd);
  if (adjustPosition === false) inputProps["adjust-position"] = "false";
  if (holdKeyboard) inputProps["hold-keyboard"] = "true";

  if (bindInput) inputProps.bindinput = bindInput;
  if (bindChange) inputProps.bindchange = bindChange;
  if (bindFocus) inputProps.bindfocus = bindFocus;
  if (bindBlur) inputProps.bindblur = bindBlur;
  if (bindConfirm) inputProps.bindconfirm = bindConfirm;
  if (bindKeyboardHeightChange) inputProps.bindkeyboardheightchange = bindKeyboardHeightChange;
  if (bindNicknameReview) inputProps.bindnicknamereview = bindNicknameReview;
  if (catchInput) inputProps.catchinput = catchInput;
  if (catchChange) inputProps.catchchange = catchChange;
  if (catchFocus) inputProps.catchfocus = catchFocus;
  if (catchBlur) inputProps.catchblur = catchBlur;
  if (catchConfirm) inputProps.catchconfirm = catchConfirm;
  if (catchKeyboardHeightChange) inputProps.catchkeyboardheightchange = catchKeyboardHeightChange;
  if (catchNicknameReview) inputProps.catchnicknamereview = catchNicknameReview;

  appendBindingAndDatasetAttrs(rest as Record<string, unknown>, inputProps);

  return React.createElement("lx-input", {
    ref: inputRefCallback,
    className,
    style,
    ...inputProps,
  });
});

LxInput.displayName = "LxInput";
