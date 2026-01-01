import React from 'react';
import { LxPicker } from 'lingxia-ui/react';
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

export default function PickerPage() {
  const { data, setCoffee, setLocation, setTime } = useLingXia();
  const { coffee, location, time = ['09', '30'] } = data;

  return (
    <div className="min-h-screen bg-gray-100">
      <div className="px-4 py-5 space-y-4">
        {/* Header */}
        <div className="bg-gradient-to-r from-purple-500 to-pink-600 rounded-xl px-4 py-4">
          <div className="text-lg text-white font-bold">LxPicker</div>
          <div className="text-xs text-white/80 mt-1">onConfirm / onCancel / onScroll</div>
        </div>

        {/* Single Column with onScroll */}
        <div className="bg-white rounded-xl p-4 space-y-3">
          <div className="text-sm font-medium text-gray-900">Single Column (with onScroll)</div>
          <LxPicker
            columns={[coffees]}
            value={coffee}
            onConfirm={(v) => setCoffee({ value: v })}
            onScroll={(v) => setCoffee({ value: v })}
            placeholder="Select coffee"
          />
          <div className="flex items-center gap-2 text-xs text-gray-500">
            <span>value:</span>
            <span className="font-mono text-purple-600">{coffee ? `"${coffee}"` : 'undefined'}</span>
          </div>
        </div>

        {/* Cascading - with custom button colors and Chinese text */}
        <div className="bg-white rounded-xl p-4 space-y-3">
          <div className="text-sm font-medium text-gray-900">Cascading (with onScroll)</div>
          <LxPicker
            columns={[continents, cities]}
            value={location}
            onConfirm={(v) => setLocation({ value: v })}
            onScroll={(v) => setLocation({ value: v })}
            placeholder="Select location(选择地点)"
            cancelText="取消"
            cancelTextColor="#FF6B6B"
            cancelButtonColor="#FFF0F0"
            confirmText="确定"
            confirmTextColor="#ffffff"
            confirmButtonColor="#10B981"
          />
          <div className="flex items-center gap-2 text-xs text-gray-500">
            <span>value:</span>
            <span className="font-mono text-purple-600">{location ? JSON.stringify(location) : 'undefined'}</span>
          </div>
        </div>

        {/* Time - Custom trigger */}
        <div className="bg-white rounded-xl p-4 space-y-3">
          <div className="text-sm font-medium text-gray-900">Multi Column</div>
          <LxPicker
            columns={[hours, minutes]}
            value={time}
            onConfirm={(v) => setTime({ value: v })}
          >
            <div className="p-3 bg-gradient-to-r from-purple-500 to-pink-500 text-white rounded-lg text-center">
              {Array.isArray(time) ? time.join(':') : '09:30'}
            </div>
          </LxPicker>
          <div className="flex items-center gap-2 text-xs text-gray-500">
            <span>value:</span>
            <span className="font-mono text-purple-600">{JSON.stringify(time)}</span>
          </div>
        </div>
      </div>
    </div>
  );
}
