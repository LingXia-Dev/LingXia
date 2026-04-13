<script setup lang="ts">
import { ref, computed, watch, nextTick, h, defineComponent, onMounted, onBeforeUnmount, type PropType } from 'vue';
import * as echarts from 'echarts';
import { useLxPage, useLxStream } from '@lingxia/vue';
import type { LxStream } from '@lingxia/bridge';
import type { Message, ChatChunk, ChartData } from './index';
import '../../tailwind.css';

const PALETTE = ['#3b82f6', '#8b5cf6', '#10b981', '#f59e0b', '#ef4444', '#06b6d4'];

function buildOption(data: ChartData): echarts.EChartsOption {
  const labels = data.series.map((s) => s.label);
  const values = data.series.map((s) => s.value);

  if (data.kind === 'pie') {
    return {
      color: PALETTE,
      tooltip: { trigger: 'item', formatter: '{b}: {d}%' },
      legend: {
        bottom: 0,
        textStyle: { fontSize: 11, color: '#6b7280' },
        icon: 'circle',
        itemWidth: 8,
        itemHeight: 8,
        itemGap: 12,
      },
      series: [
        {
          type: 'pie',
          radius: ['40%', '68%'],
          center: ['50%', '44%'],
          data: data.series.map((s) => ({ name: s.label, value: s.value })),
          label: { show: false },
          emphasis: {
            label: { show: true, fontSize: 13, fontWeight: 'bold' as const },
            scale: true,
            scaleSize: 5,
          },
          animationType: 'scale',
          animationEasing: 'elasticOut' as const,
        },
      ],
    };
  }

  const isLine = data.kind === 'line';
  return {
    color: PALETTE,
    grid: { top: 10, right: 10, bottom: 24, left: 10, containLabel: true },
    tooltip: { trigger: 'axis', axisPointer: { type: 'shadow' } },
    xAxis: {
      type: 'category',
      data: labels,
      axisLine: { lineStyle: { color: '#e5e7eb' } },
      axisTick: { show: false },
      axisLabel: { fontSize: 11, color: '#6b7280' },
    },
    yAxis: {
      type: 'value',
      splitLine: { lineStyle: { color: '#f3f4f6', type: 'dashed' } },
      axisLabel: { fontSize: 11, color: '#6b7280' },
      axisLine: { show: false },
      axisTick: { show: false },
    },
    series: [
      {
        type: isLine ? 'line' : 'bar',
        data: values,
        smooth: isLine ? 0.4 : false,
        symbolSize: isLine ? 6 : undefined,
        areaStyle: isLine
          ? {
              color: new echarts.graphic.LinearGradient(0, 0, 0, 1, [
                { offset: 0, color: 'rgba(59,130,246,0.22)' },
                { offset: 1, color: 'rgba(59,130,246,0)' },
              ]),
            }
          : undefined,
        lineStyle: isLine ? { width: 2.5 } : undefined,
        itemStyle: { borderRadius: isLine ? undefined : [4, 4, 0, 0] },
        barMaxWidth: 36,
      },
    ],
  };
}

const ChartCard = defineComponent({
  name: 'ChartCard',
  props: {
    data: { type: Object as PropType<ChartData>, required: true },
  },
  setup(props) {
    const containerRef = ref<HTMLDivElement>();
    let chart: echarts.ECharts | null = null;

    onMounted(() => {
      if (!containerRef.value) return;
      chart = echarts.init(containerRef.value, null, { renderer: 'svg' });
      chart.setOption(buildOption(props.data));
    });

    onBeforeUnmount(() => {
      chart?.dispose();
      chart = null;
    });

    const height = computed(() => (props.data.kind === 'pie' ? 210 : 180));

    return () =>
      h(
        'div',
        { class: 'mt-3 rounded-2xl overflow-hidden bg-gray-50 border border-gray-200 shadow-sm animate-chart-in' },
        [
          h(
            'p',
            { class: 'text-[10px] font-semibold tracking-widest uppercase text-gray-400 px-3.5 pt-3 pb-0.5' },
            props.data.title,
          ),
          h('div', { ref: containerRef, style: { width: '100%', height: height.value + 'px' } }),
        ],
      );
  },
});

interface StreamState {
  text: string;
  chart?: ChartData;
}

const HINTS = [
  'Tell me about LingXia streaming',
  'Show me some data',
  'How does the bridge protocol work?',
];

const { data, actions } = useLxPage<
  { messages: Message[] },
  {
    onSend: (params: { text: string }) => LxStream<ChatChunk, void>;
    onClear: () => void;
  }
>();

const messages = computed(() => data?.messages ?? []);
const inputText = ref('');
const scrollRef = ref<HTMLDivElement>();

const chat = useLxStream<typeof actions.onSend, StreamState>(actions.onSend, {
  params: () => ({ text: inputText.value }),
  manual: true,
  initial: { text: '' },
  reduce: (acc, chunk) => {
    if (chunk.type === 'token') return { ...acc, text: acc.text + chunk.token };
    if (chunk.type === 'artifact') return { ...acc, chart: chunk.chart };
    return acc;
  },
});

const streamState = computed<StreamState>(() => chat.data.value ?? { text: '' });

function scrollToBottom() {
  nextTick(() => {
    const el = scrollRef.value;
    if (el) el.scrollTop = el.scrollHeight;
  });
}

watch([messages, () => chat.data.value], scrollToBottom);

function handleSend() {
  const text = inputText.value.trim();
  if (!text || chat.streaming.value) return;
  chat.start();        // params 在此处读取 inputText.value，必须在清空之前
  inputText.value = '';
}

function handleKeyDown(e: KeyboardEvent) {
  if (e.key === 'Enter' && !e.shiftKey) {
    e.preventDefault();
    handleSend();
  }
}

function autoResize(e: Event) {
  const el = e.target as HTMLTextAreaElement;
  el.style.height = 'auto';
  el.style.height = Math.min(el.scrollHeight, 120) + 'px';
}
</script>

<template>
  <div class="flex flex-col bg-gray-100" style="height: 100vh">
    <!-- Clear button -->
    <div
      v-if="messages.length > 0 && !chat.streaming.value"
      class="absolute top-3 right-4 z-10"
    >
      <button
        class="text-xs text-blue-600 px-3 py-1 bg-white rounded-full shadow-sm active:opacity-70"
        @click="actions.onClear()"
      >
        Clear
      </button>
    </div>

    <!-- Scroll area -->
    <div ref="scrollRef" class="flex-1 overflow-y-auto px-4 py-4">
      <!-- Empty state -->
      <div
        v-if="messages.length === 0 && !chat.streaming.value"
        class="flex-1 flex flex-col items-center justify-center gap-3 px-8 text-center"
      >
        <div class="w-16 h-16 rounded-2xl bg-white shadow flex items-center justify-center">
          <svg viewBox="0 0 24 24" fill="none" class="w-8 h-8" stroke="#2563EB" stroke-width="1.5">
            <path
              stroke-linecap="round"
              stroke-linejoin="round"
              d="M8.625 12a.375.375 0 11-.75 0 .375.375 0 01.75 0zm0 0H8.25m4.125 0a.375.375 0 11-.75 0 .375.375 0 01.75 0zm0 0H12m4.125 0a.375.375 0 11-.75 0 .375.375 0 01.75 0zm0 0h-.375M21 12c0 4.556-4.03 8.25-9 8.25a9.764 9.764 0 01-2.555-.337A5.972 5.972 0 015.41 20.97a5.969 5.969 0 01-.474-.065 4.48 4.48 0 00.978-2.025c.09-.457-.133-.901-.467-1.226C3.93 16.178 3 14.189 3 12c0-4.556 4.03-8.25 9-8.25s9 3.694 9 8.25z"
            />
          </svg>
        </div>
        <div>
          <p class="text-base font-semibold text-gray-800">AI Chat</p>
          <p class="text-sm text-gray-500 mt-1">
            Streaming demo &mdash; text &amp; chart artifacts via LingXia bridge.
          </p>
        </div>
        <div class="flex flex-col gap-2 w-full mt-2">
          <div
            v-for="hint in HINTS"
            :key="hint"
            class="text-sm text-blue-600 bg-blue-50 rounded-xl px-4 py-2.5 text-left"
          >
            {{ hint }}
          </div>
        </div>
      </div>

      <!-- Messages -->
      <div v-else class="flex flex-col gap-3">
        <template v-for="msg in messages" :key="msg.id">
          <!-- User bubble -->
          <div v-if="msg.role === 'user'" class="flex justify-end">
            <div
              class="max-w-[78%] px-4 py-2.5 rounded-3xl rounded-br-md bg-blue-600 text-white text-sm leading-relaxed"
              style="word-break: break-word"
            >
              {{ msg.content }}
            </div>
          </div>

          <!-- Assistant bubble -->
          <div v-else class="flex justify-start">
            <div class="flex items-start gap-2 max-w-[90%]">
              <div class="w-7 h-7 rounded-full bg-gradient-to-br from-violet-500 to-blue-600 flex-shrink-0 flex items-center justify-center mt-0.5">
                <svg viewBox="0 0 24 24" fill="white" class="w-3.5 h-3.5">
                  <path d="M12 2a10 10 0 110 20A10 10 0 0112 2zm0 2a8 8 0 100 16A8 8 0 0012 4zm-1 5h2v2h-2V9zm0 4h2v6h-2v-6z" />
                </svg>
              </div>
              <div
                class="px-4 py-2.5 rounded-3xl rounded-bl-md bg-white border border-gray-200 text-gray-800 text-sm leading-relaxed shadow-sm"
                style="word-break: break-word"
              >
                <template v-if="msg.content">{{ msg.content }}</template>
                <span v-else class="text-gray-400 italic">...</span>
                <ChartCard v-if="msg.chart" :data="msg.chart" />
              </div>
            </div>
          </div>
        </template>

        <!-- Streaming bubble -->
        <div v-if="chat.streaming.value" class="flex justify-start">
          <div class="flex items-start gap-2 max-w-[90%]">
            <div class="w-7 h-7 rounded-full bg-gradient-to-br from-violet-500 to-blue-600 flex-shrink-0 flex items-center justify-center mt-0.5">
              <svg viewBox="0 0 24 24" fill="white" class="w-3.5 h-3.5">
                <path d="M12 2a10 10 0 110 20A10 10 0 0112 2zm0 2a8 8 0 100 16A8 8 0 0012 4zm-1 5h2v2h-2V9zm0 4h2v6h-2v-6z" />
              </svg>
            </div>
            <div
              class="px-4 py-2.5 rounded-3xl rounded-bl-md bg-white border border-gray-200 text-gray-800 text-sm leading-relaxed shadow-sm"
              style="word-break: break-word"
            >
              <template v-if="streamState.text">
                {{ streamState.text }}
                <span
                  v-if="!streamState.chart"
                  class="inline-block w-0.5 h-[1.1em] bg-blue-500 ml-0.5 align-middle animate-blink"
                />
              </template>
              <span v-else class="text-gray-400 italic">
                ...
                <span class="inline-block w-0.5 h-[1.1em] bg-blue-400 ml-0.5 align-middle animate-blink" />
              </span>
              <ChartCard v-if="streamState.chart" :data="streamState.chart" />
            </div>
          </div>
        </div>
      </div>
    </div>

    <!-- Input bar -->
    <div
      class="bg-white border-t border-gray-200 px-3 py-3 flex items-end gap-2"
      :style="{ paddingBottom: 'max(12px, env(safe-area-inset-bottom))' }"
    >
      <div class="flex-1 bg-gray-100 rounded-2xl px-3.5 py-2.5 flex items-end gap-2">
        <textarea
          :value="inputText"
          :disabled="chat.streaming.value"
          rows="1"
          placeholder="Message..."
          class="flex-1 bg-transparent text-sm text-gray-800 placeholder-gray-400 outline-none resize-none leading-relaxed"
          style="max-height: 120px; min-height: 22px"
          @input="inputText = ($event.target as HTMLTextAreaElement).value; autoResize($event)"
          @keydown="handleKeyDown"
        />
      </div>

      <button
        v-if="chat.streaming.value"
        class="w-9 h-9 flex-shrink-0 rounded-full bg-gray-800 flex items-center justify-center active:opacity-70"
        @click="chat.cancel()"
      >
        <div class="w-3 h-3 bg-white rounded-sm" />
      </button>

      <button
        v-else
        :disabled="!inputText.trim()"
        class="w-9 h-9 flex-shrink-0 rounded-full bg-blue-600 flex items-center justify-center active:opacity-70 disabled:opacity-30 disabled:cursor-not-allowed"
        @click="handleSend"
      >
        <svg viewBox="0 0 24 24" fill="white" class="w-4 h-4" style="margin-bottom: 1px">
          <path d="M12 4l8 8H14v8h-4v-8H4l8-8z" />
        </svg>
      </button>
    </div>
  </div>
</template>
