import React from 'react';
import '../../tailwind.css';

export default function SystemPage() {
  const { data, getAppBaseInfo, getSystemSetting } = useLingXia();
  const { currentType = 'appBaseInfo', appBaseInfo = null, systemSetting = null } = data;

  return (
    <div className="min-h-screen bg-gradient-to-br from-gray-50 to-gray-100">
      <div className="px-4 py-6">
        {currentType === 'appBaseInfo' && (
          <>
            <div className="mb-6 text-center">
              <h1 className="text-2xl font-light text-gray-800 mb-2">getAppBaseInfo</h1>
              <div className="w-16 h-0.5 bg-gray-400 mx-auto"></div>
            </div>

            <div className="mb-5 bg-white rounded-2xl shadow-sm border border-gray-100 overflow-hidden">
              <div className="flex items-center gap-4 px-5 py-5 border-b border-gray-100">
                <div className="flex items-center justify-center w-12 h-12 rounded-xl bg-gradient-to-br from-blue-50 to-indigo-50">
                  <span className="text-2xl">🧭</span>
                </div>
                <div className="flex-1">
                  <div className="text-sm text-gray-800 font-semibold">Fetch App Base Info</div>
                  <div className="text-xs text-gray-500 mt-0.5">Get application language settings</div>
                </div>
                <button
                  onClick={getAppBaseInfo}
                  className="px-5 py-2.5 text-sm font-medium transition-all duration-200 bg-gradient-to-r from-blue-600 to-blue-500 hover:from-blue-500 hover:to-blue-600 text-white rounded-xl shadow-sm active:scale-[0.98]"
                >
                  Get Info
                </button>
              </div>

              {appBaseInfo && (
                <div className="p-5">
                  <div className="rounded-xl border border-gray-200 bg-gradient-to-br from-gray-50 to-white p-4">
                    <div className="flex items-center gap-2 mb-4">
                      <span className="w-1 h-4 bg-blue-500 rounded-full"></span>
                      <h4 className="text-sm font-semibold text-gray-700">Result</h4>
                    </div>
                    <InfoRow label="Language" value={appBaseInfo.language} />
                  </div>
                </div>
              )}
            </div>
          </>
        )}
        {currentType === 'systemSetting' && (
          <>
            <div className="mb-6 text-center">
              <h1 className="text-2xl font-light text-gray-800 mb-2">getSystemSetting</h1>
              <div className="w-16 h-0.5 bg-gray-400 mx-auto"></div>
            </div>

            <div className="mb-5 bg-white rounded-2xl shadow-sm border border-gray-100 overflow-hidden">
              <div className="flex items-center gap-4 px-5 py-5 border-b border-gray-100">
                <div className="flex items-center justify-center w-12 h-12 rounded-xl bg-gradient-to-br from-emerald-50 to-teal-50">
                  <span className="text-2xl">⚙️</span>
                </div>
                <div className="flex-1">
                  <div className="text-sm text-gray-800 font-semibold">Fetch System Setting</div>
                  <div className="text-xs text-gray-500 mt-0.5">WiFi, location, and Bluetooth toggles</div>
                </div>
                <button
                  onClick={getSystemSetting}
                  className="px-5 py-2.5 text-sm font-medium transition-all duration-200 bg-gradient-to-r from-emerald-600 to-emerald-500 hover:from-emerald-500 hover:to-emerald-600 text-white rounded-xl shadow-sm active:scale-[0.98]"
                >
                  Get Info
                </button>
              </div>

              {systemSetting && (
                <div className="p-5">
                  <div className="rounded-xl border border-gray-200 bg-gradient-to-br from-gray-50 to-white p-4">
                    <div className="flex items-center gap-2 mb-4">
                      <span className="w-1 h-4 bg-emerald-500 rounded-full"></span>
                      <h4 className="text-sm font-semibold text-gray-700">Result</h4>
                    </div>
                    <InfoRow label="WiFi Enabled" value={formatBool(systemSetting.wifiEnabled)} />
                    <InfoRow label="Location Enabled" value={formatBool(systemSetting.locationEnabled)} />
                    <InfoRow label="Bluetooth Enabled" value={formatBool(systemSetting.bluetoothEnabled)} />
                  </div>
                </div>
              )}
            </div>
          </>
        )}
      </div>
    </div>
  );
}

interface InfoRowProps {
  label: string;
  value?: string;
}

function InfoRow({ label, value }: InfoRowProps) {
  const display = value || '--';
  return (
    <div className="flex justify-between items-center py-3 border-b border-gray-200 last:border-b-0">
      <span className="text-sm text-gray-600">{label}</span>
      <span className="text-sm font-semibold text-gray-800 px-3 py-1 bg-blue-50 rounded-lg">{display}</span>
    </div>
  );
}

function formatBool(value: boolean | undefined): string {
  if (value === undefined || value === null) {
    return '--';
  }
  return value ? 'Yes' : 'No';
}
