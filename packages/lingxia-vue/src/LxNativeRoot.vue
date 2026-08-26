<script setup lang="ts">
import { h, onBeforeUnmount, ref, useAttrs, useSlots, watch } from 'vue';
import { registerNativeRootComponent, unwrapNativeEventPayload, type NativeError } from '@lingxia/elements';
import { bindElementEvents, unbindElementEvents } from './text_component_shared.js';

const props = defineProps<{
  id?: string;
  automationId?: string;
  class?: string;
  fullscreenScope?: 'root' | 'none';
  pointerEvents?: 'auto' | 'none' | 'box-only' | 'box-none';
  hidden?: boolean;
  hiddenTransition?: 'none' | 'fade';
}>();
const slots = useSlots();
const attrs = useAttrs();

const emit = defineEmits<{
  ready: [payload: Record<string, never>];
  error: [payload: NativeError];
  pointerWithinChange: [payload: { within: boolean }];
}>();

if (typeof window !== 'undefined') {
  registerNativeRootComponent();
}

const elementRef = ref<HTMLElement | null>(null);
let bound: HTMLElement | null = null;
const listeners: Record<string, EventListenerObject> = {
  ready: { handleEvent: (event) => emit('ready', unwrapNativeEventPayload(event)) },
  error: { handleEvent: (event) => emit('error', unwrapNativeEventPayload(event)) },
  pointerwithinchange: {
    handleEvent: (event) => emit('pointerWithinChange', unwrapNativeEventPayload(event)),
  },
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
  'fullscreen-scope': props.fullscreenScope ?? 'root',
  'pointer-events': props.pointerEvents,
  hidden: props.hidden,
  'hidden-transition': props.hiddenTransition,
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
