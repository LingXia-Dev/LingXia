import React, { useState } from 'react';
import { LxPicker } from 'lingxia-components/react';
import '../../tailwind.css';

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

type ModeTab = 'selector' | 'multiSelector' | 'time' | 'date';

export default function PickerPage() {
  const [activeTab, setActiveTab] = useState<ModeTab>('selector');
  const [coffee, setCoffee] = useState<string>();
  const [location, setLocation] = useState<string[]>();
  const [multiTime, setMultiTime] = useState<string[]>(['09', '30']);
  const [time, setTime] = useState<string>();
  const [year, setYear] = useState<string>();
  const [month, setMonth] = useState<string>();
  const [date, setDate] = useState<string>();
  const [dateRange, setDateRange] = useState<string[]>();

  const tabs: ModeTab[] = ['selector', 'multiSelector', 'time', 'date'];

  return (
    <div className="min-h-screen bg-gray-100">
      <div className="px-4 py-5 space-y-4">
        {/* Header */}
        <div className="bg-gradient-to-r from-purple-500 to-pink-600 rounded-xl px-4 py-4">
          <div className="text-lg text-white font-bold">LxPicker</div>
          <div className="text-xs text-white/80 mt-1">Component like input, tap to show picker</div>
        </div>

        {/* Mode Tabs - 4 tabs */}
        <div className="grid grid-cols-4 gap-1 bg-white rounded-xl p-1">
          {tabs.map((tab) => (
            <button
              key={tab}
              onClick={() => setActiveTab(tab)}
              className={`py-2 px-1 rounded-lg font-medium text-xs ${
                activeTab === tab ? 'bg-purple-500 text-white' : 'bg-gray-100 text-gray-600'
              }`}
            >
              {tab}
            </button>
          ))}
        </div>

        {/* ===== SELECTOR MODE ===== */}
        {activeTab === 'selector' && (
          <div className="bg-white rounded-xl p-4 space-y-3">
            <div className="text-sm font-medium text-gray-900">Single Column Selector</div>
            <LxPicker
              columns={[coffees]}
              value={coffee}
              onConfirm={(v) => setCoffee(v as string)}
              onScroll={(v) => setCoffee(v as string)}
              placeholder="Select coffee"
            />
          </div>
        )}

        {/* ===== MULTI SELECTOR MODE ===== */}
        {activeTab === 'multiSelector' && (
          <>
            {/* Cascading */}
            <div className="bg-white rounded-xl p-4 space-y-3">
              <div className="text-sm font-medium text-gray-900">Cascading (Custom Colors)</div>
              <LxPicker
                columns={[continents, cities]}
                value={location}
                onConfirm={(v) => setLocation(v as string[])}
                onScroll={(v) => setLocation(v as string[])}
                placeholder="Select location"
                cancelText="取消"
                cancelTextColor="#FF6B6B"
                cancelButtonColor="#FFF0F0"
                confirmText="确定"
                confirmTextColor="#ffffff"
                confirmButtonColor="#10B981"
              />
            </div>

            {/* Multi Column with Custom Trigger */}
            <div className="bg-white rounded-xl p-4 space-y-3">
              <div className="text-sm font-medium text-gray-900">Multi Column + Custom UI Trigger</div>
              <div className="text-xs text-gray-500 mb-2">Use children prop to customize trigger appearance</div>
              <LxPicker
                columns={[hours, minutes]}
                value={multiTime}
                onConfirm={(v) => setMultiTime(v as string[])}
                onScroll={(v) => setMultiTime(v as string[])}
              >
                <div className="p-3 bg-gradient-to-r from-purple-500 to-pink-500 text-white rounded-lg text-center">
                  {multiTime.join(':')}
                </div>
              </LxPicker>
            </div>
          </>
        )}

        {/* ===== TIME MODE ===== */}
        {activeTab === 'time' && (
          <div className="bg-white rounded-xl p-4 space-y-3">
            <div className="text-sm font-medium text-gray-900">Time Picker (mode=time)</div>
            <LxPicker
              mode="time"
              value={time}
              start="09:00"
              end="18:00"
              onConfirm={(v) => setTime(v as string)}
              onScroll={(v) => setTime(v as string)}
              placeholder="Select time"
            />
          </div>
        )}

        {/* ===== DATE MODE ===== */}
        {activeTab === 'date' && (
          <>
            {/* Year Picker */}
            <div className="bg-white rounded-xl p-4 space-y-3">
              <div className="text-sm font-medium text-gray-900">Year Picker (fields=year)</div>
              <LxPicker
                mode="date"
                fields="year"
                value={year}
                start="2010"
                end="2030"
                onConfirm={(v) => setYear(v as string)}
                onScroll={(v) => setYear(v as string)}
                placeholder="Select year"
              />
            </div>

            {/* Month Picker */}
            <div className="bg-white rounded-xl p-4 space-y-3">
              <div className="text-sm font-medium text-gray-900">Month Picker (fields=month)</div>
              <LxPicker
                mode="date"
                fields="month"
                value={month}
                start="2023-01"
                end="2025-12"
                onConfirm={(v) => setMonth(v as string)}
                onScroll={(v) => setMonth(v as string)}
                placeholder="Select month"
              />
            </div>

            {/* Single Date */}
            <div className="bg-white rounded-xl p-4 space-y-3">
              <div className="text-sm font-medium text-gray-900">Day Picker (fields=day)</div>
              <LxPicker
                mode="date"
                fields="day"
                value={date}
                start="2024-01-01"
                end="2027-12-31"
                onConfirm={(v) => setDate(v as string)}
                onScroll={(v) => setDate(v as string)}
                placeholder="Select a date"
              />
            </div>

            {/* Date Range */}
            <div className="bg-white rounded-xl p-4 space-y-3">
              <div className="text-sm font-medium text-gray-900">Date Range (fields=range)</div>
              <LxPicker
                mode="date"
                fields="range"
                value={dateRange}
                onConfirm={(v) => setDateRange(v as string[])}
                onScroll={(v) => setDateRange(v as string[])}
                placeholder="Select date range"
              />
            </div>
          </>
        )}
      </div>
    </div>
  );
}
