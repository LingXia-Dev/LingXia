<template>
  <div class="min-h-screen bg-gray-100">
    <div class="px-4 py-5 space-y-4">
      <div class="bg-gradient-to-r from-purple-500 to-pink-600 rounded-xl px-4 py-4">
        <div class="text-lg text-white font-bold">LxPicker</div>
        <div class="text-xs text-white/80 mt-1">Component like input, tap to show picker</div>
      </div>

      <div class="grid grid-cols-4 gap-1 bg-white rounded-xl p-1">
        <button
          v-for="tab in tabs"
          :key="tab"
          @click="setTab(tab)"
          :class="[
            'py-2 px-1 rounded-lg font-medium text-xs',
            activeTab === tab ? 'bg-purple-500 text-white' : 'bg-gray-100 text-gray-600'
          ]"
        >
          {{ tab }}
        </button>
      </div>

      <div v-if="activeTab === 'selector'" class="bg-white rounded-xl p-4 space-y-3">
        <div class="text-sm font-medium text-gray-900">Single Column Selector</div>
        <LxPicker
          :columns="[coffees]"
          :value="coffee"
          @confirm="(value: string | string[]) => onPickerConfirm?.({ field: 'coffee', value })"
          @scroll="(value: string | string[]) => onPickerScroll?.({ field: 'coffee', value })"
          placeholder="Select coffee"
        />
      </div>

      <template v-if="activeTab === 'multiSelector'">
        <div class="bg-white rounded-xl p-4 space-y-3">
          <div class="text-sm font-medium text-gray-900">Cascading (Custom Colors)</div>
          <LxPicker
            :columns="[continents, cities]"
            :value="location"
            @confirm="(value: string | string[]) => onPickerConfirm?.({ field: 'location', value })"
            @scroll="(value: string | string[]) => onPickerScroll?.({ field: 'location', value })"
            placeholder="Select location"
            cancel-text="取消"
            cancel-text-color="#FF6B6B"
            cancel-button-color="#FFF0F0"
            confirm-text="确定"
            confirm-text-color="#ffffff"
            confirm-button-color="#10B981"
          />
        </div>

        <div class="bg-white rounded-xl p-4 space-y-3">
          <div class="text-sm font-medium text-gray-900">Multi Column + Custom UI Trigger</div>
          <div class="text-xs text-gray-500 mb-2">Use children prop to customize trigger appearance</div>
          <LxPicker
            :columns="[hours, minutes]"
            :value="multiTime"
            @confirm="(value: string | string[]) => onPickerConfirm?.({ field: 'multiTime', value })"
            @scroll="(value: string | string[]) => onPickerScroll?.({ field: 'multiTime', value })"
          >
            <div class="p-3 bg-gradient-to-r from-purple-500 to-pink-500 text-white rounded-lg text-center">
              {{ multiTimeLabel }}
            </div>
          </LxPicker>
        </div>
      </template>

      <div v-if="activeTab === 'time'" class="bg-white rounded-xl p-4 space-y-3">
        <div class="text-sm font-medium text-gray-900">Time Picker (mode=time)</div>
        <LxPicker
          mode="time"
          :value="time"
          start="09:00"
          end="18:00"
          @confirm="(value: string | string[]) => onPickerConfirm?.({ field: 'time', value })"
          @scroll="(value: string | string[]) => onPickerScroll?.({ field: 'time', value })"
          placeholder="Select time"
        />
      </div>

      <template v-if="activeTab === 'date'">
        <div class="bg-white rounded-xl p-4 space-y-3">
          <div class="text-sm font-medium text-gray-900">Year Picker (fields=year)</div>
          <LxPicker
            mode="date"
            fields="year"
            :value="year"
            start="2010"
            end="2030"
            @confirm="(value: string | string[]) => onPickerConfirm?.({ field: 'year', value })"
            @scroll="(value: string | string[]) => onPickerScroll?.({ field: 'year', value })"
            placeholder="Select year"
          />
        </div>

        <div class="bg-white rounded-xl p-4 space-y-3">
          <div class="text-sm font-medium text-gray-900">Month Picker (fields=month)</div>
          <LxPicker
            mode="date"
            fields="month"
            :value="month"
            start="2023-01"
            end="2025-12"
            @confirm="(value: string | string[]) => onPickerConfirm?.({ field: 'month', value })"
            @scroll="(value: string | string[]) => onPickerScroll?.({ field: 'month', value })"
            placeholder="Select month"
          />
        </div>

        <div class="bg-white rounded-xl p-4 space-y-3">
          <div class="text-sm font-medium text-gray-900">Day Picker (fields=day)</div>
          <LxPicker
            mode="date"
            fields="day"
            :value="date"
            start="2024-01-01"
            end="2027-12-31"
            @confirm="(value: string | string[]) => onPickerConfirm?.({ field: 'date', value })"
            @scroll="(value: string | string[]) => onPickerScroll?.({ field: 'date', value })"
            placeholder="Select a date"
          />
        </div>

        <div class="bg-white rounded-xl p-4 space-y-3">
          <div class="text-sm font-medium text-gray-900">Date Range (fields=range)</div>
          <LxPicker
            mode="date"
            fields="range"
            :value="dateRange"
            @confirm="(value: string | string[]) => onPickerConfirm?.({ field: 'dateRange', value })"
            @scroll="(value: string | string[]) => onPickerScroll?.({ field: 'dateRange', value })"
            placeholder="Select date range"
          />
        </div>
      </template>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed } from 'vue';
import { useLingXia } from '@lingxia/vue';
import { LxPicker } from '@lingxia/vue';
import '../../tailwind.css';

type ModeTab = 'selector' | 'multiSelector' | 'time' | 'date';
type PickerPageData = {
  activeTab?: ModeTab;
  coffee?: string;
  location?: string[];
  multiTime?: string[];
  time?: string;
  year?: string;
  month?: string;
  date?: string;
  dateRange?: string[];
};

const coffees = ['Espresso', 'Americano', 'Latte', 'Cappuccino', 'Mocha', 'Macchiato'];
const continents = ['Asia', 'Europe', 'America', 'Africa'];
const cities: Record<string, string[]> = {
  Asia: ['Beijing', 'Tokyo', 'Seoul', 'Singapore'],
  Europe: ['London', 'Paris', 'Berlin', 'Rome'],
  America: ['New York', 'Los Angeles', 'Toronto', 'Mexico City'],
  Africa: ['Cairo', 'Lagos', 'Nairobi', 'Johannesburg']
};
const hours = Array.from({ length: 24 }, (_, i) => i.toString().padStart(2, '0'));
const minutes = Array.from({ length: 60 }, (_, i) => i.toString().padStart(2, '0'));
const tabs: ModeTab[] = ['selector', 'multiSelector', 'time', 'date'];

const {
  data,
  setActiveTab,
  onPickerConfirm,
  onPickerScroll,
} = useLingXia() as {
  data?: PickerPageData;
  setActiveTab: (params: { tab: ModeTab }) => void;
  onPickerConfirm: (params: { field: string; value: string | string[] }) => void;
  onPickerScroll: (params: { field: string; value: string | string[] }) => void;
};
const activeTab = computed<ModeTab>(() => data?.activeTab || 'selector');
const coffee = computed(() => data?.coffee);
const location = computed(() => data?.location);
const multiTime = computed(() => data?.multiTime || ['09', '30']);
const time = computed(() => data?.time);
const year = computed(() => data?.year);
const month = computed(() => data?.month);
const date = computed(() => data?.date);
const dateRange = computed(() => data?.dateRange);
const multiTimeLabel = computed(() => multiTime.value.join(':'));

function setTab(tab: ModeTab) {
  setActiveTab?.({ tab });
}
</script>
