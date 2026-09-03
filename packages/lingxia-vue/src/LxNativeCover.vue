<script setup lang="ts">
import { h, useAttrs, useSlots } from 'vue';
import { registerNativeCoverComponent } from '@lingxia/elements';

const props = defineProps<{
  id?: string;
  automationId?: string;
  class?: string;
  pointerEvents?: 'auto' | 'none' | 'box-only' | 'box-none';
  hidden?: boolean;
  hiddenTransition?: 'none' | 'fade';
  scrim?: 'none' | 'top' | 'bottom' | 'full';
  scrimOpacity?: number;
  role?: 'group' | 'region' | 'status' | 'presentation' | 'none';
}>();
const slots = useSlots();
const attrs = useAttrs();

if (typeof window !== 'undefined') {
  registerNativeCoverComponent();
}

const render = () => h('lx-native-cover', {
  ...attrs,
  id: props.id,
  class: props.class,
  'automation-id': props.automationId,
  'pointer-events': props.pointerEvents,
  hidden: props.hidden,
  'hidden-transition': props.hiddenTransition,
  scrim: props.scrim,
  'scrim-opacity': props.scrimOpacity,
  role: props.role,
}, slots.default?.());
</script>

<template>
  <render />
</template>
