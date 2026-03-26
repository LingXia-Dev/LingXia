import { useLingXia } from '@lingxia/react';
import '../../tailwind.css';

export default function APIPage() {
  // Use LingXia hook to get data and functions
  const {
    data,
    toggleSection,
    navigateToUIPage,
    navigateToDevicePage,
    navigateToWifiPage,
    navigateToSystemPage,
    navigateToLocationPage,
    navigateToMediaPage,
    navigateToDocumentPage,
    navigateToTestMiniApp,
    navigateToCloudPage,
    openDeepSeek,
    navigateToPullDownRefreshPage,
  } = useLingXia();
  const { expandedSections = { interface: false, device: false, system: false, cloud: false, navigation: false, media: false, document: false } } = data;

  return (
    <div className="min-h-screen bg-gray-100 overflow-y-auto">
      <div className="px-3 py-2 pb-12 space-y-2">

        {/* Header Card - Description */}
        <div className="bg-white rounded-lg shadow-sm">
          <div className="px-4 py-6 text-center">
            <div className="w-12 h-12 mx-auto mb-3 text-blue-500">
              <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5">
                <path d="M13 2L3 14h9l-1 8 10-12h-9l1-8z"/>
              </svg>
            </div>
            <div className="text-base text-gray-900 font-medium">
              The following demonstrates the capabilities provided by LingXia.
            </div>
          </div>
        </div>

        {/* Cloud - Dropdown */}
        <div className="bg-white rounded-lg shadow-sm">
          <div
            className="px-4 py-4 flex items-center justify-between cursor-pointer hover:bg-gray-50"
            onClick={() => toggleSection({ section: 'cloud' })}
          >
            <div className="text-base text-gray-900">Cloud</div>
            <div className="w-6 h-6 text-gray-400">
              <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5">
                <path d="M6 19a4 4 0 0 1-.4-7.98A5.5 5.5 0 0 1 16 8.5h.5a4.5 4.5 0 1 1 .5 9H6z"/>
              </svg>
            </div>
          </div>

          {expandedSections.cloud && (
            <div className="border-t border-gray-100 bg-gray-50">
              <div
                className="px-4 py-3 hover:bg-gray-100 cursor-pointer flex items-center justify-between"
                onClick={navigateToTestMiniApp}
              >
                <div className="text-sm text-gray-700">Open Another LxApp</div>
                <div className="w-4 h-4 text-gray-400">
                  <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5">
                    <path d="M9 18l6-6-6-6"/>
                  </svg>
                </div>
              </div>
              <div
                className="px-4 py-3 hover:bg-gray-100 cursor-pointer flex items-center justify-between border-t border-gray-200"
                onClick={() => navigateToCloudPage({ type: 'auth' })}
              >
                <div className="text-sm text-gray-700">Cloud Auth Demo</div>
                <div className="w-4 h-4 text-gray-400">
                  <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5">
                    <path d="M9 18l6-6-6-6"/>
                  </svg>
                </div>
              </div>
              <div
                className="px-4 py-3 hover:bg-gray-100 cursor-pointer flex items-center justify-between border-t border-gray-200"
                onClick={() => navigateToCloudPage({ type: 'mqtt' })}
              >
                <div className="text-sm text-gray-700">Cloud MQTT Demo</div>
                <div className="w-4 h-4 text-gray-400">
                  <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5">
                    <path d="M9 18l6-6-6-6"/>
                  </svg>
                </div>
              </div>
            </div>
          )}
        </div>

        {/* Navigation - Dropdown */}
        <div className="bg-white rounded-lg shadow-sm">
          <div
            className="px-4 py-4 flex items-center justify-between cursor-pointer hover:bg-gray-50"
            onClick={() => toggleSection({ section: 'navigation' })}
          >
            <div className="text-base text-gray-900">Navigation</div>
            <div className="w-6 h-6 text-gray-400">
              <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5">
                <path d="M13 7l5 5-5 5M6 7l5 5-5 5"/>
              </svg>
            </div>
          </div>

          {expandedSections.navigation && (
            <div className="border-t border-gray-100 bg-gray-50">
              <div
                className="px-4 py-3 hover:bg-gray-100 cursor-pointer flex items-center justify-between"
                onClick={openDeepSeek}
              >
                <div className="text-sm text-gray-700">Open DeepSeek</div>
                <div className="w-4 h-4 text-gray-400">
                  <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5">
                    <path d="M9 18l6-6-6-6"/>
                  </svg>
                </div>
              </div>
            </div>
          )}
        </div>

        {/* User Interface - Dropdown */}
        <div className="bg-white rounded-lg shadow-sm">
          <div
            className="px-4 py-4 flex items-center justify-between cursor-pointer hover:bg-gray-50"
            onClick={() => toggleSection({ section: 'interface' })}
          >
            <div className="text-base text-gray-900">User Interface</div>
            <div className="w-6 h-6 text-gray-400">
              <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5">
                <path d="M3 9l9-7 9 7v11a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z"/>
                <polyline points="9,22 9,12 15,12 15,22"/>
              </svg>
            </div>
          </div>

          {expandedSections.interface && (
            <div className="border-t border-gray-100 bg-gray-50">
              <div
                className="px-4 py-3 hover:bg-gray-100 cursor-pointer flex items-center justify-between"
                onClick={() => navigateToUIPage({ type: 'navigation' })}
              >
                <div className="text-sm text-gray-700">Page Navigation</div>
                <div className="w-4 h-4 text-gray-400">
                  <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5">
                    <path d="M9 18l6-6-6-6"/>
                  </svg>
                </div>
              </div>
              <div
                className="px-4 py-3 hover:bg-gray-100 cursor-pointer flex items-center justify-between border-t border-gray-200"
                onClick={() => navigateToUIPage({ type: 'toast' })}
              >
                <div className="text-sm text-gray-700">Toast</div>
                <div className="w-4 h-4 text-gray-400">
                  <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5">
                    <path d="M9 18l6-6-6-6"/>
                  </svg>
                </div>
              </div>
              <div
                className="px-4 py-3 hover:bg-gray-100 cursor-pointer flex items-center justify-between border-t border-gray-200"
                onClick={() => navigateToUIPage({ type: 'modal' })}
              >
                <div className="text-sm text-gray-700">Modal</div>
                <div className="w-4 h-4 text-gray-400">
                  <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5">
                    <path d="M9 18l6-6-6-6"/>
                  </svg>
                </div>
              </div>
              <div
                className="px-4 py-3 hover:bg-gray-100 cursor-pointer flex items-center justify-between border-t border-gray-200"
                onClick={() => navigateToUIPage({ type: 'navbar' })}
              >
                <div className="text-sm text-gray-700">NavigationBar</div>
                <div className="w-4 h-4 text-gray-400">
                  <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5">
                    <path d="M9 18l6-6-6-6"/>
                  </svg>
                </div>
              </div>
              <div
                className="px-4 py-3 hover:bg-gray-100 cursor-pointer flex items-center justify-between border-t border-gray-200"
                onClick={() => navigateToUIPage({ type: 'tabbar' })}
              >
                <div className="text-sm text-gray-700">TabBar</div>
                <div className="w-4 h-4 text-gray-400">
                  <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5">
                    <path d="M9 18l6-6-6-6"/>
                  </svg>
                </div>
              </div>
              <div
                className="px-4 py-3 hover:bg-gray-100 cursor-pointer flex items-center justify-between border-t border-gray-200"
                onClick={() => navigateToUIPage({ type: 'actionsheet' })}
              >
                <div className="text-sm text-gray-700">Action Sheet</div>
                <div className="w-4 h-4 text-gray-400">
                  <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5">
                    <path d="M9 18l6-6-6-6"/>
                  </svg>
                </div>
              </div>
              <div
                className="px-4 py-3 hover:bg-gray-100 cursor-pointer flex items-center justify-between border-t border-gray-200"
                onClick={() => navigateToUIPage({ type: 'popup' })}
              >
                <div>
                  <div className="text-sm text-gray-700">Popup</div>
                </div>
                <div className="w-4 h-4 text-gray-400">
                  <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5">
                    <path d="M9 18l6-6-6-6"/>
                  </svg>
                </div>
              </div>
              <div
                className="px-4 py-3 hover:bg-gray-100 cursor-pointer flex items-center justify-between border-t border-gray-200"
                onClick={navigateToPullDownRefreshPage}
              >
                <div className="text-sm text-gray-700">Pull Down Refresh</div>
                <div className="w-4 h-4 text-gray-400">
                  <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5">
                    <path d="M9 18l6-6-6-6"/>
                  </svg>
                </div>
              </div>
            </div>
          )}
        </div>

        {/* System - Dropdown */}
        <div className="bg-white rounded-lg shadow-sm">
          <div
            className="px-4 py-4 flex items-center justify-between cursor-pointer hover:bg-gray-50"
            onClick={() => toggleSection({ section: 'system' })}
          >
            <div className="text-base text-gray-900">System</div>
            <div className="w-6 h-6 text-gray-400">
              <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5">
                <path d="M4 4h16v16H4z"/>
              </svg>
            </div>
          </div>

          {expandedSections.system && (
            <div className="border-t border-gray-100 bg-gray-50">
              <div
                className="px-4 py-3 hover:bg-gray-100 cursor-pointer flex items-center justify-between"
                onClick={() => navigateToSystemPage({ type: 'appBaseInfo' })}
              >
                <div className="text-sm text-gray-700">App Base Info</div>
                <div className="w-4 h-4 text-gray-400">
                  <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5">
                    <path d="M9 18l6-6-6-6"/>
                  </svg>
                </div>
              </div>
              <div
                className="px-4 py-3 hover:bg-gray-100 cursor-pointer flex items-center justify-between border-t border-gray-200"
                onClick={() => navigateToSystemPage({ type: 'systemSetting' })}
              >
                <div className="text-sm text-gray-700">System Setting</div>
                <div className="w-4 h-4 text-gray-400">
                  <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5">
                    <path d="M9 18l6-6-6-6"/>
                  </svg>
                </div>
              </div>
            </div>
          )}
        </div>

        {/* Device - Dropdown */}
        <div className="bg-white rounded-lg shadow-sm">
          <div
            className="px-4 py-4 flex items-center justify-between cursor-pointer hover:bg-gray-50"
            onClick={() => toggleSection({ section: 'device' })}
          >
            <div className="text-base text-gray-900">Device</div>
            <div className="w-6 h-6 text-gray-400">
              <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5">
                <rect x="5" y="2" width="14" height="20" rx="2"/>
                <path d="M12 18h.01"/>
              </svg>
            </div>
          </div>

          {expandedSections.device && (
            <div className="border-t border-gray-100 bg-gray-50">
              <div
                className="px-4 py-3 hover:bg-gray-100 cursor-pointer flex items-center justify-between"
                onClick={() => navigateToDevicePage({ type: 'device' })}
              >
                <div className="text-sm text-gray-700">Device Info</div>
                <div className="w-4 h-4 text-gray-400">
                  <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5">
                    <path d="M9 18l6-6-6-6"/>
                  </svg>
                </div>
              </div>
              <div
                className="px-4 py-3 hover:bg-gray-100 cursor-pointer flex items-center justify-between border-t border-gray-200"
                onClick={() => navigateToDevicePage({ type: 'screen' })}
              >
                <div className="text-sm text-gray-700">Screen Info</div>
                <div className="w-4 h-4 text-gray-400">
                  <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5">
                    <path d="M9 18l6-6-6-6"/>
                  </svg>
                </div>
              </div>
              <div
                className="px-4 py-3 hover:bg-gray-100 cursor-pointer flex items-center justify-between border-t border-gray-200"
                onClick={() => navigateToDevicePage({ type: 'vibrate' })}
              >
                <div className="text-sm text-gray-700">Vibration</div>
                <div className="w-4 h-4 text-gray-400">
                  <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5">
                    <path d="M9 18l6-6-6-6"/>
                  </svg>
                </div>
              </div>
              <div
                className="px-4 py-3 hover:bg-gray-100 cursor-pointer flex items-center justify-between border-t border-gray-200"
                onClick={() => navigateToDevicePage({ type: 'dial' })}
              >
                <div className="text-sm text-gray-700">Phone Call</div>
                <div className="w-4 h-4 text-gray-400">
                  <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5">
                    <path d="M9 18l6-6-6-6"/>
                  </svg>
                </div>
              </div>
              <div
                className="px-4 py-3 hover:bg-gray-100 cursor-pointer flex items-center justify-between border-t border-gray-200"
                onClick={() => navigateToDevicePage({ type: 'orientation' })}
              >
                <div className="text-sm text-gray-700">Device Orientation</div>
                <div className="w-4 h-4 text-gray-400">
                  <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5">
                    <path d="M9 18l6-6-6-6"/>
                  </svg>
                </div>
              </div>
              <div
                className="px-4 py-3 hover:bg-gray-100 cursor-pointer flex items-center justify-between border-t border-gray-200"
                onClick={() => navigateToDevicePage({ type: 'networkType' })}
              >
                <div className="text-sm text-gray-700">Network Type</div>
                <div className="w-4 h-4 text-gray-400">
                  <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5">
                    <path d="M9 18l6-6-6-6"/>
                  </svg>
                </div>
              </div>
              <div
                className="px-4 py-3 hover:bg-gray-100 cursor-pointer flex items-center justify-between border-t border-gray-200"
                onClick={() => navigateToDevicePage({ type: 'localIP' })}
              >
                <div className="text-sm text-gray-700">Local IP Address</div>
                <div className="w-4 h-4 text-gray-400">
                  <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5">
                    <path d="M9 18l6-6-6-6"/>
                  </svg>
                </div>
              </div>
              <div
                className="px-4 py-3 hover:bg-gray-100 cursor-pointer flex items-center justify-between border-t border-gray-200"
                onClick={() => navigateToDevicePage({ type: 'networkStatus' })}
              >
                <div className="text-sm text-gray-700">Network Status Listener</div>
                <div className="w-4 h-4 text-gray-400">
                  <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5">
                    <path d="M9 18l6-6-6-6"/>
                  </svg>
                </div>
              </div>
              <div
                className="px-4 py-3 hover:bg-gray-100 cursor-pointer flex items-center justify-between border-t border-gray-200"
                onClick={navigateToWifiPage}
              >
                <div className="text-sm text-gray-700">WiFi</div>
                <div className="w-4 h-4 text-gray-400">
                  <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5">
                    <path d="M9 18l6-6-6-6"/>
                  </svg>
                </div>
              </div>
            </div>
          )}
        </div>

        {/* Media - Dropdown */}
        <div className="bg-white rounded-lg shadow-sm">
          <div
            className="px-4 py-4 flex items-center justify-between cursor-pointer hover:bg-gray-50"
            onClick={() => toggleSection({ section: 'media' })}
          >
            <div className="text-base text-gray-900">Media & Camera</div>
            <div className="w-6 h-6 text-gray-400">
              <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5">
                <polygon points="5,3 19,12 5,21"/>
              </svg>
            </div>
          </div>

          {expandedSections.media && (
            <div className="border-t border-gray-100 bg-gray-50">
              <div
                className="px-4 py-3 hover:bg-gray-100 cursor-pointer flex items-center justify-between"
                onClick={() => navigateToMediaPage({ type: 'Pictures' })}
              >
                <div className="text-sm text-gray-700">Pictures</div>
                <div className="w-4 h-4 text-gray-400">
                  <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5">
                    <path d="M9 18l6-6-6-6"/>
                  </svg>
                </div>
              </div>
              <div
                className="px-4 py-3 hover:bg-gray-100 cursor-pointer flex items-center justify-between border-t border-gray-200"
                onClick={() => navigateToMediaPage({ type: 'Videos' })}
              >
                <div className="text-sm text-gray-700">Videos</div>
                <div className="w-4 h-4 text-gray-400">
                  <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5">
                    <path d="M9 18l6-6-6-6"/>
                  </svg>
                </div>
              </div>
              <div
                className="px-4 py-3 hover:bg-gray-100 cursor-pointer flex items-center justify-between border-t border-gray-200"
                onClick={() => navigateToMediaPage({ type: 'scanCode' })}
              >
                <div className="text-sm text-gray-700">ScanCode</div>
                <div className="w-4 h-4 text-gray-400">
                  <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5">
                    <path d="M9 18l6-6-6-6"/>
                  </svg>
                </div>
              </div>
              <div
                className="px-4 py-3 hover:bg-gray-100 cursor-pointer flex items-center justify-between border-t border-gray-200"
                onClick={() => navigateToMediaPage({ type: 'imageInfo' })}
              >
                <div>
                  <div className="text-sm text-gray-700">Image Tools</div>
                </div>
                <div className="w-4 h-4 text-gray-400">
                  <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5">
                    <path d="M9 18l6-6-6-6"/>
                  </svg>
                </div>
              </div>
              <div
                className="px-4 py-3 hover:bg-gray-100 cursor-pointer flex items-center justify-between border-t border-gray-200"
                onClick={() => navigateToMediaPage({ type: 'videoTools' })}
              >
                <div className="text-sm text-gray-700">Video Tools</div>
                <div className="w-4 h-4 text-gray-400">
                  <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5">
                    <path d="M9 18l6-6-6-6"/>
                  </svg>
                </div>
              </div>
              <div
                className="px-4 py-3 hover:bg-gray-100 cursor-pointer flex items-center justify-between border-t border-gray-200"
                onClick={() => navigateToMediaPage({ type: 'saveToAlbum' })}
              >
                <div className="text-sm text-gray-700">Save to Album</div>
                <div className="w-4 h-4 text-gray-400">
                  <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5">
                    <path d="M9 18l6-6-6-6"/>
                  </svg>
                </div>
              </div>
            </div>
          )}
        </div>

        {/* Document - Clickable */}
        <div className="bg-white rounded-lg shadow-sm">
          <div
            className="px-4 py-4 flex items-center justify-between cursor-pointer hover:bg-gray-50"
            onClick={navigateToDocumentPage}
          >
            <div className="text-base text-gray-900">Document</div>
            <div className="w-6 h-6 text-gray-400">
              <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5">
                <path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z"/>
                <polyline points="14,2 14,8 20,8"/>
              </svg>
            </div>
          </div>
        </div>

        {/* Location - Clickable */}
        <div className="bg-white rounded-lg shadow-sm">
          <div
            className="px-4 py-4 flex items-center justify-between cursor-pointer hover:bg-gray-50"
            onClick={navigateToLocationPage}
          >
            <div className="text-base text-gray-900">Location</div>
            <div className="w-6 h-6 text-gray-400">
              <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5">
                <path d="M21 10c0 7-9 13-9 13s-9-6-9-13a9 9 0 0 1 18 0z"/>
                <circle cx="12" cy="10" r="3"/>
              </svg>
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}
