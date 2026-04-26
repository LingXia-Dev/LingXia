<template>
  <div class="min-h-screen bg-gray-100 text-gray-900 flex flex-col items-center px-4 py-6">
    <div class="w-full max-w-md space-y-6">
      <header>
        <h1 class="text-lg font-semibold tracking-wide text-gray-900">Surface Page</h1>
        <p class="text-sm text-gray-500 mt-1">
          Inspect the query string and send a message to the opener.
        </p>
      </header>

      <section class="bg-white rounded-xl border border-gray-200 p-4 space-y-2 shadow-sm">
        <div class="text-xs uppercase text-gray-500 tracking-wide">Query String</div>
        <div class="font-mono text-sm text-gray-800 break-words">
          {{ queryString || '(none)' }}
        </div>
      </section>

      <section class="bg-white rounded-xl border border-gray-200 p-4 space-y-3 shadow-sm">
        <div class="text-xs uppercase text-gray-500 tracking-wide">Message</div>
        <input
          class="w-full px-3 py-2 rounded-md bg-white border border-gray-300 text-sm text-gray-900 focus:outline-none focus:ring-2 focus:ring-blue-500"
          placeholder="Message to parent page"
          ref="inputRef"
        />
        <button
          type="button"
          @click="handleSend"
          class="w-full h-10 text-sm font-medium rounded-md bg-blue-500 hover:bg-blue-600 text-white transition-colors"
        >
          Send and close
        </button>
      </section>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, computed } from 'vue';
import { useLxPage } from '@lingxia/vue';
import '../../tailwind.css';

const { data, actions } = useLxPage();
const { logSurfaceMessage } = actions;

const inputRef = ref<HTMLInputElement | null>(null);

const queryString = computed(() => data.queryString ?? '');

function handleSend() {
  const text = (inputRef.value?.value ?? '').trim();
  if (!text) return;

  try {
    logSurfaceMessage({ message: text });
    if (inputRef.value) inputRef.value.value = '';
  } catch (error) {
    console.error('logSurfaceMessage failed:', error);
  }
}
</script>
