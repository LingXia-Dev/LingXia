import React from 'react';
import '../../tailwind.css';

export default function ComponentsPage() {
  const { navigateTo } = useLingXia();

  return (
    <div className="min-h-screen bg-gray-100">
      <div className="px-3 py-2 pb-12 space-y-2">

        {/* Compact Header */}
        <div className="bg-gradient-to-r from-blue-500 to-purple-600 rounded-xl shadow-sm px-4 py-3 flex items-center gap-3">
          <div className="w-10 h-10 bg-white/20 rounded-lg flex items-center justify-center">
            <svg viewBox="0 0 24 24" fill="none" stroke="white" strokeWidth="2" className="w-5 h-5">
              <rect x="3" y="3" width="7" height="7" rx="1" />
              <rect x="14" y="3" width="7" height="7" rx="1" />
              <rect x="14" y="14" width="7" height="7" rx="1" />
              <rect x="3" y="14" width="7" height="7" rx="1" />
            </svg>
          </div>
          <div>
            <div className="text-base text-white font-semibold">Component Gallery</div>
            <div className="text-xs text-white/70">Native-backed UI components</div>
          </div>
        </div>

        {/* Component List */}
        <div className="bg-white rounded-xl shadow-sm border border-gray-200 overflow-hidden divide-y divide-gray-100">
          {/* Video */}
          <div
            className="px-4 py-3.5 hover:bg-gray-50 cursor-pointer flex items-center justify-between group transition-colors"
            onClick={() => navigateTo({ url: 'pages/video/index.tsx' })}
          >
            <div className="flex items-center gap-3">
              <div className="w-10 h-10 bg-gradient-to-br from-blue-500 to-cyan-500 rounded-lg flex items-center justify-center">
                <svg viewBox="0 0 24 24" fill="white" className="w-5 h-5">
                  <polygon points="5 3 19 12 5 21 5 3" />
                </svg>
              </div>
              <div>
                <div className="text-sm font-medium text-gray-900">Video Player</div>
                <div className="text-xs text-gray-500">Native video with controls</div>
              </div>
            </div>
            <div className="w-5 h-5 text-gray-400 group-hover:text-blue-500 transition-colors">
              <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
                <path d="M9 18l6-6-6-6" />
              </svg>
            </div>
          </div>

          {/* Navigator */}
          <div
            className="px-4 py-3.5 hover:bg-gray-50 cursor-pointer flex items-center justify-between group transition-colors"
            onClick={() => navigateTo({ url: 'pages/navigator/index.tsx' })}
          >
            <div className="flex items-center gap-3">
              <div className="w-10 h-10 bg-gradient-to-br from-emerald-500 to-teal-500 rounded-lg flex items-center justify-center">
                <svg viewBox="0 0 24 24" fill="none" stroke="white" strokeWidth="2" className="w-5 h-5">
                  <path d="M12 3l7 18-7-4-7 4 7-18z" />
                </svg>
              </div>
              <div>
                <div className="text-sm font-medium text-gray-900">Navigator</div>
                <div className="text-xs text-gray-500">Navigate pages and apps</div>
              </div>
            </div>
            <div className="w-5 h-5 text-gray-400 group-hover:text-emerald-500 transition-colors">
              <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
                <path d="M9 18l6-6-6-6" />
              </svg>
            </div>
          </div>

          {/* Picker */}
          <div
            className="px-4 py-3.5 hover:bg-gray-50 cursor-pointer flex items-center justify-between group transition-colors"
            onClick={() => navigateTo({ url: 'pages/picker/index.tsx' })}
          >
            <div className="flex items-center gap-3">
              <div className="w-10 h-10 bg-gradient-to-br from-purple-500 to-pink-500 rounded-lg flex items-center justify-center">
                <svg viewBox="0 0 24 24" fill="none" stroke="white" strokeWidth="2" className="w-5 h-5">
                  <rect x="4" y="6" width="16" height="12" rx="2" />
                  <line x1="12" y1="6" x2="12" y2="18" />
                </svg>
              </div>
              <div>
                <div className="text-sm font-medium text-gray-900">Picker</div>
                <div className="text-xs text-gray-500">Native picker with value/onChange</div>
              </div>
            </div>
            <div className="w-5 h-5 text-gray-400 group-hover:text-purple-500 transition-colors">
              <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
                <path d="M9 18l6-6-6-6" />
              </svg>
            </div>
          </div>
        </div>

      </div>
    </div>
  );
}
