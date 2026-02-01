<template>
  <div class="min-h-screen bg-gray-100">
    <div class="px-4 py-5 space-y-4">
      <!-- Header -->
      <div class="bg-gradient-to-r from-purple-500 to-pink-600 rounded-xl px-4 py-4">
        <div class="text-lg text-white font-bold">LxPicker</div>
        <div class="text-xs text-white/80 mt-1">Component like input, tap to show picker</div>
      </div>

      <!-- Mode Tabs - 4 tabs -->
      <div class="grid grid-cols-4 gap-1 bg-white rounded-xl p-1">
        <button
          v-for="tab in tabs"
          :key="tab"
          @click="activeTab = tab"
          :class="[
            'py-2 px-1 rounded-lg font-medium text-xs',
            activeTab === tab ? 'bg-purple-500 text-white' : 'bg-gray-100 text-gray-600'
          ]"
        >
          {{ tab }}
        </button>
      </div>

      <!-- ===== SELECTOR MODE ===== -->
      <div v-if="activeTab === 'selector'" class="bg-white rounded-xl p-4 space-y-3">
        <div class="text-sm font-medium text-gray-900">Single Column Selector</div>
        <LxPicker
          :columns="[coffees]"
          :value="coffee"
          @confirm="(v) => coffee = v"
          @scroll="(v) => coffee = v"
          placeholder="Select coffee"
        />
      </div>

      <!-- ===== MULTI SELECTOR MODE ===== -->
      <template v-if="activeTab === 'multiSelector'">
        <!-- Cascading -->
        <div class="bg-white rounded-xl p-4 space-y-3">
          <div class="text-sm font-medium text-gray-900">Cascading (Custom Colors)</div>
          <LxPicker
            :columns="[continents, cities]"
            :value="location"
            @confirm="(v) => location = v"
            @scroll="(v) => location = v"
            placeholder="Select location"
            cancel-text="取消"
            cancel-text-color="#FF6B6B"
            cancel-button-color="#FFF0F0"
            confirm-text="确定"
            confirm-text-color="#ffffff"
            confirm-button-color="#10B981"
          />
        </div>

        <!-- Multi Column with Custom Trigger -->
        <div class="bg-white rounded-xl p-4 space-y-3">
          <div class="text-sm font-medium text-gray-900">Multi Column + Custom UI Trigger</div>
          <div class="text-xs text-gray-500 mb-2">Use children prop to customize trigger appearance</div>
          <LxPicker
            :columns="[hours, minutes]"
            :value="multiTime"
            @confirm="(v) => multiTime = v"
            @scroll="(v) => multiTime = v"
          >
            <div class="p-3 bg-gradient-to-r from-purple-500 to-pink-500 text-white rounded-lg text-center">
              {{ multiTime.join(':') }}
            </div>
          </LxPicker>
        </div>
      </template>

      <!-- ===== TIME MODE ===== -->
      <div v-if="activeTab === 'time'" class="bg-white rounded-xl p-4 space-y-3">
        <div class="text-sm font-medium text-gray-900">Time Picker (mode=time)</div>
        <LxPicker
          mode="time"
          :value="time"
          start="09:00"
          end="18:00"
          @confirm="(v) => time = v"
          @scroll="(v) => time = v"
          placeholder="Select time"
        />
      </div>

      <!-- ===== DATE MODE ===== -->
      <template v-if="activeTab === 'date'">
        <!-- Year Picker -->
        <div class="bg-white rounded-xl p-4 space-y-3">
          <div class="text-sm font-medium text-gray-900">Year Picker (fields=year)</div>
          <LxPicker
            mode="date"
            fields="year"
            :value="year"
            start="2010"
            end="2030"
            @confirm="(v) => year = v"
            @scroll="(v) => year = v"
            placeholder="Select year"
          />
        </div>

        <!-- Month Picker -->
        <div class="bg-white rounded-xl p-4 space-y-3">
          <div class="text-sm font-medium text-gray-900">Month Picker (fields=month)</div>
          <LxPicker
            mode="date"
            fields="month"
            :value="month"
            start="2023-01"
            end="2025-12"
            @confirm="(v) => month = v"
            @scroll="(v) => month = v"
            placeholder="Select month"
          />
        </div>

        <!-- Single Date -->
        <div class="bg-white rounded-xl p-4 space-y-3">
          <div class="text-sm font-medium text-gray-900">Day Picker (fields=day)</div>
          <LxPicker
            mode="date"
            fields="day"
            :value="date"
            start="2024-01-01"
            end="2027-12-31"
            @confirm="(v) => date = v"
            @scroll="(v) => date = v"
            placeholder="Select a date"
          />
        </div>

        <!-- Date Range -->
        <div class="bg-white rounded-xl p-4 space-y-3">
          <div class="text-sm font-medium text-gray-900">Date Range (fields=range)</div>
          <LxPicker
            mode="date"
            fields="range"
            :value="dateRange"
            @confirm="(v) => dateRange = v"
            @scroll="(v) => dateRange = v"
            placeholder="Select date range"
          />
        </div>
      </template>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref } from 'vue';
import { LxPicker } from 'lingxia-components/vue';
import '../../tailwind.css';

type ModeTab = 'selector' | 'multiSelector' | 'time' | 'date';

const coffees = ['Espresso', 'Americano', 'Latte', 'Cappuccino', 'Mocha', 'Macchiato'];
const continents = ['Asia', 'Europe', 'America', 'Africa'];
const cities: Record<string, string[]> = {
  'Asia': ['Beijing', 'Tokyo', 'Seoul', 'Singapore'],
  'Europe': ['London', 'Paris', 'Berlin', 'Rome'],
  'America': ['New York', 'Los Angeles', 'Toronto', 'Mexico City'],
  'Africa': ['Cairo', 'Lagos', 'Nairobi', 'Johannesburg']
};
const hours = Array.from({ length: 24 }, (_, i) => i.toString().padStart(2, '0'));
const minutes = Array.from({ length: 60 }, (_, i) => i.toString().padStart(2, '0'));

const tabs: ModeTab[] = ['selector', 'multiSelector', 'time', 'date'];

const activeTab = ref<ModeTab>('selector');
const coffee = ref<string>();
const location = ref<string[]>();
const multiTime = ref<string[]>(['09', '30']);
const time = ref<string>();
const year = ref<string>();
const month = ref<string>();
const date = ref<string>();
const dateRange = ref<string[]>();
</script>
