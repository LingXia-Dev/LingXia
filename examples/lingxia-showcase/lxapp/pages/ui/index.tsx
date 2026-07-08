import React from 'react';
import { useLxPage } from '@lingxia/react';
import '../../tailwind.css';

// Parse a surface size input. Blank means "let the Host pick the default size";
// a bare number is absolute px ("320"); a `%` suffix is a percentage of the
// container ("80%"). Non-blank but unparseable input — a stray letter, a
// full-width "％", a non-positive or out-of-range value — throws so the demo
// reports the mistake instead of silently dropping the dimension (which would
// e.g. quietly turn a 100%/100% float into a centered, default-size one).
function parseSurfaceSize(raw: string, label: string): number | string | undefined {
  const value = raw.trim();
  if (!value) return undefined;
  if (value.endsWith('%')) {
    const pct = Number(value.slice(0, -1).trim());
    if (!Number.isFinite(pct) || pct <= 0 || pct > 100) {
      throw new Error(`${label} must be a percentage between 1% and 100% (got "${value}")`);
    }
    return `${pct}%`;
  }
  const px = Number(value);
  if (!Number.isFinite(px) || px <= 0) {
    throw new Error(`${label} must be a positive px value or a percentage like "80%" (got "${value}")`);
  }
  return px;
}

export default function UIPage() {
  // Use LingXia hook to get data and functions
  const { data, actions } = useLxPage();
  const {
    demoNavigateTo,
    demoNavigateBack,
    demoSwitchTab,
    demoRedirectTo,
    showToastWithParams,
    hideToast,
    showModalWithParams,
    clearModalResult,
    setNavigationBarTitle,
    setNavigationBarColor,
    showTabBarRedDot,
    hideTabBarRedDot,
    setTabBarBadge,
    removeTabBarBadge,
    showTabBar,
    hideTabBar,
    setTabBarStyle,
    setTabBarItem,
    chooseToastIcon,
    chooseToastPosition,
    showDemoActionSheet,
    openSurfaceDemo,
    showActiveSurface,
    hideActiveSurface,
    closeActiveSurface,
  } = actions;
  const {
    currentType = 'navigation',
    pageStack = [],
    modalResult = null,
    toastIcon = 'success',
    toastIconLabel = 'Success',
    toastIconOptions = [],
    toastPosition = 'center',
    toastPositionLabel = 'Center',
    toastPositionOptions = [],
    surfaceDemo = {},
  } = data;

  const toastIconDisplay = React.useMemo(() => {
    const match = toastIconOptions.find((option) => option.value === toastIcon);
    return match?.label || toastIconLabel || toastIcon || 'Select icon';
  }, [toastIconOptions, toastIcon, toastIconLabel]);

  const toastPositionDisplay = React.useMemo(() => {
    const match = toastPositionOptions.find((option) => option.value === toastPosition);
    return match?.label || toastPositionLabel || toastPosition || 'Select position';
  }, [toastPositionOptions, toastPosition, toastPositionLabel]);

  const surfaceMessage = (surfaceDemo && surfaceDemo.message) || '';
  const surfaceActive = surfaceDemo?.active === true;
  const surfaceVisible = surfaceDemo?.visible === true;
  const [surfaceKind, setSurfaceKind] = React.useState<'aside' | 'float' | 'window'>('aside');
  const surfaceKinds: Array<{ id: 'aside' | 'float' | 'window'; label: string; hint: string }> = [
    { id: 'aside', label: 'Aside', hint: 'Docks beside the main and splits it; a compact window folds it into a switchable tab.' },
    { id: 'float', label: 'Float', hint: 'A popup above the main; it does not take layout space (like a dialog).' },
    { id: 'window', label: 'Window', hint: 'A bare standalone window — no sidebar, no shell. Desktop only.' },
  ];
  const [surfaceEdge, setSurfaceEdge] = React.useState<'left' | 'right' | 'top' | 'bottom'>('right');
  const surfaceEdges: Array<'left' | 'right' | 'top' | 'bottom'> = ['left', 'right', 'top', 'bottom'];
  const [surfaceFloatPosition, setSurfaceFloatPosition] = React.useState<'center' | 'top' | 'bottom' | 'left' | 'right'>('center');
  const surfaceFloatPositions: Array<'center' | 'top' | 'bottom' | 'left' | 'right'> = ['center', 'top', 'bottom', 'left', 'right'];
  const [surfaceWidth, setSurfaceWidth] = React.useState('');
  const [surfaceHeight, setSurfaceHeight] = React.useState('');
  // Shown when an entered width/height can't be parsed (so a typo like a
  // full-width "％" surfaces instead of silently opening at the wrong size).
  const [sizeError, setSizeError] = React.useState('');

  // Local state for toast parameters
  const [toastTitle, setToastTitle] = React.useState('Hello Toast!');
  const [toastDuration, setToastDuration] = React.useState(2000);
  const [toastMask, setToastMask] = React.useState(false);

  // Local state for modal parameters
  const [modalTitle, setModalTitle] = React.useState('Alert');
  const [modalContent, setModalContent] = React.useState('This is a modal dialog');
  const [modalShowCancel, setModalShowCancel] = React.useState(true);
  const [modalCancelText, setModalCancelText] = React.useState('Cancel');
  const [modalConfirmText, setModalConfirmText] = React.useState('OK');

  // Local state for TabBar parameters - fixed to tab 1
  const [badgeText, setBadgeText] = React.useState('99');
  const [itemText, setItemText] = React.useState('New Tab');
  const [itemIcon, setItemIcon] = React.useState('');
  const [selectedIcon, setSelectedIcon] = React.useState('');
  const [color, setColor] = React.useState('#666666');
  const [selectedColor, setSelectedColor] = React.useState('#007AFF');
  const [backgroundColor, setBackgroundColor] = React.useState('#FFFFFF');
  const [borderStyle, setBorderStyle] = React.useState('#EEEEEE');


  return (
    <div className="h-screen bg-gradient-to-br from-gray-50 to-gray-100 flex flex-col overflow-y-auto">
      <div className="flex-1 overflow-y-auto">
        <div className="pb-6 px-4 pt-6">

        {/* Navigation Demo Section */}
        {currentType === 'navigation' && (
          <>
            <div className="mb-4 text-sm text-gray-600 font-semibold">navigateTo/Back, redirectTo</div>

        <div className="mb-5 bg-white rounded-2xl shadow-sm border border-gray-100 overflow-hidden">
          <div
            className="flex items-center justify-between px-5 py-4 hover:bg-gradient-to-r hover:from-blue-50/50 hover:to-transparent cursor-pointer border-b border-gray-100 transition-all active:scale-[0.99]"
            onClick={demoNavigateTo}
          >
            <div className="text-sm text-gray-800 font-medium">Navigate to new page</div>
            <span className="text-gray-400 text-lg">›</span>
          </div>
          <div
            className="flex items-center justify-between px-5 py-4 hover:bg-gradient-to-r hover:from-blue-50/50 hover:to-transparent cursor-pointer border-b border-gray-100 transition-all active:scale-[0.99]"
            onClick={demoNavigateBack}
          >
            <div className="text-sm text-gray-800 font-medium">Back to previous page</div>
            <span className="text-gray-400 text-lg">›</span>
          </div>
          <div
            className="flex items-center justify-between px-5 py-4 hover:bg-gradient-to-r hover:from-blue-50/50 hover:to-transparent cursor-pointer border-b border-gray-100 transition-all active:scale-[0.99]"
            onClick={demoRedirectTo}
          >
            <div className="text-sm text-gray-800 font-medium">Open in current page</div>
            <span className="text-gray-400 text-lg">›</span>
          </div>
          <div
            className="flex items-center justify-between px-5 py-4 hover:bg-gradient-to-r hover:from-blue-50/50 hover:to-transparent cursor-pointer transition-all active:scale-[0.99]"
            onClick={demoSwitchTab}
          >
            <div className="text-sm text-gray-800 font-medium">Jump to Tab page</div>
            <span className="text-gray-400 text-lg">›</span>
          </div>
        </div>
          </>
        )}

        {/* Surface Demo Section */}
        {currentType === 'surface' && (
          <>
            <div className="mt-4 mb-6 px-4 text-center">
              <h1 className="text-2xl font-light text-gray-800 mb-2">lx.surface</h1>
              <div className="w-16 h-0.5 bg-gray-400 mx-auto"></div>
            </div>

            <div className="mx-1 mb-4 bg-white rounded-xl shadow-sm border border-gray-200 overflow-hidden">
              <div className="px-4 py-4 space-y-4">
                <div className="space-y-3">
                  {/* Pick the surface kind first; the relevant placement
                      control (edge / position) appears for that kind. */}
                  <div>
                    <div className="text-xs uppercase text-gray-500 tracking-wide mb-2">Kind</div>
                    <div className="grid grid-cols-3 gap-2">
                      {surfaceKinds.map((kind) => {
                        const active = surfaceKind === kind.id;
                        const baseClass = 'py-2 text-sm rounded-lg transition-colors border';
                        const className = active
                          ? `${baseClass} bg-gray-800 border-gray-800 text-white`
                          : `${baseClass} bg-white border-gray-200 text-gray-600 hover:bg-gray-50`;
                        return (
                          <button
                            key={kind.id}
                            type="button"
                            disabled={surfaceActive}
                            className={`${className} disabled:opacity-50 disabled:cursor-not-allowed`}
                            onClick={() => setSurfaceKind(kind.id)}
                          >
                            {kind.label}
                          </button>
                        );
                      })}
                    </div>
                    <div className="mt-2 text-xs text-gray-500 leading-5 bg-gray-50 rounded-lg px-3 py-2">
                      {surfaceKinds.find((k) => k.id === surfaceKind)?.hint}
                    </div>
                  </div>

                  {surfaceKind === 'aside' && (
                    <div>
                      <div className="text-xs uppercase text-gray-500 tracking-wide mb-2">Edge</div>
                      {/* Which side the aside docks to. */}
                      <div className="grid grid-cols-2 gap-2">
                        {surfaceEdges.map((edge) => {
                          const active = surfaceEdge === edge;
                          const baseClass = 'py-2 text-sm rounded-lg transition-colors border';
                          const className = active
                            ? `${baseClass} bg-blue-500 border-blue-500 text-white`
                            : `${baseClass} bg-white border-gray-200 text-gray-600 hover:bg-gray-50`;
                          return (
                            <button
                              key={edge}
                              type="button"
                              className={className}
                              onClick={() => setSurfaceEdge(edge)}
                            >
                              {edge.charAt(0).toUpperCase() + edge.slice(1)}
                            </button>
                          );
                        })}
                      </div>
                    </div>
                  )}

                  {surfaceKind === 'float' && (
                    <div>
                      <div className="text-xs uppercase text-gray-500 tracking-wide mb-2">Position</div>
                      {/* Where the float popup sits above the main. */}
                      <div className="grid grid-cols-2 gap-2">
                        {surfaceFloatPositions.map((position) => {
                          const active = surfaceFloatPosition === position;
                          const baseClass = 'py-2 text-sm rounded-lg transition-colors border';
                          const className = active
                            ? `${baseClass} bg-indigo-500 border-indigo-500 text-white`
                            : `${baseClass} bg-white border-gray-200 text-gray-600 hover:bg-gray-50`;
                          return (
                            <button
                              key={position}
                              type="button"
                              className={className}
                              onClick={() => setSurfaceFloatPosition(position)}
                            >
                              {position.charAt(0).toUpperCase() + position.slice(1)}
                            </button>
                          );
                        })}
                      </div>
                    </div>
                  )}
                  <div>
                    <div className="text-xs uppercase text-gray-500 tracking-wide mb-2">Size (optional — px or %)</div>
                    {/* Preferred-size hint; the Host may clamp/override it. A
                        percentage (e.g. 80%) is relative to the container; a
                        bare number is absolute px. */}
                    <div className="grid grid-cols-2 gap-2">
                      <input
                        type="text"
                        inputMode="text"
                        placeholder="width (px or %)"
                        value={surfaceWidth}
                        onChange={(e) => {
                          setSurfaceWidth(e.target.value);
                          setSizeError('');
                        }}
                        className="py-2 px-3 text-sm rounded-lg border border-gray-200 focus:outline-none focus:ring-2 focus:ring-gray-400"
                      />
                      <input
                        type="text"
                        inputMode="text"
                        placeholder="height (px or %)"
                        value={surfaceHeight}
                        onChange={(e) => {
                          setSurfaceHeight(e.target.value);
                          setSizeError('');
                        }}
                        className="py-2 px-3 text-sm rounded-lg border border-gray-200 focus:outline-none focus:ring-2 focus:ring-gray-400"
                      />
                    </div>
                    {sizeError && (
                      <div data-testid="size-error" className="mt-2 text-xs text-rose-600">
                        {sizeError}
                      </div>
                    )}
                  </div>
                </div>

                <button
                  type="button"
                  data-testid="open-surface"
                  disabled={surfaceActive}
                  onClick={() => {
                    let width: number | string | undefined;
                    let height: number | string | undefined;
                    try {
                      width = parseSurfaceSize(surfaceWidth, 'Width');
                      height = parseSurfaceSize(surfaceHeight, 'Height');
                    } catch (error) {
                      setSizeError(error instanceof Error ? error.message : String(error));
                      return;
                    }
                    setSizeError('');
                    openSurfaceDemo({
                      verb: surfaceKind,
                      edge: surfaceEdge,
                      position: surfaceFloatPosition,
                      width,
                      height,
                    });
                  }}
                  className="w-full bg-gray-800 hover:bg-gray-900 disabled:bg-gray-300 disabled:cursor-not-allowed text-white py-2 px-4 rounded-lg text-sm font-medium transition-colors"
                >
                  {surfaceActive
                    ? `Open ${surfaceKind} (already open)`
                    : `Open ${surfaceKind}`}
                </button>

                <p className="text-xs text-gray-500">
                  Edge / position is applied when the surface opens. Changing it
                  while a surface is open — or across hide → show — has no effect;
                  close and re-open to apply a new value.
                </p>

                {surfaceActive && (
                  <div className="grid grid-cols-3 gap-2">
                    <button
                      type="button"
                      disabled={surfaceVisible}
                      onClick={() => showActiveSurface()}
                      className="bg-emerald-500 hover:bg-emerald-600 disabled:bg-gray-200 disabled:text-gray-500 text-white py-2 px-3 rounded-lg text-sm font-medium transition-colors"
                    >
                      Show
                    </button>
                    <button
                      type="button"
                      disabled={!surfaceVisible}
                      onClick={() => hideActiveSurface()}
                      className="bg-amber-500 hover:bg-amber-600 disabled:bg-gray-200 disabled:text-gray-500 text-white py-2 px-3 rounded-lg text-sm font-medium transition-colors"
                    >
                      Hide
                    </button>
                    <button
                      type="button"
                      onClick={() => closeActiveSurface()}
                      className="bg-rose-500 hover:bg-rose-600 text-white py-2 px-3 rounded-lg text-sm font-medium transition-colors"
                    >
                      Close
                    </button>
                  </div>
                )}

                <div className="text-xs text-gray-500 uppercase tracking-wide">Surface status</div>
                <div className="text-sm text-gray-800 bg-gray-50 rounded-lg px-3 py-2 font-mono break-words">
                  {surfaceMessage || 'No message received yet.'}
                </div>
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
                  <button
                    type="button"
                    onClick={chooseToastIcon}
                    className="w-full px-3 py-2 border border-gray-300 rounded-md flex items-center justify-between text-left text-gray-800 focus:outline-none focus:ring-2 focus:ring-blue-500"
                  >
                    <span>{toastIconDisplay}</span>
                    <span className="text-xs text-blue-500">Change</span>
                  </button>
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
                  <button
                    type="button"
                    onClick={chooseToastPosition}
                    className="w-full px-3 py-2 border border-gray-300 rounded-md flex items-center justify-between text-left text-gray-800 focus:outline-none focus:ring-2 focus:ring-blue-500"
                  >
                    <span>{toastPositionDisplay}</span>
                    <span className="text-xs text-blue-500">Change</span>
                  </button>
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

        {/* ActionSheet Demo Section */}
        {currentType === 'actionsheet' && (
          <div className="mx-1 mt-8 bg-white rounded-xl shadow-sm border border-gray-200 overflow-hidden">
            <div
              className="px-4 py-10 text-base text-blue-600 font-medium text-center cursor-pointer hover:bg-blue-50"
              onClick={showDemoActionSheet}
            >
              Show Action Sheet
            </div>
          </div>
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
          <div className="mb-5 bg-white rounded-2xl shadow-sm border border-gray-100 overflow-hidden">
            <div className="px-5 py-4">
              <div className="flex items-center gap-2 mb-4">
                <span className="w-1 h-5 bg-blue-500 rounded-full"></span>
                <div className="text-sm font-semibold text-gray-700">Current Page Stack</div>
                <span className="ml-auto px-2 py-1 bg-blue-50 text-blue-600 text-xs font-semibold rounded-full">
                  {pageStack.length}
                </span>
              </div>
              <div className="space-y-2">
                {pageStack.map((page, index) => (
                  <div key={index} className="flex flex-col gap-2 py-3 px-4 bg-gradient-to-r from-gray-50 to-white rounded-xl border border-gray-100">
                    <div className="flex items-center gap-3">
                      <span className="flex items-center justify-center w-6 h-6 rounded-full bg-blue-100 text-blue-600 text-xs font-bold">
                        {page.index + 1}
                      </span>
                      <span className="text-sm text-gray-800 font-medium flex-1 truncate">{page.route}</span>
                    </div>
                    {Object.keys(page.options).length > 0 && (
                      <div className="ml-9 text-xs text-gray-500 font-mono bg-gray-50 px-3 py-2 rounded-lg break-all">
                        {JSON.stringify(page.options, null, 2)}
                      </div>
                    )}
                  </div>
                ))}
                {pageStack.length === 0 && (
                  <div className="text-sm text-gray-500 text-center py-8">No page stack available</div>
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
                          backgroundColor: bgColor,
                          frontColor: textColor
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
                        setNavigationBarColor({ backgroundColor: "#1f2937", frontColor: "#ffffff" });
                      }}
                      className="px-2 py-1.5 bg-gray-800 text-white rounded hover:bg-gray-900 text-xs"
                    >
                      Dark
                    </button>
                    <button
                      onClick={() => {
                        setNavigationBarTitle({ title: "Blue Theme" });
                        setNavigationBarColor({ backgroundColor: "#3b82f6", frontColor: "#ffffff" });
                      }}
                      className="px-2 py-1.5 bg-blue-500 text-white rounded hover:bg-blue-600 text-xs"
                    >
                      Blue
                    </button>
                    <button
                      onClick={() => {
                        setNavigationBarTitle({ title: "Light Theme" });
                        setNavigationBarColor({ backgroundColor: "#ffffff", frontColor: "#000000" });
                      }}
                      className="px-2 py-1.5 bg-white text-black border border-gray-300 rounded hover:bg-gray-50 text-xs"
                    >
                      Light
                    </button>
                    <button
                      onClick={() => {
                        setNavigationBarTitle({ title: "Green Theme" });
                        setNavigationBarColor({ backgroundColor: "#10b981", frontColor: "#ffffff" });
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

        {/* TabBar Demo Section */}
        {currentType === 'tabbar' && (
          <>
            <div className="mt-4 mb-3 px-4 text-sm text-gray-500 font-medium">TabBar APIs</div>

            {/* Visibility Controls */}
            <div className="mx-1 mb-4 bg-white rounded-xl shadow-sm border border-gray-200 overflow-hidden">
              <div className="px-4 py-3 border-b border-gray-100">
                <h3 className="text-base font-medium text-gray-900">Visibility Controls</h3>
                <p className="text-sm text-gray-500 mt-1">Show/hide TabBar and update item text</p>
              </div>
              <div className="p-4 space-y-4">
                <div className="flex space-x-3">
                  <button
                    onClick={async () => {
                      const result = await showTabBar();
                      console.log('Show TabBar:', result);
                    }}
                    className="flex-1 bg-green-500 hover:bg-green-600 text-white py-2 px-4 rounded-lg text-sm font-medium transition-colors"
                  >
                    Show TabBar
                  </button>
                  <button
                    onClick={async () => {
                      const result = await hideTabBar();
                      console.log('Hide TabBar:', result);
                    }}
                    className="flex-1 bg-red-500 hover:bg-red-600 text-white py-2 px-4 rounded-lg text-sm font-medium transition-colors"
                  >
                    Hide TabBar
                  </button>
                </div>

                {/* Item Text Control */}
                <div className="pt-2 border-t border-gray-100">
                  <label className="block text-sm font-medium text-gray-700 mb-2">
                    Update Tab 1 Text
                  </label>
                  <div className="flex space-x-2">
                    <input
                      type="text"
                      value={itemText}
                      onChange={(e) => setItemText(e.target.value)}
                      className="flex-1 px-3 py-2 border border-gray-300 rounded-lg focus:outline-none focus:ring-2 focus:ring-blue-500"
                      placeholder="Enter new text"
                    />
                    <button
                      onClick={() => {
                        const result = setTabBarItem({ index: 1, text: itemText });
                        console.log(`Update tab 1 text to "${itemText}":`, result);
                      }}
                      className="px-4 py-2 bg-blue-500 text-white rounded-lg hover:bg-blue-600 transition-colors"
                    >
                      Update
                    </button>
                  </div>
                </div>
              </div>
            </div>



            {/* Red Dot Controls */}
            <div className="mx-1 mb-4 bg-white rounded-xl shadow-sm border border-gray-200 overflow-hidden">
              <div className="px-4 py-3 border-b border-gray-100">
                <h3 className="text-base font-medium text-gray-900">Red Dot Controls</h3>
                <p className="text-sm text-gray-500 mt-1">Show/hide red dot on tab 1</p>
              </div>
              <div className="p-4">
                <div className="flex space-x-3">
                  <button
                    onClick={() => {
                      const result = showTabBarRedDot({ index: 1 });
                      console.log('Show red dot on tab 1:', result);
                    }}
                    className="flex-1 bg-red-500 hover:bg-red-600 text-white py-2 px-4 rounded-lg text-sm font-medium transition-colors"
                  >
                    Show Red Dot
                  </button>
                  <button
                    onClick={() => {
                      const result = hideTabBarRedDot({ index: 1 });
                      console.log('Hide red dot on tab 1:', result);
                    }}
                    className="flex-1 bg-gray-500 hover:bg-gray-600 text-white py-2 px-4 rounded-lg text-sm font-medium transition-colors"
                  >
                    Hide Red Dot
                  </button>
                </div>
              </div>
            </div>

            {/* Badge Controls */}
            <div className="mx-1 mb-4 bg-white rounded-xl shadow-sm border border-gray-200 overflow-hidden">
              <div className="px-4 py-3 border-b border-gray-100">
                <h3 className="text-base font-medium text-gray-900">Badge Controls</h3>
                <p className="text-sm text-gray-500 mt-1">Set/remove badge on tab 1</p>
              </div>
              <div className="p-4 space-y-3">
                <div>
                  <label className="block text-sm font-medium text-gray-700 mb-1">Badge Text</label>
                  <input
                    type="text"
                    value={badgeText}
                    onChange={(e) => setBadgeText(e.target.value)}
                    className="w-full px-3 py-2 border border-gray-300 rounded-lg text-sm focus:outline-none focus:ring-2 focus:ring-blue-500"
                    placeholder="Enter badge text"
                  />
                </div>
                <div className="flex space-x-3">
                  <button
                    onClick={() => {
                      const result = setTabBarBadge({ index: 1, text: badgeText });
                      console.log(`Set badge "${badgeText}" on tab 1:`, result);
                    }}
                    className="flex-1 bg-orange-500 hover:bg-orange-600 text-white py-2 px-4 rounded-lg text-sm font-medium transition-colors"
                  >
                    Set Badge
                  </button>
                  <button
                    onClick={() => {
                      const result = removeTabBarBadge({ index: 1 });
                      console.log('Remove badge on tab 1:', result);
                    }}
                    className="flex-1 bg-gray-500 hover:bg-gray-600 text-white py-2 px-4 rounded-lg text-sm font-medium transition-colors"
                  >
                    Remove Badge
                  </button>
                </div>
              </div>
            </div>



            {/* Style Controls */}
            <div className="mx-1 mb-4 bg-white rounded-xl shadow-sm border border-gray-200 overflow-hidden">
              <div className="px-4 py-3 border-b border-gray-100">
                <h3 className="text-base font-medium text-gray-900">Style Controls</h3>
                <p className="text-sm text-gray-500 mt-1">Customize TabBar appearance</p>
              </div>
              <div className="p-4 space-y-3">
                <div className="grid grid-cols-2 gap-3">
                  <div>
                    <label className="block text-sm font-medium text-gray-700 mb-1">Text Color</label>
                    <div className="flex items-center space-x-2">
                      <div
                        className="w-8 h-8 border border-gray-300 rounded cursor-pointer"
                        style={{ backgroundColor: color }}
                      ></div>
                      <input
                        type="text"
                        value={color}
                        onChange={(e) => setColor(e.target.value)}
                        className="flex-1 px-2 py-1 border border-gray-300 rounded text-sm"
                        placeholder="#666666"
                      />
                    </div>
                  </div>
                  <div>
                    <label className="block text-sm font-medium text-gray-700 mb-1">Selected Color</label>
                    <div className="flex items-center space-x-2">
                      <div
                        className="w-8 h-8 border border-gray-300 rounded cursor-pointer"
                        style={{ backgroundColor: selectedColor }}
                      ></div>
                      <input
                        type="text"
                        value={selectedColor}
                        onChange={(e) => setSelectedColor(e.target.value)}
                        className="flex-1 px-2 py-1 border border-gray-300 rounded text-sm"
                        placeholder="#007AFF"
                      />
                    </div>
                  </div>
                  <div>
                    <label className="block text-sm font-medium text-gray-700 mb-1">Background</label>
                    <div className="flex items-center space-x-2">
                      <div
                        className="w-8 h-8 border border-gray-300 rounded cursor-pointer"
                        style={{ backgroundColor: backgroundColor }}
                      ></div>
                      <input
                        type="text"
                        value={backgroundColor}
                        onChange={(e) => setBackgroundColor(e.target.value)}
                        className="flex-1 px-2 py-1 border border-gray-300 rounded text-sm"
                        placeholder="#FFFFFF"
                      />
                    </div>
                  </div>
                  <div>
                    <label className="block text-sm font-medium text-gray-700 mb-1">Border</label>
                    <div className="flex items-center space-x-2">
                      <div
                        className="w-8 h-8 border border-gray-300 rounded cursor-pointer"
                        style={{ backgroundColor: borderStyle }}
                      ></div>
                      <input
                        type="text"
                        value={borderStyle}
                        onChange={(e) => setBorderStyle(e.target.value)}
                        className="flex-1 px-2 py-1 border border-gray-300 rounded text-sm"
                        placeholder="#EEEEEE"
                      />
                    </div>
                  </div>
                </div>

                <button
                  onClick={() => {
                    const result = setTabBarStyle({
                      color,
                      selectedColor,
                      backgroundColor,
                      borderStyle
                    });
                    console.log('Set TabBar style:', result);
                  }}
                  className="w-full bg-blue-500 hover:bg-blue-600 text-white py-2 px-4 rounded-lg text-sm font-medium transition-colors"
                >
                  Apply Custom Style
                </button>

                {/* Preset Themes */}
                <div className="mt-4">
                  <label className="block text-sm font-medium text-gray-700 mb-2">Preset Themes</label>
                  <div className="grid grid-cols-2 gap-2">
                    <button
                      onClick={() => {
                        const theme = { color: '#666666', selectedColor: '#007AFF', backgroundColor: '#FFFFFF', borderStyle: '#EEEEEE' };
                        setColor(theme.color);
                        setSelectedColor(theme.selectedColor);
                        setBackgroundColor(theme.backgroundColor);
                        setBorderStyle(theme.borderStyle);
                        const result = setTabBarStyle(theme);
                        console.log('Applied Default theme:', result);
                      }}
                      className="px-3 py-2 bg-gray-100 hover:bg-gray-200 text-gray-700 rounded-lg text-sm font-medium transition-colors"
                    >
                      Default
                    </button>
                    <button
                      onClick={() => {
                        const theme = { color: '#CCCCCC', selectedColor: '#0A84FF', backgroundColor: '#1C1C1E', borderStyle: '#38383A' };
                        setColor(theme.color);
                        setSelectedColor(theme.selectedColor);
                        setBackgroundColor(theme.backgroundColor);
                        setBorderStyle(theme.borderStyle);
                        const result = setTabBarStyle(theme);
                        console.log('Applied Dark theme:', result);
                      }}
                      className="px-3 py-2 bg-gray-800 hover:bg-gray-900 text-white rounded-lg text-sm font-medium transition-colors"
                    >
                      Dark
                    </button>
                    <button
                      onClick={() => {
                        const theme = { color: '#8E8E93', selectedColor: '#34C759', backgroundColor: '#F2F2F7', borderStyle: '#C6C6C8' };
                        setColor(theme.color);
                        setSelectedColor(theme.selectedColor);
                        setBackgroundColor(theme.backgroundColor);
                        setBorderStyle(theme.borderStyle);
                        const result = setTabBarStyle(theme);
                        console.log('Applied Green theme:', result);
                      }}
                      className="px-3 py-2 bg-green-100 hover:bg-green-200 text-green-700 rounded-lg text-sm font-medium transition-colors"
                    >
                      Green
                    </button>
                    <button
                      onClick={() => {
                        const theme = { color: '#8E8E93', selectedColor: '#AF52DE', backgroundColor: '#F2F2F7', borderStyle: '#C6C6C8' };
                        setColor(theme.color);
                        setSelectedColor(theme.selectedColor);
                        setBackgroundColor(theme.backgroundColor);
                        setBorderStyle(theme.borderStyle);
                        const result = setTabBarStyle(theme);
                        console.log('Applied Purple theme:', result);
                      }}
                      className="px-3 py-2 bg-purple-100 hover:bg-purple-200 text-purple-700 rounded-lg text-sm font-medium transition-colors"
                    >
                      Purple
                    </button>
                  </div>
                </div>
              </div>
            </div>





          </>
        )}

        </div>
      </div>
    </div>
  );
}
