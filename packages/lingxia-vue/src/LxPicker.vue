<script setup lang="ts">
import { ref, computed, h, onBeforeUnmount, useAttrs, useId, watch } from 'vue';
import { registerPickerComponent } from '@lingxia/elements';
import {
  buildPickerNativeAttrs,
  getPickerDisplayText,
  getPickerValueFromIndex,
} from '@lingxia/elements';
import { bindElementEvents, getCustomEventDetail, unbindElementEvents } from './text_component_shared.js';
import type { LxPickerProps } from './types.js';

const props = withDefaults(defineProps<LxPickerProps>(), {
  placeholder: 'Please select',
  disabled: false,
});
const attrs = useAttrs();

const slots = defineSlots();

const emit = defineEmits<{
  'update:modelValue': [value: string | string[]];
  confirm: [value: string | string[]];
  cancel: [];
  scroll: [value: string | string[]];
}>();

if (typeof window !== 'undefined') {
  registerPickerComponent();
}

const visible = ref(false);
const vueId = useId();
const pickerId = computed(() => `lx-picker-${vueId.replace(/[:]/g, '')}`);
const pickerRef = ref<HTMLElement | null>(null);
let boundElement: HTMLElement | null = null;
const pickerEventListeners: Record<string, EventListenerObject> = {
  change: {
    handleEvent: (event: Event) => handleChange(event),
  },
  scroll: {
    handleEvent: (event: Event) => handleScroll(event),
  },
};

const displayText = computed(() => getPickerDisplayText(props.modelValue, props.fields));

function handleChange(e: Event) {
  props.onChange?.(e);
  const detail = getCustomEventDetail<{
    confirmed?: boolean;
    cancelled?: boolean;
    value?: string | string[];
    index?: number | number[];
  }>(e);
  if (detail.confirmed) {
    if (props.mode === 'date' || props.mode === 'time') {
      const nextValue = detail.value ?? '';
      emit('update:modelValue', nextValue);
      emit('confirm', nextValue);
    } else if (detail.index !== undefined) {
      const nextValue = getPickerValueFromIndex(props.columns, detail.index);
      emit('update:modelValue', nextValue);
      emit('confirm', nextValue);
    }
    visible.value = false;
  } else if (detail.cancelled) {
    emit('cancel');
    visible.value = false;
  }
}

function handleScroll(e: Event) {
  props.onNativeScroll?.(e);
  const detail = getCustomEventDetail<{
    value?: string | string[];
    index?: number | number[];
  }>(e);
  if (detail.value !== undefined) {
    emit('scroll', detail.value);
  } else if (detail.index !== undefined) {
    emit('scroll', getPickerValueFromIndex(props.columns, detail.index));
  }
}

function handleClick() {
  if (!props.disabled) visible.value = true;
}

watch(pickerRef, (element) => {
  boundElement = bindElementEvents(boundElement, element, pickerEventListeners);
});

onBeforeUnmount(() => {
  unbindElementEvents(boundElement, pickerEventListeners);
});

const pickerProps = computed(() => {
  return buildPickerNativeAttrs({
    id: pickerId.value,
    columns: props.columns,
    mode: props.mode,
    start: props.start,
    end: props.end,
    fields: props.fields,
    modelValue: props.modelValue,
    cancelText: props.cancelText,
    cancelTextColor: props.cancelTextColor,
    cancelButtonColor: props.cancelButtonColor,
    confirmText: props.confirmText,
    confirmTextColor: props.confirmTextColor,
    confirmButtonColor: props.confirmButtonColor,
  }, attrs as Record<string, unknown>);
});

const triggerStyle = computed(() => ({
  display: 'flex',
  alignItems: 'center',
  justifyContent: 'space-between',
  padding: '12px 14px',
  backgroundColor: '#fff',
  border: '1px solid #e5e7eb',
  borderRadius: '8px',
  cursor: props.disabled ? 'not-allowed' : 'pointer',
  opacity: props.disabled ? 0.5 : 1,
  width: '100%',
  boxSizing: 'border-box',
}));

defineExpose({ el: pickerRef });

const renderPicker = () => visible.value ? h('lx-picker', { ref: pickerRef, ...pickerProps.value }) : null;
</script>

<template>
  <slot :open="handleClick" :disabled="props.disabled">
    <div :class="props.class ?? attrs.class" :style="[triggerStyle, props.style ?? attrs.style]" @click="handleClick">
      <span :style="{ color: modelValue ? '#111' : '#9ca3af' }">{{ displayText || placeholder }}</span>
      <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="#9ca3af" stroke-width="2">
        <path d="M6 9l6 6 6-6" />
      </svg>
    </div>
  </slot>
  <renderPicker />
</template>
