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
              <div class="text-xs text-gray-500 mt-0.5">Get application language settings</div>
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
                <span class="text-sm text-gray-600">Language</span>
                <span class="text-sm font-semibold text-gray-800 px-3 py-1 bg-blue-50 rounded-lg">{{ appBaseInfo.language || '--' }}</span>
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
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed } from 'vue';
import { useLxPage } from '@lingxia/vue';
import '../../tailwind.css';

const { data, actions } = useLxPage();
const { getBaseInfo, getSystemSetting } = actions;

const currentType = computed(() => data.currentType ?? 'appBaseInfo');
const appBaseInfo = computed(() => data.appBaseInfo ?? null);
const systemSetting = computed(() => data.systemSetting ?? null);

function formatBool(value: boolean | undefined): string {
  if (value === undefined || value === null) {
    return '--';
  }
  return value ? 'Yes' : 'No';
}
</script>
