import { LxPicker, useLxPage } from '@lingxia/react';
import type { PickerPage as Picker, PickerTab } from './contract';
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

const tabs: PickerTab[] = ['selector', 'multiSelector', 'time', 'date'];

export default function PickerPage() {
  const { data, actions } = useLxPage<Picker['data'], Picker['actions']>();
  const { activeTab, coffee, location, multiTime, time, year, month, date, dateRange } = data;

  return (
    <div className="min-h-screen bg-surface-100">
      <div className="px-4 py-5 space-y-4">
        <div className="bg-linear-to-r from-purple-500 to-pink-600 rounded-xl px-4 py-4">
          <div className="text-lg text-white font-bold">LxPicker</div>
          <div className="text-xs text-white/80 mt-1">Component like input, tap to show picker</div>
        </div>

        <div className="grid grid-cols-4 gap-1 bg-surface rounded-xl p-1">
          {tabs.map((tab) => (
            <button
              key={tab}
              onClick={() => actions.setActiveTab({ tab })}
              className={`py-2 px-1 rounded-lg font-medium text-xs ${
                activeTab === tab ? 'bg-purple-500 text-white' : 'bg-surface-100 text-gray-600'
              }`}
            >
              {tab}
            </button>
          ))}
        </div>

        {activeTab === 'selector' && (
          <div className="bg-surface rounded-xl p-4 space-y-3">
            <div className="text-sm font-medium text-gray-900">Single Column Selector</div>
            <LxPicker
              columns={[coffees]}
              value={coffee}
              onConfirm={(value) => actions.setValue({ field: 'coffee', value })}
              onColumnChange={(value) => actions.setValue({ field: 'coffee', value })}
              placeholder="Select coffee"
            />
          </div>
        )}

        {activeTab === 'multiSelector' && (
          <>
            <div className="bg-surface rounded-xl p-4 space-y-3">
              <div className="text-sm font-medium text-gray-900">Cascading (Custom Colors)</div>
              <LxPicker
                columns={[continents, cities]}
                value={location}
                onConfirm={(value) => actions.setValue({ field: 'location', value })}
                onColumnChange={(value) => actions.setValue({ field: 'location', value })}
                placeholder="Select location"
                cancelText="取消"
                cancelTextColor="#FF6B6B"
                cancelButtonColor="#FFF0F0"
                confirmText="确定"
                confirmTextColor="#ffffff"
                confirmButtonColor="#10B981"
              />
            </div>

            <div className="bg-surface rounded-xl p-4 space-y-3">
              <div className="text-sm font-medium text-gray-900">Multi Column + Custom UI Trigger</div>
              <div className="text-xs text-gray-500 mb-2">Use children prop to customize trigger appearance</div>
              <LxPicker
                columns={[hours, minutes]}
                value={multiTime}
                onConfirm={(value) => actions.setValue({ field: 'multiTime', value })}
                onColumnChange={(value) => actions.setValue({ field: 'multiTime', value })}
              >
                <div data-testid="picker-time-trigger" className="p-3 bg-linear-to-r from-purple-500 to-pink-500 text-white rounded-lg text-center">
                  {multiTime.join(':')}
                </div>
              </LxPicker>
            </div>
          </>
        )}

        {activeTab === 'time' && (
          <div className="bg-surface rounded-xl p-4 space-y-3">
            <div className="text-sm font-medium text-gray-900">Time Picker (mode=time)</div>
            <LxPicker
              mode="time"
              value={time}
              start="09:00"
              end="18:00"
              onConfirm={(value) => actions.setValue({ field: 'time', value })}
              onColumnChange={(value) => actions.setValue({ field: 'time', value })}
              placeholder="Select time"
            />
          </div>
        )}

        {activeTab === 'date' && (
          <>
            <div className="bg-surface rounded-xl p-4 space-y-3">
              <div className="text-sm font-medium text-gray-900">Year Picker (fields=year)</div>
              <LxPicker
                mode="date"
                fields="year"
                value={year}
                start="2010"
                end="2030"
                onConfirm={(value) => actions.setValue({ field: 'year', value })}
                onColumnChange={(value) => actions.setValue({ field: 'year', value })}
                placeholder="Select year"
              />
            </div>

            <div className="bg-surface rounded-xl p-4 space-y-3">
              <div className="text-sm font-medium text-gray-900">Month Picker (fields=month)</div>
              <LxPicker
                mode="date"
                fields="month"
                value={month}
                start="2023-01"
                end="2025-12"
                onConfirm={(value) => actions.setValue({ field: 'month', value })}
                onColumnChange={(value) => actions.setValue({ field: 'month', value })}
                placeholder="Select month"
              />
            </div>

            <div className="bg-surface rounded-xl p-4 space-y-3">
              <div className="text-sm font-medium text-gray-900">Day Picker (fields=day)</div>
              <LxPicker
                mode="date"
                fields="day"
                value={date}
                start="2024-01-01"
                end="2027-12-31"
                onConfirm={(value) => actions.setValue({ field: 'date', value })}
                onColumnChange={(value) => actions.setValue({ field: 'date', value })}
                placeholder="Select a date"
              />
            </div>

            <div className="bg-surface rounded-xl p-4 space-y-3">
              <div className="text-sm font-medium text-gray-900">Date Range (fields=range)</div>
              <LxPicker
                mode="date"
                fields="range"
                value={dateRange}
                onConfirm={(value) => actions.setValue({ field: 'dateRange', value })}
                onColumnChange={(value) => actions.setValue({ field: 'dateRange', value })}
                placeholder="Select date range"
              />
            </div>
          </>
        )}
      </div>
    </div>
  );
}
