<script setup lang="ts">
import type { LxNativeTextProps } from './types.js';
import { h, useAttrs, useSlots } from 'vue';
import { registerNativeTextComponent } from '@lingxia/elements';

const props = defineProps<LxNativeTextProps>();
const slots = useSlots();
const attrs = useAttrs();

if (typeof window !== 'undefined') {
  registerNativeTextComponent();
}

const render = () => h('lx-native-text', {
  ...attrs,
  id: props.id,
  class: props.class,
  style: props.style,
  'aria-label': props['aria-label'],
  'aria-description': props['aria-description'],
  'aria-hidden': props['aria-hidden'],
  'automation-id': props.automationId,
  'pointer-events': props.pointerEvents,
  hidden: props.hidden,
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
