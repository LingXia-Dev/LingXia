import React, { forwardRef, useCallback, useEffect, useId, useMemo, useRef } from 'react';
import {
  buildMediaSwiperNativeAttrs,
  MEDIA_SWIPER_DOM_EVENT_MAP,
  registerMediaSwiperComponent,
  type LxMediaSwiperAttributes,
} from '@lingxia/elements';
import {
  assignForwardedRef,
  bindElementEvents,
  unbindElementEvents,
} from './text_component_shared.js';

export interface LxMediaSwiperProps
  extends LxMediaSwiperAttributes,
    Omit<
      React.HTMLAttributes<HTMLElement>,
      keyof LxMediaSwiperAttributes | "children" | "dangerouslySetInnerHTML" | "ref" | "onChange" | "onTransitionEnd" | "onError"
    > {}

if (typeof window !== "undefined") {
  registerMediaSwiperComponent();
}

export const LxMediaSwiper = forwardRef<HTMLElement, LxMediaSwiperProps>(({
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
  pageBindings,
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
    const el = elementRef.current as any;
    if (el) {
      el.pageBindings = pageBindings ?? {};
    }
  }, [pageBindings]);

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
    ref: elementRefCallback,
    className,
    style,
    ...domProps,
  });
});

LxMediaSwiper.displayName = 'LxMediaSwiper';
