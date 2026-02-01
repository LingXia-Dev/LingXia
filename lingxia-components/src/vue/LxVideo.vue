<script setup lang="ts">
import { ref, computed, h, onMounted, onBeforeUnmount, useId, watch } from 'vue';
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
const handlerMap = ref(new Map<string, EventListener>());
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

function setupEventListeners() {
  const el = elementRef.value;
  if (!el) return;
  for (const [eventName, handler] of handlerMap.value.entries()) {
    el.removeEventListener(eventName, handler);
  }
  handlerMap.value.clear();
  for (const [vueEvent, domEvent] of Object.entries(eventMap)) {
    const handler = (e: Event) => emit(vueEvent as any, e);
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

const domProps = computed(() => {
  const result: Record<string, any> = { id: resolvedId.value };
  if (props.src) result.src = props.src;
  if (props.poster) result.poster = props.poster;
  if (props.autoplay) result.autoplay = true;
  if (props.loop) result.loop = true;
  if (props.muted) result.muted = true;
  if (props.controls) result.controls = true;
  if (props.live) result.live = true;
  if (props.volume !== undefined) result.volume = props.volume;
  if (props.progressBar === false) result['progress-bar'] = 'false';
  if (props.qualities?.length) result.qualities = JSON.stringify(props.qualities);
  if (props.playbackRates?.length) result['playback-rates'] = JSON.stringify(props.playbackRates);
  return result;
});

defineExpose({ el: elementRef });

const render = () => h('lx-video', { ref: elementRef, ...domProps.value, class: props.class });
</script>

<template>
  <render />
</template>
