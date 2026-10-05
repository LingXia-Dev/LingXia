<template>
  <div class="min-h-screen bg-surface-100 overflow-y-auto">
    <div class="px-3 py-3 pb-12 space-y-3">
      <div class="bg-surface rounded-lg shadow-sm">
        <div class="px-4 py-4 border-b border-line-100">
          <div class="text-base text-gray-900 font-medium">Leave guard</div>
          <div class="text-xs text-gray-500 mt-1">
            Edit the draft, then go back or home: the page stays and asks. Save, and it leaves at once.
          </div>
        </div>
        <div class="px-4 py-3 space-y-3">
          <input
            v-model="draft"
            data-testid="leave-draft"
            class="w-full rounded border border-line-200 px-3 py-2 text-sm"
            placeholder="Type something"
          />
          <div class="flex items-center justify-between text-xs text-gray-500">
            <span data-testid="leave-status">{{ dirty ? 'Unsaved changes' : 'Saved' }}</span>
            <span data-testid="leave-requests">leave requests: {{ requests }}</span>
          </div>
          <button
            data-testid="leave-save"
            class="w-full rounded bg-blue-500 py-2 text-sm text-white disabled:opacity-50"
            :disabled="!dirty"
            @click="saved = draft"
          >
            Save
          </button>
        </div>
      </div>

      <div v-if="asking" class="bg-surface rounded-lg shadow-sm px-4 py-4 space-y-3" data-testid="leave-confirm">
        <div class="text-sm text-gray-900 font-medium">Discard unsaved changes?</div>
        <div class="flex gap-2">
          <button
            data-testid="leave-keep"
            class="flex-1 rounded border border-line-200 py-2 text-sm text-gray-700"
            @click="decide(false)"
          >
            Keep editing
          </button>
          <button
            data-testid="leave-discard"
            class="flex-1 rounded bg-red-500 py-2 text-sm text-white"
            @click="decide(true)"
          >
            Discard
          </button>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, ref } from 'vue';
import { useLxLeaveGuard } from '@lingxia/vue';
import '../../tailwind.css';

const saved = ref('');
const draft = ref('');
const asking = ref(false);
const requests = ref(0);
const dirty = computed(() => draft.value !== saved.value);

let answer: ((leave: boolean) => void) | undefined;

// While the draft differs from what was saved, back and home stay here and
// ask; the promise resolves with the user's choice.
useLxLeaveGuard(dirty, () => {
  requests.value += 1;
  asking.value = true;
  return new Promise<boolean>((resolve) => {
    answer = resolve;
  });
});

function decide(leave: boolean): void {
  asking.value = false;
  answer?.(leave);
}
</script>
