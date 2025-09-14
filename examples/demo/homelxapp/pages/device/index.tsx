import React from 'react';
import '../../tailwind.css';

export default function DevicePage() {
  // Use LingXia hook to get data and functions
  const { data, getDeviceInfo } = window.useLingXia();
  const {
    deviceInfo = null
  } = data;

  return (
    <div className="min-h-screen bg-gray-100">
      <div className="max-w-md mx-auto">
        {/* Device Info Demo */}
        <div className="mt-6 mb-3 px-5 text-sm text-gray-500 font-medium">Device Information</div>
        <div className="mx-3 mb-4 bg-white rounded-xl shadow-sm border border-gray-200 overflow-hidden">
          <div className="flex items-center px-4 py-4">
            <div className="text-xl mr-4">📱</div>
            <div className="flex-1">
              <div className="text-base text-black mb-0.5 font-medium">Get Device Information</div>
              <div className="text-xs text-gray-400 leading-tight">Get device brand, model and other basic information</div>
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
                <div className="flex justify-between items-center py-2 border-b border-gray-100 last:border-b-0">
                  <span className="text-sm text-gray-600">Brand</span>
                  <span className="text-sm font-medium text-gray-900">{deviceInfo.brand || '-'}</span>
                </div>
                <div className="flex justify-between items-center py-2 border-b border-gray-100 last:border-b-0">
                  <span className="text-sm text-gray-600">Model</span>
                  <span className="text-sm font-medium text-gray-900">{deviceInfo.model || '-'}</span>
                </div>
                <div className="flex justify-between items-center py-2 border-b border-gray-100 last:border-b-0">
                  <span className="text-sm text-gray-600">System</span>
                  <span className="text-sm font-medium text-gray-900">{deviceInfo.system || '-'}</span>
                </div>
              </div>
            </div>
          )}
        </div>
      </div>
    </div>
  );
}
