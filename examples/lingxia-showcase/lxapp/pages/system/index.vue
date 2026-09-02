<template>
  <div class="min-h-screen bg-linear-to-br from-surface-50 to-surface-100" data-testid="system-page" :data-mode="currentType">
    <div class="px-4 py-6">
      <!-- App Base Info -->
      <template v-if="currentType === 'appBaseInfo'">
        <div class="mb-6 text-center">
          <h1 class="text-2xl font-light text-gray-800 mb-2">app.getBaseInfo</h1>
          <div class="w-16 h-0.5 bg-surface-400 mx-auto"></div>
        </div>

        <div class="mb-5 bg-surface rounded-2xl shadow-sm border border-line-100 overflow-hidden">
          <div class="flex items-center gap-4 px-5 py-5 border-b border-line-100">
            <div class="flex items-center justify-center w-12 h-12 rounded-xl bg-linear-to-br from-blue-50 to-indigo-50">
              <span class="text-2xl">🧭</span>
            </div>
            <div class="flex-1">
              <div class="text-sm text-gray-800 font-semibold">Fetch App Base Info</div>
              <div class="text-xs text-gray-500 mt-0.5">Get app environment info (locale, display language, OS, version)</div>
            </div>
            <button
              data-testid="system-base-info"
              @click="getBaseInfo"
              class="px-5 py-2.5 text-sm font-medium transition-all duration-200 bg-linear-to-r from-blue-600 to-blue-500 hover:from-blue-500 hover:to-blue-600 text-white rounded-xl shadow-sm active:scale-[0.98]"
            >
              Get Info
            </button>
          </div>

          <div v-if="appBaseInfo" class="p-5" data-testid="system-base-result">
            <div class="rounded-xl border border-line-200 bg-linear-to-br from-surface-50 to-surface p-4">
              <div class="flex items-center gap-2 mb-4">
                <span class="w-1 h-4 bg-blue-500 rounded-full"></span>
                <h4 class="text-sm font-semibold text-gray-700">Result</h4>
              </div>
              <div class="flex justify-between items-center py-3 border-b border-line-200">
                <span class="text-sm text-gray-600">Locale</span>
                <span class="text-sm font-semibold text-gray-800 px-3 py-1 bg-blue-50 rounded-lg">{{ appBaseInfo.locale || '--' }}</span>
              </div>
              <div class="flex justify-between items-center py-3 border-b border-line-200">
                <span class="text-sm text-gray-600">Display Language</span>
                <span class="text-sm font-semibold text-gray-800 px-3 py-1 bg-blue-50 rounded-lg">{{ appBaseInfo.displayLanguage || '--' }}</span>
              </div>
              <div class="flex justify-between items-center py-3 border-b border-line-200">
                <span class="text-sm text-gray-600">OS</span>
                <span class="text-sm font-semibold text-gray-800 px-3 py-1 bg-blue-50 rounded-lg">{{ appBaseInfo.os || '--' }}</span>
              </div>
              <div class="flex justify-between items-center py-3 border-b border-line-200">
                <span class="text-sm text-gray-600">Product Name</span>
                <span class="text-sm font-semibold text-gray-800 px-3 py-1 bg-blue-50 rounded-lg">{{ appBaseInfo.productName || '--' }}</span>
              </div>
              <div class="flex justify-between items-center py-3 border-b border-line-200">
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
          <div class="w-16 h-0.5 bg-surface-400 mx-auto"></div>
        </div>

        <div class="mb-5 bg-surface rounded-2xl shadow-sm border border-line-100 overflow-hidden">
          <div class="flex items-center gap-4 px-5 py-5 border-b border-line-100">
            <div class="flex items-center justify-center w-12 h-12 rounded-xl bg-linear-to-br from-emerald-50 to-teal-50">
              <span class="text-2xl">⚙️</span>
            </div>
            <div class="flex-1">
              <div class="text-sm text-gray-800 font-semibold">Fetch System Setting</div>
              <div class="text-xs text-gray-500 mt-0.5">WiFi, location, and Bluetooth toggles</div>
            </div>
            <button
              data-testid="system-setting-info"
              @click="getSystemSetting"
              class="px-5 py-2.5 text-sm font-medium transition-all duration-200 bg-linear-to-r from-emerald-600 to-emerald-500 hover:from-emerald-500 hover:to-emerald-600 text-white rounded-xl shadow-sm active:scale-[0.98]"
            >
              Get Info
            </button>
          </div>

          <div v-if="systemSetting" class="p-5" data-testid="system-setting-result">
            <div class="rounded-xl border border-line-200 bg-linear-to-br from-surface-50 to-surface p-4">
              <div class="flex items-center gap-2 mb-4">
                <span class="w-1 h-4 bg-emerald-500 rounded-full"></span>
                <h4 class="text-sm font-semibold text-gray-700">Result</h4>
              </div>
              <div class="flex justify-between items-center py-3 border-b border-line-200">
                <span class="text-sm text-gray-600">WiFi Enabled</span>
                <span class="text-sm font-semibold text-gray-800 px-3 py-1 bg-blue-50 rounded-lg">{{ formatBool(systemSetting.wifiEnabled) }}</span>
              </div>
              <div class="flex justify-between items-center py-3 border-b border-line-200">
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
          <div class="w-16 h-0.5 bg-surface-400 mx-auto"></div>
        </div>

        <div class="mb-5 bg-surface rounded-2xl shadow-sm border border-line-100 overflow-hidden">
          <div class="flex items-center gap-4 px-5 py-5 border-b border-line-100">
            <div class="flex items-center justify-center w-12 h-12 rounded-xl bg-linear-to-br from-amber-50 to-orange-50">
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
              :class="autostartEnabled ? 'bg-emerald-500' : 'bg-surface-300'"
            >
              <span
                class="inline-block h-5 w-5 transform rounded-full bg-surface shadow transition-transform duration-200"
                :class="autostartEnabled ? 'translate-x-6' : 'translate-x-1'"
              />
            </button>
          </div>

          <div class="p-5">
            <div class="rounded-xl border border-line-200 bg-linear-to-br from-surface-50 to-surface p-4">
              <div class="flex items-center gap-2 mb-4">
                <span class="w-1 h-4 bg-amber-500 rounded-full"></span>
                <h4 class="text-sm font-semibold text-gray-700">State</h4>
              </div>
              <div class="flex justify-between items-center py-3 border-b border-line-200">
                <span class="text-sm text-gray-600">Supported</span>
                <span class="text-sm font-semibold text-gray-800 px-3 py-1 bg-blue-50 rounded-lg">{{ formatBool(autostartSupported) }}</span>
              </div>
              <div class="flex justify-between items-center py-3 border-b border-line-200">
                <span class="text-sm text-gray-600">Enabled (OS)</span>
                <span class="text-sm font-semibold text-gray-800 px-3 py-1 bg-blue-50 rounded-lg">{{ autostartEnabled === null ? '--' : formatBool(autostartEnabled) }}</span>
              </div>
              <div v-if="autostartError" class="flex justify-between items-center py-3 border-b border-line-200">
                <span class="text-sm text-gray-600">Error</span>
                <span class="text-sm font-semibold text-gray-800 px-3 py-1 bg-blue-50 rounded-lg">{{ autostartError }}</span>
              </div>
              <div class="pt-3">
                <button
                  @click="refreshAutostart"
                  class="px-4 py-2 text-xs font-medium bg-surface-100 hover:bg-surface-200 text-gray-700 rounded-lg transition-colors"
                >
                  Re-read OS State
                </button>
              </div>
            </div>
          </div>
        </div>
      </template>

      <!-- Product cache -->
      <template v-if="currentType === 'cache'">
        <div class="mb-6 text-center">
          <h1 class="text-2xl font-light text-gray-800 mb-2">app.cache</h1>
          <div class="w-16 h-0.5 bg-surface-400 mx-auto"></div>
        </div>

        <div
          data-testid="system-cache-panel"
          class="mb-5 bg-surface rounded-2xl shadow-sm border border-line-100 overflow-hidden"
        >
          <div class="flex items-center gap-4 px-5 py-5 border-b border-line-100">
            <div class="flex items-center justify-center w-12 h-12 rounded-xl bg-linear-to-br from-sky-50 to-blue-50">
              <span class="text-2xl">🧹</span>
            </div>
            <div class="flex-1">
              <div class="text-sm text-gray-800 font-semibold">Product Cache</div>
              <div class="text-xs text-gray-500 mt-0.5">
                Every lxapp's cache, not just this one — home lxapp only
              </div>
            </div>
            <button
              :disabled="cacheBusy"
              class="px-4 py-2 text-xs font-medium bg-sky-500 hover:bg-sky-600 disabled:bg-surface-300 text-white rounded-lg transition-colors"
              @click="clearCache"
            >
              {{ cacheBusy ? 'Clearing…' : 'Clear' }}
            </button>
          </div>

          <div class="p-5">
            <div class="rounded-xl border border-line-200 bg-linear-to-br from-surface-50 to-surface p-4">
              <div class="flex items-center gap-2 mb-4">
                <span class="w-1 h-4 bg-sky-500 rounded-full"></span>
                <h4 class="text-sm font-semibold text-gray-700">State</h4>
              </div>
              <div class="flex items-center justify-between py-2">
                <span class="text-xs text-gray-500">Size</span>
                <span class="text-sm font-semibold text-gray-800 px-3 py-1 bg-blue-50 rounded-lg">{{ formatBytes(cacheBytes) }}</span>
              </div>
              <div class="flex items-center justify-between py-2">
                <span class="text-xs text-gray-500">Last Freed</span>
                <span class="text-sm font-semibold text-gray-800 px-3 py-1 bg-blue-50 rounded-lg">{{ formatBytes(cacheFreedBytes) }}</span>
              </div>
              <div v-if="cacheError" class="flex items-center justify-between py-2">
                <span class="text-xs text-gray-500">Error</span>
                <span class="text-sm font-semibold text-gray-800 px-3 py-1 bg-blue-50 rounded-lg">{{ cacheError }}</span>
              </div>
              <div class="pt-3 text-xs text-gray-500">
                Size counts LingXia-managed files only; a clear also drops the
                WebView cache, so it usually frees more than this shows.
              </div>
              <div class="pt-3">
                <button
                  class="px-4 py-2 text-xs font-medium bg-surface-100 hover:bg-surface-200 text-gray-700 rounded-lg transition-colors"
                  @click="refreshCacheSize"
                >
                  Re-read Size
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
const {
  getBaseInfo,
  getSystemSetting,
  toggleAutostart,
  refreshAutostart,
  refreshCacheSize,
  clearCache,
} = actions;

const currentType = computed(() => data.currentType ?? 'appBaseInfo');
const appBaseInfo = computed(() => data.appBaseInfo ?? null);
const systemSetting = computed(() => data.systemSetting ?? null);
const autostartSupported = computed(() => data.autostartSupported ?? false);
const autostartEnabled = computed(() => data.autostartEnabled ?? null);
const autostartError = computed(() => data.autostartError ?? '');
const cacheBytes = computed(() => data.cacheBytes ?? null);
const cacheFreedBytes = computed(() => data.cacheFreedBytes ?? null);
const cacheBusy = computed(() => data.cacheBusy ?? false);
const cacheError = computed(() => data.cacheError ?? '');

function formatBytes(value: number | null): string {
  if (typeof value !== 'number') {
    return '--';
  }
  if (value < 1024) {
    return `${value} B`;
  }
  const units = ['KB', 'MB', 'GB'];
  let size = value / 1024;
  let unit = 0;
  while (size >= 1024 && unit < units.length - 1) {
    size /= 1024;
    unit += 1;
  }
  return `${size.toFixed(1)} ${units[unit]}`;
}

function formatBool(value: boolean | undefined): string {
  if (value === undefined || value === null) {
    return '--';
  }
  return value ? 'Yes' : 'No';
}
</script>
