import React, { forwardRef, useCallback, useEffect, useRef } from "react";
import { registerNativeTextComponent } from "@lingxia/elements";
import {
  assignNativeRef,
  setOptionalAttribute,
  type LxNativeNodeProps,
} from "./shared.js";

if (typeof window !== "undefined") {
  registerNativeTextComponent();
}

export interface LxNativeTextProps extends Omit<LxNativeNodeProps, "children"> {
  maxLines?: number;
  dir?: "ltr" | "rtl" | "auto";
  fontSize?: number | string;
  fontWeight?: number | string;
  lineHeight?: number | string;
  textAlign?: "start" | "center" | "end";
  color?: string;
  children?: string | number;
}

export const LxNativeText = forwardRef<HTMLElement, LxNativeTextProps>(
  (
    {
      id,
      automationId,
      className,
      style,
      pointerEvents,
      hidden,
      hiddenTransition,
      maxLines,
      dir,
      fontSize,
      fontWeight,
      lineHeight,
      textAlign,
      color,
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
      setOptionalAttribute(el, "max-lines", maxLines);
      setOptionalAttribute(el, "dir", dir);
      setOptionalAttribute(el, "font-size", fontSize);
      setOptionalAttribute(el, "font-weight", fontWeight);
      setOptionalAttribute(el, "line-height", lineHeight);
      setOptionalAttribute(el, "text-align", textAlign);
      setOptionalAttribute(el, "color", color);
      setOptionalAttribute(el, "aria-label", aria["aria-label"]);
    }, [
      automationId,
      pointerEvents,
      hiddenTransition,
      maxLines,
      dir,
      fontSize,
      fontWeight,
      lineHeight,
      textAlign,
      color,
      aria,
    ]);

    return React.createElement(
      "lx-native-text",
      { ref: setRef, id, className, style, hidden },
      children
    );
  }
);

LxNativeText.displayName = "LxNativeText";
