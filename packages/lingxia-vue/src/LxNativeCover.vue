<script setup lang="ts">
import type { LxNativeCoverProps } from './types.js';
import { h, useAttrs, useSlots } from 'vue';
import { registerNativeCoverComponent } from '@lingxia/elements';
import { useNativeHostElement } from './text_component_shared.js';

const props = defineProps<LxNativeCoverProps>();
const slots = useSlots();
const attrs = useAttrs();

if (typeof window !== 'undefined') {
  registerNativeCoverComponent();
}

const elementRef = useNativeHostElement(props);

const render = () => h('lx-native-cover', {
  ...attrs,
  ref: elementRef,
  id: props.id,
  class: props.class,
  style: props.style,
  'aria-label': props['aria-label'],
  'aria-description': props['aria-description'],
  'aria-hidden': props['aria-hidden'],
  'automation-id': props.automationId,
  'pointer-events': props.pointerEvents,
  hidden: props.hidden,
  scrim: props.scrim,
  'scrim-opacity': props.scrimOpacity,
  role: props.role,
}, slots.default?.());
</script>

<template>
  <render />
</template>
