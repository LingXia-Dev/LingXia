import React from 'react';
import { useLingXia } from '@lingxia/core/react';
import '../../tailwind.css';

export default function DevicePage() {
  const {
    data,
    getDeviceInfo,
    getScreenInfo,
    vibrateShort,
    vibrateLong,
    makePhoneCall,
    getNetworkInfo,
    startNetworkChangeListen,
    stopNetworkChangeListen,
    setOrientationPortrait,
    setOrientationLandscape,
    startDeviceOrientationListen,
    stopDeviceOrientationListen,
    clearOrientationEvents,
  } = useLingXia();

  const {
    currentType = 'device',
    deviceInfo = null,
    screenInfo = null,
    networkInfo = null,
    networkChange = null,
    networkListening = false,
    orientationListening = false,
    deviceOrientationValue = '--',
    orientationEvents = [],
    orientationLock = '--',
  } = data;
  const orientationEventLines = Array.isArray(orientationEvents) ? orientationEvents : [];
  const [phoneNumber, setPhoneNumber] = React.useState('');

  React.useEffect(() => {
    setPhoneNumber('');
  }, [currentType]);

  const handleDial = React.useCallback(async () => {
    const trimmed = phoneNumber.trim();
    if (!trimmed) {
      return;
    }

    try {
      await makePhoneCall({ phoneNumber: trimmed });
    } catch (error) {
      console.error('makePhoneCall failed:', error);
    }
  }, [phoneNumber, makePhoneCall]);

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
            <div className="text-xs text-gray-500 mt-0.5">Brand, model, and OS version</div>
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
                <InfoRow label="OS Name" value={deviceInfo.osName} />
                <InfoRow label="OS Version" value={deviceInfo.osVersion} />
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

  const renderNetworkTypeSection = () => (
    <>
      <div className="mb-6 text-center">
        <h1 className="text-2xl font-light text-gray-800 mb-2">Network Type</h1>
        <div className="w-16 h-0.5 bg-gray-400 mx-auto"></div>
      </div>

      <div className="mb-5 bg-white rounded-2xl shadow-sm border border-gray-100 overflow-hidden">
        <div className="flex items-center gap-4 px-5 py-5 border-b border-gray-100">
          <div className="flex items-center justify-center w-12 h-12 rounded-xl bg-gradient-to-br from-cyan-50 to-sky-50">
            <span className="text-2xl">🌐</span>
          </div>
          <div className="flex-1">
            <div className="text-sm text-gray-800 font-semibold">Get Network Type</div>
            <div className="text-xs text-gray-500 mt-0.5">none / unknown / wifi / 2g / 3g / 4g / 5g / ethernet</div>
          </div>
          <button
            onClick={getNetworkInfo}
            className="px-5 py-2.5 text-sm font-medium transition-all duration-200 bg-gradient-to-r from-blue-600 to-blue-500 hover:from-blue-500 hover:to-blue-600 text-white rounded-xl shadow-sm active:scale-[0.98]"
          >
            Get Info
          </button>
        </div>

        <div className="p-5">
          <div className="rounded-xl border border-gray-200 bg-gradient-to-br from-gray-50 to-white p-4">
            <InfoRow label="Connected" value={networkInfo?.isConnected === undefined ? '--' : (networkInfo.isConnected ? 'Yes' : 'No')} />
            <InfoRow label="Network Type" value={networkInfo?.networkType || '--'} />
          </div>
        </div>
      </div>
    </>
  );

  const renderLocalIPSection = () => (
    <>
      <div className="mb-6 text-center">
        <h1 className="text-2xl font-light text-gray-800 mb-2">Local IP Addresses</h1>
        <div className="w-16 h-0.5 bg-gray-400 mx-auto"></div>
      </div>

      <div className="mb-5 bg-white rounded-2xl shadow-sm border border-gray-100 overflow-hidden">
        <div className="flex items-center gap-4 px-5 py-5 border-b border-gray-100">
          <div className="flex items-center justify-center w-12 h-12 rounded-xl bg-gradient-to-br from-teal-50 to-emerald-50">
            <span className="text-2xl">📡</span>
          </div>
          <div className="flex-1">
            <div className="text-sm text-gray-800 font-semibold">Get Local IPs (IPv4 + IPv6)</div>
            <div className="text-xs text-gray-500 mt-0.5">Current active network addresses</div>
          </div>
          <button
            onClick={getNetworkInfo}
            className="px-5 py-2.5 text-sm font-medium transition-all duration-200 bg-gradient-to-r from-teal-600 to-teal-500 hover:from-teal-500 hover:to-teal-600 text-white rounded-xl shadow-sm active:scale-[0.98]"
          >
            Get IP
          </button>
        </div>

        <div className="p-5">
          <div className="rounded-xl border border-gray-200 bg-gradient-to-br from-gray-50 to-white p-4">
            <InfoRow label="IPv4" value={networkInfo?.ipv4?.length ? networkInfo.ipv4 : '--'} />
            <InfoRow label="IPv6" value={networkInfo?.ipv6?.length ? networkInfo.ipv6 : '--'} />
          </div>
        </div>
      </div>
    </>
  );

  const renderNetworkStatusSection = () => (
    <>
      <div className="mb-6 text-center">
        <h1 className="text-2xl font-light text-gray-800 mb-2">Network Status Listener</h1>
        <div className="w-16 h-0.5 bg-gray-400 mx-auto"></div>
      </div>

      <div className="mb-5 bg-white rounded-2xl shadow-sm border border-gray-100 overflow-hidden">
        <div className="p-6 space-y-4">
          <div className="grid grid-cols-2 gap-3">
            <button
              onClick={startNetworkChangeListen}
              className="py-3 text-sm font-medium transition-all duration-200 bg-gradient-to-r from-green-600 to-green-500 hover:from-green-500 hover:to-green-600 text-white rounded-xl shadow-sm active:scale-[0.98]"
            >
              Start Listen
            </button>
            <button
              onClick={stopNetworkChangeListen}
              className="py-3 text-sm font-medium transition-all duration-200 bg-gradient-to-r from-gray-600 to-gray-500 hover:from-gray-500 hover:to-gray-600 text-white rounded-xl shadow-sm active:scale-[0.98]"
            >
              Stop Listen
            </button>
          </div>

          <div className="rounded-xl border border-gray-200 bg-gradient-to-br from-gray-50 to-white p-4">
            <InfoRow label="Listening" value={networkListening ? 'Yes' : 'No'} />
            <InfoRow label="Connected" value={networkChange?.isConnected === undefined ? '--' : (networkChange.isConnected ? 'Yes' : 'No')} />
            <InfoRow label="Network Type" value={networkChange?.networkType || '--'} />
            <InfoRow label="IPv4" value={networkChange?.ipv4?.length ? networkChange.ipv4 : '--'} />
            <InfoRow label="IPv6" value={networkChange?.ipv6?.length ? networkChange.ipv6 : '--'} />
          </div>
        </div>
      </div>
    </>
  );

  const renderOrientationSection = () => (
    <>
      <div className="mb-6 text-center">
        <h1 className="text-2xl font-light text-gray-800 mb-2">Device Orientation</h1>
        <div className="w-16 h-0.5 bg-gray-400 mx-auto"></div>
      </div>

      <div className="mb-5 bg-white rounded-2xl shadow-sm border border-gray-100 overflow-hidden">
        <div className="p-6 space-y-4">
          <div className="flex items-center gap-3">
            <div className="flex items-center justify-center w-10 h-10 rounded-xl bg-gradient-to-br from-violet-50 to-indigo-50">
              <span className="text-xl">🧭</span>
            </div>
            <div>
              <div className="text-sm text-gray-800 font-semibold">setDeviceOrientation / onDeviceOrientationChange</div>
              <div className="text-xs text-gray-500 mt-0.5">Lock orientation and listen device orientation changes</div>
            </div>
          </div>

          <div className="grid grid-cols-2 gap-3">
            <button
              onClick={setOrientationPortrait}
              className="py-3 text-sm font-medium transition-all duration-200 bg-gradient-to-r from-violet-600 to-violet-500 hover:from-violet-500 hover:to-violet-600 text-white rounded-xl shadow-sm active:scale-[0.98]"
            >
              Lock Portrait
            </button>
            <button
              onClick={setOrientationLandscape}
              className="py-3 text-sm font-medium transition-all duration-200 bg-gradient-to-r from-indigo-600 to-indigo-500 hover:from-indigo-500 hover:to-indigo-600 text-white rounded-xl shadow-sm active:scale-[0.98]"
            >
              Lock Landscape
            </button>
          </div>

          <div className="grid grid-cols-2 gap-3">
            <button
              onClick={startDeviceOrientationListen}
              className="py-3 text-sm font-medium transition-all duration-200 bg-gradient-to-r from-green-600 to-green-500 hover:from-green-500 hover:to-green-600 text-white rounded-xl shadow-sm active:scale-[0.98]"
            >
              Start Listen
            </button>
            <button
              onClick={stopDeviceOrientationListen}
              className="py-3 text-sm font-medium transition-all duration-200 bg-gradient-to-r from-gray-600 to-gray-500 hover:from-gray-500 hover:to-gray-600 text-white rounded-xl shadow-sm active:scale-[0.98]"
            >
              Stop Listen
            </button>
          </div>

          <div className="rounded-xl border border-gray-200 bg-gradient-to-br from-gray-50 to-white p-4">
            <InfoRow label="Listening" value={orientationListening ? 'Yes' : 'No'} />
            <InfoRow label="Lock Target" value={orientationLock || '--'} />
            <InfoRow label="Current Value" value={deviceOrientationValue || '--'} />
          </div>

          <div className="rounded-xl border border-gray-200 bg-gradient-to-br from-gray-50 to-white p-4">
            <div className="flex items-center justify-between mb-3">
              <h4 className="text-sm font-semibold text-gray-700">Orientation Events</h4>
              <button
                onClick={clearOrientationEvents}
                className="px-3 py-1.5 text-xs font-medium transition-all duration-200 bg-gradient-to-r from-gray-600 to-gray-500 hover:from-gray-500 hover:to-gray-600 text-white rounded-lg shadow-sm active:scale-[0.98]"
              >
                Clear Logs
              </button>
            </div>
            <div className="text-xs text-gray-700 bg-white border border-gray-200 rounded-lg p-3 max-h-56 overflow-auto whitespace-pre-wrap break-all">
              {orientationEventLines.length ? orientationEventLines.join('\n') : '--'}
            </div>
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
        {currentType === 'networkType' && renderNetworkTypeSection()}
        {currentType === 'localIP' && renderLocalIPSection()}
        {currentType === 'networkStatus' && renderNetworkStatusSection()}
        {currentType === 'orientation' && renderOrientationSection()}

        {!['device', 'screen', 'vibrate', 'dial', 'networkType', 'localIP', 'networkStatus', 'orientation'].includes(currentType) && renderDeviceInfoSection()}
      </div>
    </div>
  );
}

interface InfoRowProps {
  label: string;
  value?: string | number | string[];
  suffix?: string;
}

function InfoRow({ label, value, suffix }: InfoRowProps) {
  const display = value === undefined || value === null || value === '' ? '--' : value;
  const text = suffix && display !== '--' ? `${display}${suffix}` : display;
  const textContent = Array.isArray(text) ? text.join('\n') : String(text);
  return (
    <div className="flex justify-between items-start gap-3 py-3 border-b border-gray-200 last:border-b-0">
      <span className="text-sm text-gray-600 shrink-0">{label}</span>
      <span className="text-sm font-semibold text-gray-800 px-3 py-1 bg-blue-50 rounded-lg text-right max-w-[72%] whitespace-pre-wrap break-all">
        {textContent}
      </span>
    </div>
  );
}

function formatNumber(value: number | undefined): string {
  if (typeof value !== 'number' || Number.isNaN(value)) {
    return '--';
  }
  return Number.isInteger(value) ? value.toString() : value.toFixed(2);
}
