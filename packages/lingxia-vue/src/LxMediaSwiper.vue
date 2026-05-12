<script setup lang="ts">
import { computed, h, onBeforeUnmount, ref, useAttrs, useId, watch } from 'vue';
import {
  buildMediaSwiperNativeAttrs,
  MEDIA_SWIPER_DOM_EVENT_MAP,
  registerMediaSwiperComponent,
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
  change: [e: Event];
  transitionEnd: [e: Event];
  endReached: [e: Event];
  tap: [e: Event];
  videoEnded: [e: Event];
  error: [e: Event];
}>();

if (typeof window !== 'undefined') {
  registerMediaSwiperComponent();
}

const elementRef = ref<HTMLElement | null>(null);
const vueId = useId();
let boundElement: HTMLElement | null = null;

const resolvedId = computed(() => props.id || `lx-media-swiper-${vueId.replace(/[:]/g, '')}`);

const eventListeners: Record<string, EventListenerObject> = {
  [MEDIA_SWIPER_DOM_EVENT_MAP.onChange]: { handleEvent: (event: Event) => emit('change', event) },
  [MEDIA_SWIPER_DOM_EVENT_MAP.onTransitionEnd]: { handleEvent: (event: Event) => emit('transitionEnd', event) },
  [MEDIA_SWIPER_DOM_EVENT_MAP.onEndReached]: { handleEvent: (event: Event) => emit('endReached', event) },
  [MEDIA_SWIPER_DOM_EVENT_MAP.onTap]: { handleEvent: (event: Event) => emit('tap', event) },
  [MEDIA_SWIPER_DOM_EVENT_MAP.onVideoEnded]: { handleEvent: (event: Event) => emit('videoEnded', event) },
  [MEDIA_SWIPER_DOM_EVENT_MAP.onError]: { handleEvent: (event: Event) => emit('error', event) },
};

watch(elementRef, (element) => {
  boundElement = bindElementEvents(boundElement, element, eventListeners);
});

onBeforeUnmount(() => {
  unbindElementEvents(boundElement, eventListeners);
});

watch(
  [elementRef, () => props.pageBindings],
  () => {
    const el = elementRef.value as any;
    if (el) {
      el.pageBindings = props.pageBindings ?? {};
    }
  },
  { immediate: true }
);

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

defineExpose({
  el: elementRef,
  next: () => (elementRef.value as any)?.next?.(),
  previous: () => (elementRef.value as any)?.previous?.(),
  goToIndex: (index: number) => (elementRef.value as any)?.goToIndex?.(index),
});

const render = () => h('lx-media-swiper', { ref: elementRef, ...domProps.value });
</script>

<template>
  <render />
</template>
