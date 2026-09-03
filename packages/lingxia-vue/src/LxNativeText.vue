<script setup lang="ts">
import { h, useAttrs, useSlots } from 'vue';
import { registerNativeTextComponent } from '@lingxia/elements';

const props = defineProps<{
  id?: string;
  automationId?: string;
  class?: string;
  pointerEvents?: 'auto' | 'none' | 'box-only' | 'box-none';
  hidden?: boolean;
  hiddenTransition?: 'none' | 'fade';
  maxLines?: number;
  dir?: 'ltr' | 'rtl' | 'auto';
  fontSize?: number | string;
  fontWeight?: number | string;
  lineHeight?: number | string;
  textAlign?: 'start' | 'center' | 'end';
  color?: string;
}>();
const slots = useSlots();
const attrs = useAttrs();

if (typeof window !== 'undefined') {
  registerNativeTextComponent();
}

const render = () => h('lx-native-text', {
  ...attrs,
  id: props.id,
  class: props.class,
  'automation-id': props.automationId,
  'pointer-events': props.pointerEvents,
  hidden: props.hidden,
  'hidden-transition': props.hiddenTransition,
  'max-lines': props.maxLines,
  dir: props.dir,
  'font-size': props.fontSize,
  'font-weight': props.fontWeight,
  'line-height': props.lineHeight,
  'text-align': props.textAlign,
  color: props.color,
}, slots.default?.());
</script>

<template>
  <render />
</template>
