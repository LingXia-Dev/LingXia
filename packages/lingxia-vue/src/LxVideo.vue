<script setup lang="ts">
import { ref, computed, h, onBeforeUnmount, useAttrs, useId, watch } from 'vue';
import { registerVideoComponent, unwrapNativeEventPayload } from '@lingxia/elements';
import {
  buildVideoNativeAttrs,
  VIDEO_DOM_EVENT_MAP,
} from '@lingxia/elements';
import { bindElementEvents, unbindElementEvents } from './text_component_shared.js';
import type { LxVideoProps } from './types.js';

const props = withDefaults(defineProps<LxVideoProps>(), {
  autoplay: false,
  loop: false,
  muted: false,
  controls: true,
  progressBar: true,
  live: false,
});
const attrs = useAttrs();

const emit = defineEmits<{
  playRequest: [payload: unknown];
  play: [payload: unknown];
  playing: [payload: unknown];
  pause: [payload: unknown];
  stop: [payload: unknown];
  ended: [payload: unknown];
  timeUpdate: [payload: unknown];
  error: [payload: unknown];
  loadedMetadata: [payload: unknown];
  fullscreenChange: [payload: unknown];
  waiting: [payload: unknown];
  qualityChange: [payload: unknown];
  rateChange: [payload: unknown];
}>();

if (typeof window !== 'undefined') {
  registerVideoComponent();
}

const elementRef = ref<HTMLElement | null>(null);
const vueId = useId();
let boundElement: HTMLElement | null = null;

const resolvedId = computed(() => props.id || `lx-video-${vueId.replace(/[:]/g, '')}`);

const videoEventListeners: Record<string, EventListenerObject> = {
  [VIDEO_DOM_EVENT_MAP.onPlayRequest]: { handleEvent: (event: Event) => emit('playRequest', unwrapNativeEventPayload(event)) },
  [VIDEO_DOM_EVENT_MAP.onPlay]: { handleEvent: (event: Event) => emit('play', unwrapNativeEventPayload(event)) },
  [VIDEO_DOM_EVENT_MAP.onPlaying]: { handleEvent: (event: Event) => emit('playing', unwrapNativeEventPayload(event)) },
  [VIDEO_DOM_EVENT_MAP.onPause]: { handleEvent: (event: Event) => emit('pause', unwrapNativeEventPayload(event)) },
  [VIDEO_DOM_EVENT_MAP.onStop]: { handleEvent: (event: Event) => emit('stop', unwrapNativeEventPayload(event)) },
  [VIDEO_DOM_EVENT_MAP.onEnded]: { handleEvent: (event: Event) => emit('ended', unwrapNativeEventPayload(event)) },
  [VIDEO_DOM_EVENT_MAP.onTimeUpdate]: { handleEvent: (event: Event) => emit('timeUpdate', unwrapNativeEventPayload(event)) },
  [VIDEO_DOM_EVENT_MAP.onError]: { handleEvent: (event: Event) => emit('error', unwrapNativeEventPayload(event)) },
  [VIDEO_DOM_EVENT_MAP.onLoadedMetadata]: { handleEvent: (event: Event) => emit('loadedMetadata', unwrapNativeEventPayload(event)) },
  [VIDEO_DOM_EVENT_MAP.onFullscreenChange]: { handleEvent: (event: Event) => emit('fullscreenChange', unwrapNativeEventPayload(event)) },
  [VIDEO_DOM_EVENT_MAP.onWaiting]: { handleEvent: (event: Event) => emit('waiting', unwrapNativeEventPayload(event)) },
  [VIDEO_DOM_EVENT_MAP.onQualityChange]: { handleEvent: (event: Event) => emit('qualityChange', unwrapNativeEventPayload(event)) },
  [VIDEO_DOM_EVENT_MAP.onRateChange]: { handleEvent: (event: Event) => emit('rateChange', unwrapNativeEventPayload(event)) },
};

watch(elementRef, (element) => {
  boundElement = bindElementEvents(boundElement, element, videoEventListeners);
});

onBeforeUnmount(() => {
  unbindElementEvents(boundElement, videoEventListeners);
});

watch(
  [elementRef, () => props.contentRotate, () => props.objectFit],
  () => {
    const el = elementRef.value;
    if (!el) return;
    const contentRotate = props.contentRotate;
    if (contentRotate === undefined || contentRotate === null || contentRotate === '') {
      el.removeAttribute('content-rotate');
    } else {
      el.setAttribute('content-rotate', String(contentRotate).trim());
    }
    const objectFit = props.objectFit;
    if (objectFit === undefined || objectFit === null || objectFit === '') {
      el.removeAttribute('object-fit');
    } else {
      el.setAttribute('object-fit', String(objectFit).trim().toLowerCase());
    }
  },
  { immediate: true }
);

const domProps = computed(() => {
  const result = buildVideoNativeAttrs({
    ...props,
    id: resolvedId.value,
  }, attrs as Record<string, unknown>);
  return {
    ...result,
    class: props.class ?? attrs.class,
    style: props.style ?? attrs.style,
  };
});

defineExpose({ el: elementRef });

const render = () => h('lx-video', { ref: elementRef, ...domProps.value });
</script>

<template>
  <render />
</template>
