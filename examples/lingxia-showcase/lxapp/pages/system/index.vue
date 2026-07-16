<template>
  <div class="min-h-screen bg-gradient-to-br from-gray-50 to-gray-100">
    <div class="px-4 py-6">
      <!-- App Base Info -->
      <template v-if="currentType === 'appBaseInfo'">
        <div class="mb-6 text-center">
          <h1 class="text-2xl font-light text-gray-800 mb-2">app.getBaseInfo</h1>
          <div class="w-16 h-0.5 bg-gray-400 mx-auto"></div>
        </div>

        <div class="mb-5 bg-white rounded-2xl shadow-sm border border-gray-100 overflow-hidden">
          <div class="flex items-center gap-4 px-5 py-5 border-b border-gray-100">
            <div class="flex items-center justify-center w-12 h-12 rounded-xl bg-gradient-to-br from-blue-50 to-indigo-50">
              <span class="text-2xl">🧭</span>
            </div>
            <div class="flex-1">
              <div class="text-sm text-gray-800 font-semibold">Fetch App Base Info</div>
              <div class="text-xs text-gray-500 mt-0.5">Get app environment info (locale, display language, OS, version)</div>
            </div>
            <button
              @click="getBaseInfo"
              class="px-5 py-2.5 text-sm font-medium transition-all duration-200 bg-gradient-to-r from-blue-600 to-blue-500 hover:from-blue-500 hover:to-blue-600 text-white rounded-xl shadow-sm active:scale-[0.98]"
            >
              Get Info
            </button>
          </div>

          <div v-if="appBaseInfo" class="p-5">
            <div class="rounded-xl border border-gray-200 bg-gradient-to-br from-gray-50 to-white p-4">
              <div class="flex items-center gap-2 mb-4">
                <span class="w-1 h-4 bg-blue-500 rounded-full"></span>
                <h4 class="text-sm font-semibold text-gray-700">Result</h4>
              </div>
              <div class="flex justify-between items-center py-3 border-b border-gray-200">
                <span class="text-sm text-gray-600">Locale</span>
                <span class="text-sm font-semibold text-gray-800 px-3 py-1 bg-blue-50 rounded-lg">{{ appBaseInfo.locale || '--' }}</span>
              </div>
              <div class="flex justify-between items-center py-3 border-b border-gray-200">
                <span class="text-sm text-gray-600">Display Language</span>
                <span class="text-sm font-semibold text-gray-800 px-3 py-1 bg-blue-50 rounded-lg">{{ appBaseInfo.displayLanguage || '--' }}</span>
              </div>
              <div class="flex justify-between items-center py-3 border-b border-gray-200">
                <span class="text-sm text-gray-600">OS</span>
                <span class="text-sm font-semibold text-gray-800 px-3 py-1 bg-blue-50 rounded-lg">{{ appBaseInfo.os || '--' }}</span>
              </div>
              <div class="flex justify-between items-center py-3 border-b border-gray-200">
                <span class="text-sm text-gray-600">Product Name</span>
                <span class="text-sm font-semibold text-gray-800 px-3 py-1 bg-blue-50 rounded-lg">{{ appBaseInfo.productName || '--' }}</span>
              </div>
              <div class="flex justify-between items-center py-3 border-b border-gray-200">
                <span class="text-sm text-gray-600">Product Version</span>
                <span class="text-sm font-semibold text-gray-800 px-3 py-1 bg-blue-50 rounded-lg">{{ appBaseInfo.version || '--' }}</span>
              </div>
              <div class="flex justify-between items-center py-3">
                <span class="text-sm text-gray-600">SDK Version</span>
                <span class="text-sm font-semibold text-gray-800 px-3 py-1 bg-blue-50 rounded-lg">{{ appBaseInfo.SDKVersion || '--' }}</span>
              </div>
            </div>
          </div>
        </div>
      </template>

      <!-- System Setting -->
      <template v-if="currentType === 'systemSetting'">
        <div class="mb-6 text-center">
          <h1 class="text-2xl font-light text-gray-800 mb-2">getSystemSetting</h1>
          <div class="w-16 h-0.5 bg-gray-400 mx-auto"></div>
        </div>

        <div class="mb-5 bg-white rounded-2xl shadow-sm border border-gray-100 overflow-hidden">
          <div class="flex items-center gap-4 px-5 py-5 border-b border-gray-100">
            <div class="flex items-center justify-center w-12 h-12 rounded-xl bg-gradient-to-br from-emerald-50 to-teal-50">
              <span class="text-2xl">⚙️</span>
            </div>
            <div class="flex-1">
              <div class="text-sm text-gray-800 font-semibold">Fetch System Setting</div>
              <div class="text-xs text-gray-500 mt-0.5">WiFi, location, and Bluetooth toggles</div>
            </div>
            <button
              @click="getSystemSetting"
              class="px-5 py-2.5 text-sm font-medium transition-all duration-200 bg-gradient-to-r from-emerald-600 to-emerald-500 hover:from-emerald-500 hover:to-emerald-600 text-white rounded-xl shadow-sm active:scale-[0.98]"
            >
              Get Info
            </button>
          </div>

          <div v-if="systemSetting" class="p-5">
            <div class="rounded-xl border border-gray-200 bg-gradient-to-br from-gray-50 to-white p-4">
              <div class="flex items-center gap-2 mb-4">
                <span class="w-1 h-4 bg-emerald-500 rounded-full"></span>
                <h4 class="text-sm font-semibold text-gray-700">Result</h4>
              </div>
              <div class="flex justify-between items-center py-3 border-b border-gray-200">
                <span class="text-sm text-gray-600">WiFi Enabled</span>
                <span class="text-sm font-semibold text-gray-800 px-3 py-1 bg-blue-50 rounded-lg">{{ formatBool(systemSetting.wifiEnabled) }}</span>
              </div>
              <div class="flex justify-between items-center py-3 border-b border-gray-200">
                <span class="text-sm text-gray-600">Location Enabled</span>
                <span class="text-sm font-semibold text-gray-800 px-3 py-1 bg-blue-50 rounded-lg">{{ formatBool(systemSetting.locationEnabled) }}</span>
              </div>
              <div class="flex justify-between items-center py-3">
                <span class="text-sm text-gray-600">Bluetooth Enabled</span>
                <span class="text-sm font-semibold text-gray-800 px-3 py-1 bg-blue-50 rounded-lg">{{ formatBool(systemSetting.bluetoothEnabled) }}</span>
              </div>
            </div>
          </div>
        </div>
      </template>

      <!-- Autostart -->
      <template v-if="currentType === 'autostart'">
        <div class="mb-6 text-center">
          <h1 class="text-2xl font-light text-gray-800 mb-2">app.autostart</h1>
          <div class="w-16 h-0.5 bg-gray-400 mx-auto"></div>
        </div>

        <div class="mb-5 bg-white rounded-2xl shadow-sm border border-gray-100 overflow-hidden">
          <div class="flex items-center gap-4 px-5 py-5 border-b border-gray-100">
            <div class="flex items-center justify-center w-12 h-12 rounded-xl bg-gradient-to-br from-amber-50 to-orange-50">
              <span class="text-2xl">🚀</span>
            </div>
            <div class="flex-1">
              <div class="text-sm text-gray-800 font-semibold">Launch at Startup</div>
              <div class="text-xs text-gray-500 mt-0.5">
                {{ autostartSupported ? 'Register this app as a login / startup item' : 'Not available on this platform' }}
              </div>
            </div>
            <button
              v-if="autostartSupported"
              @click="toggleAutostart"
              class="relative inline-flex h-7 w-12 items-center rounded-full transition-colors duration-200"
              :class="autostartEnabled ? 'bg-emerald-500' : 'bg-gray-300'"
            >
              <span
                class="inline-block h-5 w-5 transform rounded-full bg-white shadow transition-transform duration-200"
                :class="autostartEnabled ? 'translate-x-6' : 'translate-x-1'"
              />
            </button>
          </div>

          <div class="p-5">
            <div class="rounded-xl border border-gray-200 bg-gradient-to-br from-gray-50 to-white p-4">
              <div class="flex items-center gap-2 mb-4">
                <span class="w-1 h-4 bg-amber-500 rounded-full"></span>
                <h4 class="text-sm font-semibold text-gray-700">State</h4>
              </div>
              <div class="flex justify-between items-center py-3 border-b border-gray-200">
                <span class="text-sm text-gray-600">Supported</span>
                <span class="text-sm font-semibold text-gray-800 px-3 py-1 bg-blue-50 rounded-lg">{{ formatBool(autostartSupported) }}</span>
              </div>
              <div class="flex justify-between items-center py-3 border-b border-gray-200">
                <span class="text-sm text-gray-600">Enabled (OS)</span>
                <span class="text-sm font-semibold text-gray-800 px-3 py-1 bg-blue-50 rounded-lg">{{ autostartEnabled === null ? '--' : formatBool(autostartEnabled) }}</span>
              </div>
              <div v-if="autostartError" class="flex justify-between items-center py-3 border-b border-gray-200">
                <span class="text-sm text-gray-600">Error</span>
                <span class="text-sm font-semibold text-gray-800 px-3 py-1 bg-blue-50 rounded-lg">{{ autostartError }}</span>
              </div>
              <div class="pt-3">
                <button
                  @click="refreshAutostart"
                  class="px-4 py-2 text-xs font-medium bg-gray-100 hover:bg-gray-200 text-gray-700 rounded-lg transition-colors"
                >
                  Re-read OS State
                </button>
              </div>
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

const { data, actions } = useLxPage();
const { getBaseInfo, getSystemSetting, toggleAutostart, refreshAutostart } = actions;

const currentType = computed(() => data.currentType ?? 'appBaseInfo');
const appBaseInfo = computed(() => data.appBaseInfo ?? null);
const systemSetting = computed(() => data.systemSetting ?? null);
const autostartSupported = computed(() => data.autostartSupported ?? false);
const autostartEnabled = computed(() => data.autostartEnabled ?? null);
const autostartError = computed(() => data.autostartError ?? '');

function formatBool(value: boolean | undefined): string {
  if (value === undefined || value === null) {
    return '--';
  }
  return value ? 'Yes' : 'No';
}
</script>
