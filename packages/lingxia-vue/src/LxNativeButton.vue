<script setup lang="ts">
import { onBeforeUnmount, ref, watch } from 'vue';
import {
  registerNativeButtonComponent,
  unwrapNativeEventPayload,
  type PressPayload,
  type FocusPayload,
  type PointerPayload,
} from '@lingxia/elements';
import { bindElementEvents, unbindElementEvents } from './text_component_shared.js';

defineProps<{
  id?: string;
  automationId?: string;
  class?: string;
  label?: string;
  icon?: string | { resource: unknown };
  iconPosition?: 'start' | 'end';
  intent?: 'neutral' | 'accent' | 'destructive';
  emphasis?: 'primary' | 'secondary' | 'quiet';
  size?: 'compact' | 'regular';
  hitSlop?: number;
  disabled?: boolean;
  pressed?: boolean;
  expanded?: boolean;
  loading?: boolean;
  pointerEvents?: 'auto' | 'none' | 'box-only' | 'box-none';
  hidden?: boolean;
  hiddenTransition?: 'none' | 'fade';
}>();

const emit = defineEmits<{
  press: [payload: PressPayload];
  focus: [payload: FocusPayload];
  blur: [payload: FocusPayload];
  pointerEnter: [payload: PointerPayload];
  pointerLeave: [payload: PointerPayload];
}>();

if (typeof window !== 'undefined') {
  registerNativeButtonComponent();
}

const elementRef = ref<HTMLElement | null>(null);
let bound: HTMLElement | null = null;
const listeners: Record<string, EventListenerObject> = {
  press: { handleEvent: (event) => emit('press', unwrapNativeEventPayload(event)) },
  focus: { handleEvent: (event) => emit('focus', unwrapNativeEventPayload(event)) },
  blur: { handleEvent: (event) => emit('blur', unwrapNativeEventPayload(event)) },
  pointerenter: { handleEvent: (event) => emit('pointerEnter', unwrapNativeEventPayload(event)) },
  pointerleave: { handleEvent: (event) => emit('pointerLeave', unwrapNativeEventPayload(event)) },
};

watch(elementRef, (element) => {
  bound = bindElementEvents(bound, element, listeners);
});
onBeforeUnmount(() => unbindElementEvents(bound, listeners));
</script>

<template>
  <lx-native-button
    ref="elementRef"
    :id="id"
    :class="class"
    :automation-id="automationId"
    :label="label"
    :icon="typeof icon === 'string' ? icon : undefined"
    :icon-position="iconPosition"
    :intent="intent"
    :emphasis="emphasis"
    :size="size"
    :hit-slop="hitSlop"
    :disabled="disabled"
    :pressed="pressed"
    :expanded="expanded"
    :loading="loading"
    :pointer-events="pointerEvents"
    :hidden="hidden"
    :hidden-transition="hiddenTransition"
  >
    <slot />
  </lx-native-button>
</template>
