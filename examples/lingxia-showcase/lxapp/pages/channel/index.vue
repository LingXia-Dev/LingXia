<template>
  <div class="min-h-screen bg-gray-50 px-4 py-6">
    <div class="mb-4 flex items-center gap-2">
      <div
        class="h-2 w-2 rounded-full"
        :class="session.connected.value ? 'bg-green-500' : session.connecting.value ? 'bg-yellow-400' : 'bg-gray-300'"
      />
      <span class="text-xs text-gray-500">
        {{ session.connected.value ? 'Connected' : session.connecting.value ? 'Connecting...' : 'Disconnected' }}
      </span>
    </div>

    <div class="mb-6 flex gap-2 overflow-x-auto">
      <button
        v-for="symbol in symbols"
        :key="symbol"
        type="button"
        @click="switchSymbol(symbol)"
        class="rounded-full px-4 py-2 text-sm font-medium transition-colors"
        :class="symbol === active ? 'bg-blue-600 text-white' : 'border border-gray-200 bg-white text-gray-700'"
      >
        {{ symbol }}
      </button>
    </div>

    <div class="mb-6 rounded-2xl border border-gray-200 bg-white p-6 shadow-sm">
      <p class="mb-1 text-xs font-medium uppercase tracking-wider text-gray-400">
        {{ active || '---' }}
      </p>
      <div class="flex items-baseline gap-3">
        <span class="text-4xl font-bold text-gray-900">
          {{ latest ? `$${latest.price.toFixed(2)}` : '---' }}
        </span>
        <span
          v-if="latest"
          class="text-lg font-semibold"
          :class="changeColor"
        >
          {{ changePrefix }}{{ latest.change.toFixed(2) }}
        </span>
      </div>
      <p v-if="latest" class="mt-1 text-xs text-gray-400">
        {{ formatTime(latest.timestamp) }}
      </p>
    </div>

    <div class="overflow-hidden rounded-2xl border border-gray-200 bg-white shadow-sm">
      <div class="border-b border-gray-100 px-4 py-3">
        <p class="text-xs font-semibold uppercase tracking-wider text-gray-500">Recent Ticks</p>
      </div>
      <div class="max-h-64 overflow-y-auto">
        <p v-if="history.length === 0" class="px-4 py-6 text-center text-sm text-gray-400">
          Waiting for data...
        </p>
        <div v-else class="divide-y divide-gray-50">
          <div
            v-for="(tick, index) in reversedHistory"
            :key="`${tick.timestamp}-${index}`"
            class="flex items-center justify-between px-4 py-2.5"
          >
            <span class="font-mono text-sm text-gray-800">${{ tick.price.toFixed(2) }}</span>
            <div class="flex items-center gap-3">
              <span
                class="text-sm font-medium"
                :class="tick.change > 0 ? 'text-green-600' : tick.change < 0 ? 'text-red-500' : 'text-gray-500'"
              >
                {{ tick.change > 0 ? '+' : '' }}{{ tick.change.toFixed(2) }}
              </span>
              <span class="text-xs text-gray-400">{{ formatTime(tick.timestamp) }}</span>
            </div>
          </div>
        </div>
      </div>
    </div>

    <div class="mt-6 flex gap-3">
      <button
        type="button"
        @click="session.close()"
        :disabled="!session.connected.value"
        class="flex-1 rounded-xl bg-gray-200 py-3 text-sm font-medium text-gray-700 disabled:opacity-40"
      >
        Disconnect
      </button>
      <button
        type="button"
        @click="session.reopen()"
        :disabled="session.connected.value || session.connecting.value"
        class="flex-1 rounded-xl bg-blue-600 py-3 text-sm font-medium text-white disabled:opacity-40"
      >
        Reconnect
      </button>
    </div>

    <p v-if="session.error.value" class="mt-3 text-center text-xs text-red-500">
      Error: {{ session.error.value.code }}{{ session.error.value.message ? ` - ${session.error.value.message}` : '' }}
    </p>
  </div>
</template>

<script setup lang="ts">
import { computed, ref, watch } from 'vue';
import { useLxChannel } from '@lingxia/vue';
import type { ClientCommand, ServerMessage } from './index';
import '../../tailwind.css';

interface TickerSnapshot {
  symbol: string;
  price: number;
  change: number;
  timestamp: number;
}

function openTickerSession(params: Record<string, unknown>) {
  const bridge = (window as typeof window & {
    LingXiaBridge?: {
      raw?: {
        channel?: {
          open?: <TIn, TOut>(topic: string, payload: Record<string, unknown>) => Promise<unknown>;
        };
      };
    };
  }).LingXiaBridge;

  if (!bridge?.raw?.channel?.open) {
    return Promise.reject(new Error('LingXiaBridge channel API is not ready'));
  }

  return bridge.raw.channel.open<ServerMessage, ClientCommand>('tickerSession', params);
}

const session = useLxChannel(openTickerSession, {
  params: () => ({}),
});

const symbols = ref<string[]>([]);
const active = ref('');
const history = ref<TickerSnapshot[]>([]);
const latest = ref<TickerSnapshot | null>(null);

watch(
  () => session.last.value,
  (msg) => {
    if (!msg) return;
    if (msg.type === 'init') {
      symbols.value = msg.symbols;
      active.value = msg.active;
      history.value = [];
      latest.value = null;
      return;
    }

    const snapshot: TickerSnapshot = {
      symbol: msg.symbol,
      price: msg.price,
      change: msg.change,
      timestamp: msg.timestamp,
    };
    latest.value = snapshot;
    history.value = [...history.value.slice(-29), snapshot];
  },
);

const reversedHistory = computed(() => [...history.value].reverse());
const changeColor = computed(() => {
  if (!latest.value) return 'text-gray-600';
  if (latest.value.change > 0) return 'text-green-600';
  if (latest.value.change < 0) return 'text-red-500';
  return 'text-gray-600';
});
const changePrefix = computed(() => (latest.value && latest.value.change > 0 ? '+' : ''));

function switchSymbol(symbol: string) {
  if (symbol === active.value || !session.connected.value) return;
  active.value = symbol;
  history.value = [];
  latest.value = null;
  session.send({ type: 'subscribe', symbol });
}

function formatTime(timestamp: number) {
  return new Date(timestamp).toLocaleTimeString();
}
</script>
