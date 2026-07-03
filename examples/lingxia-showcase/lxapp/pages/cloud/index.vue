<template>
  <div class="min-h-screen bg-gradient-to-br from-gray-50 to-gray-100">
    <div class="px-4 py-6">
      <template v-if="type === 'mqtt'">
        <div class="mb-5 bg-white rounded-2xl shadow-sm border border-gray-100 overflow-hidden">
          <div class="px-5 py-5 border-b border-gray-100">
            <div class="text-sm text-gray-800 font-semibold">Runtime Connection</div>
            <div class="text-xs text-gray-500 mt-0.5">
              MQTT runtime status for the shared cloud session.
            </div>
          </div>
          <div class="p-5">
            <div class="grid gap-2 rounded-xl bg-sky-50/70 p-4 text-xs text-gray-600">
              <div class="flex items-center justify-between gap-3">
                <span>Connection state</span>
                <span class="flex items-center gap-1.5">
                  <span class="inline-block w-2 h-2 rounded-full" :class="mqttStateDotClass" />
                  <span class="font-mono font-semibold" :class="mqttStateColorClass">{{ mqttRuntimeState }}</span>
                </span>
              </div>
              <div class="flex items-center justify-between gap-3">
                <span>Last error</span>
                <span class="font-mono text-right" :class="mqttLastError ? 'text-red-600' : 'text-gray-800'">{{ mqttLastError || '-' }}</span>
              </div>
            </div>
          </div>
        </div>

        <div class="mb-5 bg-white rounded-2xl shadow-sm border border-gray-100 overflow-hidden">
          <div class="px-5 py-5 border-b border-gray-100">
            <div class="text-sm text-gray-800 font-semibold">Topic Subscription</div>
            <div class="text-xs text-gray-500 mt-0.5">
              Manage the demo topic subscription and inspect how many messages this page has received.
            </div>
          </div>
          <div class="p-5 space-y-4">
            <div class="rounded-xl border border-gray-200 bg-emerald-50/60 p-4">
              <div class="flex items-start justify-between gap-3">
                <div>
                  <div class="text-xs uppercase tracking-wide text-emerald-700">Topic</div>
                  <div class="mt-2 font-mono text-sm text-gray-900 break-all">{{ mqttTopicFilter }}</div>
                </div>
                <span
                  class="shrink-0 rounded-full px-2.5 py-1 text-xs font-semibold"
                  :class="mqttSubscribed ? 'bg-emerald-100 text-emerald-700' : 'bg-gray-100 text-gray-500'"
                >
                  {{ mqttSubscribed ? 'active' : 'inactive' }}
                </span>
              </div>
              <div class="mt-3 text-sm font-semibold text-gray-800">{{ mqttStatus }}</div>
              <div class="mt-4 grid grid-cols-2 gap-3">
                <button
                  @click="startMqttDemo"
                  :disabled="mqttSubscribed"
                  class="py-3 text-sm font-medium rounded-xl transition-all duration-200 shadow-sm active:scale-[0.98]"
                  :class="
                    mqttSubscribed
                      ? 'bg-gray-100 text-gray-400'
                      : 'bg-gradient-to-r from-emerald-600 to-emerald-500 text-white'
                  "
                >
                  Subscribe
                </button>
                <button
                  @click="stopMqttDemo"
                  :disabled="!mqttSubscribed"
                  class="py-3 text-sm font-medium rounded-xl transition-all duration-200 shadow-sm active:scale-[0.98]"
                  :class="
                    mqttSubscribed
                      ? 'bg-gradient-to-r from-rose-600 to-rose-500 text-white'
                      : 'bg-gray-100 text-gray-400'
                  "
                >
                  Unsubscribe
                </button>
              </div>
            </div>

            <div class="grid gap-2 rounded-xl bg-white p-4 text-xs text-gray-600 border border-gray-200">
              <div class="flex items-center justify-between gap-3">
                <span>Subscription state</span>
                <span class="font-mono font-semibold" :class="mqttSubscribed ? 'text-emerald-600' : 'text-gray-500'">
                  {{ mqttSubscribed ? 'active' : 'inactive' }}
                </span>
              </div>
              <div class="flex items-center justify-between gap-3">
                <span>Messages received</span>
                <span class="font-mono text-gray-800">{{ mqttMessageCount }}</span>
              </div>
            </div>
          </div>
        </div>

        <div class="bg-white rounded-2xl shadow-sm border border-gray-100 overflow-hidden">
          <div class="px-5 py-5 border-b border-gray-100">
            <div class="text-sm text-gray-800 font-semibold">Latest Message</div>
            <div class="text-xs text-gray-500 mt-0.5">Incoming message data is written back through setData.</div>
          </div>
          <div class="p-5">
            <div
              v-if="mqttMessageCount === 0"
              class="rounded-xl border border-dashed border-gray-300 bg-gray-50 p-6 text-sm text-gray-500"
            >
              No message yet. Publish one to the topic above.
            </div>
            <div v-else class="rounded-xl border border-gray-200 bg-gradient-to-br from-gray-50 to-white p-4">
              <div class="grid gap-3 text-sm">
                <div>
                  <div class="text-xs uppercase tracking-wide text-gray-500">Topic</div>
                  <div class="mt-1 font-mono text-gray-800 break-all">{{ mqttLastTopic }}</div>
                </div>
                <div>
                  <div class="text-xs uppercase tracking-wide text-gray-500">Received at</div>
                  <div class="mt-1 text-gray-800">{{ mqttLastReceivedAt }}</div>
                </div>
                <div>
                  <div class="text-xs uppercase tracking-wide text-gray-500">Payload</div>
                  <pre class="mt-1 rounded-lg bg-slate-900 text-slate-100 p-3 text-xs whitespace-pre-wrap break-all">{{ mqttLastPayload || '(empty payload)' }}</pre>
                </div>
              </div>
            </div>
          </div>
        </div>
      </template>

      <template v-else-if="type === 'functions'">
        <div class="mb-5 bg-white rounded-2xl shadow-sm border border-gray-100 overflow-hidden">
          <div class="px-5 py-5 border-b border-gray-100">
            <div class="text-sm text-gray-800 font-semibold">Cloud Functions</div>
            <div class="text-xs text-gray-500 mt-0.5">
              Invoke demo cloud functions for the current lxapp.
            </div>
          </div>
          <div class="p-5 space-y-4">
            <div class="rounded-xl border border-gray-200 bg-gray-50 p-4 text-sm text-gray-700">
              {{ functionsStatus }}
            </div>
            <div class="grid grid-cols-3 gap-3">
              <button
                v-for="name in functionsAvailable"
                :key="name"
                @click="callNamedFunction({ name })"
                class="rounded-xl bg-gradient-to-r from-indigo-600 to-indigo-500 px-3 py-3 text-sm font-medium text-white shadow-sm active:scale-[0.98]"
              >
                {{ name }}
              </button>
            </div>
            <div v-if="functionsLastCall || functionsLastResult" class="rounded-xl border border-gray-200 bg-white p-4">
              <div class="text-xs uppercase tracking-wide text-gray-500">Last call</div>
              <div class="mt-1 font-mono text-sm text-gray-800">{{ functionsLastCall || '-' }}</div>
              <div class="mt-3 text-xs uppercase tracking-wide text-gray-500">Result</div>
              <pre class="mt-1 whitespace-pre-wrap break-all rounded-lg bg-slate-900 p-3 text-xs text-slate-100">{{ functionsLastResult || '-' }}</pre>
            </div>
          </div>
        </div>
      </template>

      <template v-else>
        <div class="mb-5 bg-white rounded-2xl shadow-sm border border-gray-100 overflow-hidden">
          <div class="p-6">
            <div class="flex items-center gap-3 mb-4">
              <div class="flex items-center justify-center w-10 h-10 rounded-xl bg-gradient-to-br from-sky-50 to-cyan-50">
                <span class="text-xl">C</span>
              </div>
              <div>
                <div class="text-sm text-gray-800 font-semibold">Cloud Authentication</div>
                <div class="text-xs text-gray-500 mt-0.5">
                  Login returns the active identity; the home lxapp manages identity list, activation and logout.
                </div>
              </div>
            </div>
            <div class="grid grid-cols-2 gap-3">
              <!-- Login is the unauthenticated entry; once an identity is active
                   it silently refreshes, so hide it in favor of Add Identity. -->
              <button
                v-if="!tenant"
                @click="loginInteractive"
                class="py-3 text-sm font-medium transition-all duration-200 rounded-xl shadow-sm active:scale-[0.98] bg-gradient-to-r from-sky-600 to-sky-500 hover:from-sky-500 hover:to-sky-600 text-white"
              >
                Interactive Login
              </button>
              <button
                @click="addTenant"
                class="py-3 text-sm font-medium transition-all duration-200 rounded-xl shadow-sm active:scale-[0.98] bg-gradient-to-r from-emerald-600 to-emerald-500 hover:from-emerald-500 hover:to-emerald-600 text-white"
              >
                Add Identity
              </button>
              <button
                @click="logoutCurrentTenant"
                class="py-3 text-sm font-medium transition-all duration-200 rounded-xl shadow-sm active:scale-[0.98] bg-gradient-to-r from-rose-600 to-rose-500 hover:from-rose-500 hover:to-rose-600 text-white"
              >
                Logout Current Tenant
              </button>
            </div>
          </div>
        </div>

        <div class="mb-5 bg-white rounded-2xl shadow-sm border border-gray-100 overflow-hidden">
          <div class="flex items-center gap-4 px-5 py-5 border-b border-gray-100">
            <div class="flex items-center justify-center w-12 h-12 rounded-xl bg-gradient-to-br from-blue-50 to-indigo-50">
              <span class="text-2xl">S</span>
            </div>
            <div class="flex-1">
              <div class="text-sm text-gray-800 font-semibold">Current State</div>
              <div class="text-xs text-gray-500 mt-0.5">Latest cloud auth snapshot from the runtime</div>
            </div>
          </div>
          <div class="p-5">
            <div class="rounded-xl border border-gray-200 bg-gradient-to-br from-gray-50 to-white p-4">
              <div class="flex items-center gap-2 mb-4">
                <span class="w-1 h-4 bg-blue-500 rounded-full"></span>
                <h4 class="text-sm font-semibold text-gray-700">Status</h4>
              </div>
              <div class="flex justify-between items-center py-3 border-b border-gray-200 gap-4">
                <span class="text-sm text-gray-600">Status</span>
                <span class="text-sm font-semibold px-3 py-1 rounded-lg text-right" :class="authStatusColorClass">{{ status }}</span>
              </div>
              <div class="flex justify-between items-center py-3 border-b border-gray-200 gap-4">
                <span class="text-sm text-gray-600">Active Tenant</span>
                <div class="flex items-center gap-3 px-3 py-2 bg-blue-50 rounded-lg">
                  <img
                    v-if="getTenantLogoUrl(tenant)"
                    :src="getTenantLogoUrl(tenant)"
                    :alt="getTenantName(tenant)"
                    class="w-8 h-8 rounded-full border border-white shadow-sm bg-white object-cover"
                  />
                  <span class="text-sm font-semibold text-gray-800 text-right">
                    {{ tenant ? getTenantName(tenant) : 'No active tenant' }}
                  </span>
                  <span
                    v-if="getTenantShortName(tenant)"
                    class="text-xs font-semibold px-2 py-0.5 rounded-full bg-blue-100 text-blue-700"
                  >
                    {{ getTenantShortName(tenant) }}
                  </span>
                </div>
              </div>


            </div>
          </div>
        </div>

        <div class="bg-white rounded-2xl shadow-sm border border-gray-100 overflow-hidden">
          <div class="flex items-center gap-4 px-5 py-5 border-b border-gray-100">
            <div class="flex items-center justify-center w-12 h-12 rounded-xl bg-gradient-to-br from-purple-50 to-pink-50">
              <span class="text-2xl">T</span>
            </div>
            <div class="flex-1">
              <div class="text-sm text-gray-800 font-semibold">Tenants</div>
              <div class="text-xs text-gray-500 mt-0.5">Activate or remove identities returned by lx.auth.list()</div>
            </div>
            <div class="text-xs text-gray-500">{{ tenants.length }}</div>
          </div>
          <div class="p-5">
            <div v-if="tenants.length === 0" class="text-sm text-gray-500">
              No tenants yet. Run interactive login first.
            </div>
            <div v-else class="space-y-3">
              <button
                v-for="(item, index) in tenants"
                :key="`${getTenantId(item) || 'tenant'}-${index}`"
                :disabled="!getTenantId(item) || getTenantId(item) === activeTenantId"
                @click="getTenantId(item) && activateTenant({ tenantId: getTenantId(item) })"
                class="w-full p-4 rounded-xl border text-left transition-colors"
                :class="
                  getTenantId(item) !== '' && getTenantId(item) === activeTenantId
                    ? 'border-sky-200 bg-sky-50'
                    : 'border-gray-200 bg-white hover:border-emerald-200 hover:bg-emerald-50/30'
                "
              >
                <div class="flex items-center justify-between gap-3">
                  <div class="flex items-center gap-3 min-w-0">
                    <img
                      v-if="getTenantLogoUrl(item)"
                      :src="getTenantLogoUrl(item)"
                      :alt="getTenantName(item)"
                      class="w-10 h-10 rounded-full border border-gray-200 bg-white object-cover shrink-0"
                    />
                    <div class="min-w-0">
                      <div class="flex items-center gap-2">
                        <span class="text-sm font-semibold text-gray-800">{{ getTenantName(item) }}</span>
                        <span
                          v-if="getTenantShortName(item)"
                          class="text-xs font-semibold px-2 py-0.5 rounded-full bg-gray-100 text-gray-600"
                        >
                          {{ getTenantShortName(item) }}
                        </span>
                      </div>
                      <div class="mt-1 text-xs text-gray-500 break-all">{{ getTenantId(item) || 'Missing tenantId' }}</div>
                    </div>
                  </div>
                  <div class="text-xs px-2 py-1 rounded-full bg-gray-100 text-gray-600">
                    {{ getTenantId(item) !== '' && getTenantId(item) === activeTenantId ? 'Active' : getTenantId(item) ? 'Switch' : 'Unavailable' }}
                  </div>
                </div>
              </button>
            </div>
          </div>
        </div>
      </template>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed } from 'vue';
import { useLxPage } from '@lingxia/vue';
import '../../tailwind.css';

type TenantLike = {
  tenantId?: string;
  tenantName?: string;
  displayName?: string;
  shortName?: string;
  logoUrl?: string;
};

type CloudPageType = 'auth' | 'mqtt' | 'functions';

type PageData = {
  type?: CloudPageType;
  status?: string;
  tenant?: TenantLike | null;
  tenants?: TenantLike[];
  mqttStatus?: string;
  mqttRuntimeState?: string;
  mqttLastError?: string;
  mqttSubscribed?: boolean;
  mqttTopicFilter?: string;
  mqttMessageCount?: number;
  mqttLastTopic?: string;
  mqttLastPayload?: string;
  mqttLastReceivedAt?: string;
  functionsStatus?: string;
  functionsAvailable?: string[];
  functionsLastCall?: string;
  functionsLastResult?: string;
};

type PageActions = {
  loginInteractive: () => void | Promise<void>;
  addTenant: () => void | Promise<void>;
  logoutCurrentTenant: () => void | Promise<void>;
  activateTenant: (params: { tenantId: string }) => void | Promise<void>;
  startMqttDemo: () => void | Promise<void>;
  stopMqttDemo: () => void | Promise<void>;
  callNamedFunction: (params: { name: string }) => void | Promise<void>;
};

const { data, actions } = useLxPage<PageData, PageActions>();
const {
  loginInteractive,
  addTenant,
  logoutCurrentTenant,
  activateTenant,
  startMqttDemo,
  stopMqttDemo,
  callNamedFunction,
} = actions;

const type = computed(() => data.type || 'auth');
const status = computed(() => data.status || 'Idle');
const tenant = computed(() => data.tenant || null);
const tenants = computed(() => data.tenants || []);
const mqttStatus = computed(() => data.mqttStatus || 'Idle');
const mqttRuntimeState = computed(() => data.mqttRuntimeState || 'idle');
const mqttLastError = computed(() => data.mqttLastError || '');
const mqttSubscribed = computed(() => !!data.mqttSubscribed);
const mqttTopicFilter = computed(() => data.mqttTopicFilter || 'demo/mqtt');
const mqttMessageCount = computed(() => data.mqttMessageCount || 0);
const mqttLastTopic = computed(() => data.mqttLastTopic || '');
const mqttLastPayload = computed(() => data.mqttLastPayload || '');
const mqttLastReceivedAt = computed(() => data.mqttLastReceivedAt || '');
const functionsStatus = computed(() => data.functionsStatus || 'Idle');
const functionsAvailable = computed(() => data.functionsAvailable || []);
const functionsLastCall = computed(() => data.functionsLastCall || '');
const functionsLastResult = computed(() => data.functionsLastResult || '');
const activeTenantId = computed(() => getTenantId(tenant.value));

const mqttStateColorClass = computed(() => {
  switch (mqttRuntimeState.value) {
    case 'connected': return 'text-emerald-600';
    case 'connecting':
    case 'reconnecting': return 'text-amber-600';
    case 'disconnected':
    case 'error': return 'text-red-600';
    default: return 'text-gray-600';
  }
});

const mqttStateDotClass = computed(() => {
  switch (mqttRuntimeState.value) {
    case 'connected': return 'bg-emerald-500';
    case 'connecting':
    case 'reconnecting': return 'bg-amber-500';
    case 'disconnected':
    case 'error': return 'bg-red-500';
    default: return 'bg-gray-400';
  }
});

const authStatusColorClass = computed(() => {
  const s = status.value.toLowerCase();
  if (s.includes('failed') || s.includes('error')) return 'text-red-600 bg-red-50';
  if (s.includes('succeeded') || s === 'ready') return 'text-emerald-700 bg-emerald-50';
  if (s.includes('...') || s.includes('starting') || s.includes('switching') || s.includes('logging')) return 'text-amber-700 bg-amber-50';
  return 'text-gray-800 bg-blue-50';
});

function getTenantId(item: TenantLike | null | undefined): string {
  return item?.tenantId || '';
}

function getTenantName(item: TenantLike | null | undefined): string {
  return item?.displayName || item?.tenantName || item?.tenantId || 'Unknown tenant';
}

function getTenantLogoUrl(item: TenantLike | null | undefined): string {
  return item?.logoUrl || '';
}

function getTenantShortName(item: TenantLike | null | undefined): string {
  return item?.shortName || '';
}


</script>
