<script setup lang="ts">
import { computed, h, onBeforeUnmount, ref, useAttrs, watch } from 'vue';
import { registerTextareaComponent } from '@lingxia/elements';
import { buildTextareaNativeAttrs } from '@lingxia/elements';
import type { LxTextareaProps } from './types.js';
import {
  bindElementEvents,
  getCustomEventDetail,
  unbindElementEvents,
} from './text_component_shared.js';

const props = withDefaults(defineProps<LxTextareaProps>(), {
  disabled: false,
});
const attrs = useAttrs();

const emit = defineEmits<{
  'update:modelValue': [value: string];
  input: [detail: Record<string, unknown>];
  change: [detail: Record<string, unknown>];
  focus: [detail: Record<string, unknown>];
  blur: [detail: Record<string, unknown>];
  confirm: [detail: Record<string, unknown>];
  lineChange: [detail: Record<string, unknown>];
  keyboardHeightChange: [detail: Record<string, unknown>];
}>();

if (typeof window !== 'undefined') {
  registerTextareaComponent();
}

const elementRef = ref<HTMLElement | null>(null);
let boundElement: HTMLElement | null = null;

const textareaEventListeners: Record<string, EventListenerObject> = {
  input: {
    handleEvent: (event: Event) => {
      const detail = getCustomEventDetail<Record<string, unknown>>(event);
      if (typeof detail.value === 'string') {
        emit('update:modelValue', detail.value);
      }
      emit('input', detail);
    },
  },
  change: {
    handleEvent: (event: Event) => emit('change', getCustomEventDetail<Record<string, unknown>>(event)),
  },
  focus: {
    handleEvent: (event: Event) => emit('focus', getCustomEventDetail<Record<string, unknown>>(event)),
  },
  blur: {
    handleEvent: (event: Event) => emit('blur', getCustomEventDetail<Record<string, unknown>>(event)),
  },
  confirm: {
    handleEvent: (event: Event) => emit('confirm', getCustomEventDetail<Record<string, unknown>>(event)),
  },
  linechange: {
    handleEvent: (event: Event) => emit('lineChange', getCustomEventDetail<Record<string, unknown>>(event)),
  },
  keyboardheightchange: {
    handleEvent: (event: Event) => emit('keyboardHeightChange', getCustomEventDetail<Record<string, unknown>>(event)),
  },
};

watch(elementRef, (element) => {
  boundElement = bindElementEvents(boundElement, element, textareaEventListeners);
});

onBeforeUnmount(() => {
  unbindElementEvents(boundElement, textareaEventListeners);
});

const textareaProps = computed<Record<string, unknown>>(() => {
  const result: Record<string, unknown> = buildTextareaNativeAttrs({
    ...props,
    id: props.id ?? (typeof attrs.id === 'string' ? attrs.id : undefined),
  }, attrs as Record<string, unknown>, elementRef.value !== null);
  result.class = props.class ?? attrs.class;
  result.style = props.style ?? attrs.style;
  return result;
});

defineExpose({ el: elementRef });

const renderTextarea = () => h('lx-textarea', { ref: elementRef, ...textareaProps.value });
</script>

<template>
  <renderTextarea />
</template>
