<script setup lang="ts">
import { ref, computed, h, onBeforeUnmount, useAttrs, useId, watch, type CSSProperties } from 'vue';
import { registerPickerComponent } from '../picker.js';
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
const boundElement = ref<HTMLElement | null>(null);
const changeListener: EventListenerObject = {
  handleEvent: (event: Event) => handleChange(event),
};
const scrollListener: EventListenerObject = {
  handleEvent: (event: Event) => handleScroll(event),
};

const isDateMode = computed(() => props.mode === 'date' || props.mode === 'time');
const isCascading = computed(() => 
  props.columns?.length === 2 && typeof props.columns[1] === 'object' && !Array.isArray(props.columns[1])
);
const isSingle = computed(() => props.columns?.length === 1);

function normalizeBindingAttrName(key: string): string {
  return key.replace(/[^a-zA-Z0-9]/g, '').toLowerCase();
}

function getIndexFromValue(): number | number[] {
  if (!props.columns) return 0;
  if (isSingle.value) {
    if (!props.modelValue || typeof props.modelValue !== 'string') return 0;
    const idx = (props.columns?.[0] as string[])?.indexOf(props.modelValue) ?? -1;
    return idx >= 0 ? idx : 0;
  }
  if (!props.modelValue || !Array.isArray(props.modelValue)) {
    return Array.from({ length: props.columns.length }, () => 0);
  }
  if (isCascading.value) {
    const [keys, map] = props.columns as [string[], Record<string, string[]>];
    const keyIdx = Math.max(0, keys.indexOf(props.modelValue[0]));
    const valIdx = Math.max(0, map[keys[keyIdx]]?.indexOf(props.modelValue[1]) ?? 0);
    return [keyIdx, valIdx];
  }
  const idxs = props.modelValue.map((v, i) => Math.max(0, (props.columns![i] as string[])?.indexOf(v) ?? 0));
  while (idxs.length < props.columns.length) idxs.push(0);
  return idxs;
}

function getValueFromIndex(cols: typeof props.columns, index: number | number[]): string | string[] {
  if (!cols) return '';
  const cascading = cols.length === 2 && typeof cols[1] === 'object' && !Array.isArray(cols[1]);
  if (typeof index === 'number') return (cols[0] as string[])[index] ?? '';
  if (cascading) {
    const [keys, map] = cols as [string[], Record<string, string[]>];
    const key = keys[index[0]] ?? '';
    return [key, map[key]?.[index[1]] ?? ''];
  }
  return index.map((idx, col) => (cols[col] as string[])?.[idx] ?? '');
}

const displayText = computed(() => {
  if (!props.modelValue) return '';
  if (props.fields === 'range' && Array.isArray(props.modelValue)) {
    return `${props.modelValue[0]} ~ ${props.modelValue[1]}`;
  }
  return typeof props.modelValue === 'string' ? props.modelValue : props.modelValue.join(' - ');
});

function handleChange(e: Event) {
  if (typeof props.onChange === 'function') {
    props.onChange(e);
  }
  const detail = (e as CustomEvent).detail;
  if (!detail) return;
  if (detail.confirmed) {
    if (props.mode === 'date' || props.mode === 'time') {
      emit('update:modelValue', detail.value);
      emit('confirm', detail.value);
    } else if (detail.index !== undefined) {
      const value = getValueFromIndex(props.columns, detail.index);
      emit('update:modelValue', value);
      emit('confirm', value);
    }
    visible.value = false;
  } else if (detail.cancelled) {
    emit('cancel');
    visible.value = false;
  }
}

function handleScroll(e: Event) {
  if (typeof props.onNativeScroll === 'function') {
    props.onNativeScroll(e);
  }
  const detail = (e as CustomEvent).detail;
  if (!detail) return;
  if (detail.value !== undefined) {
    emit('scroll', detail.value);
  } else if (detail.index !== undefined) {
    emit('scroll', getValueFromIndex(props.columns, detail.index));
  }
}

function handleClick() {
  if (!props.disabled) visible.value = true;
}

function bindPickerEvents(el: HTMLElement | null) {
  if (boundElement.value && boundElement.value !== el) {
    boundElement.value.removeEventListener('change', changeListener);
    boundElement.value.removeEventListener('scroll', scrollListener);
    boundElement.value = null;
  }
  if (el && boundElement.value !== el) {
    el.addEventListener('change', changeListener);
    el.addEventListener('scroll', scrollListener);
    boundElement.value = el;
  }
}

watch(pickerRef, bindPickerEvents);

onBeforeUnmount(() => {
  if (boundElement.value) {
    boundElement.value.removeEventListener('change', changeListener);
    boundElement.value.removeEventListener('scroll', scrollListener);
  }
});

const pickerProps = computed(() => {
  const result: Record<string, string> = { id: pickerId.value };
  if (isDateMode.value) {
    result.mode = props.mode!;
    if (props.fields) result.fields = props.fields;
    if (props.modelValue) {
      result.value = typeof props.modelValue === 'string' ? props.modelValue : JSON.stringify(props.modelValue);
    }
    if (props.start) result.start = props.start;
    if (props.end) result.end = props.end;
  } else {
    result.mode = isCascading.value ? 'cascading' : (isSingle.value ? 'selector' : 'multiSelector');
    result.columns = JSON.stringify(props.columns ?? []);
    result['default-index'] = JSON.stringify(getIndexFromValue());
  }
  if (props.cancelText) result['cancel-text'] = props.cancelText;
  if (props.cancelTextColor) result['cancel-text-color'] = props.cancelTextColor;
  if (props.cancelButtonColor) result['cancel-button-color'] = props.cancelButtonColor;
  if (props.confirmText) result['confirm-text'] = props.confirmText;
  if (props.confirmTextColor) result['confirm-text-color'] = props.confirmTextColor;
  if (props.confirmButtonColor) result['confirm-button-color'] = props.confirmButtonColor;
  if (props.bindChange) result.bindchange = props.bindChange;
  if (props.bindScroll) result.bindscroll = props.bindScroll;
  if (props.catchChange) result.catchchange = props.catchChange;
  if (props.catchScroll) result.catchscroll = props.catchScroll;
  for (const [key, value] of Object.entries(attrs)) {
    if (typeof value !== 'string') continue;
    if (key.startsWith('data-')) {
      result[key] = value;
      continue;
    }
    if (key.startsWith('bind') || key.startsWith('catch')) {
      result[normalizeBindingAttrName(key)] = value;
    }
  }
  return result;
});

const triggerStyle = computed<CSSProperties>(() => ({
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
  ...props.style,
}));

defineExpose({ el: pickerRef });

const renderPicker = () => visible.value ? h('lx-picker', { ref: pickerRef, ...pickerProps.value }) : null;
</script>

<template>
  <slot :open="handleClick" :disabled="props.disabled">
    <div :class="props.class" :style="triggerStyle" @click="handleClick">
      <span :style="{ color: modelValue ? '#111' : '#9ca3af' }">{{ displayText || placeholder }}</span>
      <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="#9ca3af" stroke-width="2">
        <path d="M6 9l6 6 6-6" />
      </svg>
    </div>
  </slot>
  <renderPicker />
</template>
