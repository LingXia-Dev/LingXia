<template>
  <div class="min-h-screen bg-gray-100">
    <div class="px-4 py-6">

      <!-- Header -->
      <div class="text-center mb-8">
        <h1 class="text-2xl font-light text-gray-800 mb-2">getLocation</h1>
        <div class="w-16 h-0.5 bg-gray-400 mx-auto"></div>
      </div>

      <!-- Location Display -->
      <div class="bg-white rounded-lg shadow-sm p-6 mb-8">
        <div class="text-center">
          <div class="text-gray-600 mb-4">Current Location</div>

          <template v-if="isLoading">
            <div class="flex items-center justify-center py-8">
              <div class="animate-spin rounded-full h-8 w-8 border-b-2 border-blue-500"></div>
              <span class="ml-3 text-gray-600">Getting location...</span>
            </div>
          </template>

          <template v-else-if="location">
            <div class="space-y-4">
              <div class="text-2xl font-light text-gray-800">
                {{ formatCoordinate(location.longitude, 'longitude') }}
                {{ formatCoordinate(location.latitude, 'latitude') }}
              </div>

              <!-- Location Details -->
              <div class="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 gap-4 mt-6 text-sm">
                <div class="text-center">
                  <div class="text-gray-500">Longitude</div>
                  <div class="font-medium">{{ location.longitude?.toFixed(6) || '--' }}</div>
                </div>
                <div class="text-center">
                  <div class="text-gray-500">Latitude</div>
                  <div class="font-medium">{{ location.latitude?.toFixed(6) || '--' }}</div>
                </div>
                <div class="text-center">
                  <div class="text-gray-500">Accuracy</div>
                  <div class="font-medium">{{ location.accuracy ? `${location.accuracy.toFixed(1)}m` : '--' }}</div>
                </div>
                <div class="text-center">
                  <div class="text-gray-500">Altitude</div>
                  <div class="font-medium">{{ location.altitude ? `${location.altitude.toFixed(1)}m` : '--' }}</div>
                </div>
                <div class="text-center">
                  <div class="text-gray-500">Speed</div>
                  <div class="font-medium">{{ location.speed ? `${location.speed.toFixed(1)}m/s` : '--' }}</div>
                </div>
              </div>
            </div>
          </template>

          <template v-else>
            <div :class="['py-8', locationError ? 'text-red-500' : 'text-gray-500']">
              {{ locationError || 'No location data available' }}
            </div>
          </template>
        </div>
      </div>

      <!-- Action Buttons -->
      <div class="space-y-3">
        <button
          @click="getLocation"
          :disabled="isLoading"
          class="w-full bg-blue-500 hover:bg-blue-600 disabled:bg-gray-400 text-white font-medium py-4 px-6 rounded-lg transition-colors"
        >
          {{ isLoading ? 'Getting Location...' : 'Get Location' }}
        </button>

        <button
          @click="clearLocation"
          class="w-full bg-white hover:bg-gray-50 text-gray-700 font-medium py-4 px-6 rounded-lg border border-gray-300 transition-colors"
        >
          Clear
        </button>
      </div>

    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, onMounted, onUnmounted } from 'vue';
import { useLxPage } from '@lingxia/vue';
import '../../tailwind.css';

const { data, actions } = useLxPage();
const { getLocation, clearLocation } = actions;

const location = computed(() => data.location ?? null);
const locationError = computed(() => data.locationError ?? '');
const isLoading = computed(() => data.isLoading ?? false);

function formatCoordinate(value: number | null | undefined, axis: 'latitude' | 'longitude'): string {
  if (value === null || value === undefined) {
    return '--';
  }

  const absolute = Math.abs(value);
  const degrees = Math.floor(absolute);
  const minutes = Math.floor((absolute - degrees) * 60);
  const direction = axis === 'latitude'
    ? value >= 0 ? 'N' : 'S'
    : value >= 0 ? 'E' : 'W';

  return `${direction}: ${degrees}°${minutes.toString().padStart(2, '0')}'`;
}

onMounted(() => {
  document.body.className = 'location-page';
});

onUnmounted(() => {
  document.body.className = '';
});
</script>
