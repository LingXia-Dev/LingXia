import React, { forwardRef, useCallback, useEffect, useRef } from "react";
import {
  registerNativeSliderComponent,
  type NativeHandler,
  type ValuePayload,
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
  registerNativeSliderComponent();
}

export interface LxNativeSliderProps extends Omit<LxNativeNodeProps, "children"> {
  value?: number;
  min?: number;
  max?: number;
  step?: number;
  bufferedValue?: number;
  valueLabel?: "none" | "value" | "time";
  disabled?: boolean;
  tabIndex?: 0 | -1;
  onValueChange?: NativeHandler<ValuePayload>;
  onValueCommit?: NativeHandler<ValuePayload>;
  onFocus?: NativeHandler<FocusPayload>;
  onBlur?: NativeHandler<FocusPayload>;
  onPointerEnter?: NativeHandler<PointerPayload>;
  onPointerLeave?: NativeHandler<PointerPayload>;
}

export const LxNativeSlider = forwardRef<HTMLElement, LxNativeSliderProps>(
  (
    {
      id,
      automationId,
      className,
      style,
      pointerEvents,
      hidden,
      hiddenTransition,
      value,
      min,
      max,
      step,
      bufferedValue,
      valueLabel,
      disabled,
      tabIndex,
      onValueChange,
      onValueCommit,
      onFocus,
      onBlur,
      onPointerEnter,
      onPointerLeave,
      ...aria
    },
    ref
  ) => {
    const elementRef = useRef<HTMLElement | null>(null);
    const boundRef = useRef<HTMLElement | null>(null);
    const handlers = useRef({
      onValueChange,
      onValueCommit,
      onFocus,
      onBlur,
      onPointerEnter,
      onPointerLeave,
    });
    handlers.current = {
      onValueChange,
      onValueCommit,
      onFocus,
      onBlur,
      onPointerEnter,
      onPointerLeave,
    };
    const listeners = useRef({
      valuechange: payloadListener<ValuePayload>(() => handlers.current.onValueChange),
      valuecommit: payloadListener<ValuePayload>(() => handlers.current.onValueCommit),
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
      const el = elementRef.current;
      if (!el) return;
      setOptionalAttribute(el, "automation-id", automationId);
      setOptionalAttribute(el, "pointer-events", pointerEvents);
      setOptionalAttribute(el, "hidden-transition", hiddenTransition);
      setOptionalAttribute(el, "value", value);
      setOptionalAttribute(el, "min", min);
      setOptionalAttribute(el, "max", max);
      setOptionalAttribute(el, "step", step);
      setOptionalAttribute(el, "buffered-value", bufferedValue);
      setOptionalAttribute(el, "value-label", valueLabel);
      setOptionalAttribute(el, "disabled", disabled);
      applyNativeAria(el, aria);
      if (typeof tabIndex === "number") {
        el.tabIndex = tabIndex;
      }
    }, [
      automationId,
      pointerEvents,
      hiddenTransition,
      value,
      min,
      max,
      step,
      bufferedValue,
      valueLabel,
      disabled,
      tabIndex,
      aria,
    ]);

    return React.createElement("lx-native-slider", {
      ref: setRef,
      id,
      className,
      style,
      hidden,
    });
  }
);

LxNativeSlider.displayName = "LxNativeSlider";
