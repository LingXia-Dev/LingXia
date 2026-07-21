<template>
  <div class="flex h-screen flex-col bg-gray-100" data-testid="stream-page">
    <div v-if="messages.length > 0 && !chat.streaming.value" class="absolute right-4 top-3 z-10">
      <button
        type="button"
        @click="actions.onClear()"
        class="rounded-full bg-white px-3 py-1 text-xs text-blue-600 shadow-sm active:opacity-70"
      >
        Clear
      </button>
    </div>

    <div ref="scrollRef" class="flex-1 overflow-y-auto px-4 py-4">
      <div v-if="messages.length === 0 && !chat.streaming.value" class="flex h-full flex-col items-center justify-center gap-3 px-8 text-center">
        <div class="flex h-16 w-16 items-center justify-center rounded-2xl bg-white shadow">
          <svg viewBox="0 0 24 24" fill="none" class="h-8 w-8" stroke="#2563EB" stroke-width="1.5">
            <path
              stroke-linecap="round"
              stroke-linejoin="round"
              d="M8.625 12a.375.375 0 11-.75 0 .375.375 0 01.75 0zm0 0H8.25m4.125 0a.375.375 0 11-.75 0 .375.375 0 01.75 0zm0 0H12m4.125 0a.375.375 0 11-.75 0 .375.375 0 01.75 0zm0 0h-.375M21 12c0 4.556-4.03 8.25-9 8.25a9.764 9.764 0 01-2.555-.337A5.972 5.972 0 015.41 20.97a5.969 5.969 0 01-.474-.065 4.48 4.48 0 00.978-2.025c.09-.457-.133-.901-.467-1.226C3.93 16.178 3 14.189 3 12c0-4.556 4.03-8.25 9-8.25s9 3.694 9 8.25z"
            />
          </svg>
        </div>
        <div>
          <p class="text-base font-semibold text-gray-800">Stream Demo</p>
          <p class="mt-1 text-sm text-gray-500">
            Async generator streaming - text and chart artifacts via LingXia bridge.
          </p>
        </div>
        <div class="mt-2 flex w-full flex-col gap-2">
          <div
            v-for="hint in hints"
            :key="hint"
            class="rounded-xl bg-blue-50 px-4 py-2.5 text-left text-sm text-blue-600"
          >
            {{ hint }}
          </div>
        </div>
      </div>

      <div v-else class="flex flex-col gap-3">
        <div
          v-for="message in messages"
          :key="message.id"
          data-testid="stream-message"
          :data-role="message.role"
          class="flex"
          :class="message.role === 'user' ? 'justify-end' : 'justify-start'"
        >
          <div v-if="message.role === 'user'" class="max-w-[78%] rounded-3xl rounded-br-md bg-blue-600 px-4 py-2.5 text-sm leading-relaxed text-white break-words">
            {{ message.content }}
          </div>

          <div v-else class="flex max-w-[90%] items-start gap-2">
            <div class="mt-0.5 flex h-7 w-7 flex-shrink-0 items-center justify-center rounded-full bg-gradient-to-br from-violet-500 to-blue-600">
              <svg viewBox="0 0 24 24" fill="white" class="h-3.5 w-3.5">
                <path d="M12 2a10 10 0 110 20A10 10 0 0112 2zm0 2a8 8 0 100 16A8 8 0 0012 4zm-1 5h2v2h-2V9zm0 4h2v6h-2v-6z" />
              </svg>
            </div>
            <div class="break-words rounded-3xl rounded-bl-md border border-gray-200 bg-white px-4 py-2.5 text-sm leading-relaxed text-gray-800 shadow-sm">
              <span v-if="message.content">{{ message.content }}</span>
              <span v-else class="italic text-gray-400">...</span>
              <ChartCard v-if="message.chart" :data="message.chart" />
            </div>
          </div>
        </div>

        <div v-if="chat.streaming.value" class="flex justify-start" data-testid="stream-live">
          <div class="flex max-w-[90%] items-start gap-2">
            <div class="mt-0.5 flex h-7 w-7 flex-shrink-0 items-center justify-center rounded-full bg-gradient-to-br from-violet-500 to-blue-600">
              <svg viewBox="0 0 24 24" fill="white" class="h-3.5 w-3.5">
                <path d="M12 2a10 10 0 110 20A10 10 0 0112 2zm0 2a8 8 0 100 16A8 8 0 0012 4zm-1 5h2v2h-2V9zm0 4h2v6h-2v-6z" />
              </svg>
            </div>
            <div class="break-words rounded-3xl rounded-bl-md border border-gray-200 bg-white px-4 py-2.5 text-sm leading-relaxed text-gray-800 shadow-sm">
              <template v-if="streamState.text">
                {{ streamState.text }}
                <span
                  v-if="!streamState.chart"
                  class="ml-0.5 inline-block h-[1.1em] w-0.5 animate-blink align-middle bg-blue-500"
                />
              </template>
              <span v-else class="italic text-gray-400">
                ...
                <span class="ml-0.5 inline-block h-[1.1em] w-0.5 animate-blink align-middle bg-blue-400" />
              </span>
              <ChartCard v-if="streamState.chart" :data="streamState.chart" />
            </div>
          </div>
        </div>
      </div>
    </div>

    <div
      class="flex items-end gap-2 border-t border-gray-200 bg-white px-3 py-3"
      style="padding-bottom: max(12px, env(safe-area-inset-bottom));"
    >
      <div class="flex flex-1 items-end gap-2 rounded-2xl bg-gray-100 px-3.5 py-2.5">
        <textarea
          data-testid="stream-input"
          :data-controlled-value="inputText"
          ref="textareaRef"
          v-model="inputText"
          rows="1"
          placeholder="Message..."
          :disabled="chat.streaming.value"
          class="max-h-[120px] min-h-[22px] flex-1 resize-none bg-transparent text-sm leading-relaxed text-gray-800 outline-none placeholder:text-gray-400"
          @input="autoResize"
          @keydown.enter.exact.prevent="handleSend"
        />
      </div>

      <button
        v-if="chat.streaming.value"
        data-testid="stream-stop"
        type="button"
        @click="chat.cancel()"
        class="flex h-9 w-9 flex-shrink-0 items-center justify-center rounded-full bg-gray-800 active:opacity-70"
      >
        <div class="h-3 w-3 rounded-sm bg-white" />
      </button>

      <button
        v-else
        data-testid="stream-send"
        type="button"
        :disabled="!inputText.trim()"
        class="flex h-9 w-9 flex-shrink-0 items-center justify-center rounded-full bg-blue-600 active:opacity-70 disabled:cursor-not-allowed disabled:opacity-30"
        @click="handleSend"
      >
        <svg viewBox="0 0 24 24" fill="white" class="h-4 w-4" style="margin-bottom: 1px;">
          <path d="M12 4l8 8H14v8h-4v-8H4l8-8z" />
        </svg>
      </button>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, nextTick, ref, watch } from 'vue';
import { useLxPage, useLxStream } from '@lingxia/vue';
import type { LxStream } from '@lingxia/bridge';
import type { ChartData, ChatChunk, Message } from './index';
import ChartCard from './ChartCard.vue';
import '../../tailwind.css';

interface PageData {
  messages: Message[];
}

interface PageActions {
  onSend: (params: { text: string }) => LxStream<ChatChunk, void>;
  onClear: () => void;
}

interface StreamState {
  text: string;
  chart?: ChartData;
}

const hints = [
  'Tell me about LingXia streaming',
  'Show me some data',
  'How does the bridge protocol work?',
];

const { data, actions } = useLxPage<PageData, PageActions>();
const messages = computed(() => data?.messages ?? []);
const inputText = ref('');
const submittedText = ref('');
const textareaRef = ref<HTMLTextAreaElement | null>(null);
const scrollRef = ref<HTMLDivElement | null>(null);

const chat = useLxStream<typeof actions.onSend, StreamState>(actions.onSend, {
  params: () => ({ text: submittedText.value }),
  manual: true,
  initial: { text: '' },
  reduce: (acc, chunk) => {
    if (chunk.type === 'token') return { ...acc, text: acc.text + chunk.token };
    if (chunk.type === 'artifact') return { ...acc, chart: chunk.chart };
    return acc;
  },
});

const streamState = computed(() => chat.data.value ?? { text: '' });

watch(
  [
    () => messages.value.length,
    () => streamState.value.text,
    () => streamState.value.chart?.title,
    () => chat.streaming.value,
  ],
  async () => {
    await nextTick();
    const element = scrollRef.value;
    if (element) {
      element.scrollTop = element.scrollHeight;
    }
  },
);

function autoResize() {
  const element = textareaRef.value;
  if (!element) return;
  element.style.height = 'auto';
  element.style.height = `${Math.min(element.scrollHeight, 120)}px`;
}

function resetTextarea() {
  const element = textareaRef.value;
  if (!element) return;
  element.style.height = 'auto';
}

function handleSend() {
  const text = inputText.value.trim();
  if (!text || chat.streaming.value) return;
  submittedText.value = text;
  inputText.value = '';
  resetTextarea();
  chat.start();
}
</script>

<style scoped>
.animate-blink {
  animation: blink 1s step-end infinite;
}

@keyframes blink {
  0%, 45% {
    opacity: 1;
  }
  50%, 100% {
    opacity: 0;
  }
}
</style>
