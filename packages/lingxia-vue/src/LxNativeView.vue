<script setup lang="ts">
import { h, useAttrs, useSlots } from 'vue';
import { registerNativeViewComponent } from '@lingxia/elements';

const props = defineProps<{
  id?: string;
  automationId?: string;
  class?: string;
  pointerEvents?: 'auto' | 'none' | 'box-only' | 'box-none';
  hidden?: boolean;
  hiddenTransition?: 'none' | 'fade';
  role?: 'group' | 'region' | 'status' | 'presentation' | 'none';
}>();
const slots = useSlots();
const attrs = useAttrs();

if (typeof window !== 'undefined') {
  registerNativeViewComponent();
}

const render = () => h('lx-native-view', {
  ...attrs,
  id: props.id,
  class: props.class,
  'automation-id': props.automationId,
  'pointer-events': props.pointerEvents,
  hidden: props.hidden,
  'hidden-transition': props.hiddenTransition,
  role: props.role,
}, slots.default?.());
</script>

<template>
  <render />
</template>
