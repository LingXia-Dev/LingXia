<script setup lang="ts">
import { h, onBeforeUnmount, ref, useAttrs, watch } from 'vue';
import {
  registerNativeSliderComponent,
  unwrapNativeEventPayload,
  type ValuePayload,
  type FocusPayload,
  type PointerPayload,
} from '@lingxia/elements';
import { bindElementEvents, unbindElementEvents } from './text_component_shared.js';

const props = defineProps<{
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
const attrs = useAttrs();
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

const render = () => h('lx-native-slider', {
  ...attrs,
  ref: elementRef,
  id: props.id,
  class: props.class,
  'automation-id': props.automationId,
  value: props.value,
  min: props.min,
  max: props.max,
  step: props.step,
  'buffered-value': props.bufferedValue,
  'value-label': props.valueLabel,
  disabled: props.disabled,
  'pointer-events': props.pointerEvents,
  hidden: props.hidden,
  'hidden-transition': props.hiddenTransition,
});
</script>

<template>
  <render />
</template>
