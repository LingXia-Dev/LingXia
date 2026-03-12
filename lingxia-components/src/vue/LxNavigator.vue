<script setup lang="ts">
import { ref, h, onMounted, onBeforeUnmount } from 'vue';
import type { LxNavigatorEvent } from '../navigator.js';
import type { LxNavigatorProps } from './types.js';
import '../navigator.js';

const props = withDefaults(defineProps<LxNavigatorProps>(), {
  openType: 'navigate',
  delta: 1,
  hoverClass: 'navigator-hover',
  hoverStopPropagation: false,
  hoverStartTime: 20,
  hoverStayTime: 70,
});

const slots = defineSlots();

const emit = defineEmits<{
  success: [e: LxNavigatorEvent];
  fail: [e: LxNavigatorEvent];
  complete: [e: LxNavigatorEvent];
}>();

const elementRef = ref<HTMLElement | null>(null);

const handleSuccess = (e: Event) => emit('success', e as LxNavigatorEvent);
const handleFail = (e: Event) => emit('fail', e as LxNavigatorEvent);
const handleComplete = (e: Event) => emit('complete', e as LxNavigatorEvent);
const successListener: EventListenerObject = { handleEvent: handleSuccess };
const failListener: EventListenerObject = { handleEvent: handleFail };
const completeListener: EventListenerObject = { handleEvent: handleComplete };

onMounted(() => {
  const el = elementRef.value;
  if (!el) return;
  el.addEventListener('success', successListener);
  el.addEventListener('fail', failListener);
  el.addEventListener('complete', completeListener);
});

onBeforeUnmount(() => {
  const el = elementRef.value;
  if (!el) return;
  el.removeEventListener('success', successListener);
  el.removeEventListener('fail', failListener);
  el.removeEventListener('complete', completeListener);
});

defineExpose({ el: elementRef });

const render = () => h(
  'lx-navigator',
  {
    ref: elementRef,
    url: props.url,
    'open-type': props.openType,
    target: props.target,
    delta: String(props.delta),
    'app-id': props.appId,
    path: props.path,
    'phone-number': props.phoneNumber,
    'hover-class': props.hoverClass,
    'hover-stop-propagation': String(props.hoverStopPropagation),
    'hover-start-time': String(props.hoverStartTime),
    'hover-stay-time': String(props.hoverStayTime),
    class: props.class,
    style: props.style,
  },
  slots.default?.()
);
</script>

<template>
  <render />
</template>
