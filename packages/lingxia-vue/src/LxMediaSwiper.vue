<script setup lang="ts">
import { computed, h, onBeforeUnmount, ref, useAttrs, useId, watch } from 'vue';
import {
  buildMediaSwiperNativeAttrs,
  MEDIA_SWIPER_DOM_EVENT_MAP,
  registerMediaSwiperComponent,
  unwrapNativeEventPayload,
  type LxMediaSwiperEventPayloads as Payloads,
  type LxMediaSwiperHandle,
} from '@lingxia/elements';
import { bindElementEvents, unbindElementEvents } from './text_component_shared.js';
import type { LxMediaSwiperProps } from './types.js';

const props = withDefaults(defineProps<LxMediaSwiperProps>(), {
  loop: false,
  autoplay: false,
  interval: 5000,
  animation: 'slide',
  direction: 'horizontal',
  objectFit: 'cover',
  controls: false,
  muted: true,
  dots: false,
  swipeEnabled: true,
});
const attrs = useAttrs();

const emit = defineEmits<{
  change: [payload: Payloads['onChange']];
  transitionEnd: [payload: Payloads['onTransitionEnd']];
  endReached: [payload: Payloads['onEndReached']];
  tap: [payload: Payloads['onTap']];
  videoEnded: [payload: Payloads['onVideoEnded']];
  error: [payload: Payloads['onError']];
}>();

if (typeof window !== 'undefined') {
  registerMediaSwiperComponent();
}

const elementRef = ref<(HTMLElement & LxMediaSwiperHandle) | null>(null);
const vueId = useId();
let boundElement: HTMLElement | null = null;

const resolvedId = computed(() => props.id || `lx-media-swiper-${vueId.replace(/[:]/g, '')}`);

const eventListeners: Record<string, EventListenerObject> = {
  [MEDIA_SWIPER_DOM_EVENT_MAP.onChange]: { handleEvent: (event: Event) => emit('change', unwrapNativeEventPayload(event)) },
  [MEDIA_SWIPER_DOM_EVENT_MAP.onTransitionEnd]: { handleEvent: (event: Event) => emit('transitionEnd', unwrapNativeEventPayload(event)) },
  [MEDIA_SWIPER_DOM_EVENT_MAP.onEndReached]: { handleEvent: (event: Event) => emit('endReached', unwrapNativeEventPayload(event)) },
  [MEDIA_SWIPER_DOM_EVENT_MAP.onTap]: { handleEvent: (event: Event) => emit('tap', unwrapNativeEventPayload(event)) },
  [MEDIA_SWIPER_DOM_EVENT_MAP.onVideoEnded]: { handleEvent: (event: Event) => emit('videoEnded', unwrapNativeEventPayload(event)) },
  [MEDIA_SWIPER_DOM_EVENT_MAP.onError]: { handleEvent: (event: Event) => emit('error', unwrapNativeEventPayload(event)) },
};

watch(elementRef, (element) => {
  boundElement = bindElementEvents(boundElement, element, eventListeners);
});

onBeforeUnmount(() => {
  unbindElementEvents(boundElement, eventListeners);
});


const domProps = computed(() => {
  const result = buildMediaSwiperNativeAttrs({
    ...props,
    id: resolvedId.value,
  }, attrs as Record<string, unknown>);
  return {
    ...result,
    class: props.class ?? attrs.class,
    style: props.style ?? attrs.style,
  };
});

const handle: LxMediaSwiperHandle & { el: typeof elementRef } = {
  el: elementRef,
  next: () => elementRef.value?.next(),
  previous: () => elementRef.value?.previous(),
  goToIndex: (index: number) => elementRef.value?.goToIndex(index),
};
defineExpose(handle);

const render = () => h('lx-media-swiper', { ref: elementRef, ...domProps.value });
</script>

<template>
  <render />
</template>
