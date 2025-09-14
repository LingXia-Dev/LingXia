import React from 'react';
import '../../tailwind.css';

export default function UIPage() {
  // Use LingXia hook to get data and functions
  const { data, demoNavigateTo, demoNavigateBack, demoSwitchTab, demoRedirectTo } = window.useLingXia();
  const { currentType = 'navigation', pageStack = [] } = data;

  return (
    <div className="min-h-screen bg-gray-100 overflow-y-auto">
      <div className="max-w-md mx-auto pb-6">


        {/* Navigation Demo Section */}
        <div className="mt-4 mb-3 px-4 text-sm text-gray-500 font-medium">navigateTo/Back, redirectTo</div>

        <div className="mx-3 mb-4 bg-white rounded-xl shadow-sm border border-gray-200 overflow-hidden">
          <div
            className="flex items-center px-4 py-3 hover:bg-gray-50 cursor-pointer border-b border-gray-100"
            onClick={demoNavigateTo}
          >
            <div className="text-base text-black">Navigate to new page</div>
          </div>
          <div
            className="flex items-center px-4 py-3 hover:bg-gray-50 cursor-pointer border-b border-gray-100"
            onClick={demoNavigateBack}
          >
            <div className="text-base text-black">Back to previous page</div>
          </div>
          <div
            className="flex items-center px-4 py-3 hover:bg-gray-50 cursor-pointer border-b border-gray-100"
            onClick={demoRedirectTo}
          >
            <div className="text-base text-black">Open in current page</div>
          </div>
          <div
            className="flex items-center px-4 py-3 hover:bg-gray-50 cursor-pointer"
            onClick={demoSwitchTab}
          >
            <div className="text-base text-black">Jump to Tab page</div>
          </div>
        </div>



        {/* Page Stack Info */}
        <div className="mx-3 mb-4 bg-white rounded-xl shadow-sm border border-gray-200 overflow-hidden">
          <div className="px-4 py-4">
            <div className="text-sm font-medium text-gray-700 mb-3">Current Page Stack</div>
            <div className="max-h-64 overflow-y-auto space-y-2">
              {pageStack.map((page, index) => (
                <div key={index} className="flex items-center justify-between py-2 px-3 bg-gray-50 rounded-lg">
                  <div className="flex items-center">
                    <span className="text-xs font-medium text-blue-600 mr-2">#{page.index + 1}</span>
                    <span className="text-sm text-gray-700">{page.route}</span>
                  </div>
                  {Object.keys(page.options).length > 0 && (
                    <div className="text-xs text-gray-500">
                      {JSON.stringify(page.options)}
                    </div>
                  )}
                </div>
              ))}
              {pageStack.length === 0 && (
                <div className="text-sm text-gray-500 text-center py-2">No page stack available</div>
              )}
            </div>
          </div>
        </div>


      </div>
    </div>
  );
}
