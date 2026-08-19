<script setup lang="ts">
import { registerNativeViewComponent } from '@lingxia/elements';

defineProps<{
  id?: string;
  automationId?: string;
  class?: string;
  pointerEvents?: 'auto' | 'none' | 'box-only' | 'box-none';
  hidden?: boolean;
  hiddenTransition?: 'none' | 'fade';
  role?: 'group' | 'region' | 'status' | 'presentation' | 'none';
}>();

if (typeof window !== 'undefined') {
  registerNativeViewComponent();
}
</script>

<template>
  <lx-native-view
    :id="id"
    :class="class"
    :automation-id="automationId"
    :pointer-events="pointerEvents"
    :hidden="hidden"
    :hidden-transition="hiddenTransition"
    :role="role"
  >
    <slot />
  </lx-native-view>
</template>
