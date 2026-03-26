import { useLingXia } from '@lingxia/react';
import '../../tailwind.css';

export default function PullDownRefreshPage() {
  const {
    data,
    startRefresh,
    stopRefresh,
  } = useLingXia();

  const {
    refreshCount = 0,
    lastRefreshTime = null,
    isRefreshing = false,
  } = data;

  return (
    <div className="min-h-screen bg-gray-100">
      <div className="px-4 py-6 pb-12 space-y-4">

        {/* Header Info Card */}
        <div className="bg-white rounded-xl shadow-sm border border-gray-200 overflow-hidden">
          <div className="px-4 py-5 text-center">
            <div className="w-12 h-12 mx-auto mb-3 text-blue-500">
              <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5">
                <path d="M4 4v5h.582m15.356 2A8.001 8.001 0 004.582 9m0 0H9m11 11v-5h-.581m0 0a8.003 8.003 0 01-15.357-2m15.357 2H15"/>
              </svg>
            </div>
            <div className="text-base text-gray-900 font-medium mb-2">
              Pull Down Refresh Demo
            </div>
            <div className="text-sm text-gray-500">
              Pull down on this page to trigger refresh, or use the buttons below to control it programmatically.
            </div>
          </div>
        </div>

        {/* Status Card */}
        <div className="bg-white rounded-xl shadow-sm border border-gray-200 overflow-hidden">
          <div className="px-4 py-3 border-b border-gray-100">
            <h3 className="text-base font-medium text-gray-900">Refresh Status</h3>
          </div>
          <div className="px-4 py-4 space-y-3">
            <div className="flex items-center justify-between">
              <span className="text-sm text-gray-600">Status</span>
              <span className={`text-sm font-medium ${isRefreshing ? 'text-blue-600' : 'text-green-600'}`}>
                {isRefreshing ? 'Refreshing...' : 'Idle'}
              </span>
            </div>
            <div className="flex items-center justify-between">
              <span className="text-sm text-gray-600">Refresh Count</span>
              <span className="text-sm font-medium text-gray-900">{refreshCount}</span>
            </div>
            <div className="flex items-center justify-between">
              <span className="text-sm text-gray-600">Last Refresh</span>
              <span className="text-sm font-medium text-gray-900">
                {lastRefreshTime || 'Never'}
              </span>
            </div>
          </div>
        </div>

        {/* API Controls */}
        <div className="bg-white rounded-xl shadow-sm border border-gray-200 overflow-hidden">
          <div className="px-4 py-3 border-b border-gray-100">
            <h3 className="text-base font-medium text-gray-900">API Controls</h3>
            <p className="text-sm text-gray-500 mt-1">Control refresh programmatically</p>
          </div>
          <div className="p-4 space-y-3">
            <button
              onClick={startRefresh}
              disabled={isRefreshing}
              className={`w-full py-3 px-4 rounded-lg text-sm font-medium transition-colors ${
                isRefreshing
                  ? 'bg-gray-100 text-gray-400 cursor-not-allowed'
                  : 'bg-blue-500 hover:bg-blue-600 text-white'
              }`}
            >
              lx.startPullDownRefresh()
            </button>
            <button
              onClick={stopRefresh}
              className="w-full bg-gray-500 hover:bg-gray-600 text-white py-3 px-4 rounded-lg text-sm font-medium transition-colors"
            >
              lx.stopPullDownRefresh()
            </button>
          </div>
        </div>

      </div>
    </div>
  );
}
