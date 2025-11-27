import React from 'react';
import '../../tailwind.css';

declare const lx: {
  makePhoneCall: (options: { phoneNumber: string }) => Promise<unknown> | unknown;
};

export default function DevicePage() {
  const {
    data,
    getDeviceInfo,
    getScreenInfo,
    vibrateShort,
    vibrateLong,
  } = useLingXia();

  const {
    currentType = 'device',
    deviceInfo = null,
    screenInfo = null,
  } = data;
  const [phoneNumber, setPhoneNumber] = React.useState('');

  React.useEffect(() => {
    document.body.classList.add('api-page');
    return () => document.body.classList.remove('api-page');
  }, []);

  React.useEffect(() => {
    setPhoneNumber('');
  }, [currentType]);

  const handleDial = React.useCallback(async () => {
    const trimmed = phoneNumber.trim();
    if (!trimmed) {
      window.alert?.('Please enter a phone number');
      return;
    }

    try {
      await lx.makePhoneCall({ phoneNumber: trimmed });
    } catch (error) {
      console.error('makePhoneCall failed:', error);
    }
  }, [phoneNumber]);

  const renderDeviceInfoSection = () => (
    <>
      <div className="mb-6 text-center">
        <h1 className="text-2xl font-light text-gray-800 mb-2">Device Information</h1>
        <div className="w-16 h-0.5 bg-gray-400 mx-auto"></div>
      </div>

      <div className="mb-5 bg-white rounded-2xl shadow-sm border border-gray-100 overflow-hidden">
        <div className="flex items-center gap-4 px-5 py-5 border-b border-gray-100">
          <div className="flex items-center justify-center w-12 h-12 rounded-xl bg-gradient-to-br from-blue-50 to-indigo-50">
            <span className="text-2xl">📱</span>
          </div>
          <div className="flex-1">
            <div className="text-sm text-gray-800 font-semibold">Get Device Information</div>
            <div className="text-xs text-gray-500 mt-0.5">Brand, model, and system version</div>
          </div>
          <button
            onClick={getDeviceInfo}
            className="px-5 py-2.5 text-sm font-medium transition-all duration-200 bg-gradient-to-r from-blue-600 to-blue-500 hover:from-blue-500 hover:to-blue-600 text-white rounded-xl shadow-sm active:scale-[0.98]"
          >
            Get Info
          </button>
        </div>

        {deviceInfo && (
          <div className="p-5">
            <div className="rounded-xl border border-gray-200 bg-gradient-to-br from-gray-50 to-white p-4">
              <div className="flex items-center gap-2 mb-4">
                <span className="w-1 h-4 bg-blue-500 rounded-full"></span>
                <h4 className="text-sm font-semibold text-gray-700">Device Information</h4>
              </div>
              <div className="space-y-0">
                <InfoRow label="Brand" value={deviceInfo.brand} />
                <InfoRow label="Market Name" value={deviceInfo.marketName || deviceInfo.model} />
                <InfoRow label="Model" value={deviceInfo.model} />
                <InfoRow label="System" value={deviceInfo.system} />
              </div>
            </div>
          </div>
        )}
      </div>
    </>
  );

  const renderScreenInfoSection = () => (
    <>
      <div className="mb-6 text-center">
        <h1 className="text-2xl font-light text-gray-800 mb-2">Screen Information</h1>
        <div className="w-16 h-0.5 bg-gray-400 mx-auto"></div>
      </div>

      <div className="mb-5 bg-white rounded-2xl shadow-sm border border-gray-100 overflow-hidden">
        <div className="flex items-center gap-4 px-5 py-5 border-b border-gray-100">
          <div className="flex items-center justify-center w-12 h-12 rounded-xl bg-gradient-to-br from-purple-50 to-pink-50">
            <span className="text-2xl">🖥️</span>
          </div>
          <div className="flex-1">
            <div className="text-sm text-gray-800 font-semibold">Get Screen Information</div>
            <div className="text-xs text-gray-500 mt-0.5">Screen dimensions and scale</div>
          </div>
          <button
            onClick={getScreenInfo}
            className="px-5 py-2.5 text-sm font-medium transition-all duration-200 bg-gradient-to-r from-blue-600 to-blue-500 hover:from-blue-500 hover:to-blue-600 text-white rounded-xl shadow-sm active:scale-[0.98]"
          >
            Get Info
          </button>
        </div>

        {screenInfo && (
          <div className="p-5">
            <div className="rounded-xl border border-gray-200 bg-gradient-to-br from-gray-50 to-white p-4">
              <div className="flex items-center gap-2 mb-4">
                <span className="w-1 h-4 bg-purple-500 rounded-full"></span>
                <h4 className="text-sm font-semibold text-gray-700">Screen Information</h4>
              </div>
              <div className="space-y-0">
                <InfoRow label="Width" value={formatNumber(screenInfo.width)} suffix="px" />
                <InfoRow label="Height" value={formatNumber(screenInfo.height)} suffix="px" />
                <InfoRow label="Scale" value={formatNumber(screenInfo.scale)} />
              </div>
            </div>
          </div>
        )}
      </div>
    </>
  );

  const renderVibrationSection = () => (
    <>
      <div className="mb-6 text-center">
        <h1 className="text-2xl font-light text-gray-800 mb-2">Device Vibration</h1>
        <div className="w-16 h-0.5 bg-gray-400 mx-auto"></div>
      </div>

      <div className="mb-5 bg-white rounded-2xl shadow-sm border border-gray-100 overflow-hidden">
        <div className="p-6">
          <div className="flex items-center gap-3 mb-4">
            <div className="flex items-center justify-center w-10 h-10 rounded-xl bg-gradient-to-br from-green-50 to-emerald-50">
              <span className="text-xl">📳</span>
            </div>
            <div>
              <div className="text-sm text-gray-800 font-semibold">Trigger Vibration</div>
              <div className="text-xs text-gray-500 mt-0.5">Test short or long vibration</div>
            </div>
          </div>
          <div className="grid grid-cols-2 gap-3">
            <button
              onClick={vibrateShort}
              className="py-3 text-sm font-medium transition-all duration-200 bg-gradient-to-r from-blue-600 to-blue-500 hover:from-blue-500 hover:to-blue-600 text-white rounded-xl shadow-sm active:scale-[0.98]"
            >
              Short
            </button>
            <button
              onClick={vibrateLong}
              className="py-3 text-sm font-medium transition-all duration-200 bg-gradient-to-r from-indigo-600 to-indigo-500 hover:from-indigo-500 hover:to-indigo-600 text-white rounded-xl shadow-sm active:scale-[0.98]"
            >
              Long
            </button>
          </div>
        </div>
      </div>
    </>
  );

  const renderDialSection = () => (
    <>
      <div className="mb-6 text-center">
        <h1 className="text-2xl font-light text-gray-800 mb-2">Phone Call</h1>
        <div className="w-16 h-0.5 bg-gray-400 mx-auto"></div>
      </div>

      <div className="mb-5 bg-white rounded-2xl shadow-sm border border-gray-100 overflow-hidden">
        <div className="p-6">
          <div className="flex items-center gap-3 mb-5">
            <div className="flex items-center justify-center w-10 h-10 rounded-xl bg-gradient-to-br from-orange-50 to-red-50">
              <span className="text-xl">📞</span>
            </div>
            <div>
              <div className="text-sm text-gray-800 font-semibold">Dial Phone Number</div>
              <div className="text-xs text-gray-500 mt-0.5">Initiate a native dialer call</div>
            </div>
          </div>
          <div className="space-y-3">
            <input
              type="tel"
              inputMode="tel"
              value={phoneNumber}
              onChange={(event) => setPhoneNumber(event.target.value)}
              className="w-full px-4 py-3 text-sm border border-gray-200 rounded-xl bg-white focus:outline-none focus:ring-2 focus:ring-blue-500 focus:border-transparent transition-all"
              placeholder="Enter phone number"
            />
            <button
              onClick={handleDial}
              className="w-full py-3 text-sm font-medium transition-all duration-200 bg-gradient-to-r from-blue-600 to-blue-500 hover:from-blue-500 hover:to-blue-600 text-white rounded-xl shadow-sm active:scale-[0.98]"
            >
              Call
            </button>
          </div>
        </div>
      </div>
    </>
  );

  return (
    <div className="min-h-screen bg-gradient-to-br from-gray-50 to-gray-100">
      <div className="px-4 py-6">
        {currentType === 'device' && renderDeviceInfoSection()}
        {currentType === 'screen' && renderScreenInfoSection()}
        {currentType === 'vibrate' && renderVibrationSection()}
        {currentType === 'dial' && renderDialSection()}

        {!['device', 'screen', 'vibrate', 'dial'].includes(currentType) && renderDeviceInfoSection()}
      </div>
    </div>
  );
}

interface InfoRowProps {
  label: string;
  value?: string | number;
  suffix?: string;
}

function InfoRow({ label, value, suffix }: InfoRowProps) {
  const display = value === undefined || value === null || value === '' ? '--' : value;
  const text = suffix && display !== '--' ? `${display}${suffix}` : display;
  return (
    <div className="flex justify-between items-center py-3 border-b border-gray-200 last:border-b-0">
      <span className="text-sm text-gray-600">{label}</span>
      <span className="text-sm font-semibold text-gray-800 px-3 py-1 bg-blue-50 rounded-lg">{text}</span>
    </div>
  );
}

function formatNumber(value: number | undefined): string {
  if (typeof value !== 'number' || Number.isNaN(value)) {
    return '--';
  }
  return Number.isInteger(value) ? value.toString() : value.toFixed(2);
}
