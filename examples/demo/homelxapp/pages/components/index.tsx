import React from 'react';
import '../../tailwind.css';

export default function ComponentsPage() {
  const { data, toggleSection, navigateToVideoDemo } = useLingXia();
  const { expandedSections = { media: true } } = data;

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

        {/* Media Components - Dropdown */}
        <div className="bg-white rounded-xl shadow-sm border border-gray-200 overflow-hidden">
          <div
            className="px-4 py-4 flex items-center justify-between cursor-pointer hover:bg-gray-50 active:bg-gray-100 transition-colors"
            onClick={() => toggleSection({ section: 'media' })}
          >
            <div className="flex items-center gap-3">
              <div className="w-10 h-10 bg-gradient-to-br from-pink-500 to-rose-500 rounded-lg flex items-center justify-center">
                <svg viewBox="0 0 24 24" fill="white" className="w-5 h-5">
                  <path d="M4 4h16a2 2 0 0 1 2 2v12a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V6a2 2 0 0 1 2-2z" />
                  <path d="M10 9l5 3-5 3V9z" fill="currentColor" opacity="0.8" />
                </svg>
              </div>
              <div>
                <div className="text-base font-semibold text-gray-900">Media Components</div>
                <div className="text-xs text-gray-500">Video, Audio & Image</div>
              </div>
            </div>
            <div className="w-6 h-6 text-gray-400">
              <svg
                viewBox="0 0 24 24"
                fill="none"
                stroke="currentColor"
                strokeWidth="2"
                className={`transform transition-transform duration-200 ${expandedSections.media ? 'rotate-180' : ''}`}
              >
                <path d="M6 9l6 6 6-6" />
              </svg>
            </div>
          </div>

          {expandedSections.media && (
            <div className="border-t border-gray-100 bg-gray-50/50">
              <div
                className="px-4 py-3.5 hover:bg-white cursor-pointer flex items-center justify-between group transition-colors"
                onClick={navigateToVideoDemo}
              >
                <div className="flex items-center gap-3">
                  <div className="w-8 h-8 bg-gradient-to-br from-blue-500 to-cyan-500 rounded-lg flex items-center justify-center">
                    <svg viewBox="0 0 24 24" fill="white" className="w-4 h-4">
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
            </div>
          )}
        </div>

      </div>
    </div>
  );
}