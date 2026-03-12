import React from 'react';
import { useLingXia } from '@lingxia/core/react';
import { LxPicker } from '@lingxia/components/react';
import '../../tailwind.css';

const coffees = ['Espresso', 'Americano', 'Latte', 'Cappuccino', 'Mocha', 'Macchiato'];
const continents = ['Asia', 'Europe', 'America', 'Africa'];
const cities: Record<string, string[]> = {
  Asia: ['Beijing', 'Tokyo', 'Seoul', 'Singapore'],
  Europe: ['London', 'Paris', 'Berlin', 'Rome'],
  America: ['New York', 'Los Angeles', 'Toronto', 'Mexico City'],
  Africa: ['Cairo', 'Lagos', 'Nairobi', 'Johannesburg'],
};
const hours = Array.from({ length: 24 }, (_, i) => i.toString().padStart(2, '0'));
const minutes = Array.from({ length: 60 }, (_, i) => i.toString().padStart(2, '0'));

type ModeTab = 'selector' | 'multiSelector' | 'time' | 'date';

type PageData = {
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

type PageActions = {
  data?: PageData;
  setActiveTab?: (params: { tab: ModeTab }) => void;
};

const tabs: ModeTab[] = ['selector', 'multiSelector', 'time', 'date'];

export default function PickerPage() {
  const { data, setActiveTab } = useLingXia() as PageActions;
  const activeTab: ModeTab = data?.activeTab || 'selector';
  const coffee = data?.coffee;
  const location = data?.location;
  const multiTime = data?.multiTime || ['09', '30'];
  const time = data?.time;
  const year = data?.year;
  const month = data?.month;
  const date = data?.date;
  const dateRange = data?.dateRange;

  const changeTab = React.useCallback(
    (tab: ModeTab) => {
      setActiveTab?.({ tab });
    },
    [setActiveTab],
  );

  return (
    <div className="min-h-screen bg-gray-100">
      <div className="px-4 py-5 space-y-4">
        <div className="bg-gradient-to-r from-purple-500 to-pink-600 rounded-xl px-4 py-4">
          <div className="text-lg text-white font-bold">LxPicker</div>
          <div className="text-xs text-white/80 mt-1">Component like input, tap to show picker</div>
        </div>

        <div className="grid grid-cols-4 gap-1 bg-white rounded-xl p-1">
          {tabs.map((tab) => (
            <button
              key={tab}
              onClick={() => changeTab(tab)}
              className={`py-2 px-1 rounded-lg font-medium text-xs ${
                activeTab === tab ? 'bg-purple-500 text-white' : 'bg-gray-100 text-gray-600'
              }`}
            >
              {tab}
            </button>
          ))}
        </div>

        {activeTab === 'selector' && (
          <div className="bg-white rounded-xl p-4 space-y-3">
            <div className="text-sm font-medium text-gray-900">Single Column Selector</div>
            <LxPicker
              columns={[coffees]}
              value={coffee}
              data-field="coffee"
              bindChange="onPickerChange"
              bindScroll="onPickerScroll"
              placeholder="Select coffee"
            />
          </div>
        )}

        {activeTab === 'multiSelector' && (
          <>
            <div className="bg-white rounded-xl p-4 space-y-3">
              <div className="text-sm font-medium text-gray-900">Cascading (Custom Colors)</div>
              <LxPicker
                columns={[continents, cities]}
                value={location}
                data-field="location"
                bindChange="onPickerChange"
                bindScroll="onPickerScroll"
                placeholder="Select location"
                cancelText="取消"
                cancelTextColor="#FF6B6B"
                cancelButtonColor="#FFF0F0"
                confirmText="确定"
                confirmTextColor="#ffffff"
                confirmButtonColor="#10B981"
              />
            </div>

            <div className="bg-white rounded-xl p-4 space-y-3">
              <div className="text-sm font-medium text-gray-900">Multi Column + Custom UI Trigger</div>
              <div className="text-xs text-gray-500 mb-2">Use children prop to customize trigger appearance</div>
              <LxPicker
                columns={[hours, minutes]}
                value={multiTime}
                data-field="multiTime"
                bindChange="onPickerChange"
                bindScroll="onPickerScroll"
              >
                <div className="p-3 bg-gradient-to-r from-purple-500 to-pink-500 text-white rounded-lg text-center">
                  {multiTime.join(':')}
                </div>
              </LxPicker>
            </div>
          </>
        )}

        {activeTab === 'time' && (
          <div className="bg-white rounded-xl p-4 space-y-3">
            <div className="text-sm font-medium text-gray-900">Time Picker (mode=time)</div>
            <LxPicker
              mode="time"
              value={time}
              start="09:00"
              end="18:00"
              data-field="time"
              bindChange="onPickerChange"
              bindScroll="onPickerScroll"
              placeholder="Select time"
            />
          </div>
        )}

        {activeTab === 'date' && (
          <>
            <div className="bg-white rounded-xl p-4 space-y-3">
              <div className="text-sm font-medium text-gray-900">Year Picker (fields=year)</div>
              <LxPicker
                mode="date"
                fields="year"
                value={year}
                start="2010"
                end="2030"
                data-field="year"
                bindChange="onPickerChange"
                bindScroll="onPickerScroll"
                placeholder="Select year"
              />
            </div>

            <div className="bg-white rounded-xl p-4 space-y-3">
              <div className="text-sm font-medium text-gray-900">Month Picker (fields=month)</div>
              <LxPicker
                mode="date"
                fields="month"
                value={month}
                start="2023-01"
                end="2025-12"
                data-field="month"
                bindChange="onPickerChange"
                bindScroll="onPickerScroll"
                placeholder="Select month"
              />
            </div>

            <div className="bg-white rounded-xl p-4 space-y-3">
              <div className="text-sm font-medium text-gray-900">Day Picker (fields=day)</div>
              <LxPicker
                mode="date"
                fields="day"
                value={date}
                start="2024-01-01"
                end="2027-12-31"
                data-field="date"
                bindChange="onPickerChange"
                bindScroll="onPickerScroll"
                placeholder="Select a date"
              />
            </div>

            <div className="bg-white rounded-xl p-4 space-y-3">
              <div className="text-sm font-medium text-gray-900">Date Range (fields=range)</div>
              <LxPicker
                mode="date"
                fields="range"
                value={dateRange}
                data-field="dateRange"
                bindChange="onPickerChange"
                bindScroll="onPickerScroll"
                placeholder="Select date range"
              />
            </div>
          </>
        )}
      </div>
    </div>
  );
}
