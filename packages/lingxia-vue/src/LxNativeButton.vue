<script setup lang="ts">
import type { LxNativeButtonProps } from './types.js';
import { h, onBeforeUnmount, useAttrs, useSlots, watch } from 'vue';
import {
  registerNativeButtonComponent,
  unwrapNativeEventPayload,
  type PressPayload,
} from '@lingxia/elements';
import { bindElementEvents, unbindElementEvents, useNativeHostElement } from './text_component_shared.js';

const props = defineProps<LxNativeButtonProps>();
const slots = useSlots();
const attrs = useAttrs();

const emit = defineEmits<{
  press: [payload: PressPayload];
}>();

if (typeof window !== 'undefined') {
  registerNativeButtonComponent();
}

const elementRef = useNativeHostElement(props);
let bound: HTMLElement | null = null;
const listeners: Record<string, EventListenerObject> = {
  press: { handleEvent: (event) => emit('press', unwrapNativeEventPayload(event)) },
};

watch(elementRef, (element) => {
  bound = bindElementEvents(bound, element, listeners);
});
onBeforeUnmount(() => unbindElementEvents(bound, listeners));

const render = () => h('lx-native-button', {
  ...attrs,
  ref: elementRef,
  id: props.id,
  class: props.class,
  style: props.style,
  'aria-label': props['aria-label'],
  'aria-description': props['aria-description'],
  'aria-hidden': props['aria-hidden'],
  'automation-id': props.automationId,
  label: props.label,
  icon: props.icon,
  'icon-position': props.iconPosition,
  intent: props.intent,
  emphasis: props.emphasis,
  size: props.size,
  'hit-slop': props.hitSlop,
  disabled: props.disabled,
  pressed: props.pressed,
  expanded: props.expanded,
  loading: props.loading,
  tabindex: props.tabIndex,
  'pointer-events': props.pointerEvents,
  hidden: props.hidden,
}, slots.default?.());
</script>

<template>
  <render />
</template>
