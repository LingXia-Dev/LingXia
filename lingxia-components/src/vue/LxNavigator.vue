<script setup lang="ts">
import { ref, h, onMounted, onBeforeUnmount, useAttrs } from 'vue';
import type { LxNavigatorEvent } from '../navigator.js';
import { buildNavigatorNativeAttrs } from '../native_component_wrapper_shared.js';
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
const attrs = useAttrs();

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
    ...buildNavigatorNativeAttrs({
      url: props.url,
      openType: props.openType,
      target: props.target,
      delta: props.delta,
      appId: props.appId,
      path: props.path,
      phoneNumber: props.phoneNumber,
      hoverClass: props.hoverClass,
      hoverStopPropagation: props.hoverStopPropagation,
      hoverStartTime: props.hoverStartTime,
      hoverStayTime: props.hoverStayTime,
    }, attrs as Record<string, unknown>),
    class: props.class ?? attrs.class,
    style: props.style ?? attrs.style,
  },
  slots.default?.()
);
</script>

<template>
  <render />
</template>
