import React, { forwardRef, useCallback, useEffect, useRef } from "react";
import {
  registerNativeButtonComponent,
  type NativeActionIcon,
  type NativeHandler,
  type PressPayload,
  type FocusPayload,
  type PointerPayload,
} from "@lingxia/elements";
import {
  assignNativeRef,
  applyNativeAria,
  bindNativeEvents,
  payloadListener,
  setOptionalAttribute,
  type LxNativeNodeProps,
} from "./shared.js";

if (typeof window !== "undefined") {
  registerNativeButtonComponent();
}

export interface LxNativeButtonProps extends LxNativeNodeProps {
  label?: string;
  icon?: NativeActionIcon | { resource: unknown };
  iconPosition?: "start" | "end";
  intent?: "neutral" | "accent" | "destructive";
  emphasis?: "primary" | "secondary" | "quiet";
  size?: "compact" | "regular";
  hitSlop?: number;
  disabled?: boolean;
  pressed?: boolean;
  expanded?: boolean;
  loading?: boolean;
  tabIndex?: 0 | -1;
  onPress?: NativeHandler<PressPayload>;
  onFocus?: NativeHandler<FocusPayload>;
  onBlur?: NativeHandler<FocusPayload>;
  onPointerEnter?: NativeHandler<PointerPayload>;
  onPointerLeave?: NativeHandler<PointerPayload>;
}

export const LxNativeButton = forwardRef<HTMLElement, LxNativeButtonProps>(
  (
    {
      id,
      automationId,
      className,
      style,
      pointerEvents,
      hidden,
      hiddenTransition,
      label,
      icon,
      iconPosition,
      intent,
      emphasis,
      size,
      hitSlop,
      disabled,
      pressed,
      expanded,
      loading,
      tabIndex,
      onPress,
      onFocus,
      onBlur,
      onPointerEnter,
      onPointerLeave,
      children,
      ...aria
    },
    ref
  ) => {
    const elementRef = useRef<HTMLElement | null>(null);
    const boundRef = useRef<HTMLElement | null>(null);
    const handlers = useRef({ onPress, onFocus, onBlur, onPointerEnter, onPointerLeave });
    handlers.current = { onPress, onFocus, onBlur, onPointerEnter, onPointerLeave };
    const listeners = useRef({
      press: payloadListener<PressPayload>(() => handlers.current.onPress),
      focus: payloadListener<FocusPayload>(() => handlers.current.onFocus),
      blur: payloadListener<FocusPayload>(() => handlers.current.onBlur),
      pointerenter: payloadListener<PointerPayload>(() => handlers.current.onPointerEnter),
      pointerleave: payloadListener<PointerPayload>(() => handlers.current.onPointerLeave),
    });

    const setRef = useCallback(
      (element: HTMLElement | null) => {
        boundRef.current = bindNativeEvents(boundRef.current, element, listeners.current);
        elementRef.current = element;
        assignNativeRef(ref, element);
      },
      [ref]
    );

    useEffect(
      () => () => {
        bindNativeEvents(boundRef.current, null, listeners.current);
        boundRef.current = null;
      },
      []
    );

    useEffect(() => {
      const el = elementRef.current as (HTMLElement & { icon?: unknown }) | null;
      if (!el) return;
      setOptionalAttribute(el, "automation-id", automationId);
      setOptionalAttribute(el, "pointer-events", pointerEvents);
      setOptionalAttribute(el, "hidden-transition", hiddenTransition);
      setOptionalAttribute(el, "label", label);
      setOptionalAttribute(el, "icon-position", iconPosition);
      setOptionalAttribute(el, "intent", intent);
      setOptionalAttribute(el, "emphasis", emphasis);
      setOptionalAttribute(el, "size", size);
      setOptionalAttribute(el, "hit-slop", hitSlop);
      setOptionalAttribute(el, "disabled", disabled);
      setOptionalAttribute(el, "pressed", pressed);
      setOptionalAttribute(el, "expanded", expanded);
      setOptionalAttribute(el, "loading", loading);
      applyNativeAria(el, aria);
      if (typeof tabIndex === "number") {
        el.tabIndex = tabIndex;
      }
      if (icon && typeof icon === "object") {
        el.icon = icon;
      } else {
        setOptionalAttribute(el, "icon", icon);
      }
    }, [
      automationId,
      pointerEvents,
      hiddenTransition,
      label,
      icon,
      iconPosition,
      intent,
      emphasis,
      size,
      hitSlop,
      disabled,
      pressed,
      expanded,
      loading,
      tabIndex,
      aria,
    ]);

    return React.createElement(
      "lx-native-button",
      { ref: setRef, id, className, style, hidden },
      children
    );
  }
);

LxNativeButton.displayName = "LxNativeButton";
