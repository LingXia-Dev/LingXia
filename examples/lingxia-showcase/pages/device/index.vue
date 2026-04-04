<template>
  <div class="min-h-screen bg-gradient-to-br from-gray-50 to-gray-100">
    <div class="px-4 py-6">
      <!-- Device Info Section -->
      <template v-if="currentType === 'device' || !['device', 'screen', 'vibrate', 'dial', 'orientation'].includes(currentType)">
        <div class="mb-6 text-center">
          <h1 class="text-2xl font-light text-gray-800 mb-2">Device Information</h1>
          <div class="w-16 h-0.5 bg-gray-400 mx-auto"></div>
        </div>

        <div class="mb-5 bg-white rounded-2xl shadow-sm border border-gray-100 overflow-hidden">
          <div class="flex items-center gap-4 px-5 py-5 border-b border-gray-100">
            <div class="flex items-center justify-center w-12 h-12 rounded-xl bg-gradient-to-br from-blue-50 to-indigo-50">
              <span class="text-2xl">📱</span>
            </div>
            <div class="flex-1">
              <div class="text-sm text-gray-800 font-semibold">Get Device Information</div>
              <div class="text-xs text-gray-500 mt-0.5">Brand, model, and OS version</div>
            </div>
            <button
              @click="getDeviceInfo"
              class="px-5 py-2.5 text-sm font-medium transition-all duration-200 bg-gradient-to-r from-blue-600 to-blue-500 hover:from-blue-500 hover:to-blue-600 text-white rounded-xl shadow-sm active:scale-[0.98]"
            >
              Get Info
            </button>
          </div>

          <div v-if="deviceInfo" class="p-5">
            <div class="rounded-xl border border-gray-200 bg-gradient-to-br from-gray-50 to-white p-4">
              <div class="flex items-center gap-2 mb-4">
                <span class="w-1 h-4 bg-blue-500 rounded-full"></span>
                <h4 class="text-sm font-semibold text-gray-700">Device Information</h4>
              </div>
              <div class="flex justify-between items-center py-3 border-b border-gray-200">
                <span class="text-sm text-gray-600">Brand</span>
                <span class="text-sm font-semibold text-gray-800 px-3 py-1 bg-blue-50 rounded-lg">{{ deviceInfo.brand || '--' }}</span>
              </div>
              <div class="flex justify-between items-center py-3 border-b border-gray-200">
                <span class="text-sm text-gray-600">Market Name</span>
                <span class="text-sm font-semibold text-gray-800 px-3 py-1 bg-blue-50 rounded-lg">{{ deviceInfo.marketName || deviceInfo.model || '--' }}</span>
              </div>
              <div class="flex justify-between items-center py-3 border-b border-gray-200">
                <span class="text-sm text-gray-600">Model</span>
                <span class="text-sm font-semibold text-gray-800 px-3 py-1 bg-blue-50 rounded-lg">{{ deviceInfo.model || '--' }}</span>
              </div>
              <div class="flex justify-between items-center py-3 border-b border-gray-200">
                <span class="text-sm text-gray-600">OS Name</span>
                <span class="text-sm font-semibold text-gray-800 px-3 py-1 bg-blue-50 rounded-lg">{{ deviceInfo.osName || '--' }}</span>
              </div>
              <div class="flex justify-between items-center py-3">
                <span class="text-sm text-gray-600">OS Version</span>
                <span class="text-sm font-semibold text-gray-800 px-3 py-1 bg-blue-50 rounded-lg">{{ deviceInfo.osVersion || '--' }}</span>
              </div>
            </div>
          </div>
        </div>
      </template>

      <!-- Screen Info Section -->
      <template v-if="currentType === 'screen'">
        <div class="mb-6 text-center">
          <h1 class="text-2xl font-light text-gray-800 mb-2">Screen Information</h1>
          <div class="w-16 h-0.5 bg-gray-400 mx-auto"></div>
        </div>

        <div class="mb-5 bg-white rounded-2xl shadow-sm border border-gray-100 overflow-hidden">
          <div class="flex items-center gap-4 px-5 py-5 border-b border-gray-100">
            <div class="flex items-center justify-center w-12 h-12 rounded-xl bg-gradient-to-br from-purple-50 to-pink-50">
              <span class="text-2xl">🖥️</span>
            </div>
            <div class="flex-1">
              <div class="text-sm text-gray-800 font-semibold">Get Screen Information</div>
              <div class="text-xs text-gray-500 mt-0.5">Screen dimensions and scale</div>
            </div>
            <button
              @click="getScreenInfo"
              class="px-5 py-2.5 text-sm font-medium transition-all duration-200 bg-gradient-to-r from-blue-600 to-blue-500 hover:from-blue-500 hover:to-blue-600 text-white rounded-xl shadow-sm active:scale-[0.98]"
            >
              Get Info
            </button>
          </div>

          <div v-if="screenInfo" class="p-5">
            <div class="rounded-xl border border-gray-200 bg-gradient-to-br from-gray-50 to-white p-4">
              <div class="flex items-center gap-2 mb-4">
                <span class="w-1 h-4 bg-purple-500 rounded-full"></span>
                <h4 class="text-sm font-semibold text-gray-700">Screen Information</h4>
              </div>
              <div class="flex justify-between items-center py-3 border-b border-gray-200">
                <span class="text-sm text-gray-600">Width</span>
                <span class="text-sm font-semibold text-gray-800 px-3 py-1 bg-blue-50 rounded-lg">{{ formatNumber(screenInfo.width) }}px</span>
              </div>
              <div class="flex justify-between items-center py-3 border-b border-gray-200">
                <span class="text-sm text-gray-600">Height</span>
                <span class="text-sm font-semibold text-gray-800 px-3 py-1 bg-blue-50 rounded-lg">{{ formatNumber(screenInfo.height) }}px</span>
              </div>
              <div class="flex justify-between items-center py-3">
                <span class="text-sm text-gray-600">Scale</span>
                <span class="text-sm font-semibold text-gray-800 px-3 py-1 bg-blue-50 rounded-lg">{{ formatNumber(screenInfo.scale) }}</span>
              </div>
            </div>
          </div>
        </div>
      </template>

      <!-- Vibrate Section -->
      <template v-if="currentType === 'vibrate'">
        <div class="mb-6 text-center">
          <h1 class="text-2xl font-light text-gray-800 mb-2">Device Vibration</h1>
          <div class="w-16 h-0.5 bg-gray-400 mx-auto"></div>
        </div>

        <div class="mb-5 bg-white rounded-2xl shadow-sm border border-gray-100 overflow-hidden">
          <div class="p-6">
            <div class="flex items-center gap-3 mb-4">
              <div class="flex items-center justify-center w-10 h-10 rounded-xl bg-gradient-to-br from-green-50 to-emerald-50">
                <span class="text-xl">📳</span>
              </div>
              <div>
                <div class="text-sm text-gray-800 font-semibold">Trigger Vibration</div>
                <div class="text-xs text-gray-500 mt-0.5">Test short or long vibration</div>
              </div>
            </div>
            <div class="grid grid-cols-2 gap-3">
              <button
                @click="vibrateShort"
                class="py-3 text-sm font-medium transition-all duration-200 bg-gradient-to-r from-blue-600 to-blue-500 hover:from-blue-500 hover:to-blue-600 text-white rounded-xl shadow-sm active:scale-[0.98]"
              >
                Short
              </button>
              <button
                @click="vibrateLong"
                class="py-3 text-sm font-medium transition-all duration-200 bg-gradient-to-r from-indigo-600 to-indigo-500 hover:from-indigo-500 hover:to-indigo-600 text-white rounded-xl shadow-sm active:scale-[0.98]"
              >
                Long
              </button>
            </div>
          </div>
        </div>
      </template>

      <!-- Phone Call Section -->
      <template v-if="currentType === 'dial'">
        <div class="mb-6 text-center">
          <h1 class="text-2xl font-light text-gray-800 mb-2">Phone Call</h1>
          <div class="w-16 h-0.5 bg-gray-400 mx-auto"></div>
        </div>

        <div class="mb-5 bg-white rounded-2xl shadow-sm border border-gray-100 overflow-hidden">
          <div class="p-6">
            <div class="flex items-center gap-3 mb-5">
              <div class="flex items-center justify-center w-10 h-10 rounded-xl bg-gradient-to-br from-orange-50 to-red-50">
                <span class="text-xl">📞</span>
              </div>
              <div>
                <div class="text-sm text-gray-800 font-semibold">Dial Phone Number</div>
                <div class="text-xs text-gray-500 mt-0.5">Initiate a native dialer call</div>
              </div>
            </div>
            <div class="space-y-3">
              <input
                type="tel"
                inputmode="tel"
                v-model="phoneNumber"
                class="w-full px-4 py-3 text-sm border border-gray-200 rounded-xl bg-white focus:outline-none focus:ring-2 focus:ring-blue-500 focus:border-transparent transition-all"
                placeholder="Enter phone number"
              />
              <button
                @click="handleDial"
                class="w-full py-3 text-sm font-medium transition-all duration-200 bg-gradient-to-r from-blue-600 to-blue-500 hover:from-blue-500 hover:to-blue-600 text-white rounded-xl shadow-sm active:scale-[0.98]"
              >
                Call
              </button>
            </div>
          </div>
        </div>
      </template>

      <!-- Device Orientation Section -->
      <template v-if="currentType === 'orientation'">
        <div class="mb-6 text-center">
          <h1 class="text-2xl font-light text-gray-800 mb-2">Device Orientation</h1>
          <div class="w-16 h-0.5 bg-gray-400 mx-auto"></div>
        </div>

        <div class="mb-5 bg-white rounded-2xl shadow-sm border border-gray-100 overflow-hidden">
          <div class="p-6 space-y-4">
            <div class="flex items-center gap-3">
              <div class="flex items-center justify-center w-10 h-10 rounded-xl bg-gradient-to-br from-violet-50 to-indigo-50">
                <span class="text-xl">🧭</span>
              </div>
              <div>
                <div class="text-sm text-gray-800 font-semibold">setDeviceOrientation / onDeviceOrientationChange</div>
                <div class="text-xs text-gray-500 mt-0.5">Lock orientation and listen device orientation changes</div>
              </div>
            </div>

            <div class="grid grid-cols-2 gap-3">
              <button
                @click="setOrientationPortrait"
                class="py-3 text-sm font-medium transition-all duration-200 bg-gradient-to-r from-violet-600 to-violet-500 hover:from-violet-500 hover:to-violet-600 text-white rounded-xl shadow-sm active:scale-[0.98]"
              >
                Lock Portrait
              </button>
              <button
                @click="setOrientationLandscape"
                class="py-3 text-sm font-medium transition-all duration-200 bg-gradient-to-r from-indigo-600 to-indigo-500 hover:from-indigo-500 hover:to-indigo-600 text-white rounded-xl shadow-sm active:scale-[0.98]"
              >
                Lock Landscape
              </button>
            </div>

            <div class="grid grid-cols-2 gap-3">
              <button
                @click="startDeviceOrientationListen"
                class="py-3 text-sm font-medium transition-all duration-200 bg-gradient-to-r from-green-600 to-green-500 hover:from-green-500 hover:to-green-600 text-white rounded-xl shadow-sm active:scale-[0.98]"
              >
                Start Listen
              </button>
              <button
                @click="stopDeviceOrientationListen"
                class="py-3 text-sm font-medium transition-all duration-200 bg-gradient-to-r from-gray-600 to-gray-500 hover:from-gray-500 hover:to-gray-600 text-white rounded-xl shadow-sm active:scale-[0.98]"
              >
                Stop Listen
              </button>
            </div>

            <div class="rounded-xl border border-gray-200 bg-gradient-to-br from-gray-50 to-white p-4">
              <div class="flex justify-between items-center py-3 border-b border-gray-200">
                <span class="text-sm text-gray-600">Listening</span>
                <span class="text-sm font-semibold text-gray-800 px-3 py-1 bg-blue-50 rounded-lg">{{ orientationListening ? 'Yes' : 'No' }}</span>
              </div>
              <div class="flex justify-between items-center py-3 border-b border-gray-200">
                <span class="text-sm text-gray-600">Lock Target</span>
                <span class="text-sm font-semibold text-gray-800 px-3 py-1 bg-blue-50 rounded-lg">{{ orientationLock || '--' }}</span>
              </div>
              <div class="flex justify-between items-center py-3">
                <span class="text-sm text-gray-600">Current Value</span>
                <span class="text-sm font-semibold text-gray-800 px-3 py-1 bg-blue-50 rounded-lg">{{ deviceOrientationValue || '--' }}</span>
              </div>
            </div>

            <div class="rounded-xl border border-gray-200 bg-gradient-to-br from-gray-50 to-white p-4">
              <div class="flex items-center justify-between mb-3">
                <h4 class="text-sm font-semibold text-gray-700">Orientation Events</h4>
                <button
                  @click="clearOrientationEvents"
                  class="px-3 py-1.5 text-xs font-medium transition-all duration-200 bg-gradient-to-r from-gray-600 to-gray-500 hover:from-gray-500 hover:to-gray-600 text-white rounded-lg shadow-sm active:scale-[0.98]"
                >
                  Clear Logs
                </button>
              </div>
              <div class="text-xs text-gray-700 bg-white border border-gray-200 rounded-lg p-3 max-h-56 overflow-auto whitespace-pre-wrap break-all">
                {{ orientationEventsText }}
              </div>
            </div>
          </div>
        </div>
      </template>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, watch } from 'vue';
import { useLxPage } from '@lingxia/vue';
import '../../tailwind.css';

const { data, actions } = useLxPage();
const {
  getDeviceInfo,
  getScreenInfo,
  vibrateShort,
  vibrateLong,
  makePhoneCall,
  setOrientationPortrait,
  setOrientationLandscape,
  startDeviceOrientationListen,
  stopDeviceOrientationListen,
  clearOrientationEvents,
} = actions;

const phoneNumber = ref('');

const currentType = computed(() => data.currentType ?? 'device');
const deviceInfo = computed(() => data.deviceInfo ?? null);
const screenInfo = computed(() => data.screenInfo ?? null);
const orientationListening = computed(() => data.orientationListening ?? false);
const orientationLock = computed(() => data.orientationLock ?? '');
const deviceOrientationValue = computed(() => data.deviceOrientationValue ?? '');
const orientationEvents = computed(() => Array.isArray(data.orientationEvents) ? data.orientationEvents : []);
const orientationEventsText = computed(() => orientationEvents.value.length ? orientationEvents.value.join('\n') : '--');

watch(currentType, () => {
  phoneNumber.value = '';
});

function handleDial() {
  const trimmed = phoneNumber.value.trim();
  if (!trimmed) {
    return;
  }
  makePhoneCall({ phoneNumber: trimmed });
}

function formatNumber(value: number | undefined): string {
  if (typeof value !== 'number' || Number.isNaN(value)) {
    return '--';
  }
  return Number.isInteger(value) ? value.toString() : value.toFixed(2);
}

</script>
