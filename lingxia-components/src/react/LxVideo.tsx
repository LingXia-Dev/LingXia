import React, { forwardRef, useCallback, useEffect, useId, useMemo, useRef } from 'react';
import { registerVideoComponent, type LxVideoAttributes } from '../video.js';
import {
  buildVideoNativeAttrs,
  VIDEO_DOM_EVENT_MAP,
} from '../native_component_wrapper_shared.js';
import {
  assignForwardedRef,
  bindElementEvents,
  unbindElementEvents,
} from './text_component_shared.js';

export interface LxVideoProps
  extends LxVideoAttributes,
    Omit<
      React.HTMLAttributes<HTMLElement>,
      keyof LxVideoAttributes | "children" | "dangerouslySetInnerHTML" | "ref" | "onPlaying"
    > {}

if (typeof window !== "undefined") {
  registerVideoComponent();
}
export const LxVideo = forwardRef<HTMLElement, LxVideoProps>(({
  id,
  src,
  poster,
  objectFit,
  rotate,
  autoplay,
  loop,
  muted,
  controls,
  progressBar,
  live,
  volume,
  qualities,
  playbackRates,
  onPlayRequest,
  onPlay,
  onPlaying,
  onPause,
  onStop,
  onEnded,
  onTimeUpdate,
  onError,
  onLoadedMetadata,
  onFullscreenChange,
  onWaiting,
  onQualityChange,
  onRateChange,
  bindPlayRequest,
  bindPlay,
  bindPlaying,
  bindPause,
  bindStop,
  bindEnded,
  bindTimeUpdate,
  bindError,
  bindLoadedMetadata,
  bindFullscreenChange,
  bindWaiting,
  bindQualityChange,
  bindRateChange,
  catchPlayRequest,
  catchPlay,
  catchPlaying,
  catchPause,
  catchStop,
  catchEnded,
  catchTimeUpdate,
  catchError,
  catchLoadedMetadata,
  catchFullscreenChange,
  catchWaiting,
  catchQualityChange,
  catchRateChange,
  className,
  style,
  ...rest
}, ref) => {
  const elementRef = useRef<HTMLElement | null>(null);
  const boundElementRef = useRef<HTMLElement | null>(null);
  const reactId = useId();
  const resolvedId = useMemo(() => {
    if (id) return id;
    return `lx-video-${reactId.replace(/[:]/g, "")}`;
  }, [id, reactId]);
  const handlerRef = useRef({
    onPlayRequest,
    onPlay,
    onPlaying,
    onPause,
    onStop,
    onEnded,
    onTimeUpdate,
    onError,
    onLoadedMetadata,
    onFullscreenChange,
    onWaiting,
    onQualityChange,
    onRateChange,
  });
  handlerRef.current = {
    onPlayRequest,
    onPlay,
    onPlaying,
    onPause,
    onStop,
    onEnded,
    onTimeUpdate,
    onError,
    onLoadedMetadata,
    onFullscreenChange,
    onWaiting,
    onQualityChange,
    onRateChange,
  };
  const listenerMapRef = useRef<Record<string, EventListenerObject>>(
    Object.fromEntries(
      Object.entries(VIDEO_DOM_EVENT_MAP).map(([propKey, eventName]) => [
        eventName,
        {
          handleEvent: (event: Event) => {
            const handler = handlerRef.current[propKey as keyof typeof handlerRef.current];
            if (typeof handler === "function") {
              handler(event);
            }
          },
        } satisfies EventListenerObject,
      ])
    )
  );
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
    if (rotate === undefined || rotate === null) {
      el.removeAttribute("rotate");
    } else {
      el.setAttribute("rotate", String(rotate).trim());
    }
    if (objectFit === undefined || objectFit === null) {
      el.removeAttribute("object-fit");
    } else {
      el.setAttribute("object-fit", String(objectFit).trim().toLowerCase());
    }
  }, [rotate, objectFit]);

  const domProps = buildVideoNativeAttrs({
    id: resolvedId,
    src,
    poster,
    autoplay,
    loop,
    muted,
    controls,
    progressBar,
    live,
    volume,
    qualities,
    playbackRates,
    bindPlayRequest,
    bindPlay,
    bindPlaying,
    bindPause,
    bindStop,
    bindEnded,
    bindTimeUpdate,
    bindError,
    bindLoadedMetadata,
    bindFullscreenChange,
    bindWaiting,
    bindQualityChange,
    bindRateChange,
    catchPlayRequest,
    catchPlay,
    catchPlaying,
    catchPause,
    catchStop,
    catchEnded,
    catchTimeUpdate,
    catchError,
    catchLoadedMetadata,
    catchFullscreenChange,
    catchWaiting,
    catchQualityChange,
    catchRateChange,
  }, rest as Record<string, unknown>);

  return React.createElement('lx-video', {
    ref: elementRefCallback,
    className,
    style,
    ...domProps
  });
});

LxVideo.displayName = 'LxVideo';
