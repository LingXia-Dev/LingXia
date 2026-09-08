<script setup lang="ts">
import type { LxNativeRootProps } from './types.js';
import { h, onBeforeUnmount, useAttrs, useSlots, watch } from 'vue';
import { registerNativeRootComponent, unwrapNativeEventPayload, type NativeError } from '@lingxia/elements';
import { bindElementEvents, unbindElementEvents, useNativeHostElement } from './text_component_shared.js';

const props = defineProps<LxNativeRootProps>();
const slots = useSlots();
const attrs = useAttrs();

const emit = defineEmits<{
  ready: [payload: Record<string, never>];
  error: [payload: NativeError];
}>();

if (typeof window !== 'undefined') {
  registerNativeRootComponent();
}

const elementRef = useNativeHostElement(props);
let bound: HTMLElement | null = null;
const listeners: Record<string, EventListenerObject> = {
  ready: { handleEvent: (event) => emit('ready', unwrapNativeEventPayload(event)) },
  error: { handleEvent: (event) => emit('error', unwrapNativeEventPayload(event)) },
};

watch(elementRef, (element) => {
  bound = bindElementEvents(bound, element, listeners);
});
onBeforeUnmount(() => unbindElementEvents(bound, listeners));

const retry = async () => {
  const el = elementRef.value as { retry?: () => Promise<void> } | null;
  await el?.retry?.();
};

defineExpose({ retry, el: elementRef });

const render = () => h('lx-native-root', {
  ...attrs,
  ref: elementRef,
  id: props.id,
  class: props.class,
  style: props.style,
  'aria-label': props['aria-label'],
  'aria-description': props['aria-description'],
  'aria-hidden': props['aria-hidden'],
  'pointer-events': props.pointerEvents,
  hidden: props.hidden,
  'automation-id': props.automationId,
}, [
  slots.default?.(),
  slots.fallback ? h('div', {
    'data-lx-native-fallback': '',
    hidden: true,
    'aria-hidden': 'true',
  }, slots.fallback()) : null,
]);
</script>

<template>
  <render />
</template>
