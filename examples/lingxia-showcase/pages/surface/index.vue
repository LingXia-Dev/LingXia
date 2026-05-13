<template>
  <div class="min-h-screen bg-gray-100 text-gray-900 flex flex-col items-center px-4 py-6">
    <div class="w-full max-w-md space-y-6">
      <header>
        <h1 class="text-lg font-semibold tracking-wide text-gray-900">Surface Page</h1>
        <p class="text-sm text-gray-500 mt-1">
          Inspect the query string, send messages, and toggle hide / close.
        </p>
      </header>

      <section class="bg-white rounded-xl border border-gray-200 p-4 space-y-2 shadow-sm">
        <div class="text-xs uppercase text-gray-500 tracking-wide">Query String</div>
        <div class="font-mono text-sm text-gray-800 break-words">
          {{ queryString || '(none)' }}
        </div>
      </section>

      <section class="bg-white rounded-xl border border-gray-200 p-4 space-y-3 shadow-sm">
        <div class="text-xs uppercase text-gray-500 tracking-wide">Page lifecycle</div>
        <div class="grid grid-cols-2 gap-2 text-sm">
          <div class="bg-gray-50 rounded-md px-3 py-2">
            <div class="text-xs text-gray-500">onShow</div>
            <div class="font-mono text-base text-gray-900">{{ showCount }}</div>
          </div>
          <div class="bg-gray-50 rounded-md px-3 py-2">
            <div class="text-xs text-gray-500">onHide</div>
            <div class="font-mono text-base text-gray-900">{{ hideCount }}</div>
          </div>
        </div>
        <div class="text-xs text-gray-500">
          Last event: <span class="font-mono text-gray-800">{{ lastLifecycle }}</span>
        </div>
      </section>

      <section class="bg-white rounded-xl border border-gray-200 p-4 space-y-3 shadow-sm">
        <div class="text-xs uppercase text-gray-500 tracking-wide">In-page counter</div>
        <div class="font-mono text-2xl text-gray-900">{{ counter }}</div>
        <div class="text-xs text-gray-500">
          Hide preserves this counter; close resets it on re-open.
        </div>
        <button type="button" @click="counter++"
          class="w-full h-10 text-sm font-medium rounded-md bg-gray-200 hover:bg-gray-300 text-gray-900 transition-colors">
          Increment
        </button>
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
          Send then close
        </button>
      </section>

      <section class="bg-white rounded-xl border border-gray-200 p-4 space-y-3 shadow-sm">
        <div class="text-xs uppercase text-gray-500 tracking-wide">Self actions</div>
        <button type="button" @click="hideSelf?.()"
          class="w-full h-10 text-sm font-medium rounded-md bg-amber-500 hover:bg-amber-600 text-white transition-colors">
          Hide (parent can show again)
        </button>
        <button type="button" @click="closeSelf?.()"
          class="w-full h-10 text-sm font-medium rounded-md bg-rose-500 hover:bg-rose-600 text-white transition-colors">
          Close (destroys this page)
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
const { logSurfaceMessage, hideSelf, closeSelf } = actions;

const inputRef = ref<HTMLInputElement | null>(null);
// Counter survives hide() → show() (page mount stays alive) but resets on close().
const counter = ref(0);

const queryString = computed(() => data.queryString ?? '');
const showCount = computed(() => data.showCount ?? 0);
const hideCount = computed(() => data.hideCount ?? 0);
const lastLifecycle = computed(() => data.lastLifecycle ?? 'onLoad');

function handleSend() {
  const text = (inputRef.value?.value ?? '').trim();
  if (!text) return;

  try {
    logSurfaceMessage({ message: text });
    if (inputRef.value) inputRef.value.value = '';
    closeSelf?.();
  } catch (error) {
    console.error('logSurfaceMessage failed:', error);
  }
}
</script>
