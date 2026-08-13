<template>
  <div class="min-h-screen bg-surface-50" data-testid="lifecycle-page">
    <div class="px-4 py-5 space-y-4">
      <!-- Header -->
      <div class="bg-linear-to-br from-violet-500 via-violet-600 to-fuchsia-600 rounded-2xl px-5 py-6 shadow-lg">
        <div class="text-xl text-white font-bold">Page Reset</div>
        <div class="text-sm text-white/80 mt-1">Leaving a page ends its instance</div>
        <div class="text-xs text-white/70 mt-3 leading-relaxed">
          Bump both counters, open the popup, then go back and enter this page again.
          Logic <code>data</code>, view state and the DOM all come back fresh — the popup
          does not follow you in.
        </div>
      </div>

      <!-- Instance identity -->
      <div class="bg-surface rounded-xl shadow-sm border border-line-100 overflow-hidden">
        <div class="px-4 py-3 border-b border-line-100">
          <h3 class="text-base font-medium text-gray-900">Logic instance</h3>
          <p class="text-xs text-gray-500 mt-0.5">A new tag means a new Page instance</p>
        </div>
        <div class="px-4 py-4 space-y-3">
          <div class="flex items-center justify-between">
            <span class="text-sm text-gray-600">This instance</span>
            <span data-testid="lifecycle-instance-tag" class="text-sm font-mono font-medium text-violet-600 dark:text-violet-400">
              #{{ instanceTag || '…' }}
            </span>
          </div>
          <div class="flex items-center justify-between">
            <span class="text-sm text-gray-600">Previous instance</span>
            <span data-testid="lifecycle-previous-tag" class="text-sm font-mono text-gray-500">
              #{{ previousInstanceTag || '…' }}
            </span>
          </div>
          <div class="text-[11px] text-gray-400 leading-relaxed pt-1">
            The previous tag is read back from <code>lx.getStorage()</code>, which is
            app-scoped and survives the reset.
          </div>
        </div>
      </div>

      <!-- Counters -->
      <div class="bg-surface rounded-xl shadow-sm border border-line-100 overflow-hidden">
        <div class="px-4 py-3 border-b border-line-100">
          <h3 class="text-base font-medium text-gray-900">State that resets</h3>
        </div>
        <div class="p-4 grid grid-cols-2 gap-3">
          <button
            data-testid="lifecycle-bump-logic"
            @click="bumpLogicCounter"
            class="flex flex-col items-center justify-center py-4 px-2 bg-violet-50 hover:bg-violet-100 active:bg-violet-200 text-violet-700 dark:text-violet-400 rounded-xl transition-colors"
          >
            <span class="text-2xl font-bold" data-testid="lifecycle-logic-counter">{{ logicCounter }}</span>
            <span class="text-sm font-medium mt-1">Logic +1</span>
            <span class="text-[10px] opacity-70">this.setData</span>
          </button>
          <button
            data-testid="lifecycle-bump-view"
            @click="viewCounter += 1"
            class="flex flex-col items-center justify-center py-4 px-2 bg-cyan-50 hover:bg-cyan-100 active:bg-cyan-200 text-cyan-700 dark:text-cyan-400 rounded-xl transition-colors"
          >
            <span class="text-2xl font-bold" data-testid="lifecycle-view-counter">{{ viewCounter }}</span>
            <span class="text-sm font-medium mt-1">View +1</span>
            <span class="text-[10px] opacity-70">ref()</span>
          </button>
        </div>
        <div class="px-4 pb-4 space-y-3">
          <button
            data-testid="lifecycle-open-popup"
            @click="popupOpen = true"
            class="w-full py-2.5 px-4 bg-linear-to-r from-violet-500 to-fuchsia-500 hover:from-violet-600 hover:to-fuchsia-600 text-white rounded-lg text-sm font-medium transition-all shadow-sm"
          >
            Open H5 popup
          </button>
          <button
            data-testid="lifecycle-go-back"
            @click="goBack"
            class="w-full py-2.5 px-4 bg-surface-100 hover:bg-surface-200 active:bg-surface-300 text-gray-700 rounded-lg text-sm font-medium transition-colors"
          >
            Go back (lx.navigateBack)
          </button>
        </div>
      </div>

      <!-- Lifecycle log -->
      <div class="bg-surface rounded-xl shadow-sm border border-line-100 overflow-hidden">
        <div class="px-4 py-3 border-b border-line-100">
          <h3 class="text-base font-medium text-gray-900">Lifecycle of this instance</h3>
          <p class="text-xs text-gray-500 mt-0.5">
            Stored in <code>data</code>, so it starts over with every entry
          </p>
        </div>
        <div class="p-4">
          <div v-if="events.length === 0" class="text-xs text-gray-400">No events yet</div>
          <div v-else class="space-y-2" data-testid="lifecycle-events">
            <div
              v-for="(event, index) in events"
              :key="`${event}-${index}`"
              class="text-xs font-mono text-gray-700 bg-surface-50 border border-line-100 rounded-lg px-3 py-2"
            >
              {{ event }}
            </div>
          </div>
          <div class="text-[11px] text-gray-400 leading-relaxed mt-3">
            Backgrounding the app fires <code>onHide</code> / <code>onShow</code> on the same
            instance. Only leaving the page fires <code>onUnload</code> and ends it.
          </div>
        </div>
      </div>
    </div>

    <!-- A plain H5 overlay — the kind that used to still be open on re-entry -->
    <div
      v-if="popupOpen"
      data-testid="lifecycle-popup"
      class="fixed inset-0 z-50 flex items-center justify-center bg-black/50 px-8"
      @click="popupOpen = false"
    >
      <div
        class="bg-surface rounded-2xl shadow-xl px-5 py-6 w-full max-w-xs text-center"
        @click.stop
      >
        <div class="text-base font-semibold text-gray-900">Pure H5 popup</div>
        <div class="text-xs text-gray-500 mt-2 leading-relaxed">
          Leave the page with this open. When you come back it is gone, because the
          document was reloaded behind the scenes.
        </div>
        <button
          data-testid="lifecycle-close-popup"
          @click="popupOpen = false"
          class="mt-4 w-full py-2 px-4 bg-surface-100 hover:bg-surface-200 text-gray-700 rounded-lg text-sm font-medium transition-colors"
        >
          Close
        </button>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, ref } from 'vue';
import { useLxPage } from '@lingxia/vue';
import '../../tailwind.css';

const { data, actions } = useLxPage();
const { bumpLogicCounter, goBack } = actions;

// View-local state: never leaves the WebView, so only a document reload clears it.
const viewCounter = ref(0);
const popupOpen = ref(false);

const instanceTag = computed(() => data.instanceTag ?? '');
const previousInstanceTag = computed(() => data.previousInstanceTag ?? '');
const logicCounter = computed(() => data.logicCounter ?? 0);
const events = computed<string[]>(() => data.events ?? []);
</script>
