import React, { forwardRef, useCallback, useEffect, useRef } from "react";
import { registerNativeViewComponent } from "@lingxia/elements";
import {
  assignNativeRef,
  applyNativeAria,
  setOptionalAttribute,
  type LxNativeNodeProps,
} from "./shared.js";

if (typeof window !== "undefined") {
  registerNativeViewComponent();
}

export interface LxNativeViewProps extends LxNativeNodeProps {
  role?: "group" | "region" | "status" | "presentation" | "none";
}

export const LxNativeView = forwardRef<HTMLElement, LxNativeViewProps>(
  (
    {
      id,
      automationId,
      className,
      style,
      pointerEvents,
      hidden,
      hiddenTransition,
      role,
      children,
      ...aria
    },
    ref
  ) => {
    const elementRef = useRef<HTMLElement | null>(null);
    const setRef = useCallback(
      (element: HTMLElement | null) => {
        elementRef.current = element;
        assignNativeRef(ref, element);
      },
      [ref]
    );

    useEffect(() => {
      const el = elementRef.current;
      if (!el) return;
      setOptionalAttribute(el, "automation-id", automationId);
      setOptionalAttribute(el, "pointer-events", pointerEvents);
      setOptionalAttribute(el, "hidden-transition", hiddenTransition);
      setOptionalAttribute(el, "role", role);
      applyNativeAria(el, aria);
    }, [automationId, pointerEvents, hiddenTransition, role, aria]);

    return React.createElement(
      "lx-native-view",
      { ref: setRef, id, className, style, hidden },
      children
    );
  }
);

LxNativeView.displayName = "LxNativeView";
