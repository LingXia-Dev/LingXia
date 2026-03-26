<template>
  <div class="min-h-screen bg-gradient-to-br from-gray-50 to-gray-100">
    <div class="px-4 py-6">
      <template v-if="type === 'mqtt'">
        <div class="mb-5 bg-white rounded-2xl shadow-sm border border-gray-100 overflow-hidden">
          <div class="px-5 py-5 border-b border-gray-100">
            <div class="text-sm text-gray-800 font-semibold">Subscription</div>
            <div class="text-xs text-gray-500 mt-0.5">
              Publish to this short topic from the demo environment and watch the latest frame appear below.
            </div>
          </div>
          <div class="p-5 space-y-4">
            <div class="rounded-xl border border-gray-200 bg-emerald-50/60 p-4">
              <div class="text-xs uppercase tracking-wide text-emerald-700">Status</div>
              <div class="mt-2 text-sm font-semibold text-gray-800">{{ mqttStatus }}</div>
              <div class="mt-2 text-xs text-gray-500">Messages received: {{ mqttMessageCount }}</div>
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
            <div class="rounded-xl border border-gray-200 bg-gray-50 p-4">
              <div class="text-xs uppercase tracking-wide text-gray-500">Topic</div>
              <div class="mt-2 font-mono text-sm text-gray-800 break-all">{{ mqttTopicFilter }}</div>
              <div class="mt-3 text-xs text-gray-500">
                QoS 1 guarantees delivery after subscribe. It does not replay messages published before subscribe.
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
                  Use the home lxapp to start login and inspect auth state.
                </div>
              </div>
            </div>
            <div class="grid grid-cols-2 gap-3">
              <button
                @click="loginInteractive"
                class="py-3 text-sm font-medium transition-all duration-200 rounded-xl shadow-sm active:scale-[0.98] bg-gradient-to-r from-sky-600 to-sky-500 hover:from-sky-500 hover:to-sky-600 text-white"
              >
                Interactive Login
              </button>
              <button
                @click="getAccessToken"
                class="py-3 text-sm font-medium transition-all duration-200 rounded-xl shadow-sm active:scale-[0.98] bg-gradient-to-r from-emerald-600 to-emerald-500 hover:from-emerald-500 hover:to-emerald-600 text-white"
              >
                Get Access Token
              </button>
              <button
                @click="logoutCurrentTenant"
                class="col-span-2 py-3 text-sm font-medium transition-all duration-200 rounded-xl shadow-sm active:scale-[0.98] bg-gradient-to-r from-rose-600 to-rose-500 hover:from-rose-500 hover:to-rose-600 text-white"
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
                <span class="text-sm font-semibold text-gray-800 px-3 py-1 bg-blue-50 rounded-lg text-right">{{ status }}</span>
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
                </div>
              </div>
              <div class="py-3">
                <div class="text-sm text-gray-600 mb-2">Access Token</div>
                <div class="text-sm font-medium text-gray-800 px-3 py-3 bg-emerald-50 rounded-lg break-all">
                  {{ summarizeToken(accessToken) }}
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
              <div class="text-xs text-gray-500 mt-0.5">Switch the active tenant after login</div>
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
                @click="getTenantId(item) && switchTenant({ tenantId: getTenantId(item) })"
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
                      <div class="text-sm font-semibold text-gray-800">{{ getTenantName(item) }}</div>
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
import { useLingXia } from '@lingxia/vue';
import '../../tailwind.css';

type TenantLike = {
  tenantId?: string;
  tenantName?: string;
  displayName?: string;
  logoUrl?: string;
};

type CloudPageType = 'auth' | 'mqtt';

type PageData = {
  type?: CloudPageType;
  status?: string;
  tenant?: TenantLike | null;
  tenants?: TenantLike[];
  accessToken?: string;
  mqttStatus?: string;
  mqttSubscribed?: boolean;
  mqttTopicFilter?: string;
  mqttMessageCount?: number;
  mqttLastTopic?: string;
  mqttLastPayload?: string;
  mqttLastReceivedAt?: string;
};

type PageActions = {
  loginInteractive: () => void | Promise<void>;
  getAccessToken: () => void | Promise<void>;
  logoutCurrentTenant: () => void | Promise<void>;
  switchTenant: (params: { tenantId: string }) => void | Promise<void>;
  startMqttDemo: () => void | Promise<void>;
  stopMqttDemo: () => void | Promise<void>;
};

const {
  data,
  loginInteractive,
  getAccessToken,
  logoutCurrentTenant,
  switchTenant,
  startMqttDemo,
  stopMqttDemo,
} = useLingXia<PageData, PageActions>();

const type = computed(() => data.value.type || 'auth');
const status = computed(() => data.value.status || 'Idle');
const tenant = computed(() => data.value.tenant || null);
const tenants = computed(() => data.value.tenants || []);
const accessToken = computed(() => data.value.accessToken || '');
const mqttStatus = computed(() => data.value.mqttStatus || 'Idle');
const mqttSubscribed = computed(() => !!data.value.mqttSubscribed);
const mqttTopicFilter = computed(() => data.value.mqttTopicFilter || 'demo/mqtt');
const mqttMessageCount = computed(() => data.value.mqttMessageCount || 0);
const mqttLastTopic = computed(() => data.value.mqttLastTopic || '');
const mqttLastPayload = computed(() => data.value.mqttLastPayload || '');
const mqttLastReceivedAt = computed(() => data.value.mqttLastReceivedAt || '');
const activeTenantId = computed(() => getTenantId(tenant.value));

function getTenantId(item: TenantLike | null | undefined): string {
  return item?.tenantId || '';
}

function getTenantName(item: TenantLike | null | undefined): string {
  return item?.displayName || item?.tenantName || item?.tenantId || 'Unknown tenant';
}

function getTenantLogoUrl(item: TenantLike | null | undefined): string {
  return item?.logoUrl || '';
}

function summarizeToken(token: string): string {
  if (!token) {
    return 'No token fetched yet';
  }
  if (token.length <= 24) {
    return token;
  }
  return `${token.slice(0, 12)}...${token.slice(-8)}`;
}
</script>
