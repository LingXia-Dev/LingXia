<template>
  <div class="min-h-screen bg-gradient-to-br from-gray-50 to-gray-100">
    <div class="px-4 py-6">
      <div class="mb-6 text-center">
        <h1 class="text-2xl font-light text-gray-800 mb-2">WiFi Management</h1>
        <div class="w-16 h-0.5 bg-gray-400 mx-auto"></div>
      </div>

      <!-- WiFi Module Control -->
      <div class="mb-5 bg-white rounded-2xl shadow-sm border border-gray-100 overflow-hidden">
        <div class="p-6">
          <div class="flex items-center gap-3 mb-4">
            <div class="flex items-center justify-center w-10 h-10 rounded-xl bg-gradient-to-br from-emerald-50 to-green-50">
              <span class="text-xl">🧩</span>
            </div>
            <div>
              <div class="text-sm text-gray-800 font-semibold">WiFi Module</div>
              <div class="text-xs text-gray-500 mt-0.5">Initialize or stop WiFi module</div>
            </div>
          </div>
          <div class="grid grid-cols-2 gap-3">
            <button
              @click="startWifi"
              :disabled="wifiModuleEnabled"
              :class="[
                'py-3 text-sm font-medium transition-all duration-200 rounded-xl shadow-sm active:scale-[0.98]',
                wifiModuleEnabled ? 'bg-gray-200 text-gray-400 cursor-not-allowed' : 'bg-gradient-to-r from-green-600 to-green-500 hover:from-green-500 hover:to-green-600 text-white'
              ]"
            >
              Start WiFi
            </button>
            <button
              @click="stopWifi"
              :disabled="!wifiModuleEnabled"
              :class="[
                'py-3 text-sm font-medium transition-all duration-200 rounded-xl shadow-sm active:scale-[0.98]',
                !wifiModuleEnabled ? 'bg-gray-200 text-gray-400 cursor-not-allowed' : 'bg-gradient-to-r from-red-600 to-red-500 hover:from-red-500 hover:to-red-600 text-white'
              ]"
            >
              Stop WiFi
            </button>
          </div>
        </div>
      </div>

      <!-- Get Connected WiFi -->
      <div v-if="wifiModuleEnabled" class="mb-5 bg-white rounded-2xl shadow-sm border border-gray-100 overflow-hidden">
        <div class="flex items-center gap-4 px-5 py-5 border-b border-gray-100">
          <div class="flex items-center justify-center w-12 h-12 rounded-xl bg-gradient-to-br from-blue-50 to-indigo-50">
            <span class="text-2xl">📶</span>
          </div>
          <div class="flex-1">
            <div class="text-sm text-gray-800 font-semibold">Connected WiFi</div>
            <div class="text-xs text-gray-500 mt-0.5">Get current WiFi connection info</div>
          </div>
          <button
            @click="getConnectedWifi"
            class="px-5 py-2.5 text-sm font-medium transition-all duration-200 bg-gradient-to-r from-blue-600 to-blue-500 hover:from-blue-500 hover:to-blue-600 text-white rounded-xl shadow-sm active:scale-[0.98]"
          >
            Get Info
          </button>
        </div>

        <div v-if="connectedWifi" class="p-5">
          <div class="rounded-xl border border-gray-200 bg-gradient-to-br from-gray-50 to-white p-4">
            <div class="flex items-center gap-2 mb-4">
              <span class="w-1 h-4 bg-blue-500 rounded-full"></span>
              <h4 class="text-sm font-semibold text-gray-700">Connected Network</h4>
            </div>
            <div class="flex justify-between items-center py-3 border-b border-gray-200">
              <span class="text-sm text-gray-600">SSID</span>
              <span class="text-sm font-semibold text-gray-800 px-3 py-1 bg-blue-50 rounded-lg">{{ connectedWifi.SSID ?? connectedWifi.ssid ?? '--' }}</span>
            </div>
            <div class="flex justify-between items-center py-3 border-b border-gray-200">
              <span class="text-sm text-gray-600">BSSID</span>
              <span class="text-sm font-semibold text-gray-800 px-3 py-1 bg-blue-50 rounded-lg">{{ connectedWifi.BSSID ?? connectedWifi.bssid ?? '--' }}</span>
            </div>
            <div class="flex justify-between items-center py-3 border-b border-gray-200">
              <span class="text-sm text-gray-600">Secure</span>
              <span class="text-sm font-semibold text-gray-800 px-3 py-1 bg-blue-50 rounded-lg">{{ connectedWifi.secure ? 'Yes' : 'No' }}</span>
            </div>
            <div class="flex justify-between items-center py-3 border-b border-gray-200">
              <span class="text-sm text-gray-600">Signal</span>
              <span class="text-sm font-semibold text-gray-800 px-3 py-1 bg-blue-50 rounded-lg">{{ connectedWifi.signalStrength ?? '--' }}%</span>
            </div>
            <div class="flex justify-between items-center py-3">
              <span class="text-sm text-gray-600">Frequency</span>
              <span class="text-sm font-semibold text-gray-800 px-3 py-1 bg-blue-50 rounded-lg">{{ connectedWifi.frequency ?? '--' }} MHz</span>
            </div>
          </div>
        </div>
      </div>

      <!-- WiFi Connected Events -->
      <div v-if="wifiModuleEnabled" class="mb-5 bg-white rounded-2xl shadow-sm border border-gray-100 overflow-hidden">
        <div class="flex items-center gap-4 px-5 py-5 border-b border-gray-100">
          <div class="flex items-center justify-center w-12 h-12 rounded-xl bg-gradient-to-br from-amber-50 to-orange-50">
            <span class="text-2xl">🔔</span>
          </div>
          <div class="flex-1">
            <div class="text-sm text-gray-800 font-semibold">WiFi Connected Events</div>
            <div class="text-xs text-gray-500 mt-0.5">Listen to WiFi connection changes</div>
          </div>
          <div class="flex items-center gap-2">
            <button
              @click="handleStartWifiConnected"
              :disabled="wifiListenerEnabled"
              :class="[
                'px-4 py-2 text-xs font-medium transition-all duration-200 rounded-lg shadow-sm active:scale-[0.98]',
                wifiListenerEnabled ? 'bg-gray-200 text-gray-400 cursor-not-allowed' : 'bg-gradient-to-r from-amber-500 to-orange-500 hover:from-amber-400 hover:to-orange-500 text-white'
              ]"
            >
              On
            </button>
            <button
              @click="handleStopWifiConnected"
              :disabled="!wifiListenerEnabled"
              :class="[
                'px-4 py-2 text-xs font-medium transition-all duration-200 rounded-lg shadow-sm active:scale-[0.98]',
                !wifiListenerEnabled ? 'bg-gray-200 text-gray-400 cursor-not-allowed' : 'bg-gradient-to-r from-gray-600 to-gray-500 hover:from-gray-500 hover:to-gray-600 text-white'
              ]"
            >
              Off
            </button>
          </div>
        </div>
        <div class="p-5">
          <div class="flex items-center justify-between text-xs text-gray-500 mb-3">
            <span>Listening: {{ wifiListenerEnabled ? 'On' : 'Off' }}</span>
            <button
              v-if="wifiConnectedEvents.length > 0"
              @click="handleClearWifiEvents"
              class="text-xs text-gray-500 hover:text-gray-700 underline"
            >
              Clear
            </button>
          </div>
          <div v-if="wifiConnectedEvents.length === 0" class="text-sm text-gray-500">No events yet.</div>
          <div v-else class="space-y-3">
            <div
              v-for="event in wifiConnectedEvents"
              :key="event.id"
              class="rounded-lg border border-gray-200 bg-white p-3 text-xs text-gray-600"
            >
              <div class="flex items-center justify-between mb-2">
                <span class="text-gray-500">{{ event.time }}</span>
                <span class="text-gray-500">{{ typeof event.signalStrength === 'number' ? `${event.signalStrength}%` : '--' }}</span>
              </div>
              <div class="text-sm font-semibold text-gray-800">{{ event.ssid || '--' }}</div>
              <div class="mt-1 text-[11px] text-gray-500 space-y-0.5">
                <div v-if="event.bssid">BSSID: {{ event.bssid }}</div>
                <div v-if="typeof event.frequency === 'number'">Frequency: {{ event.frequency }} MHz</div>
                <div>State: {{ event.state ?? (event.connected === undefined ? '--' : event.connected ? 'Connected' : 'Disconnected') }}</div>
                <div>Secure: {{ event.secure === undefined ? '--' : event.secure ? 'Yes' : 'No' }}</div>
              </div>
            </div>
          </div>
        </div>
      </div>

      <!-- Get WiFi List -->
      <div v-if="wifiModuleEnabled" class="mb-5 bg-white rounded-2xl shadow-sm border border-gray-100 overflow-hidden">
        <div class="flex items-center gap-4 px-5 py-5 border-b border-gray-100">
          <div class="flex items-center justify-center w-12 h-12 rounded-xl bg-gradient-to-br from-purple-50 to-pink-50">
            <span class="text-2xl">📋</span>
          </div>
          <div class="flex-1">
            <div class="text-sm text-gray-800 font-semibold">Scan WiFi Networks</div>
            <div class="text-xs text-gray-500 mt-0.5">Get list of available networks</div>
          </div>
          <button
            @click="getWifiList"
            class="px-5 py-2.5 text-sm font-medium transition-all duration-200 bg-gradient-to-r from-purple-600 to-purple-500 hover:from-purple-500 hover:to-purple-600 text-white rounded-xl shadow-sm active:scale-[0.98]"
          >
            Scan
          </button>
        </div>

        <div v-if="wifiList && wifiList.length > 0" class="p-5">
          <div class="rounded-xl border border-gray-200 bg-gradient-to-br from-gray-50 to-white p-4">
            <div class="flex items-center gap-2 mb-4">
              <span class="w-1 h-4 bg-purple-500 rounded-full"></span>
              <h4 class="text-sm font-semibold text-gray-700">Available Networks ({{ wifiList.length }})</h4>
            </div>
            <div class="space-y-3 max-h-96 overflow-y-auto">
              <div
                v-for="(wifi, index) in wifiList"
                :key="index"
                @click="wifiSsid = (wifi.SSID ?? wifi.ssid ?? '').toString()"
                class="p-3 bg-white rounded-lg border border-gray-200 cursor-pointer hover:border-emerald-200 hover:bg-emerald-50/30 transition-colors"
              >
                <div class="flex items-center justify-between mb-2">
                  <span class="text-sm font-semibold text-gray-800">{{ wifi.SSID ?? wifi.ssid }}</span>
                  <span class="text-xs px-2 py-1 rounded-full bg-blue-50 text-blue-600">
                    {{ typeof wifi.signalStrength === 'number' ? `${wifi.signalStrength}%` : '--' }}
                  </span>
                </div>
                <div class="text-xs text-gray-500 space-y-1">
                  <div v-if="wifi.BSSID ?? wifi.bssid">BSSID: {{ wifi.BSSID ?? wifi.bssid }}</div>
                  <div v-if="typeof wifi.frequency === 'number'">Frequency: {{ wifi.frequency }} MHz</div>
                  <div>Security: {{ wifi.secure ? '🔒 Secured' : '🔓 Open' }}</div>
                </div>
              </div>
            </div>
          </div>
        </div>
      </div>

      <!-- Connect WiFi -->
      <div v-if="wifiModuleEnabled" class="mb-5 bg-white rounded-2xl shadow-sm border border-gray-100 overflow-hidden">
        <div class="p-6">
          <div class="flex items-center gap-3 mb-4">
            <div class="flex items-center justify-center w-10 h-10 rounded-xl bg-gradient-to-br from-emerald-50 to-lime-50">
              <span class="text-xl">🔗</span>
            </div>
            <div>
              <div class="text-sm text-gray-800 font-semibold">Connect to WiFi</div>
              <div class="text-xs text-gray-500 mt-0.5">
                {{ wifiList && wifiList.length > 0 ? 'Click a network above or enter SSID manually' : 'Provide SSID and password' }}
              </div>
            </div>
          </div>
          <div class="space-y-3">
            <input
              v-model="wifiSsid"
              class="w-full px-4 py-3 text-sm border border-gray-200 rounded-xl bg-white focus:outline-none focus:ring-2 focus:ring-emerald-500 focus:border-transparent transition-all"
              :placeholder="wifiList && wifiList.length > 0 ? 'Enter SSID manually or click a network above' : 'SSID'"
            />
            <input
              type="password"
              v-model="wifiPassword"
              class="w-full px-4 py-3 text-sm border border-gray-200 rounded-xl bg-white focus:outline-none focus:ring-2 focus:ring-emerald-500 focus:border-transparent transition-all"
              placeholder="Password (optional)"
            />
            <button
              @click="handleConnectWifi"
              class="w-full py-3 text-sm font-medium transition-all duration-200 bg-gradient-to-r from-emerald-600 to-emerald-500 hover:from-emerald-500 hover:to-emerald-600 text-white rounded-xl shadow-sm active:scale-[0.98]"
            >
              Connect
            </button>
          </div>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, watch } from 'vue';
import '../../tailwind.css';

type WifiInfo = {
  SSID?: string;
  ssid?: string;
  BSSID?: string;
  bssid?: string;
  secure?: boolean;
  signalStrength?: number;
  frequency?: number;
  connected?: boolean;
  state?: string;
};

type WifiEvent = {
  id: string;
  time: string;
  ssid: string;
  bssid?: string;
  secure?: boolean;
  signalStrength?: number;
  frequency?: number;
  connected?: boolean;
  state?: string;
};

declare function useLingXia(): any;

const {
  data,
  startWifi,
  stopWifi,
  getWifiList,
  getConnectedWifi,
  connectWifi,
  onWifiConnected,
  offWifiConnected,
  clearWifiConnectedEvents,
} = useLingXia();

const wifiSsid = ref('');
const wifiPassword = ref('');

const wifiList = computed<WifiInfo[]>(() => data.wifiList ?? []);
const connectedWifi = computed<WifiInfo | null>(() => data.connectedWifi ?? null);
const wifiModuleEnabled = computed(() => data.wifiModuleEnabled ?? false);
const wifiListenerEnabled = computed(() => data.wifiListenerEnabled ?? false);
const wifiConnectedEvents = computed<WifiEvent[]>(() => data.wifiConnectedEvents ?? []);

// Auto turn off listener when WiFi module is disabled
watch(wifiModuleEnabled, (enabled) => {
  if (!enabled && wifiListenerEnabled.value) {
    offWifiConnected?.();
  }
});

function handleConnectWifi() {
  const ssid = wifiSsid.value.trim();
  const password = wifiPassword.value.trim();
  if (!ssid) {
    window.alert?.('Please enter SSID');
    return;
  }
  connectWifi({ SSID: ssid, password: password || undefined });
}

function handleStartWifiConnected() {
  if (wifiListenerEnabled.value) return;
  onWifiConnected?.();
}

function handleStopWifiConnected() {
  if (!wifiListenerEnabled.value) return;
  offWifiConnected?.();
}

function handleClearWifiEvents() {
  clearWifiConnectedEvents?.();
}
</script>
