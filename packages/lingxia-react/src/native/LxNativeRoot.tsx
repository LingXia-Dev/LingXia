import React, { forwardRef, useCallback, useEffect, useImperativeHandle, useRef } from "react";
import {
  registerNativeRootComponent,
  type NativeError,
  type NativeHandler,
} from "@lingxia/elements";
import {
  assignNativeRef,
  bindNativeEvents,
  payloadListener,
  setOptionalAttribute,
  type LxNativeNodeProps,
} from "./shared.js";

if (typeof window !== "undefined") {
  registerNativeRootComponent();
}

export interface LxNativeRootHandle {
  retry(): Promise<void>;
  getLayout(): { x: number; y: number; width: number; height: number };
}

export interface LxNativeRootProps extends LxNativeNodeProps {
  fullscreenScope?: "root" | "none";
  fallback?: React.ReactNode;
  onReady?: NativeHandler<Record<string, never>>;
  onError?: NativeHandler<NativeError>;
  onPointerWithinChange?: NativeHandler<{ within: boolean }>;
}

export const LxNativeRoot = forwardRef<LxNativeRootHandle, LxNativeRootProps>(
  (
    {
      id,
      automationId,
      className,
      style,
      pointerEvents,
      hidden,
      hiddenTransition,
      fullscreenScope = "root",
      fallback,
      onReady,
      onError,
      onPointerWithinChange,
      children,
      ...aria
    },
    ref
  ) => {
    const elementRef = useRef<HTMLElement | null>(null);
    const boundRef = useRef<HTMLElement | null>(null);
    const handlers = useRef({ onReady, onError, onPointerWithinChange });
    handlers.current = { onReady, onError, onPointerWithinChange };

    const listeners = useRef({
      ready: payloadListener<Record<string, never>>(() => handlers.current.onReady),
      error: payloadListener<NativeError>(() => handlers.current.onError),
      pointerwithinchange: payloadListener<{ within: boolean }>(
        () => handlers.current.onPointerWithinChange
      ),
    });

    useImperativeHandle(ref, () => ({
      retry: async () => {
        const el = elementRef.current as { retry?: () => Promise<void> } | null;
        await el?.retry?.();
      },
      getLayout: () => {
        const el = elementRef.current;
        if (!el) return { x: 0, y: 0, width: 0, height: 0 };
        const rect = el.getBoundingClientRect();
        return { x: rect.left, y: rect.top, width: rect.width, height: rect.height };
      },
    }));

    const setRef = useCallback((element: HTMLElement | null) => {
      boundRef.current = bindNativeEvents(boundRef.current, element, listeners.current);
      elementRef.current = element;
      assignNativeRef(ref, element as unknown as LxNativeRootHandle);
    }, [ref]);

    useEffect(
      () => () => {
        unbindSafe();
      },
      []
    );

    function unbindSafe(): void {
      if (boundRef.current) {
        bindNativeEvents(boundRef.current, null, listeners.current);
        boundRef.current = null;
      }
    }

    useEffect(() => {
      const el = elementRef.current;
      if (!el) return;
      setOptionalAttribute(el, "automation-id", automationId);
      setOptionalAttribute(el, "pointer-events", pointerEvents);
      setOptionalAttribute(el, "hidden-transition", hiddenTransition);
      setOptionalAttribute(el, "fullscreen-scope", fullscreenScope);
      setOptionalAttribute(el, "aria-label", aria["aria-label"]);
      setOptionalAttribute(el, "aria-description", aria["aria-description"]);
      if (aria["aria-hidden"] === undefined) {
        el.removeAttribute("aria-hidden");
      } else {
        el.setAttribute("aria-hidden", aria["aria-hidden"] ? "true" : "false");
      }
    }, [
      automationId,
      pointerEvents,
      hiddenTransition,
      fullscreenScope,
      aria["aria-label"],
      aria["aria-description"],
      aria["aria-hidden"],
    ]);

    return React.createElement(
      "lx-native-root",
      {
        ref: setRef,
        id,
        className,
        style,
        hidden,
      },
      children,
      fallback != null
        ? React.createElement(
            "div",
            { "data-lx-native-fallback": "", hidden: true, "aria-hidden": true },
            fallback
          )
        : null
    );
  }
);

LxNativeRoot.displayName = "LxNativeRoot";
