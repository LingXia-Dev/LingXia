<template>
  <div
    data-testid="bridge-repro-page"
    data-automation-contract="bridge-v1"
    class="min-h-screen space-y-3 p-4 text-sm"
  >
    <div
      id="bootstrap-verdict"
      class="rounded-lg px-4 py-3 text-lg font-bold text-white"
      :class="bootstrapColor"
    >
      Bootstrap: {{ bootstrapVerdict }}
    </div>

    <div class="space-y-1 rounded-lg bg-white p-4">
      <div id="bootstrap-ready">bridge ready: {{ bridgeReady ? 'yes' : 'no' }}</div>
      <div id="bootstrap-snapshot">initial snapshot: {{ snapshotReady ? 'yes' : 'no' }}</div>
      <div id="bootstrap-probe">
        logic call:
        {{
          probe
            ? `yes (${probe.logicReceivedAt - probe.viewSentAt} ms)`
            : probeError
              ? `FAILED: ${probeError}`
              : 'waiting'
        }}
      </div>
      <div id="bootstrap-logic-loaded">
        logic ready:
        {{ snapshotReady ? logicReadyTiming(data.logicLoadedAt, viewStartedAt) : '-' }}
      </div>
      <div class="text-xs text-gray-500">Restart the lxapp to repeat.</div>
    </div>

    <div
      id="stream-verdict"
      class="rounded-lg px-4 py-3 text-lg font-bold text-white"
      :class="streamColor"
    >
      Stream: {{ streamVerdict }}
    </div>

    <div class="space-y-1 rounded-lg bg-white p-4">
      <div id="stat-received">received: {{ audit.received }}</div>
      <div id="stat-last">last seq: {{ audit.last }}</div>
      <div id="stat-first">first tick after: {{ audit.firstAtMs ?? '-' }} ms</div>
      <div id="stat-gaps" :class="audit.gaps.length ? 'font-semibold text-red-600' : ''">
        gaps: {{ audit.gaps.length ? audit.gaps.join(', ') : 'none' }}
      </div>
      <div id="stat-error" :class="streamError ? 'font-semibold text-red-600' : ''">
        stream error: {{ streamError ?? 'none' }}
      </div>
      <div id="stat-reconnects">reconnects: {{ reconnects }}</div>
      <div id="stat-echo">{{ echoLog || 'echo: -' }}</div>
    </div>

    <div class="flex gap-2">
      <button
        id="btn-reconnect"
        class="flex-1 rounded-lg bg-red-500 py-3 font-semibold text-white active:opacity-70"
        @click="reconnect"
      >
        Reconnect
      </button>
      <button
        id="btn-echo"
        class="flex-1 rounded-lg bg-blue-500 py-3 font-semibold text-white active:opacity-70"
        @click="echo"
      >
        Echo
      </button>
    </div>

    <div class="flex gap-2">
      <button
        id="btn-restart"
        class="flex-1 rounded-lg bg-gray-700 py-3 font-semibold text-white active:opacity-70"
        @click="restartStream"
      >
        {{ audit.received === 0 ? 'Start stream' : 'Restart stream' }}
      </button>
      <button
        id="btn-stop"
        class="rounded-lg bg-gray-500 px-5 py-3 font-semibold text-white active:opacity-70"
        @click="stopStream"
      >
        Stop
      </button>
    </div>

    <p class="text-xs leading-relaxed text-gray-500">
      Start the stream, then reconnect. Any gap or bridge error fails the check. Echo may time out
      while the long stream occupies AppService; try it while idle.
    </p>
  </div>
</template>

<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref, watch } from 'vue';
import { useLxPage, useLxStream } from '@lingxia/vue';
import type { LxStream } from '@lingxia/bridge';
import type { BootstrapData, BootstrapProbe, Tick } from './index';
import '../../tailwind.css';

interface AuditState {
  last: number;
  received: number;
  gaps: string[];
  firstAtMs: number | null;
}

interface PageActions {
  onBootstrapProbe: (params: { nonce: number; viewSentAt: number }) => Promise<BootstrapProbe>;
  onTicks: () => LxStream<Tick, void>;
  onEcho: (params: { n: number }) => Promise<{ n: number; ts: number }>;
}

const INITIAL_AUDIT: AuditState = { last: 0, received: 0, gaps: [], firstAtMs: null };
const BOOTSTRAP_TIMEOUT_MS = 5000;
const viewStartedAt = Date.now();

function logicReadyTiming(logicLoadedAt: number, startedAt: number): string {
  const offset = logicLoadedAt - startedAt;
  return offset <= 0 ? 'preloaded' : `${offset} ms after view`;
}

function isBridgeReady(): boolean {
  const bridge = (window as unknown as {
    LingXiaBridge?: { isReady?: () => boolean };
  }).LingXiaBridge;
  return bridge?.isReady?.() === true;
}

function forceReconnect(): boolean {
  const fn = (window as unknown as Record<string, unknown>).__lxForceDownstreamReconnect;
  if (typeof fn !== 'function') return false;
  (fn as () => void)();
  return true;
}

const { data, actions } = useLxPage<BootstrapData, PageActions>();
const bridgeReady = ref(isBridgeReady());
const probe = ref<BootstrapProbe | null>(null);
const probeError = ref<string | null>(null);
const deadlineReached = ref(false);
const streamError = ref<string | null>(null);
const reconnects = ref(0);
const echoLog = ref('');
let echoSeq = 0;
let bridgePoll: number | undefined;
let bootstrapDeadline: number | undefined;

const ticks = useLxStream<typeof actions.onTicks, AuditState>(actions.onTicks, {
  manual: true,
  initial: INITIAL_AUDIT,
  reduce: (acc, tick) => {
    const gaps = [...acc.gaps];
    if (acc.last > 0 && tick.seq > acc.last + 1) {
      gaps.push(`${acc.last + 1}..${tick.seq - 1}`);
    }
    return {
      last: tick.seq,
      received: acc.received + 1,
      gaps,
      firstAtMs: acc.firstAtMs ?? Date.now() - viewStartedAt,
    };
  },
});

const audit = computed(() => ticks.data.value ?? INITIAL_AUDIT);
const snapshotReady = computed(() => data.bootstrapMarker === 'initial logic snapshot received');
const bootstrapPassed = computed(() => bridgeReady.value && snapshotReady.value && probe.value !== null);
const bootstrapFailed = computed(() => (
  probeError.value !== null || (deadlineReached.value && !bootstrapPassed.value)
));
const bootstrapVerdict = computed(() => (
  bootstrapPassed.value ? 'PASS' : bootstrapFailed.value ? 'FAIL' : 'WAITING'
));
const bootstrapColor = computed(() => (
  bootstrapVerdict.value === 'PASS'
    ? 'bg-green-600'
    : bootstrapVerdict.value === 'FAIL'
      ? 'bg-red-600'
      : 'bg-gray-400'
));
const streamFailed = computed(() => audit.value.gaps.length > 0 || streamError.value !== null);
const streamVerdict = computed(() => (
  audit.value.received === 0 ? 'WAITING' : streamFailed.value ? 'FAIL' : 'PASS'
));
const streamColor = computed(() => (
  streamVerdict.value === 'PASS'
    ? 'bg-green-600'
    : streamVerdict.value === 'FAIL'
      ? 'bg-red-600'
      : 'bg-gray-400'
));

onMounted(() => {
  bridgePoll = window.setInterval(() => {
    bridgeReady.value = isBridgeReady();
  }, 50);
  bootstrapDeadline = window.setTimeout(() => {
    deadlineReached.value = true;
  }, BOOTSTRAP_TIMEOUT_MS);

  actions.onBootstrapProbe({ nonce: 1, viewSentAt: viewStartedAt }).then(
    (result) => { probe.value = result; },
    (error) => { probeError.value = String((error as Error)?.message ?? error); },
  );
});

onBeforeUnmount(() => {
  if (bridgePoll !== undefined) window.clearInterval(bridgePoll);
  if (bootstrapDeadline !== undefined) window.clearTimeout(bootstrapDeadline);
  ticks.cancel();
});

watch(ticks.error, (error) => {
  streamError.value = error ? String(error.message ?? error) : null;
});

function reconnect() {
  if (!forceReconnect()) {
    echoLog.value = 'reconnect hook unavailable (non-Apple transport)';
    return;
  }
  reconnects.value += 1;
}

async function echo() {
  const n = ++echoSeq;
  const startedAt = Date.now();
  try {
    const result = await actions.onEcho({ n });
    echoLog.value = `echo #${result.n} ok in ${Date.now() - startedAt}ms`;
  } catch (error) {
    echoLog.value = `echo #${n} FAILED: ${String((error as Error)?.message ?? error)}`;
  }
}

function restartStream() {
  streamError.value = null;
  ticks.start();
}

function stopStream() {
  ticks.cancel();
}
</script>
