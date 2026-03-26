import React, { forwardRef, useCallback, useEffect, useRef } from "react";
import { registerInputComponent } from "@lingxia/elements";
import { buildInputNativeAttrs } from "@lingxia/elements";
import {
  assignForwardedRef,
  bindElementEvents,
  getCustomEventDetail,
  unbindElementEvents,
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

  // Logic bindings (CLI-generated)
  pageBindings?: Record<string, string>;

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
  pageBindings,
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
    onKeyboardHeightChange,
    onNicknameReview,
  });
  handlerRef.current = {
    onInput,
    onChange,
    onFocus,
    onBlur,
    onConfirm,
    onKeyboardHeightChange,
    onNicknameReview,
  };
  const listenerMapRef = useRef<Record<string, EventListenerObject>>({
    input: {
      handleEvent: (event: Event) => handlerRef.current.onInput?.(getCustomEventDetail<LxInputEventDetail>(event)),
    },
    change: {
      handleEvent: (event: Event) => handlerRef.current.onChange?.(getCustomEventDetail<LxInputEventDetail>(event)),
    },
    focus: {
      handleEvent: (event: Event) => handlerRef.current.onFocus?.(getCustomEventDetail<LxInputEventDetail>(event)),
    },
    blur: {
      handleEvent: (event: Event) => handlerRef.current.onBlur?.(getCustomEventDetail<LxInputEventDetail>(event)),
    },
    confirm: {
      handleEvent: (event: Event) => handlerRef.current.onConfirm?.(getCustomEventDetail<LxInputEventDetail>(event)),
    },
    keyboardheightchange: {
      handleEvent: (event: Event) => handlerRef.current.onKeyboardHeightChange?.(getCustomEventDetail<LxInputEventDetail>(event)),
    },
    nicknamereview: {
      handleEvent: (event: Event) => handlerRef.current.onNicknameReview?.(getCustomEventDetail<LxInputEventDetail>(event)),
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

  useEffect(() => {
    const el = elementRef.current;
    if (!el) return;
    const setAttr = (name: string, next: string | null) => {
      if (next === null) {
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
  }, [type, password, value, focus, maxlength, placeholderStyle]);

  // Set pageBindings property on custom element
  useEffect(() => {
    const el = elementRef.current as any;
    if (el && pageBindings) {
      el.pageBindings = pageBindings;
    }
  }, [pageBindings]);

  const inputProps = buildInputNativeAttrs({
    id,
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
  }, rest as Record<string, unknown>, boundElementRef.current !== null);

  return React.createElement("lx-input", {
    ref: elementRefCallback,
    className,
    style,
    ...inputProps,
  });
});

LxInput.displayName = "LxInput";
