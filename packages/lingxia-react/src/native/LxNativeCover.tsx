import React, { forwardRef, useCallback, useEffect, useRef } from "react";
import { registerNativeCoverComponent } from "@lingxia/elements";
import {
  assignNativeRef,
  applyNativeAria,
  setOptionalAttribute,
  type LxNativeNodeProps,
} from "./shared.js";

if (typeof window !== "undefined") {
  registerNativeCoverComponent();
}

export interface LxNativeCoverProps extends LxNativeNodeProps {
  scrim?: "none" | "top" | "bottom" | "full";
  scrimOpacity?: number;
  role?: "group" | "region" | "status" | "presentation" | "none";
}

export const LxNativeCover = forwardRef<HTMLElement, LxNativeCoverProps>(
  (
    {
      id,
      automationId,
      className,
      style,
      pointerEvents,
      hidden,
      hiddenTransition,
      scrim,
      scrimOpacity,
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
      setOptionalAttribute(el, "scrim", scrim);
      setOptionalAttribute(el, "scrim-opacity", scrimOpacity);
      setOptionalAttribute(el, "role", role);
      applyNativeAria(el, aria);
    }, [automationId, pointerEvents, hiddenTransition, scrim, scrimOpacity, role, aria]);

    return React.createElement(
      "lx-native-cover",
      { ref: setRef, id, className, style, hidden },
      children
    );
  }
);

LxNativeCover.displayName = "LxNativeCover";
