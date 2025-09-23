import React from 'react';
import '../../tailwind.css';

declare const lx: {
  makePhoneCall: (options: { phoneNumber: string }) => Promise<unknown> | unknown;
};

export default function DevicePage() {
  // Use LingXia hook to get data and functions
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
      <div className="mt-6 mb-3 px-5 text-sm text-gray-500 font-medium">Device Information</div>
      <div className="mx-3 mb-4 bg-white rounded-xl shadow-sm border border-gray-200 overflow-hidden">
        <div className="flex items-center px-4 py-4">
          <div className="text-xl mr-4">📱</div>
          <div className="flex-1">
            <div className="text-base text-black mb-0.5 font-medium">Get Device Information</div>
            <div className="text-xs text-gray-400 leading-tight">Retrieve device brand, model and system version</div>
          </div>
          <div className="flex items-center gap-2 ml-3">
            <button
              onClick={getDeviceInfo}
              className="h-7 px-3 text-xs font-medium transition-all duration-200 bg-blue-500 hover:bg-blue-600 text-white border-0 shadow-sm rounded"
            >
              Get
            </button>
          </div>
        </div>

        {deviceInfo && (
          <div className="mx-4 mb-4 p-4 bg-gray-50 rounded-lg border border-gray-200">
            <h4 className="text-sm font-medium text-gray-700 mb-3">Device Information</h4>
            <div className="space-y-2">
              <InfoRow label="Brand" value={deviceInfo.brand} />
              <InfoRow label="Model" value={deviceInfo.model} />
              <InfoRow label="System" value={deviceInfo.system} />
            </div>
          </div>
        )}
      </div>
    </>
  );

  const renderScreenInfoSection = () => (
    <>
      <div className="mt-6 mb-3 px-5 text-sm text-gray-500 font-medium">Screen Information</div>
      <div className="mx-3 mb-4 bg-white rounded-xl shadow-sm border border-gray-200 overflow-hidden">
        <div className="flex items-center px-4 py-4">
          <div className="text-xl mr-4">🖥️</div>
          <div className="flex-1">
            <div className="text-base text-black mb-0.5 font-medium">Get Screen Information</div>
            <div className="text-xs text-gray-400 leading-tight">Fetch screen width, height and scale</div>
          </div>
          <button
            onClick={getScreenInfo}
            className="h-7 px-3 text-xs font-medium transition-all duration-200 bg-blue-500 hover:bg-blue-600 text-white border-0 shadow-sm rounded"
          >
            Get
          </button>
        </div>

        {screenInfo && (
          <div className="mx-4 mb-4 p-4 bg-gray-50 rounded-lg border border-gray-200">
            <h4 className="text-sm font-medium text-gray-700 mb-3">Screen Information</h4>
            <div className="space-y-2">
              <InfoRow label="Width" value={formatNumber(screenInfo.width)} suffix="px" />
              <InfoRow label="Height" value={formatNumber(screenInfo.height)} suffix="px" />
              <InfoRow label="Scale" value={formatNumber(screenInfo.scale)} />
            </div>
          </div>
        )}
      </div>
    </>
  );

  const renderVibrationSection = () => (
    <>
      <div className="mt-6 mb-3 px-5 text-sm text-gray-500 font-medium">Device Vibration</div>
      <div className="mx-3 mb-4 bg-white rounded-xl shadow-sm border border-gray-200 overflow-hidden">
        <div className="px-4 py-4">
          <div className="text-base text-black font-medium mb-1">Trigger Vibration</div>
          <div className="text-xs text-gray-400 leading-tight mb-4">Use short or long pulse to test device vibration API</div>
          <div className="flex items-center gap-3">
            <button
              onClick={vibrateShort}
              className="h-8 px-4 text-xs font-medium transition-all duration-200 bg-blue-500 hover:bg-blue-600 text-white border-0 shadow-sm rounded"
            >
              Short Vibration
            </button>
            <button
              onClick={vibrateLong}
              className="h-8 px-4 text-xs font-medium transition-all duration-200 bg-indigo-500 hover:bg-indigo-600 text-white border-0 shadow-sm rounded"
            >
              Long Vibration
            </button>
          </div>
        </div>
      </div>
    </>
  );

  const renderDialSection = () => (
    <>
      <div className="mt-6 mb-3 px-5 text-sm text-gray-500 font-medium">Phone Call</div>
      <div className="mx-3 mb-4 bg-white rounded-xl shadow-sm border border-gray-200 overflow-hidden">
        <div className="px-4 py-5">
          <div className="text-base text-black font-medium mb-1">Dial Phone Number</div>
          <div className="text-xs text-gray-400 leading-tight mb-4">Enter a phone number to initiate a native dialer call</div>
          <div className="space-y-3">
            <input
              type="tel"
              inputMode="tel"
              value={phoneNumber}
              onChange={(event) => setPhoneNumber(event.target.value)}
              className="w-full px-3 py-2 text-sm border border-gray-300 rounded focus:outline-none focus:ring-2 focus:ring-blue-500"
              placeholder="Enter phone number"
            />
            <button
              onClick={handleDial}
              className="w-full h-9 text-sm font-medium transition-all duration-200 bg-blue-500 hover:bg-blue-600 text-white border-0 shadow-sm rounded"
            >
              Call
            </button>
          </div>
        </div>
      </div>
    </>
  );

  return (
    <div className="min-h-screen bg-gray-100">
      <div className="max-w-md mx-auto pb-10">
        {currentType === 'device' && renderDeviceInfoSection()}
        {currentType === 'screen' && renderScreenInfoSection()}
        {currentType === 'vibrate' && renderVibrationSection()}
        {currentType === 'dial' && renderDialSection()}

        {/* Fallback to device info when type is unrecognized */}
        {![ 'device', 'screen', 'vibrate', 'dial' ].includes(currentType) && renderDeviceInfoSection()}
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
  const display = value === undefined || value === null || value === '' ? '-' : value;
  const text = suffix && display !== '-' ? `${display}${suffix}` : display;
  return (
    <div className="flex justify-between items-center py-2 border-b border-gray-100 last:border-b-0">
      <span className="text-sm text-gray-600">{label}</span>
      <span className="text-sm font-medium text-gray-900">{text}</span>
    </div>
  );
}

function formatNumber(value: number | undefined): string {
  if (typeof value !== 'number' || Number.isNaN(value)) {
    return '-';
  }
  return Number.isInteger(value) ? value.toString() : value.toFixed(2);
}
