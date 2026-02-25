import React from 'react';
import { useLingXia } from '@lingxia/web-runtime/react';
import '../../tailwind.css';

type WifiInfo = {
  SSID?: string;
  ssid?: string;
  BSSID?: string;
  bssid?: string;
  secure?: boolean;
  signalStrength?: number;
  frequency?: number;
  connected?: boolean;
  state?: string;
};

type PageData = {
  wifiList?: WifiInfo[] | null;
  connectedWifi?: WifiInfo | null;
  wifiSystemEnabled?: boolean;
  wifiModuleEnabled?: boolean;
  wifiListenerEnabled?: boolean;
  wifiConnectedEvents?: Array<{
    id: string;
    time: string;
    ssid: string;
    bssid?: string;
    secure?: boolean;
    signalStrength?: number;
    frequency?: number;
    connected?: boolean;
    state?: string;
  }>;
};

type PageActions = {
  data: PageData;
  startWifi(): void | Promise<unknown>;
  stopWifi(): void | Promise<unknown>;
  getWifiList(): void | Promise<unknown>;
  getConnectedWifi(): void | Promise<unknown>;
  connectWifi(options: { SSID: string; password?: string }): void | Promise<unknown>;
  onWifiConnected(): void | Promise<unknown>;
  offWifiConnected(): void | Promise<unknown>;
  clearWifiConnectedEvents(): void | Promise<unknown>;
};

export default function WifiPage() {
  const {
    data,
    startWifi,
    stopWifi,
    getWifiList,
    getConnectedWifi,
    connectWifi,
    onWifiConnected,
    offWifiConnected,
    clearWifiConnectedEvents,
  } = useLingXia();

  const {
    wifiList = null,
    connectedWifi = null,
    wifiModuleEnabled = false,
    wifiListenerEnabled = false,
    wifiConnectedEvents = [],
  } = data;

  const [wifiSsid, setWifiSsid] = React.useState('');
  const [wifiPassword, setWifiPassword] = React.useState('');

  // Note: getSystemSetting is called from Logic layer (index.js) in onLoad/onShow
  // and updates data.wifiSystemEnabled via setData. No View layer call needed.

  // Auto turn off listener when WiFi module is disabled
  React.useEffect(() => {
    if (!wifiModuleEnabled && wifiListenerEnabled) {
      if (typeof offWifiConnected === 'function') {
        try {
          const result = offWifiConnected();
          if (result && typeof (result as Promise<unknown>).then === 'function') {
            (result as Promise<unknown>).catch((error: unknown) => {
              console.error('offWifiConnected failed:', error);
            });
          }
        } catch (error) {
          console.error('offWifiConnected failed:', error);
        }
      }
    }
  }, [offWifiConnected, wifiListenerEnabled, wifiModuleEnabled]);

  const handleConnectWifi = React.useCallback(async () => {
    const ssid = wifiSsid.trim();
    const password = wifiPassword.trim();

    if (!ssid) {
      window.alert?.('Please enter SSID');
      return;
    }

    try {
      await connectWifi({
        SSID: ssid,
        password: password || undefined,
      });
      console.log('WiFi connection requested:', ssid);
    } catch (error) {
      console.error('connectWifi failed:', error);
    }
  }, [connectWifi, wifiPassword, wifiSsid]);

  const handleStartWifiConnected = React.useCallback(() => {
    if (wifiListenerEnabled || typeof onWifiConnected !== 'function') {
      return;
    }

    try {
      const result = onWifiConnected();
      if (result && typeof (result as Promise<unknown>).then === 'function') {
        (result as Promise<unknown>).catch((error: unknown) => {
          console.error('onWifiConnected failed:', error);
        });
      }
    } catch (error) {
      console.error('onWifiConnected failed:', error);
    }
  }, [onWifiConnected, wifiListenerEnabled]);

  const handleStopWifiConnected = React.useCallback(() => {
    if (!wifiListenerEnabled || typeof offWifiConnected !== 'function') {
      return;
    }

    try {
      const result = offWifiConnected();
      if (result && typeof (result as Promise<unknown>).then === 'function') {
        (result as Promise<unknown>).catch((error: unknown) => {
          console.error('offWifiConnected failed:', error);
        });
      }
    } catch (error) {
      console.error('offWifiConnected failed:', error);
    }
  }, [offWifiConnected, wifiListenerEnabled]);

  const handleClearWifiEvents = React.useCallback(() => {
    if (typeof clearWifiConnectedEvents !== 'function') {
      return;
    }
    try {
      const result = clearWifiConnectedEvents();
      if (result && typeof (result as Promise<unknown>).then === 'function') {
        (result as Promise<unknown>).catch((error: unknown) => {
          console.error('clearWifiConnectedEvents failed:', error);
        });
      }
    } catch (error) {
      console.error('clearWifiConnectedEvents failed:', error);
    }
  }, [clearWifiConnectedEvents]);

  return (
    <div className="min-h-screen bg-gradient-to-br from-gray-50 to-gray-100">
      <div className="px-4 py-6">
        <div className="mb-6 text-center">
          <h1 className="text-2xl font-light text-gray-800 mb-2">WiFi Management</h1>
          <div className="w-16 h-0.5 bg-gray-400 mx-auto"></div>
        </div>

        {/* WiFi Module Control */}
        <div className="mb-5 bg-white rounded-2xl shadow-sm border border-gray-100 overflow-hidden">
          <div className="p-6">
            <div className="flex items-center gap-3 mb-4">
              <div className="flex items-center justify-center w-10 h-10 rounded-xl bg-gradient-to-br from-emerald-50 to-green-50">
                <span className="text-xl">🧩</span>
              </div>
              <div>
                <div className="text-sm text-gray-800 font-semibold">WiFi Module</div>
                <div className="text-xs text-gray-500 mt-0.5">Initialize or stop WiFi module</div>
              </div>
            </div>
            <div className="grid grid-cols-2 gap-3">
              <button
                onClick={startWifi}
                disabled={wifiModuleEnabled}
                className={`py-3 text-sm font-medium transition-all duration-200 rounded-xl shadow-sm active:scale-[0.98] ${wifiModuleEnabled
                  ? 'bg-gray-200 text-gray-400 cursor-not-allowed'
                  : 'bg-gradient-to-r from-green-600 to-green-500 hover:from-green-500 hover:to-green-600 text-white'
                  }`}
              >
                Start WiFi
              </button>
              <button
                onClick={stopWifi}
                disabled={!wifiModuleEnabled}
                className={`py-3 text-sm font-medium transition-all duration-200 rounded-xl shadow-sm active:scale-[0.98] ${!wifiModuleEnabled
                  ? 'bg-gray-200 text-gray-400 cursor-not-allowed'
                  : 'bg-gradient-to-r from-red-600 to-red-500 hover:from-red-500 hover:to-red-600 text-white'
                  }`}
              >
                Stop WiFi
              </button>
            </div>
          </div>
        </div>

        {/* Get Connected WiFi */}
        {wifiModuleEnabled && (
          <div className="mb-5 bg-white rounded-2xl shadow-sm border border-gray-100 overflow-hidden">
            <div className="flex items-center gap-4 px-5 py-5 border-b border-gray-100">
              <div className="flex items-center justify-center w-12 h-12 rounded-xl bg-gradient-to-br from-blue-50 to-indigo-50">
                <span className="text-2xl">📶</span>
              </div>
              <div className="flex-1">
                <div className="text-sm text-gray-800 font-semibold">Connected WiFi</div>
                <div className="text-xs text-gray-500 mt-0.5">Get current WiFi connection info</div>
              </div>
              <button
                onClick={getConnectedWifi}
                className="px-5 py-2.5 text-sm font-medium transition-all duration-200 bg-gradient-to-r from-blue-600 to-blue-500 hover:from-blue-500 hover:to-blue-600 text-white rounded-xl shadow-sm active:scale-[0.98]"
              >
                Get Info
              </button>
            </div>

            {connectedWifi && (
              <div className="p-5">
                <div className="rounded-xl border border-gray-200 bg-gradient-to-br from-gray-50 to-white p-4">
                  <div className="flex items-center gap-2 mb-4">
                    <span className="w-1 h-4 bg-blue-500 rounded-full"></span>
                    <h4 className="text-sm font-semibold text-gray-700">Connected Network</h4>
                  </div>
                  <div className="space-y-0">
                    <InfoRow label="SSID" value={connectedWifi.SSID ?? connectedWifi.ssid} />
                    <InfoRow label="BSSID" value={connectedWifi.BSSID ?? connectedWifi.bssid} />
                    <InfoRow label="Secure" value={connectedWifi.secure ? 'Yes' : 'No'} />
                    <InfoRow label="Signal" value={connectedWifi.signalStrength} suffix="%" />
                    <InfoRow label="Frequency" value={connectedWifi.frequency} suffix=" MHz" />
                  </div>
                </div>
              </div>
            )}
          </div>
        )}

        {/* WiFi Connected Events */}
        {wifiModuleEnabled && (
          <div className="mb-5 bg-white rounded-2xl shadow-sm border border-gray-100 overflow-hidden">
            <div className="flex items-center gap-4 px-5 py-5 border-b border-gray-100">
              <div className="flex items-center justify-center w-12 h-12 rounded-xl bg-gradient-to-br from-amber-50 to-orange-50">
                <span className="text-2xl">🔔</span>
              </div>
              <div className="flex-1">
                <div className="text-sm text-gray-800 font-semibold">WiFi Connected Events</div>
                <div className="text-xs text-gray-500 mt-0.5">Listen to WiFi connection changes</div>
              </div>
              <div className="flex items-center gap-2">
                <button
                  onClick={handleStartWifiConnected}
                  disabled={wifiListenerEnabled}
                  className={`px-4 py-2 text-xs font-medium transition-all duration-200 rounded-lg shadow-sm active:scale-[0.98] ${wifiListenerEnabled
                    ? 'bg-gray-200 text-gray-400 cursor-not-allowed'
                    : 'bg-gradient-to-r from-amber-500 to-orange-500 hover:from-amber-400 hover:to-orange-500 text-white'
                    }`}
                >
                  On
                </button>
                <button
                  onClick={handleStopWifiConnected}
                  disabled={!wifiListenerEnabled}
                  className={`px-4 py-2 text-xs font-medium transition-all duration-200 rounded-lg shadow-sm active:scale-[0.98] ${!wifiListenerEnabled
                    ? 'bg-gray-200 text-gray-400 cursor-not-allowed'
                    : 'bg-gradient-to-r from-gray-600 to-gray-500 hover:from-gray-500 hover:to-gray-600 text-white'
                    }`}
                >
                  Off
                </button>
              </div>
            </div>
            <div className="p-5">
              <div className="flex items-center justify-between text-xs text-gray-500 mb-3">
                <span>Listening: {wifiListenerEnabled ? 'On' : 'Off'}</span>
                {wifiConnectedEvents.length > 0 && (
                  <button
                    onClick={handleClearWifiEvents}
                    className="text-xs text-gray-500 hover:text-gray-700 underline"
                  >
                    Clear
                  </button>
                )}
              </div>
              {wifiConnectedEvents.length === 0 ? (
                <div className="text-sm text-gray-500">No events yet.</div>
              ) : (
                <div className="space-y-3">
                  {wifiConnectedEvents.map((event) => (
                    <div
                      key={event.id}
                      className="rounded-lg border border-gray-200 bg-white p-3 text-xs text-gray-600"
                    >
                      <div className="flex items-center justify-between mb-2">
                        <span className="text-gray-500">{event.time}</span>
                        <span className="text-gray-500">
                          {typeof event.signalStrength === 'number' ? `${event.signalStrength}%` : '--'}
                        </span>
                      </div>
                      <div className="text-sm font-semibold text-gray-800">
                        {event.ssid || '--'}
                      </div>
                      <div className="mt-1 text-[11px] text-gray-500 space-y-0.5">
                        {event.bssid && <div>BSSID: {event.bssid}</div>}
                        {typeof event.frequency === 'number' && (
                          <div>Frequency: {event.frequency} MHz</div>
                        )}
                        <div>
                          State:{' '}
                          {event.state ??
                            (event.connected === undefined
                              ? '--'
                              : event.connected
                                ? 'Connected'
                                : 'Disconnected')}
                        </div>
                        <div>
                          Secure:{' '}
                          {event.secure === undefined ? '--' : event.secure ? 'Yes' : 'No'}
                        </div>
                      </div>
                    </div>
                  ))}
                </div>
              )}
            </div>
          </div>
        )}

        {/* Get WiFi List */}
        {wifiModuleEnabled && (
          <div className="mb-5 bg-white rounded-2xl shadow-sm border border-gray-100 overflow-hidden">
            <div className="flex items-center gap-4 px-5 py-5 border-b border-gray-100">
              <div className="flex items-center justify-center w-12 h-12 rounded-xl bg-gradient-to-br from-purple-50 to-pink-50">
                <span className="text-2xl">📋</span>
              </div>
              <div className="flex-1">
                <div className="text-sm text-gray-800 font-semibold">Scan WiFi Networks</div>
                <div className="text-xs text-gray-500 mt-0.5">Get list of available networks</div>
              </div>
              <button
                onClick={getWifiList}
                className="px-5 py-2.5 text-sm font-medium transition-all duration-200 bg-gradient-to-r from-purple-600 to-purple-500 hover:from-purple-500 hover:to-purple-600 text-white rounded-xl shadow-sm active:scale-[0.98]"
              >
                Scan
              </button>
            </div>

            {wifiList && Array.isArray(wifiList) && wifiList.length > 0 && (
              <div className="p-5">
                <div className="rounded-xl border border-gray-200 bg-gradient-to-br from-gray-50 to-white p-4">
                  <div className="flex items-center gap-2 mb-4">
                    <span className="w-1 h-4 bg-purple-500 rounded-full"></span>
                    <h4 className="text-sm font-semibold text-gray-700">Available Networks ({wifiList.length})</h4>
                  </div>
                  <div className="space-y-3 max-h-96 overflow-y-auto">
                    {wifiList.map((wifi: WifiInfo, index: number) => (
                      <div
                        key={index}
                        onClick={() => {
                          setWifiSsid((wifi.SSID ?? wifi.ssid ?? '').toString());
                        }}
                        className="p-3 bg-white rounded-lg border border-gray-200 cursor-pointer hover:border-emerald-200 hover:bg-emerald-50/30 transition-colors"
                      >
                        <div className="flex items-center justify-between mb-2">
                          <span className="text-sm font-semibold text-gray-800">
                            {wifi.SSID ?? wifi.ssid}
                          </span>
                          <span className="text-xs px-2 py-1 rounded-full bg-blue-50 text-blue-600">
                            {typeof wifi.signalStrength === 'number' ? `${wifi.signalStrength}%` : '--'}
                          </span>
                        </div>
                        <div className="text-xs text-gray-500 space-y-1">
                          {(wifi.BSSID ?? wifi.bssid) && (
                            <div>BSSID: {wifi.BSSID ?? wifi.bssid}</div>
                          )}
                          {typeof wifi.frequency === 'number' && (
                            <div>Frequency: {wifi.frequency} MHz</div>
                          )}
                          <div>Security: {wifi.secure ? '🔒 Secured' : '🔓 Open'}</div>
                        </div>
                      </div>
                    ))}
                  </div>
                </div>
              </div>
            )}
          </div>
        )}

        {/* Connect WiFi */}
        {wifiModuleEnabled && (
          <div className="mb-5 bg-white rounded-2xl shadow-sm border border-gray-100 overflow-hidden">
            <div className="p-6">
              <div className="flex items-center gap-3 mb-4">
                <div className="flex items-center justify-center w-10 h-10 rounded-xl bg-gradient-to-br from-emerald-50 to-lime-50">
                  <span className="text-xl">🔗</span>
                </div>
                <div>
                  <div className="text-sm text-gray-800 font-semibold">Connect to WiFi</div>
                  <div className="text-xs text-gray-500 mt-0.5">
                    {wifiList && Array.isArray(wifiList) && wifiList.length > 0
                      ? 'Click a network below or enter SSID manually'
                      : 'Provide SSID and password'}
                  </div>
                </div>
              </div>
              <div className="space-y-3">
                <input
                  value={wifiSsid}
                  onChange={(event) => setWifiSsid(event.target.value)}
                  className="w-full px-4 py-3 text-sm border border-gray-200 rounded-xl bg-white focus:outline-none focus:ring-2 focus:ring-emerald-500 focus:border-transparent transition-all"
                  placeholder={
                    wifiList && Array.isArray(wifiList) && wifiList.length > 0
                      ? 'Enter SSID manually or click a network below'
                      : 'SSID'
                  }
                />
                <input
                  type="password"
                  value={wifiPassword}
                  onChange={(event) => setWifiPassword(event.target.value)}
                  className="w-full px-4 py-3 text-sm border border-gray-200 rounded-xl bg-white focus:outline-none focus:ring-2 focus:ring-emerald-500 focus:border-transparent transition-all"
                  placeholder="Password (optional)"
                />
                <button
                  onClick={handleConnectWifi}
                  className="w-full py-3 text-sm font-medium transition-all duration-200 bg-gradient-to-r from-emerald-600 to-emerald-500 hover:from-emerald-500 hover:to-emerald-600 text-white rounded-xl shadow-sm active:scale-[0.98]"
                >
                  Connect
                </button>
              </div>
            </div>
          </div>
        )}
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
