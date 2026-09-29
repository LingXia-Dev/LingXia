import React, { forwardRef, useCallback, useEffect, useId, useMemo, useRef } from 'react';
import {
  buildMediaSwiperNativeAttrs,
  MEDIA_SWIPER_DOM_EVENT_MAP,
  registerMediaSwiperComponent,
  unwrapNativeEventPayload,
  type LxMediaSwiperAttributes,
  type LxMediaSwiperEventHandlers,
  type LxMediaSwiperEventPayloads,
  type LxMediaSwiperHandle,
} from '@lingxia/elements';
import {
  assignForwardedRef,
  bindElementEvents,
  pickDomEventHandlers,
  unbindElementEvents,
} from './text_component_shared.js';

export interface LxMediaSwiperProps
  extends Omit<LxMediaSwiperAttributes, keyof LxMediaSwiperEventPayloads | "ref">,
    LxMediaSwiperEventHandlers,
    Omit<
      React.HTMLAttributes<HTMLElement>,
      keyof LxMediaSwiperAttributes | "children" | "dangerouslySetInnerHTML" | "ref"
    > {}

/** The swiper element as a ref sees it. */
export type LxMediaSwiperRef = HTMLElement & LxMediaSwiperHandle;

if (typeof window !== "undefined") {
  registerMediaSwiperComponent();
}

export const LxMediaSwiper = forwardRef<LxMediaSwiperRef, LxMediaSwiperProps>(({
  id,
  items,
  index,
  initialIndex,
  loop,
  autoplay,
  interval,
  animation,
  animationDuration,
  direction,
  contentRotate,
  objectFit,
  controls,
  muted,
  dots,
  swipeEnabled,
  peek,
  onChange,
  onTransitionEnd,
  onEndReached,
  onTap,
  onVideoEnded,
  onError,
  className,
  style,
  ...rest
}, ref) => {
  const elementRef = useRef<HTMLElement | null>(null);
  const boundElementRef = useRef<HTMLElement | null>(null);
  const reactId = useId();
  const resolvedId = useMemo(() => {
    if (id) return id;
    return `lx-media-swiper-${reactId.replace(/[:]/g, "")}`;
  }, [id, reactId]);

  const handlerRef = useRef({
    onChange,
    onTransitionEnd,
    onEndReached,
    onTap,
    onVideoEnded,
    onError,
  });
  handlerRef.current = {
    onChange,
    onTransitionEnd,
    onEndReached,
    onTap,
    onVideoEnded,
    onError,
  };

  const listenerMapRef = useRef<Record<string, EventListenerObject>>(
    Object.fromEntries(
      Object.entries(MEDIA_SWIPER_DOM_EVENT_MAP).map(([propKey, eventName]) => [
        eventName,
        {
          handleEvent: (event: Event) => {
            const handler = handlerRef.current[propKey as keyof typeof handlerRef.current] as
              | ((payload: unknown) => void)
              | undefined;
            if (typeof handler === "function") {
              handler(unwrapNativeEventPayload(event));
            }
          },
        } satisfies EventListenerObject,
      ])
    )
  );

  const elementRefCallback = useCallback((element: HTMLElement | null) => {
    boundElementRef.current = bindElementEvents(boundElementRef.current, element, listenerMapRef.current);
    elementRef.current = element;
    assignForwardedRef(ref, element as LxMediaSwiperRef | null);
  }, [ref]);

  useEffect(() => () => {
    unbindElementEvents(boundElementRef.current, listenerMapRef.current);
    boundElementRef.current = null;
    elementRef.current = null;
  }, []);

  const domProps = buildMediaSwiperNativeAttrs({
    id: resolvedId,
    items,
    index,
    initialIndex,
    loop,
    autoplay,
    interval,
    animation,
    animationDuration,
    direction,
    contentRotate,
    objectFit,
    controls,
    muted,
    dots,
    swipeEnabled,
    peek,
  }, rest as Record<string, unknown>);

  return React.createElement('lx-media-swiper', {
    ...pickDomEventHandlers(rest as Record<string, unknown>),
    ref: elementRefCallback,
    className,
    style,
    ...domProps,
  });
});

LxMediaSwiper.displayName = 'LxMediaSwiper';
