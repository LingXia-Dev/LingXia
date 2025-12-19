import React, { forwardRef, useEffect, useId, useMemo, useRef } from 'react';
import { registerVideoComponent, type LxVideoAttributes } from '../video.js';

export interface LxVideoProps
  extends LxVideoAttributes,
    Omit<React.HTMLAttributes<HTMLElement>, keyof LxVideoAttributes | "children" | "dangerouslySetInnerHTML" | "ref"> {}

// Ensure the custom element is registered exactly once when running in a browser
if (typeof window !== "undefined") {
  registerVideoComponent();
}

export const LxVideo = forwardRef<HTMLElement, LxVideoProps>((props, ref) => {
  const innerRef = useRef<HTMLElement>(null);
  const handlerRef = useRef<Map<string, EventListenerOrEventListenerObject>>(new Map());
  const reactId = useId();
  const resolvedId = useMemo(() => {
    if (props.id) return props.id;
    return `lx-video-${reactId.replace(/[:]/g, "")}`;
  }, [props.id, reactId]);
  const combinedRef = (node: HTMLElement | null) => {
    innerRef.current = node;
    if (!ref) return;
    if (typeof ref === "function") {
      ref(node);
    } else {
      (ref as React.MutableRefObject<HTMLElement | null>).current = node;
    }
  };

  // Handle events manually to bypass React synthetic event issues with Custom Elements
  useEffect(() => {
    const el = innerRef.current;
    if (!el) return;

    const prev = handlerRef.current;
    const next = new Map<string, EventListenerOrEventListenerObject>();

    for (const [key, value] of Object.entries(props)) {
      if (!key.startsWith("on") || typeof value !== "function") continue;
      const eventName = key.substring(2).toLowerCase();
      next.set(eventName, value);
      const prevHandler = prev.get(eventName);
      if (prevHandler) el.removeEventListener(eventName, prevHandler);
      el.addEventListener(eventName, value);
    }

    for (const [eventName, handler] of prev.entries()) {
      if (!next.has(eventName)) {
        el.removeEventListener(eventName, handler);
      }
    }

    handlerRef.current = next;
    return () => {
      for (const [eventName, handler] of next.entries()) {
        el.removeEventListener(eventName, handler);
      }
    };
  }, [props]);

  // Filter out event props and React-only props before passing to the custom element
  const domProps: Record<string, any> = {};
  for (const [key, value] of Object.entries(props)) {
    if (key.startsWith("on") && typeof value === "function") continue;
    if (key === "children" || key === "dangerouslySetInnerHTML" || key === "ref") continue;

    let attrName = key;
    if (key === "playbackRates") attrName = "playback-rates";
    if (key === "progressBar") {
      if (value === false) {
        domProps["progress-bar"] = "false";
      }
      continue;
    }

    domProps[attrName] = (key === "qualities" || key === "playbackRates") && Array.isArray(value)
      ? JSON.stringify(value)
      : value;
  }
  domProps.id = resolvedId;

  // @ts-ignore - Custom element
  return React.createElement('lx-video', {
    ref: combinedRef,
    ...domProps
  });
});

LxVideo.displayName = 'LxVideo';
