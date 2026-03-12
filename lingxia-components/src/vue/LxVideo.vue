<script setup lang="ts">
import { ref, computed, h, onMounted, onBeforeUnmount, useAttrs, useId, watch } from 'vue';
import { registerVideoComponent } from '../video.js';
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
  playRequest: [e: Event];
  play: [e: Event];
  playing: [e: Event];
  pause: [e: Event];
  stop: [e: Event];
  ended: [e: Event];
  timeUpdate: [e: Event];
  error: [e: Event];
  loadedMetadata: [e: Event];
  fullscreenChange: [e: Event];
  waiting: [e: Event];
  qualityChange: [e: Event];
  rateChange: [e: Event];
}>();

if (typeof window !== 'undefined') {
  registerVideoComponent();
}

const elementRef = ref<HTMLElement | null>(null);
const handlerMap = ref(new Map<string, EventListenerOrEventListenerObject>());
const vueId = useId();

const resolvedId = computed(() => props.id || `lx-video-${vueId.replace(/[:]/g, '')}`);

const eventMap: Record<string, string> = {
  playRequest: 'playrequest',
  play: 'play',
  playing: 'playing',
  pause: 'pause',
  stop: 'stop',
  ended: 'ended',
  timeUpdate: 'timeupdate',
  error: 'error',
  loadedMetadata: 'loadedmetadata',
  fullscreenChange: 'fullscreenchange',
  waiting: 'waiting',
  qualityChange: 'qualitychange',
  rateChange: 'ratechange',
};

function normalizeBindingAttrName(key: string): string {
  return key.replace(/[^a-zA-Z0-9]/g, '').toLowerCase();
}

function setupEventListeners() {
  const el = elementRef.value;
  if (!el) return;
  for (const [eventName, handler] of handlerMap.value.entries()) {
    el.removeEventListener(eventName, handler);
  }
  handlerMap.value.clear();
  for (const [vueEvent, domEvent] of Object.entries(eventMap)) {
    const handler: EventListenerObject = {
      handleEvent: (e: Event) => emit(vueEvent as any, e),
    };
    el.addEventListener(domEvent, handler);
    handlerMap.value.set(domEvent, handler);
  }
}

function cleanupEventListeners() {
  const el = elementRef.value;
  if (!el) return;
  for (const [eventName, handler] of handlerMap.value.entries()) {
    el.removeEventListener(eventName, handler);
  }
  handlerMap.value.clear();
}

onMounted(setupEventListeners);
onBeforeUnmount(cleanupEventListeners);
watch(elementRef, setupEventListeners);

watch(
  [elementRef, () => props.rotate, () => props.objectFit],
  () => {
    const el = elementRef.value;
    if (!el) return;
    const rotate = props.rotate;
    if (rotate === undefined || rotate === null || rotate === '') {
      el.removeAttribute('rotate');
    } else {
      el.setAttribute('rotate', String(rotate).trim());
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
  const result: Record<string, any> = { id: resolvedId.value };
  if (props.src) result.src = props.src;
  if (props.poster) result.poster = props.poster;
  if (props.autoplay) result.autoplay = true;
  if (props.loop) result.loop = true;
  if (props.muted) result.muted = true;
  if (props.controls) result.controls = true;
  if (props.live) result.live = '';
  if (props.volume !== undefined) result.volume = props.volume;
  if (props.progressBar === false) result['progress-bar'] = 'false';
  if (props.qualities?.length) result.qualities = JSON.stringify(props.qualities);
  if (props.playbackRates?.length) result['playback-rates'] = JSON.stringify(props.playbackRates);
  for (const [key, value] of Object.entries(props as Record<string, unknown>)) {
    if (typeof value !== 'string') continue;
    if (key.startsWith('bind') || key.startsWith('catch')) {
      result[normalizeBindingAttrName(key)] = value;
    }
  }
  for (const [key, value] of Object.entries(attrs)) {
    if (typeof value !== 'string') continue;
    if (key.startsWith('data-')) {
      result[key] = value;
      continue;
    }
    if (key.startsWith('bind') || key.startsWith('catch')) {
      result[normalizeBindingAttrName(key)] = value;
    }
  }
  return result;
});

defineExpose({ el: elementRef });

const render = () => h('lx-video', { ref: elementRef, ...domProps.value, class: props.class });
</script>

<template>
  <render />
</template>
