import { useLxPage } from '@lingxia/react';
import '../../tailwind.css';

export default function SystemPage() {
  const { data, actions } = useLxPage();
  const { getBaseInfo, getSystemSetting, toggleAutostart, refreshAutostart } = actions;
  const {
    currentType = 'appBaseInfo',
    appBaseInfo = null,
    systemSetting = null,
    autostartSupported = false,
    autostartEnabled = null,
    autostartError = '',
  } = data;

  return (
    <div className="min-h-screen bg-gradient-to-br from-gray-50 to-gray-100" data-testid="system-page" data-mode={currentType}>
      <div className="px-4 py-6">
        {currentType === 'appBaseInfo' && (
          <>
            <div className="mb-6 text-center">
              <h1 className="text-2xl font-light text-gray-800 mb-2">app.getBaseInfo</h1>
              <div className="w-16 h-0.5 bg-gray-400 mx-auto"></div>
            </div>

            <div className="mb-5 bg-white rounded-2xl shadow-sm border border-gray-100 overflow-hidden">
              <div className="flex items-center gap-4 px-5 py-5 border-b border-gray-100">
                <div className="flex items-center justify-center w-12 h-12 rounded-xl bg-gradient-to-br from-blue-50 to-indigo-50">
                  <span className="text-2xl">🧭</span>
                </div>
                <div className="flex-1">
                  <div className="text-sm text-gray-800 font-semibold">Fetch App Base Info</div>
                  <div className="text-xs text-gray-500 mt-0.5">Get app environment info (locale, display language, OS, version)</div>
                </div>
                <button
                  data-testid="system-base-info"
                  onClick={getBaseInfo}
                  className="px-5 py-2.5 text-sm font-medium transition-all duration-200 bg-gradient-to-r from-blue-600 to-blue-500 hover:from-blue-500 hover:to-blue-600 text-white rounded-xl shadow-sm active:scale-[0.98]"
                >
                  Get Info
                </button>
              </div>

              {appBaseInfo && (
                <div className="p-5" data-testid="system-base-result">
                  <div className="rounded-xl border border-gray-200 bg-gradient-to-br from-gray-50 to-white p-4">
                    <div className="flex items-center gap-2 mb-4">
                      <span className="w-1 h-4 bg-blue-500 rounded-full"></span>
                      <h4 className="text-sm font-semibold text-gray-700">Result</h4>
                    </div>
                    <InfoRow label="Locale" value={appBaseInfo.locale} />
                    <InfoRow label="Display Language" value={appBaseInfo.displayLanguage} />
                    <InfoRow label="OS" value={appBaseInfo.os} />
                    <InfoRow label="Product Name" value={appBaseInfo.productName} />
                    <InfoRow label="Product Version" value={appBaseInfo.version} />
                    <InfoRow label="SDK Version" value={appBaseInfo.SDKVersion} />
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
                  data-testid="system-setting-info"
                  onClick={getSystemSetting}
                  className="px-5 py-2.5 text-sm font-medium transition-all duration-200 bg-gradient-to-r from-emerald-600 to-emerald-500 hover:from-emerald-500 hover:to-emerald-600 text-white rounded-xl shadow-sm active:scale-[0.98]"
                >
                  Get Info
                </button>
              </div>

              {systemSetting && (
                <div className="p-5" data-testid="system-setting-result">
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
        {currentType === 'autostart' && (
          <>
            <div className="mb-6 text-center">
              <h1 className="text-2xl font-light text-gray-800 mb-2">app.autostart</h1>
              <div className="w-16 h-0.5 bg-gray-400 mx-auto"></div>
            </div>

            <div className="mb-5 bg-white rounded-2xl shadow-sm border border-gray-100 overflow-hidden">
              <div className="flex items-center gap-4 px-5 py-5 border-b border-gray-100">
                <div className="flex items-center justify-center w-12 h-12 rounded-xl bg-gradient-to-br from-amber-50 to-orange-50">
                  <span className="text-2xl">🚀</span>
                </div>
                <div className="flex-1">
                  <div className="text-sm text-gray-800 font-semibold">Launch at Startup</div>
                  <div className="text-xs text-gray-500 mt-0.5">
                    {autostartSupported
                      ? 'Register this app as a login / startup item'
                      : 'Not available on this platform'}
                  </div>
                </div>
                {autostartSupported && (
                  <button
                    onClick={toggleAutostart}
                    className={`relative inline-flex h-7 w-12 items-center rounded-full transition-colors duration-200 ${
                      autostartEnabled ? 'bg-emerald-500' : 'bg-gray-300'
                    }`}
                  >
                    <span
                      className={`inline-block h-5 w-5 transform rounded-full bg-white shadow transition-transform duration-200 ${
                        autostartEnabled ? 'translate-x-6' : 'translate-x-1'
                      }`}
                    />
                  </button>
                )}
              </div>

              <div className="p-5">
                <div className="rounded-xl border border-gray-200 bg-gradient-to-br from-gray-50 to-white p-4">
                  <div className="flex items-center gap-2 mb-4">
                    <span className="w-1 h-4 bg-amber-500 rounded-full"></span>
                    <h4 className="text-sm font-semibold text-gray-700">State</h4>
                  </div>
                  <InfoRow label="Supported" value={formatBool(autostartSupported)} />
                  <InfoRow
                    label="Enabled (OS)"
                    value={autostartEnabled === null ? '--' : formatBool(autostartEnabled)}
                  />
                  {autostartError && <InfoRow label="Error" value={autostartError} />}
                  <div className="pt-3">
                    <button
                      onClick={refreshAutostart}
                      className="px-4 py-2 text-xs font-medium bg-gray-100 hover:bg-gray-200 text-gray-700 rounded-lg transition-colors"
                    >
                      Re-read OS State
                    </button>
                  </div>
                </div>
              </div>
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
