<script setup lang="ts">
import { registerNativeCoverComponent } from '@lingxia/elements';

defineProps<{
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

if (typeof window !== 'undefined') {
  registerNativeCoverComponent();
}
</script>

<template>
  <lx-native-cover
    :id="id"
    :class="class"
    :automation-id="automationId"
    :pointer-events="pointerEvents"
    :hidden="hidden"
    :hidden-transition="hiddenTransition"
    :scrim="scrim"
    :scrim-opacity="scrimOpacity"
    :role="role"
  >
    <slot />
  </lx-native-cover>
</template>
