<script setup lang="ts">
import { onBeforeUnmount, ref, watch } from 'vue';
import { registerNativeRootComponent, unwrapNativeEventPayload, type NativeError } from '@lingxia/elements';
import { bindElementEvents, unbindElementEvents } from './text_component_shared.js';

defineProps<{
  id?: string;
  automationId?: string;
  class?: string;
  fullscreenScope?: 'root' | 'none';
  pointerEvents?: 'auto' | 'none' | 'box-only' | 'box-none';
  hidden?: boolean;
  hiddenTransition?: 'none' | 'fade';
}>();

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
</script>

<template>
  <lx-native-root
    ref="elementRef"
    :id="id"
    :class="class"
    :fullscreen-scope="fullscreenScope ?? 'root'"
    :pointer-events="pointerEvents"
    :hidden="hidden"
    :hidden-transition="hiddenTransition"
    :automation-id="automationId"
  >
    <slot />
    <div v-if="$slots.fallback" data-lx-native-fallback hidden aria-hidden="true">
      <slot name="fallback" />
    </div>
  </lx-native-root>
</template>
