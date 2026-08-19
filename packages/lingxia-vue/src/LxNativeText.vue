<script setup lang="ts">
import { registerNativeTextComponent } from '@lingxia/elements';

defineProps<{
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

if (typeof window !== 'undefined') {
  registerNativeTextComponent();
}
</script>

<template>
  <lx-native-text
    :id="id"
    :class="class"
    :automation-id="automationId"
    :pointer-events="pointerEvents"
    :hidden="hidden"
    :hidden-transition="hiddenTransition"
    :max-lines="maxLines"
    :dir="dir"
    :font-size="fontSize"
    :font-weight="fontWeight"
    :line-height="lineHeight"
    :text-align="textAlign"
    :color="color"
  >
    <slot />
  </lx-native-text>
</template>
