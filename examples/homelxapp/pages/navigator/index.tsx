import React from 'react';
import { LxNavigator } from '@lingxia/react';
import '../../tailwind.css';

export default function NavigatorPage() {
  const [logs, setLogs] = React.useState<string[]>([]);

  const addLog = (message: string) => {
    setLogs(prev => [`[${new Date().toLocaleTimeString()}] ${message}`, ...prev].slice(0, 10));
  };

  const getErrMsg = (event: any): string => {
    return event?.detail?.errMsg || 'Unknown error';
  };

  const onFailWithMessage = (label: string) => (event: any) => {
    addLog(`✗ ${label}: ${getErrMsg(event)}`);
  };

  return (
    <div className="min-h-screen bg-gray-50">
      <div className="px-4 py-5 space-y-4">
        {/* Header */}
        <div className="bg-gradient-to-br from-blue-500 via-blue-600 to-cyan-600 rounded-2xl px-5 py-6 shadow-lg">
          <div className="flex items-center gap-3 mb-2">
            <div className="w-10 h-10 bg-white/20 backdrop-blur-sm rounded-xl flex items-center justify-center">
              <svg viewBox="0 0 24 24" fill="none" stroke="white" strokeWidth="2.5" className="w-6 h-6">
                <path d="M13 2L3 14h9l-1 8 10-12h-9l1-8z" />
              </svg>
            </div>
            <div>
              <div className="text-xl text-white font-bold">LxNavigator</div>
              <div className="text-sm text-white/80">Declarative navigation component</div>
            </div>
          </div>
          <div className="text-xs text-white/70 mt-3 leading-relaxed">
            Navigate between pages, open external apps, and handle browser URLs with a simple declarative API
          </div>
        </div>

        {/* In-App Navigation */}
        <div className="space-y-3">
          <div className="flex items-center gap-2 px-1">
            <div className="w-1 h-4 bg-blue-500 rounded-full" />
            <h2 className="text-base font-semibold text-gray-900">In-App Navigation</h2>
          </div>

          <div className="bg-white rounded-xl shadow-sm border border-gray-100 overflow-hidden">
            <div className="p-4">
              <div className="text-xs text-gray-500 mb-3 font-medium uppercase tracking-wider">Methods</div>
              <div className="grid grid-cols-2 gap-3">
                {/* Navigate */}
                <LxNavigator
                  url="pages/device/index?type=device"
                  openType="navigate"
                  onSuccess={() => addLog('✓ Navigate to home')}
                >
                  <div className="flex flex-col items-center justify-center py-4 px-2 bg-blue-50 hover:bg-blue-100 active:bg-blue-200 text-blue-700 rounded-xl transition-colors h-full">
                    <span className="text-lg mb-1">➡️</span>
                    <span className="text-sm font-medium">Navigate</span>
                    <span className="text-[10px] opacity-70">Push to stack</span>
                  </div>
                </LxNavigator>

                {/* Redirect */}
                <LxNavigator
                  url="pages/device/index?type=device"
                  openType="redirect"
                  onSuccess={() => addLog('✓ Redirect to home')}
                >
                  <div className="flex flex-col items-center justify-center py-4 px-2 bg-purple-50 hover:bg-purple-100 active:bg-purple-200 text-purple-700 rounded-xl transition-colors h-full">
                    <span className="text-lg mb-1">🔀</span>
                    <span className="text-sm font-medium">Redirect</span>
                    <span className="text-[10px] opacity-70">Replace current</span>
                  </div>
                </LxNavigator>

                {/* Navigate Back */}
                <LxNavigator
                  openType="navigateBack"
                  delta={1}
                  onSuccess={() => addLog('✓ Back 1 page')}
                >
                  <div className="flex flex-col items-center justify-center py-4 px-2 bg-gray-100 hover:bg-gray-200 active:bg-gray-300 text-gray-700 rounded-xl transition-colors h-full">
                    <span className="text-lg mb-1">⬅️</span>
                    <span className="text-sm font-medium">Back</span>
                    <span className="text-[10px] opacity-70">Pop from stack</span>
                  </div>
                </LxNavigator>

                {/* ReLaunch */}
                <LxNavigator
                  url="pages/device/index?type=screen"
                  openType="reLaunch"
                  onSuccess={() => addLog('✓ ReLaunch to home')}
                >
                  <div className="flex flex-col items-center justify-center py-4 px-2 bg-orange-50 hover:bg-orange-100 active:bg-orange-200 text-orange-700 rounded-xl transition-colors h-full">
                    <span className="text-lg mb-1">🚀</span>
                    <span className="text-sm font-medium">ReLaunch</span>
                    <span className="text-[10px] opacity-70">Reset all</span>
                  </div>
                </LxNavigator>
              </div>
            </div>

            {/* Switch Tab */}
            <div className="p-4 border-t border-gray-100">
              <div className="flex items-start justify-between mb-3">
                <div>
                  <div className="text-sm font-medium text-gray-900">Switch Tab</div>
                  <div className="text-xs text-gray-500 mt-0.5">Navigate to tab bar page</div>
                </div>
              </div>
              <div className="grid grid-cols-3 gap-2">
                <LxNavigator
                  url="pages/home/index"
                  openType="switchTab"
                  onSuccess={() => addLog('✓ Switch to Home tab')}
                >
                  <div className="py-2 px-3 bg-blue-50 hover:bg-blue-100 text-blue-600 rounded-lg text-xs font-medium text-center transition-colors">
                    🏠 Home
                  </div>
                </LxNavigator>
                <LxNavigator
                  url="pages/API/index"
                  openType="switchTab"
                  onSuccess={() => addLog('✓ Switch to API tab')}
                >
                  <div className="py-2 px-3 bg-purple-50 hover:bg-purple-100 text-purple-600 rounded-lg text-xs font-medium text-center transition-colors">
                    📡 API
                  </div>
                </LxNavigator>
                <LxNavigator
                  url="pages/todo/index"
                  openType="switchTab"
                  onSuccess={() => addLog('✓ Switch to Todo tab')}
                >
                  <div className="py-2 px-3 bg-green-50 hover:bg-green-100 text-green-600 rounded-lg text-xs font-medium text-center transition-colors">
                    ✓ Todo
                  </div>
                </LxNavigator>
              </div>
            </div>
          </div>
        </div>

        {/* External Navigation */}
        <div className="space-y-3">
          <div className="flex items-center gap-2 px-1">
            <div className="w-1 h-4 bg-green-500 rounded-full" />
            <h2 className="text-base font-semibold text-gray-900">External Navigation</h2>
          </div>

          <div className="bg-white rounded-xl shadow-sm border border-gray-100 overflow-hidden">
            <div className="p-4 space-y-3">
              <LxNavigator
                appId="testminiapp"
                onSuccess={() => addLog('✓ Opening other LxApp')}
                onFail={onFailWithMessage('Failed to open LxApp')}
              >
                <div className="w-full py-2.5 px-4 bg-gradient-to-r from-green-500 to-emerald-500 hover:from-green-600 hover:to-emerald-600 text-white rounded-lg text-sm font-medium text-center transition-all shadow-sm">
                  <div className="flex items-center justify-center gap-2">
                    <span>📱</span>
                    <span>Open Other LxApp</span>
                  </div>
                </div>
              </LxNavigator>

              <LxNavigator
                url="https://www.deepseek.com"
                target="self"
                onSuccess={() => addLog('✓ Opening DeepSeek in-app')}
                onFail={onFailWithMessage('Failed to open in-app browser')}
              >
                <div className="w-full py-2.5 px-4 bg-gradient-to-r from-blue-600 to-cyan-600 hover:from-blue-700 hover:to-cyan-700 text-white rounded-lg text-sm font-medium text-center transition-all shadow-sm">
                  <div className="flex items-center justify-center gap-2">
                    <span>🔗</span>
                    <span>Open DeepSeek</span>
                  </div>
                </div>
              </LxNavigator>

              <LxNavigator
                url="https://www.deepseek.com"
                target="browser"
                onSuccess={() => addLog('✓ Opening DeepSeek in external browser')}
                onFail={onFailWithMessage('Failed to open external browser')}
              >
                <div className="w-full py-2.5 px-4 bg-gradient-to-r from-gray-600 to-gray-700 hover:from-gray-700 hover:to-gray-800 text-white rounded-lg text-sm font-medium text-center transition-all shadow-sm">
                  <div className="flex items-center justify-center gap-2">
                    <span>🌐</span>
                    <span>Open DeepSeek in Default Browser</span>
                  </div>
                </div>
              </LxNavigator>
            </div>
          </div>
        </div>

        {/* Phone Call */}
        <div className="space-y-3">
          <div className="flex items-center gap-2 px-1">
            <div className="w-1 h-4 bg-rose-500 rounded-full" />
            <h2 className="text-base font-semibold text-gray-900">Phone Call</h2>
          </div>

          <div className="bg-white rounded-xl shadow-sm border border-gray-100 overflow-hidden">
            <div className="p-4">
              <div className="flex items-start justify-between mb-3">
                <div>
                  <div className="text-sm font-medium text-gray-900">Make Phone Call</div>
                  <div className="text-xs text-gray-500 mt-0.5">Trigger system dialer with tel open-type</div>
                </div>
              </div>
              <LxNavigator
                openType="tel"
                phoneNumber="10086"
                onSuccess={() => addLog('✓ Making phone call')}
                onFail={onFailWithMessage('Failed to make call')}
              >
                <div className="w-full py-3 px-4 bg-gradient-to-r from-rose-500 to-pink-500 hover:from-rose-600 hover:to-pink-600 active:from-rose-700 active:to-pink-700 text-white rounded-xl text-sm font-medium text-center transition-all shadow-sm">
                  <div className="flex items-center justify-center gap-3">
                    <div className="w-8 h-8 bg-white/20 rounded-full flex items-center justify-center">
                      <svg viewBox="0 0 24 24" fill="currentColor" className="w-4 h-4">
                        <path d="M6.62 10.79c1.44 2.83 3.76 5.14 6.59 6.59l2.2-2.2c.27-.27.67-.36 1.02-.24 1.12.37 2.33.57 3.57.57.55 0 1 .45 1 1V20c0 .55-.45 1-1 1-9.39 0-17-7.61-17-17 0-.55.45-1 1-1h3.5c.55 0 1 .45 1 1 0 1.25.2 2.45.57 3.57.11.35.03.74-.25 1.02l-2.2 2.2z"/>
                      </svg>
                    </div>
                    <div className="flex flex-col items-start">
                      <span className="text-white/80 text-xs">Call</span>
                      <span className="text-base font-semibold tracking-wide">10086</span>
                    </div>
                  </div>
                </div>
              </LxNavigator>
            </div>
          </div>
        </div>

        {/* Event Logs */}
        <div className="bg-white rounded-xl shadow-sm border border-gray-100 overflow-hidden">
          <div className="p-4">
            <div className="text-xs text-gray-500 mb-3 font-medium uppercase tracking-wider">Event Logs</div>
            {logs.length === 0 ? (
              <div className="text-xs text-gray-400">No events yet</div>
            ) : (
              <div className="space-y-2">
                {logs.map((log, index) => (
                  <div
                    key={`${log}-${index}`}
                    className="text-xs text-gray-700 bg-gray-50 border border-gray-100 rounded-lg px-3 py-2 break-all"
                  >
                    {log}
                  </div>
                ))}
              </div>
            )}
          </div>
        </div>

        {/* Info Card */}
        <div className="bg-blue-50 border border-blue-100 rounded-xl p-4">
          <div className="flex gap-3">
            <div className="text-blue-500 flex-shrink-0 mt-0.5">
              <svg viewBox="0 0 24 24" fill="currentColor" className="w-5 h-5">
                <path d="M12 2C6.48 2 2 6.48 2 12s4.48 10 10 10 10-4.48 10-10S17.52 2 12 2zm1 15h-2v-2h2v2zm0-4h-2V7h2v6z" />
              </svg>
            </div>
            <div className="flex-1">
              <div className="text-sm font-medium text-blue-900 mb-1">Smart & Simple</div>
              <div className="text-xs text-blue-700 leading-relaxed">
                • HTTPS URLs → auto open in browser<br />
                • appId → auto target other lxapp<br />
                • Pass data via query string in path
              </div>
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}
