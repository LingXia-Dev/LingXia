<script setup lang="ts">
import { h, onBeforeUnmount, ref, useAttrs, useSlots, watch } from 'vue';
import {
  registerNativeButtonComponent,
  unwrapNativeEventPayload,
  type PressPayload,
  type FocusPayload,
  type PointerPayload,
} from '@lingxia/elements';
import { bindElementEvents, unbindElementEvents } from './text_component_shared.js';

const props = defineProps<{
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
const slots = useSlots();
const attrs = useAttrs();

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

const render = () => h('lx-native-button', {
  ...attrs,
  ref: elementRef,
  id: props.id,
  class: props.class,
  'automation-id': props.automationId,
  label: props.label,
  icon: typeof props.icon === 'string' ? props.icon : undefined,
  'icon-position': props.iconPosition,
  intent: props.intent,
  emphasis: props.emphasis,
  size: props.size,
  'hit-slop': props.hitSlop,
  disabled: props.disabled,
  pressed: props.pressed,
  expanded: props.expanded,
  loading: props.loading,
  'pointer-events': props.pointerEvents,
  hidden: props.hidden,
  'hidden-transition': props.hiddenTransition,
}, slots.default?.());
</script>

<template>
  <render />
</template>
