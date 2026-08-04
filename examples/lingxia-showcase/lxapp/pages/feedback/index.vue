<template>
  <main class="min-h-screen bg-surface-50 px-5 py-6 text-gray-900">
    <div class="mx-auto flex w-full max-w-lg flex-col gap-6">
      <header>
        <div class="mb-3 flex h-11 w-11 items-center justify-center rounded-2xl bg-emerald-100 text-xl">
          ✦
        </div>
        <h1 class="text-2xl font-semibold tracking-tight">Help us improve</h1>
        <p class="mt-2 text-sm leading-6 text-gray-500">
          Share a bug, an idea, or anything that felt confusing in the showcase.
        </p>
      </header>

      <section class="space-y-3">
        <div class="text-sm font-medium text-gray-700">What is this about?</div>
        <div class="grid grid-cols-3 gap-2">
          <button
            v-for="item in categories"
            :key="item"
            type="button"
            @click="category = item"
            :class="[
              'h-10 rounded-xl border text-sm font-medium transition-colors',
              category === item
                ? 'border-emerald-500 bg-emerald-50 text-emerald-700 dark:text-emerald-400'
                : 'border-line-200 bg-surface text-gray-600'
            ]"
          >
            {{ item }}
          </button>
        </div>
      </section>

      <label class="space-y-2">
        <span class="text-sm font-medium text-gray-700">Your feedback</span>
        <textarea
          v-model="message"
          placeholder="What happened, and what would you prefer?"
          rows="6"
          class="w-full resize-none rounded-2xl border border-line-200 bg-surface px-4 py-3 text-sm leading-6 outline-hidden transition focus:border-emerald-500 focus:ring-2 focus:ring-emerald-100"
        />
      </label>

      <label class="space-y-2">
        <span class="text-sm font-medium text-gray-700">Email (optional)</span>
        <input
          v-model="email"
          type="email"
          placeholder="you@example.com"
          class="h-11 w-full rounded-xl border border-line-200 bg-surface px-4 text-sm outline-hidden transition focus:border-emerald-500 focus:ring-2 focus:ring-emerald-100"
        />
      </label>

      <button
        type="button"
        :disabled="!message.trim() || submitting"
        @click="submit"
        class="h-12 rounded-2xl bg-surface-900 text-sm font-semibold text-white transition disabled:cursor-not-allowed disabled:opacity-35"
      >
        {{ submitting ? 'Sending…' : 'Send feedback' }}
      </button>

      <p class="text-center text-xs text-gray-400">
        Demo only — submissions are written to the Logic log.
      </p>
    </div>
  </main>
</template>

<script setup lang="ts">
import { ref } from 'vue';
import { useLxPage } from '@lingxia/vue';
import '../../tailwind.css';
import './feedback.css';

type FeedbackCategory = 'Product' | 'Bug' | 'Idea';

type PageData = Record<string, never>;

type PageActions = {
  submitFeedback: (params: {
    category: FeedbackCategory;
    message: string;
    email: string;
  }) => Promise<void>;
};

const categories: FeedbackCategory[] = ['Product', 'Bug', 'Idea'];
const { actions } = useLxPage<PageData, PageActions>();
const category = ref<FeedbackCategory>('Product');
const message = ref('');
const email = ref('');
const submitting = ref(false);

async function submit() {
  if (!message.value.trim() || submitting.value) return;
  submitting.value = true;
  try {
    await actions.submitFeedback({
      category: category.value,
      message: message.value,
      email: email.value,
    });
  } finally {
    submitting.value = false;
  }
}
</script>
