import React from 'react';
import '../../tailwind.css';

export default function UIPage() {
  // Use LingXia hook to get data and functions
  const { data, demoNavigateTo, demoNavigateBack, demoSwitchTab, demoRedirectTo,
          showToastWithParams, hideToast, showModalWithParams, clearModalResult,
          setNavigationBarTitle, setNavigationBarColor } = useLingXia();
  const { currentType = 'navigation', pageStack = [], modalResult = null } = data;

  // Local state for toast parameters
  const [toastTitle, setToastTitle] = React.useState('Hello Toast!');
  const [toastIcon, setToastIcon] = React.useState('success');
  const [toastDuration, setToastDuration] = React.useState(2000);
  const [toastPosition, setToastPosition] = React.useState('center');
  const [toastMask, setToastMask] = React.useState(false);

  // Local state for modal parameters
  const [modalTitle, setModalTitle] = React.useState('Alert');
  const [modalContent, setModalContent] = React.useState('This is a modal dialog');
  const [modalShowCancel, setModalShowCancel] = React.useState(true);
  const [modalCancelText, setModalCancelText] = React.useState('Cancel');
  const [modalConfirmText, setModalConfirmText] = React.useState('OK');


  return (
    <div className="min-h-screen bg-gray-100 overflow-y-auto">
      <div className="max-w-md mx-auto pb-6 px-2 pt-2">

        {/* Navigation Demo Section */}
        {currentType === 'navigation' && (
          <>
            <div className="mt-4 mb-3 px-4 text-sm text-gray-500 font-medium">navigateTo/Back, redirectTo</div>

        <div className="mx-1 mb-4 bg-white rounded-xl shadow-sm border border-gray-200 overflow-hidden">
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
          </>
        )}

        {/* Toast Demo Section */}
        {currentType === 'toast' && (
          <>
            <div className="mt-4 mb-3 px-4 text-sm text-gray-500 font-medium">Toast Parameters</div>

            {/* Toast Parameters */}
            <div className="mx-1 mb-4 bg-white rounded-xl shadow-sm border border-gray-200 overflow-hidden">
              <div className="px-3 py-3 space-y-3">

                {/* Title Input */}
                <div>
                  <label className="block text-sm font-medium text-gray-700 mb-2">Title</label>
                  <input
                    type="text"
                    value={toastTitle}
                    onChange={(e) => setToastTitle(e.target.value)}
                    className="w-full px-3 py-2 border border-gray-300 rounded-md focus:outline-none focus:ring-2 focus:ring-blue-500"
                    placeholder="Enter toast title"
                  />
                </div>

                {/* Icon Selection */}
                <div>
                  <label className="block text-sm font-medium text-gray-700 mb-2">Icon</label>
                  <select
                    value={toastIcon}
                    onChange={(e) => setToastIcon(e.target.value)}
                    className="w-full px-3 py-2 border border-gray-300 rounded-md focus:outline-none focus:ring-2 focus:ring-blue-500"
                  >
                    <option value="success">Success</option>
                    <option value="error">Error</option>
                    <option value="loading">Loading</option>
                    <option value="none">None</option>
                  </select>
                </div>

                {/* Duration Input */}
                <div>
                  <label className="block text-sm font-medium text-gray-700 mb-2">Duration (ms)</label>
                  <input
                    type="number"
                    value={toastDuration}
                    onChange={(e) => setToastDuration(parseInt(e.target.value) || 2000)}
                    className="w-full px-3 py-2 border border-gray-300 rounded-md focus:outline-none focus:ring-2 focus:ring-blue-500"
                    min="500"
                    max="10000"
                    step="500"
                  />
                </div>

                {/* Position Selection */}
                <div>
                  <label className="block text-sm font-medium text-gray-700 mb-2">Position</label>
                  <select
                    value={toastPosition}
                    onChange={(e) => setToastPosition(e.target.value)}
                    className="w-full px-3 py-2 border border-gray-300 rounded-md focus:outline-none focus:ring-2 focus:ring-blue-500"
                  >
                    <option value="top">Top</option>
                    <option value="center">Center</option>
                    <option value="bottom">Bottom</option>
                  </select>
                </div>

                {/* Mask Checkbox */}
                <div className="flex items-center">
                  <input
                    type="checkbox"
                    id="toastMask"
                    checked={toastMask}
                    onChange={(e) => setToastMask(e.target.checked)}
                    className="h-4 w-4 text-blue-600 focus:ring-blue-500 border-gray-300 rounded"
                  />
                  <label htmlFor="toastMask" className="ml-2 block text-sm text-gray-700">
                    Show mask (prevents interaction)
                  </label>
                </div>
              </div>
            </div>

            {/* Action Buttons */}
            <div className="mx-1 mb-4 bg-white rounded-xl shadow-sm border border-gray-200 overflow-hidden">
              <div
                className="flex items-center justify-center px-4 py-4 hover:bg-gray-50 cursor-pointer border-b border-gray-100"
                onClick={() => showToastWithParams({
                  title: toastTitle,
                  icon: toastIcon,
                  duration: toastDuration,
                  position: toastPosition,
                  mask: toastMask
                })}
              >
                <div className="text-base text-blue-600 font-medium">Show Toast</div>
              </div>
              <div
                className="flex items-center justify-center px-4 py-4 hover:bg-gray-50 cursor-pointer"
                onClick={hideToast}
              >
                <div className="text-base text-red-600 font-medium">Hide Toast</div>
              </div>
            </div>
          </>
        )}

        {/* Modal Demo Section */}
        {currentType === 'modal' && (
          <>
            <div className="mt-4 mb-3 px-4 text-sm text-gray-500 font-medium">Modal Parameters</div>

            {/* Modal Parameters */}
            <div className="mx-1 mb-4 bg-white rounded-xl shadow-sm border border-gray-200 overflow-hidden">
              <div className="px-3 py-3 space-y-3">

                {/* Title Input */}
                <div>
                  <label className="block text-sm font-medium text-gray-700 mb-2">Title (optional)</label>
                  <input
                    type="text"
                    value={modalTitle}
                    onChange={(e) => setModalTitle(e.target.value)}
                    className="w-full px-3 py-2 border border-gray-300 rounded-md focus:outline-none focus:ring-2 focus:ring-blue-500"
                    placeholder="Leave empty for no title"
                  />
                </div>



                {/* Content Input */}
                <div>
                  <label className="block text-sm font-medium text-gray-700 mb-2">Content</label>
                  <textarea
                    value={modalContent}
                    onChange={(e) => setModalContent(e.target.value)}
                    className="w-full px-3 py-2 border border-gray-300 rounded-md focus:outline-none focus:ring-2 focus:ring-blue-500"
                    placeholder="Enter modal content"
                    rows={3}
                  />
                </div>

                {/* Show Cancel Checkbox */}
                <div className="flex items-center">
                  <input
                    type="checkbox"
                    id="modalShowCancel"
                    checked={modalShowCancel}
                    onChange={(e) => setModalShowCancel(e.target.checked)}
                    className="h-4 w-4 text-blue-600 focus:ring-blue-500 border-gray-300 rounded"
                  />
                  <label htmlFor="modalShowCancel" className="ml-2 block text-sm text-gray-700">
                    Show cancel button
                  </label>
                </div>

                {/* Cancel Text Input */}
                <div>
                  <label className="block text-sm font-medium text-gray-700 mb-2">Cancel Button Text</label>
                  <input
                    type="text"
                    value={modalCancelText}
                    onChange={(e) => setModalCancelText(e.target.value)}
                    className="w-full px-3 py-2 border border-gray-300 rounded-md focus:outline-none focus:ring-2 focus:ring-blue-500"
                    placeholder="Cancel button text"
                  />
                </div>

                {/* Confirm Text Input */}
                <div>
                  <label className="block text-sm font-medium text-gray-700 mb-2">Confirm Button Text</label>
                  <input
                    type="text"
                    value={modalConfirmText}
                    onChange={(e) => setModalConfirmText(e.target.value)}
                    className="w-full px-3 py-2 border border-gray-300 rounded-md focus:outline-none focus:ring-2 focus:ring-blue-500"
                    placeholder="Confirm button text"
                  />
                </div>


              </div>
            </div>

            {/* Action Button */}
            <div className="mx-1 mb-4 bg-white rounded-xl shadow-sm border border-gray-200 overflow-hidden">
              <div
                className="flex items-center justify-center px-4 py-4 hover:bg-gray-50 cursor-pointer"
                onClick={() => showModalWithParams({
                  title: modalTitle,
                  content: modalContent,
                  showCancel: modalShowCancel,
                  cancelText: modalCancelText,
                  confirmText: modalConfirmText
                })}
              >
                <div className="text-base text-blue-600 font-medium">Show Modal</div>
              </div>
            </div>

            {/* Result Display */}
            {modalResult && (
              <div className="mx-1 mb-4 bg-white rounded-xl shadow-sm border border-gray-200 overflow-hidden">
                <div className="px-3 py-3">
                  <div className="text-sm font-medium text-gray-700 mb-3">Modal Result</div>
                  <div className="bg-gray-50 rounded-lg p-3">
                    <pre className="text-xs text-gray-600 whitespace-pre-wrap">
                      {JSON.stringify(modalResult, null, 2)}
                    </pre>
                  </div>
                  <div
                    className="mt-3 text-center text-sm text-red-600 cursor-pointer hover:text-red-800"
                    onClick={clearModalResult}
                  >
                    Clear Result
                  </div>
                </div>
              </div>
            )}
          </>
        )}

        {/* Page Stack Info - Only show for navigation */}
        {currentType === 'navigation' && (
          <div className="mx-1 mb-4 bg-white rounded-xl shadow-sm border border-gray-200 overflow-hidden">
            <div className="px-3 py-3">
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
        )}

        {/* NavigationBar Demo Section */}
        {currentType === 'navbar' && (
          <>
            <div className="mt-4 mb-3 px-4 text-sm text-gray-500 font-medium">NavigationBar APIs</div>

            {/* NavigationBar Controls */}
            <div className="mx-1 mb-4 bg-white rounded-xl shadow-sm border border-gray-200 overflow-hidden">
              <div className="p-4 space-y-4">

                {/* Set Title */}
                <div>
                  <label className="block text-sm font-medium text-gray-700 mb-1">Title</label>
                  <div className="flex space-x-2">
                    <input
                      type="text"
                      id="navbarTitle"
                      placeholder="Enter title"
                      className="flex-1 px-2 py-1.5 text-sm border border-gray-300 rounded focus:outline-none focus:ring-1 focus:ring-blue-500"
                    />
                    <button
                      onClick={() => {
                        const title = document.getElementById('navbarTitle').value;
                        if (title) {
                          setNavigationBarTitle({ title });
                        }
                      }}
                      className="px-3 py-1.5 text-sm bg-blue-500 text-white rounded hover:bg-blue-600 focus:outline-none focus:ring-1 focus:ring-blue-500"
                    >
                      Set
                    </button>
                  </div>
                </div>

                {/* Set Colors */}
                <div>
                  <label className="block text-sm font-medium text-gray-700 mb-1">Colors</label>
                  <div className="space-y-2">
                    <div className="grid grid-cols-2 gap-2">
                      <input
                        type="text"
                        id="navbarBgColor"
                        placeholder="Background #ffffff"
                        className="px-2 py-1.5 text-sm border border-gray-300 rounded focus:outline-none focus:ring-1 focus:ring-blue-500"
                      />
                      <input
                        type="text"
                        id="navbarTextColor"
                        placeholder="Text #000000"
                        className="px-2 py-1.5 text-sm border border-gray-300 rounded focus:outline-none focus:ring-1 focus:ring-blue-500"
                      />
                    </div>
                    <button
                      onClick={() => {
                        const bgColor = document.getElementById('navbarBgColor').value || '#ffffff';
                        const textColor = document.getElementById('navbarTextColor').value || '#000000';
                        setNavigationBarColor({
                          background_color: bgColor,
                          front_color: textColor
                        });
                      }}
                      className="w-full px-3 py-1.5 text-sm bg-green-500 text-white rounded hover:bg-green-600 focus:outline-none focus:ring-1 focus:ring-green-500"
                    >
                      Set Colors
                    </button>
                  </div>
                </div>



                {/* Preset Examples */}
                <div>
                  <label className="block text-sm font-medium text-gray-700 mb-1">Presets</label>
                  <div className="grid grid-cols-2 gap-1.5">
                    <button
                      onClick={() => {
                        setNavigationBarTitle({ title: "Dark Theme" });
                        setNavigationBarColor({ background_color: "#1f2937", front_color: "#ffffff" });
                      }}
                      className="px-2 py-1.5 bg-gray-800 text-white rounded hover:bg-gray-900 text-xs"
                    >
                      Dark
                    </button>
                    <button
                      onClick={() => {
                        setNavigationBarTitle({ title: "Blue Theme" });
                        setNavigationBarColor({ background_color: "#3b82f6", front_color: "#ffffff" });
                      }}
                      className="px-2 py-1.5 bg-blue-500 text-white rounded hover:bg-blue-600 text-xs"
                    >
                      Blue
                    </button>
                    <button
                      onClick={() => {
                        setNavigationBarTitle({ title: "Light Theme" });
                        setNavigationBarColor({ background_color: "#ffffff", front_color: "#000000" });
                      }}
                      className="px-2 py-1.5 bg-white text-black border border-gray-300 rounded hover:bg-gray-50 text-xs"
                    >
                      Light
                    </button>
                    <button
                      onClick={() => {
                        setNavigationBarTitle({ title: "Green Theme" });
                        setNavigationBarColor({ background_color: "#10b981", front_color: "#ffffff" });
                      }}
                      className="px-2 py-1.5 bg-green-500 text-white rounded hover:bg-green-600 text-xs"
                    >
                      Green
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
