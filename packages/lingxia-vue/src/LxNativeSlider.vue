<script setup lang="ts">
import { onBeforeUnmount, ref, watch } from 'vue';
import {
  registerNativeSliderComponent,
  unwrapNativeEventPayload,
  type ValuePayload,
  type FocusPayload,
  type PointerPayload,
} from '@lingxia/elements';
import { bindElementEvents, unbindElementEvents } from './text_component_shared.js';

defineProps<{
  id?: string;
  automationId?: string;
  class?: string;
  value?: number;
  min?: number;
  max?: number;
  step?: number;
  bufferedValue?: number;
  valueLabel?: 'none' | 'value' | 'time';
  disabled?: boolean;
  pointerEvents?: 'auto' | 'none' | 'box-only' | 'box-none';
  hidden?: boolean;
  hiddenTransition?: 'none' | 'fade';
}>();

const emit = defineEmits<{
  valueChange: [payload: ValuePayload];
  valueCommit: [payload: ValuePayload];
  focus: [payload: FocusPayload];
  blur: [payload: FocusPayload];
  pointerEnter: [payload: PointerPayload];
  pointerLeave: [payload: PointerPayload];
}>();

if (typeof window !== 'undefined') {
  registerNativeSliderComponent();
}

const elementRef = ref<HTMLElement | null>(null);
let bound: HTMLElement | null = null;
const listeners: Record<string, EventListenerObject> = {
  valuechange: { handleEvent: (event) => emit('valueChange', unwrapNativeEventPayload(event)) },
  valuecommit: { handleEvent: (event) => emit('valueCommit', unwrapNativeEventPayload(event)) },
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
  <lx-native-slider
    ref="elementRef"
    :id="id"
    :class="class"
    :automation-id="automationId"
    :value="value"
    :min="min"
    :max="max"
    :step="step"
    :buffered-value="bufferedValue"
    :value-label="valueLabel"
    :disabled="disabled"
    :pointer-events="pointerEvents"
    :hidden="hidden"
    :hidden-transition="hiddenTransition"
  />
</template>
