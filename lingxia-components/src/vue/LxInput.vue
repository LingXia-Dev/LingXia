<script setup lang="ts">
import { computed, h, onBeforeUnmount, ref, useAttrs, watch } from 'vue';
import { registerInputComponent } from '../input.js';
import type { LxInputProps } from './types.js';
import {
  appendBindingAndDatasetAttrs,
  bindElementEvents,
  getCustomEventDetail,
  unbindElementEvents,
} from './text_component_shared.js';

const props = withDefaults(defineProps<LxInputProps>(), {
  type: 'text',
  disabled: false,
  confirmType: 'done',
});
const attrs = useAttrs();

const emit = defineEmits<{
  'update:modelValue': [value: string];
  input: [detail: Record<string, unknown>];
  change: [detail: Record<string, unknown>];
  focus: [detail: Record<string, unknown>];
  blur: [detail: Record<string, unknown>];
  confirm: [detail: Record<string, unknown>];
  keyboardHeightChange: [detail: Record<string, unknown>];
  nicknameReview: [detail: Record<string, unknown>];
}>();

if (typeof window !== 'undefined') {
  registerInputComponent();
}

const elementRef = ref<HTMLElement | null>(null);
let boundElement: HTMLElement | null = null;

const inputEventListeners: Record<string, EventListenerObject> = {
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
  keyboardheightchange: {
    handleEvent: (event: Event) => emit('keyboardHeightChange', getCustomEventDetail<Record<string, unknown>>(event)),
  },
  nicknamereview: {
    handleEvent: (event: Event) => emit('nicknameReview', getCustomEventDetail<Record<string, unknown>>(event)),
  },
};

watch(elementRef, (element) => {
  boundElement = bindElementEvents(boundElement, element, inputEventListeners);
});

onBeforeUnmount(() => {
  unbindElementEvents(boundElement, inputEventListeners);
});

const inputProps = computed<Record<string, unknown>>(() => {
  const result: Record<string, unknown> = {};
  const explicitId =
    typeof props.id === 'string'
      ? props.id.trim()
      : typeof attrs.id === 'string'
        ? attrs.id.trim()
        : '';

  if (explicitId.length > 0) result.id = explicitId;

  if (props.modelValue !== undefined) {
    result.value = props.modelValue;
  } else if (props.value !== undefined) {
    result.value = props.value;
  } else if (!elementRef.value && props.defaultValue !== undefined) {
    result.value = props.defaultValue;
  }

  if (props.type) result.type = props.type;
  if (props.password) result.password = 'true';
  if (props.placeholder) result.placeholder = props.placeholder;
  if (props.placeholderStyle) result['placeholder-style'] = props.placeholderStyle;
  if (props.placeholderClass) result['placeholder-class'] = props.placeholderClass;
  if (props.maxlength !== undefined) result.maxlength = String(props.maxlength);
  if (props.cursorSpacing !== undefined) result['cursor-spacing'] = String(props.cursorSpacing);
  if (props.autoFocus) result['auto-focus'] = 'true';
  if (props.disabled) result.disabled = 'true';
  if (props.focus !== undefined) result.focus = props.focus ? 'true' : 'false';
  if (props.confirmType) result['confirm-type'] = props.confirmType;
  if (props.alwaysEmbed) result['always-embed'] = 'true';
  if (props.confirmHold) result['confirm-hold'] = 'true';
  if (props.cursor !== undefined) result.cursor = String(props.cursor);
  if (props.cursorColor) result['cursor-color'] = props.cursorColor;
  if (props.selectionStart !== undefined) result['selection-start'] = String(props.selectionStart);
  if (props.selectionEnd !== undefined) result['selection-end'] = String(props.selectionEnd);
  if (props.adjustPosition === false) result['adjust-position'] = 'false';
  if (props.holdKeyboard) result['hold-keyboard'] = 'true';

  if (props.bindInput) result.bindinput = props.bindInput;
  if (props.bindChange) result.bindchange = props.bindChange;
  if (props.bindFocus) result.bindfocus = props.bindFocus;
  if (props.bindBlur) result.bindblur = props.bindBlur;
  if (props.bindConfirm) result.bindconfirm = props.bindConfirm;
  if (props.bindKeyboardHeightChange) result.bindkeyboardheightchange = props.bindKeyboardHeightChange;
  if (props.bindNicknameReview) result.bindnicknamereview = props.bindNicknameReview;
  if (props.catchInput) result.catchinput = props.catchInput;
  if (props.catchChange) result.catchchange = props.catchChange;
  if (props.catchFocus) result.catchfocus = props.catchFocus;
  if (props.catchBlur) result.catchblur = props.catchBlur;
  if (props.catchConfirm) result.catchconfirm = props.catchConfirm;
  if (props.catchKeyboardHeightChange) result.catchkeyboardheightchange = props.catchKeyboardHeightChange;
  if (props.catchNicknameReview) result.catchnicknamereview = props.catchNicknameReview;

  appendBindingAndDatasetAttrs(attrs as Record<string, unknown>, result as Record<string, string>);

  result.class = props.class ?? attrs.class;
  result.style = props.style ?? attrs.style;
  return result;
});

defineExpose({ el: elementRef });

const renderInput = () => h('lx-input', { ref: elementRef, ...inputProps.value });
</script>

<template>
  <renderInput />
</template>
