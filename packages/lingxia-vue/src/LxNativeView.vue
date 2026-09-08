<script setup lang="ts">
import type { LxNativeViewProps } from './types.js';
import { h, useAttrs, useSlots } from 'vue';
import { registerNativeViewComponent } from '@lingxia/elements';

const props = defineProps<LxNativeViewProps>();
const slots = useSlots();
const attrs = useAttrs();

if (typeof window !== 'undefined') {
  registerNativeViewComponent();
}

const render = () => h('lx-native-view', {
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
  role: props.role,
}, slots.default?.());
</script>

<template>
  <render />
</template>
