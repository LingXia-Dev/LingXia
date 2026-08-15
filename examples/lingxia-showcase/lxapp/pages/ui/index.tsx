import React from 'react';
import { useLxPage } from '@lingxia/react';
import { PageStackCard } from '../../shared/components/page-stack';
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
    bumpLogicCounter,
    showToastWithParams,
    hideToast,
    showModalWithParams,
    clearModalResult,
    updateNavigationBarTitle,
    updateNavigationBarColors,
    enableTabBarRedDot,
    disableTabBarRedDot,
    updateTabBarBadge,
    clearTabBarBadge,
    revealTabBar,
    concealTabBar,
    updateTabBarForegrounds,
    updateTabBarItem,
    setAppearance,
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
    instanceTag = '',
    previousInstanceTag = '',
    logicCounter = 0,
    events = [],
    modalResult = null,
    toastIcon = 'success',
    toastIconLabel = 'Success',
    toastIconOptions = [],
    toastPosition = 'center',
    toastPositionLabel = 'Center',
    toastPositionOptions = [],
    surfaceDemo = {},
    chromeError = '',
    appearance = { preference: 'auto', resolved: 'light' },
  } = data;

  // View-local state: never leaves the WebView, so only a document reload
  // (a fresh instance) clears it.
  const [viewCounter, setViewCounter] = React.useState(0);
  const [popupOpen, setPopupOpen] = React.useState(false);

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
  const [surfaceKind, setSurfaceKind] = React.useState<'aside' | 'float' | 'window' | 'lxapp'>('aside');
  const surfaceKinds: Array<{ id: 'aside' | 'float' | 'window' | 'lxapp'; label: string; hint: string }> = [
    { id: 'aside', label: 'Aside', hint: 'Docks beside the main and splits it; a compact window folds it into a switchable tab.' },
    { id: 'float', label: 'Float', hint: 'A popup above the main; it does not take layout space (like a dialog).' },
    { id: 'window', label: 'Window', hint: 'A bare standalone window — no sidebar, no shell. Desktop only.' },
    { id: 'lxapp', label: 'Lxapp', hint: 'Opens another lxapp (the chat app) docked as an aside. Home-app privilege.' },
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


  return (
    <div className="h-screen bg-linear-to-br from-surface-50 to-surface-100 flex flex-col overflow-y-auto">
      <div className="flex-1 overflow-y-auto">
        <div className="pb-6 px-4 pt-6">
        {chromeError && (
          <div className="mb-4 rounded-lg border border-red-200 bg-red-50 px-3 py-2 text-sm text-red-700 dark:text-red-400">
            {chromeError}
          </div>
        )}

        {/* Navigation Demo Section */}
        {currentType === 'navigation' && (
          <>
            <div className="mb-4 text-sm text-gray-600 font-semibold">navigateTo/Back, redirectTo</div>

        <div className="mb-5 bg-surface rounded-2xl shadow-sm border border-line-100 overflow-hidden">
          <div
            data-testid="ui-navigate-to"
            className="flex items-center justify-between px-5 py-4 hover:bg-linear-to-r hover:from-blue-50/50 hover:to-transparent cursor-pointer border-b border-line-100 transition-all active:scale-[0.99]"
            onClick={demoNavigateTo}
          >
            <div>
              <div className="text-sm text-gray-800 font-medium">Push this page again</div>
              <div className="text-xs text-gray-500 mt-0.5">navigateTo → this route; every entry is a fresh instance, watch the stack grow</div>
            </div>
            <span className="text-gray-400 text-lg">›</span>
          </div>
          <div
            data-testid="ui-navigate-back"
            className="flex items-center justify-between px-5 py-4 hover:bg-linear-to-r hover:from-blue-50/50 hover:to-transparent cursor-pointer border-b border-line-100 transition-all active:scale-[0.99]"
            onClick={demoNavigateBack}
          >
            <div className="text-sm text-gray-800 font-medium">Back to previous page</div>
            <span className="text-gray-400 text-lg">›</span>
          </div>
          <div
            data-testid="ui-redirect-to"
            className="flex items-center justify-between px-5 py-4 hover:bg-linear-to-r hover:from-blue-50/50 hover:to-transparent cursor-pointer border-b border-line-100 transition-all active:scale-[0.99]"
            onClick={demoRedirectTo}
          >
            <div className="text-sm text-gray-800 font-medium">Replace this page (redirectTo)</div>
            <span className="text-gray-400 text-lg">›</span>
          </div>
          <div
            data-testid="ui-switch-tab"
            className="flex items-center justify-between px-5 py-4 hover:bg-linear-to-r hover:from-blue-50/50 hover:to-transparent cursor-pointer transition-all active:scale-[0.99]"
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
              <div className="w-16 h-0.5 bg-surface-400 mx-auto"></div>
            </div>

            <div className="mx-1 mb-4 bg-surface rounded-xl shadow-sm border border-line-200 overflow-hidden">
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
                          ? `${baseClass} bg-surface-800 border-line-800 text-white`
                          : `${baseClass} bg-surface border-line-200 text-gray-600 hover:bg-surface-50`;
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
                    <div className="mt-2 text-xs text-gray-500 leading-5 bg-surface-50 rounded-lg px-3 py-2">
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
                            : `${baseClass} bg-surface border-line-200 text-gray-600 hover:bg-surface-50`;
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
                            : `${baseClass} bg-surface border-line-200 text-gray-600 hover:bg-surface-50`;
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
                        className="py-2 px-3 text-sm rounded-lg border border-line-200 focus:outline-hidden focus:ring-2 focus:ring-surface-400"
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
                        className="py-2 px-3 text-sm rounded-lg border border-line-200 focus:outline-hidden focus:ring-2 focus:ring-surface-400"
                      />
                    </div>
                    {sizeError && (
                      <div data-testid="size-error" className="mt-2 text-xs text-rose-600 dark:text-rose-400">
                        {sizeError}
                      </div>
                    )}
                  </div>
                </div>

                <button
                  type="button"
                  data-testid="open-surface"
                  data-surface-width={surfaceWidth}
                  data-surface-height={surfaceHeight}
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
                  className="w-full bg-surface-800 hover:bg-surface-900 disabled:bg-surface-300 disabled:cursor-not-allowed text-white py-2 px-4 rounded-lg text-sm font-medium transition-colors"
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
                      className="bg-emerald-500 hover:bg-emerald-600 disabled:bg-surface-200 disabled:text-gray-500 text-white py-2 px-3 rounded-lg text-sm font-medium transition-colors"
                    >
                      Show
                    </button>
                    <button
                      type="button"
                      disabled={!surfaceVisible}
                      onClick={() => hideActiveSurface()}
                      className="bg-amber-500 hover:bg-amber-600 disabled:bg-surface-200 disabled:text-gray-500 text-white py-2 px-3 rounded-lg text-sm font-medium transition-colors"
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
                <div className="text-sm text-gray-800 bg-surface-50 rounded-lg px-3 py-2 font-mono break-words">
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
            <div className="mx-1 mb-4 bg-surface rounded-xl shadow-sm border border-line-200 overflow-hidden">
              <div className="px-3 py-3 space-y-3">

                {/* Title Input */}
                <div>
                  <label className="block text-sm font-medium text-gray-700 mb-2">Title</label>
                  <input
                    type="text"
                    value={toastTitle}
                    onChange={(e) => setToastTitle(e.target.value)}
                    className="w-full px-3 py-2 border border-line-300 rounded-md focus:outline-hidden focus:ring-2 focus:ring-blue-500"
                    placeholder="Enter toast title"
                  />
                </div>

                {/* Icon Selection */}
                <div>
                  <label className="block text-sm font-medium text-gray-700 mb-2">Icon</label>
                  <button
                    type="button"
                    onClick={chooseToastIcon}
                    className="w-full px-3 py-2 border border-line-300 rounded-md flex items-center justify-between text-left text-gray-800 focus:outline-hidden focus:ring-2 focus:ring-blue-500"
                  >
                    <span>{toastIconDisplay}</span>
                    <span className="text-xs text-blue-500 dark:text-blue-400">Change</span>
                  </button>
                </div>

                {/* Duration Input */}
                <div>
                  <label className="block text-sm font-medium text-gray-700 mb-2">Duration (ms)</label>
                  <input
                    type="number"
                    value={toastDuration}
                    onChange={(e) => setToastDuration(parseInt(e.target.value) || 2000)}
                    className="w-full px-3 py-2 border border-line-300 rounded-md focus:outline-hidden focus:ring-2 focus:ring-blue-500"
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
                    className="w-full px-3 py-2 border border-line-300 rounded-md flex items-center justify-between text-left text-gray-800 focus:outline-hidden focus:ring-2 focus:ring-blue-500"
                  >
                    <span>{toastPositionDisplay}</span>
                    <span className="text-xs text-blue-500 dark:text-blue-400">Change</span>
                  </button>
                </div>

                {/* Mask Checkbox */}
                <div className="flex items-center">
                  <input
                    type="checkbox"
                    id="toastMask"
                    checked={toastMask}
                    onChange={(e) => setToastMask(e.target.checked)}
                    className="h-4 w-4 text-blue-600 dark:text-blue-400 focus:ring-blue-500 border-line-300 rounded"
                  />
                  <label htmlFor="toastMask" className="ml-2 block text-sm text-gray-700">
                    Show mask (prevents interaction)
                  </label>
                </div>
              </div>
            </div>

            {/* Action Buttons */}
            <div className="mx-1 mb-4 bg-surface rounded-xl shadow-sm border border-line-200 overflow-hidden">
              <div
                className="flex items-center justify-center px-4 py-4 hover:bg-surface-50 cursor-pointer border-b border-line-100"
                onClick={() => showToastWithParams({
                  title: toastTitle,
                  icon: toastIcon,
                  duration: toastDuration,
                  position: toastPosition,
                  mask: toastMask
                })}
              >
                <div className="text-base text-blue-600 dark:text-blue-400 font-medium">Show Toast</div>
              </div>
              <div
                className="flex items-center justify-center px-4 py-4 hover:bg-surface-50 cursor-pointer"
                onClick={hideToast}
              >
                <div className="text-base text-red-600 dark:text-red-400 font-medium">Hide Toast</div>
              </div>
            </div>
          </>
        )}

        {/* ActionSheet Demo Section */}
        {currentType === 'actionsheet' && (
          <div className="mx-1 mt-8 bg-surface rounded-xl shadow-sm border border-line-200 overflow-hidden">
            <div
              className="px-4 py-10 text-base text-blue-600 dark:text-blue-400 font-medium text-center cursor-pointer hover:bg-blue-50"
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
            <div className="mx-1 mb-4 bg-surface rounded-xl shadow-sm border border-line-200 overflow-hidden">
              <div className="px-3 py-3 space-y-3">

                {/* Title Input */}
                <div>
                  <label className="block text-sm font-medium text-gray-700 mb-2">Title (optional)</label>
                  <input
                    type="text"
                    value={modalTitle}
                    onChange={(e) => setModalTitle(e.target.value)}
                    className="w-full px-3 py-2 border border-line-300 rounded-md focus:outline-hidden focus:ring-2 focus:ring-blue-500"
                    placeholder="Leave empty for no title"
                  />
                </div>



                {/* Content Input */}
                <div>
                  <label className="block text-sm font-medium text-gray-700 mb-2">Content</label>
                  <textarea
                    value={modalContent}
                    onChange={(e) => setModalContent(e.target.value)}
                    className="w-full px-3 py-2 border border-line-300 rounded-md focus:outline-hidden focus:ring-2 focus:ring-blue-500"
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
                    className="h-4 w-4 text-blue-600 dark:text-blue-400 focus:ring-blue-500 border-line-300 rounded"
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
                    className="w-full px-3 py-2 border border-line-300 rounded-md focus:outline-hidden focus:ring-2 focus:ring-blue-500"
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
                    className="w-full px-3 py-2 border border-line-300 rounded-md focus:outline-hidden focus:ring-2 focus:ring-blue-500"
                    placeholder="Confirm button text"
                  />
                </div>


              </div>
            </div>

            {/* Action Button */}
            <div className="mx-1 mb-4 bg-surface rounded-xl shadow-sm border border-line-200 overflow-hidden">
              <div
                className="flex items-center justify-center px-4 py-4 hover:bg-surface-50 cursor-pointer"
                onClick={() => showModalWithParams({
                  title: modalTitle,
                  content: modalContent,
                  showCancel: modalShowCancel,
                  cancelText: modalCancelText,
                  confirmText: modalConfirmText
                })}
              >
                <div className="text-base text-blue-600 dark:text-blue-400 font-medium">Show Modal</div>
              </div>
            </div>

            {/* Result Display */}
            {modalResult && (
              <div className="mx-1 mb-4 bg-surface rounded-xl shadow-sm border border-line-200 overflow-hidden">
                <div className="px-3 py-3">
                  <div className="text-sm font-medium text-gray-700 mb-3">Modal Result</div>
                  <div className="bg-surface-50 rounded-lg p-3">
                    <pre className="text-xs text-gray-600 whitespace-pre-wrap">
                      {JSON.stringify(modalResult, null, 2)}
                    </pre>
                  </div>
                  <div
                    className="mt-3 text-center text-sm text-red-600 dark:text-red-400 cursor-pointer hover:text-red-800 dark:hover:text-red-400"
                    onClick={clearModalResult}
                  >
                    Clear Result
                  </div>
                </div>
              </div>
            )}
          </>
        )}

        {/* Page stack + instance lifecycle - Only show for navigation */}
        {currentType === 'navigation' && (
          <div className="mb-5 space-y-4">
            <PageStackCard
              stack={pageStack}
              badge={`${pageStack.length}/10`}
              testId="ui-page-stack"
            />

            {/* Instance identity */}
            <div className="bg-surface rounded-xl shadow-sm border border-line-100 overflow-hidden">
              <div className="px-4 py-3 border-b border-line-100">
                <h3 className="text-base font-medium text-gray-900">Logic instance</h3>
                <p className="text-xs text-gray-500 mt-0.5">
                  Push this page again and a new tag appears — every entry is a new Page instance
                </p>
              </div>
              <div className="px-4 py-4 space-y-3">
                <div className="flex items-center justify-between">
                  <span className="text-sm text-gray-600">This instance</span>
                  <span data-testid="lifecycle-instance-tag" className="text-sm font-mono font-medium text-violet-600 dark:text-violet-400">
                    #{instanceTag || '…'}
                  </span>
                </div>
                <div className="flex items-center justify-between">
                  <span className="text-sm text-gray-600">Previous instance</span>
                  <span data-testid="lifecycle-previous-tag" className="text-sm font-mono text-gray-500">
                    #{previousInstanceTag || '…'}
                  </span>
                </div>
                <div className="text-[11px] text-gray-400 leading-relaxed pt-1">
                  The previous tag is read back from <code>lx.getStorage()</code>, which is
                  app-scoped and survives the reset.
                </div>
              </div>
            </div>

            {/* State that resets */}
            <div className="bg-surface rounded-xl shadow-sm border border-line-100 overflow-hidden">
              <div className="px-4 py-3 border-b border-line-100">
                <h3 className="text-base font-medium text-gray-900">State that resets</h3>
                <p className="text-xs text-gray-500 mt-0.5">
                  Dirty all three, leave, come back — everything is fresh
                </p>
              </div>
              <div className="p-4 grid grid-cols-2 gap-3">
                <button
                  data-testid="lifecycle-bump-logic"
                  onClick={bumpLogicCounter}
                  className="flex flex-col items-center justify-center py-4 px-2 bg-violet-50 hover:bg-violet-100 active:bg-violet-200 text-violet-700 dark:text-violet-400 rounded-xl transition-colors"
                >
                  <span className="text-2xl font-bold" data-testid="lifecycle-logic-counter">{logicCounter}</span>
                  <span className="text-sm font-medium mt-1">Logic +1</span>
                  <span className="text-[10px] opacity-70">this.setData</span>
                </button>
                <button
                  data-testid="lifecycle-bump-view"
                  onClick={() => setViewCounter(count => count + 1)}
                  className="flex flex-col items-center justify-center py-4 px-2 bg-cyan-50 hover:bg-cyan-100 active:bg-cyan-200 text-cyan-700 dark:text-cyan-400 rounded-xl transition-colors"
                >
                  <span className="text-2xl font-bold" data-testid="lifecycle-view-counter">{viewCounter}</span>
                  <span className="text-sm font-medium mt-1">View +1</span>
                  <span className="text-[10px] opacity-70">useState</span>
                </button>
              </div>
              <div className="px-4 pb-4">
                <button
                  data-testid="lifecycle-open-popup"
                  onClick={() => setPopupOpen(true)}
                  className="w-full py-2.5 px-4 bg-linear-to-r from-violet-500 to-fuchsia-500 hover:from-violet-600 hover:to-fuchsia-600 text-white rounded-lg text-sm font-medium transition-all shadow-sm"
                >
                  Open H5 popup
                </button>
              </div>
            </div>

            {/* Lifecycle log */}
            <div className="bg-surface rounded-xl shadow-sm border border-line-100 overflow-hidden">
              <div className="px-4 py-3 border-b border-line-100">
                <h3 className="text-base font-medium text-gray-900">Lifecycle of this instance</h3>
                <p className="text-xs text-gray-500 mt-0.5">
                  Stored in <code>data</code>, so it starts over with every entry
                </p>
              </div>
              <div className="p-4">
                {events.length === 0 ? (
                  <div className="text-xs text-gray-400">No events yet</div>
                ) : (
                  <div className="space-y-2" data-testid="lifecycle-events">
                    {events.map((event: string, index: number) => (
                      <div
                        key={`${event}-${index}`}
                        className="text-xs font-mono text-gray-700 bg-surface-50 border border-line-100 rounded-lg px-3 py-2"
                      >
                        {event}
                      </div>
                    ))}
                  </div>
                )}
                <div className="text-[11px] text-gray-400 leading-relaxed mt-3">
                  Backgrounding the app fires <code>onHide</code> / <code>onShow</code> on the same
                  instance. Only leaving the page fires <code>onUnload</code> and ends it.
                </div>
              </div>
            </div>
          </div>
        )}

        {/* A plain H5 overlay — the kind that used to still be open on re-entry */}
        {popupOpen && (
          <div
            data-testid="lifecycle-popup"
            className="fixed inset-0 z-50 flex items-center justify-center bg-black/50 px-8"
            onClick={() => setPopupOpen(false)}
          >
            <div
              className="bg-surface rounded-2xl shadow-xl px-5 py-6 w-full max-w-xs text-center"
              onClick={event => event.stopPropagation()}
            >
              <div className="text-base font-semibold text-gray-900">Pure H5 popup</div>
              <div className="text-xs text-gray-500 mt-2 leading-relaxed">
                Leave the page with this open. When you come back it is gone, because the
                document was reloaded behind the scenes.
              </div>
              <button
                className="mt-4 w-full py-2 rounded-lg bg-surface-100 hover:bg-surface-200 text-sm text-gray-700"
                onClick={() => setPopupOpen(false)}
              >
                Close
              </button>
            </div>
          </div>
        )}

        {/* NavigationBar Demo Section */}
        {currentType === 'navbar' && (
          <>
            <div className="mt-4 mb-3 px-4 text-sm text-gray-500 font-medium">NavigationBar APIs</div>

            {/* NavigationBar Controls */}
            <div className="mx-1 mb-4 bg-surface rounded-xl shadow-sm border border-line-200 overflow-hidden">
              <div className="p-4 space-y-4">

                {/* Set Title */}
                <div>
                  <label className="block text-sm font-medium text-gray-700 mb-1">Title</label>
                  <div className="flex space-x-2">
                    <input
                      type="text"
                      id="navbarTitle"
                      placeholder="Enter title"
                      className="flex-1 px-2 py-1.5 text-sm border border-line-300 rounded focus:outline-hidden focus:ring-1 focus:ring-blue-500"
                    />
                    <button
                      onClick={() => {
                        const title = document.getElementById('navbarTitle').value;
                        if (title) {
                          updateNavigationBarTitle({ title });
                        }
                      }}
                      className="px-3 py-1.5 text-sm bg-blue-500 text-white rounded hover:bg-blue-600 focus:outline-hidden focus:ring-1 focus:ring-blue-500"
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
                        className="px-2 py-1.5 text-sm border border-line-300 rounded focus:outline-hidden focus:ring-1 focus:ring-blue-500"
                      />
                      <input
                        type="text"
                        id="navbarTextColor"
                        placeholder="Text #000000"
                        className="px-2 py-1.5 text-sm border border-line-300 rounded focus:outline-hidden focus:ring-1 focus:ring-blue-500"
                      />
                    </div>
                    <button
                      onClick={() => {
                        const bgColor = document.getElementById('navbarBgColor').value || '#ffffff';
                        const textColor = document.getElementById('navbarTextColor').value || '#000000';
                        updateNavigationBarColors({
                          backgroundColor: bgColor,
                          frontColor: textColor
                        });
                      }}
                      className="w-full px-3 py-1.5 text-sm bg-green-500 text-white rounded hover:bg-green-600 focus:outline-hidden focus:ring-1 focus:ring-green-500"
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
                        updateNavigationBarTitle({ title: "Dark Theme" });
                        updateNavigationBarColors({ backgroundColor: "#1f2937", frontColor: "#ffffff" });
                      }}
                      className="px-2 py-1.5 bg-surface-800 text-white rounded hover:bg-surface-900 text-xs"
                    >
                      Dark
                    </button>
                    <button
                      onClick={() => {
                        updateNavigationBarTitle({ title: "Blue Theme" });
                        updateNavigationBarColors({ backgroundColor: "#3b82f6", frontColor: "#ffffff" });
                      }}
                      className="px-2 py-1.5 bg-blue-500 text-white rounded hover:bg-blue-600 text-xs"
                    >
                      Blue
                    </button>
                    <button
                      onClick={() => {
                        updateNavigationBarTitle({ title: "Light Theme" });
                        updateNavigationBarColors({ backgroundColor: "#ffffff", frontColor: "#000000" });
                      }}
                      className="px-2 py-1.5 bg-surface text-foreground border border-line-300 rounded hover:bg-surface-50 text-xs"
                    >
                      Light
                    </button>
                    <button
                      onClick={() => {
                        updateNavigationBarTitle({ title: "Green Theme" });
                        updateNavigationBarColors({ backgroundColor: "#10b981", frontColor: "#ffffff" });
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

        {/* Appearance Demo Section */}
        {currentType === 'appearance' && (
          <>
            <div className="mt-4 mb-3 px-4 text-sm text-gray-500 font-medium">Appearance APIs</div>

            <div className="mx-1 mb-4 bg-surface rounded-xl shadow-sm border border-line-200 overflow-hidden">
              <div className="px-4 py-3 border-b border-line-100">
                <h3 className="text-base font-medium text-gray-900">Light / Dark</h3>
                <p className="text-sm text-gray-500 mt-1">
                  <code className="text-xs">lx.appearance.set()</code> picks this lxapp&apos;s branch
                  independently of the host shell; the preference persists per lxapp.
                </p>
              </div>

              <div className="p-4 space-y-4">
                <div className="grid grid-cols-3 gap-1.5" data-testid="ui-appearance">
                  {(['auto', 'light', 'dark'] as const).map((option) => (
                    <button
                      key={option}
                      type="button"
                      data-testid={`ui-appearance-${option}`}
                      data-selected={appearance.preference === option}
                      onClick={() => setAppearance({ preference: option })}
                      className={`px-2 py-1.5 rounded text-xs font-medium capitalize border transition-colors ${
                        appearance.preference === option
                          ? 'bg-blue-500 text-white border-blue-500'
                          : 'bg-surface text-gray-700 border-line-300 hover:bg-surface-50'
                      }`}
                    >
                      {option}
                    </button>
                  ))}
                </div>

                <div className="rounded-lg bg-surface-50 border border-line-100 divide-y divide-line-100">
                  <div className="flex items-center justify-between px-3 py-2">
                    <span className="text-sm text-gray-500">Preference</span>
                    <span className="text-sm font-medium text-gray-900" data-testid="ui-appearance-preference">
                      {appearance.preference}
                    </span>
                  </div>
                  <div className="flex items-center justify-between px-3 py-2">
                    <span className="text-sm text-gray-500">Resolved</span>
                    <span className="text-sm font-medium text-gray-900" data-testid="ui-appearance-resolved">
                      {appearance.resolved}
                    </span>
                  </div>
                </div>

                <p className="text-xs text-gray-400 leading-relaxed">
                  The runtime projects the resolved branch into every page as{' '}
                  <code>color-scheme</code> plus <code>data-theme</code> on{' '}
                  <code>&lt;html&gt;</code>. This app&apos;s Tailwind palette is bound to CSS
                  variables keyed off that attribute, so pages need no per-element dark variants.
                </p>
              </div>
            </div>
          </>
        )}

        {/* TabBar Demo Section */}
        {currentType === 'tabbar' && (
          <>
            <div className="mt-4 mb-3 px-4 text-sm text-gray-500 font-medium">TabBar APIs</div>

            {/* Visibility Controls */}
            <div className="mx-1 mb-4 bg-surface rounded-xl shadow-sm border border-line-200 overflow-hidden">
              <div className="px-4 py-3 border-b border-line-100">
                <h3 className="text-base font-medium text-gray-900">Visibility Controls</h3>
                <p className="text-sm text-gray-500 mt-1">Show/hide TabBar and update item text</p>
              </div>
              <div className="p-4 space-y-4">
                <div className="flex space-x-3">
                  <button
                    onClick={async () => {
                      const result = await revealTabBar();
                      console.log('Show TabBar:', result);
                      // Toast at resolve time: the bar must already be visible.
                      showToastWithParams({ title: 'shown', icon: 'success', duration: 800 });
                    }}
                    className="flex-1 bg-green-500 hover:bg-green-600 text-white py-2 px-4 rounded-lg text-sm font-medium transition-colors"
                  >
                    Show TabBar
                  </button>
                  <button
                    onClick={async () => {
                      const result = await concealTabBar();
                      console.log('Hide TabBar:', result);
                      // Toast at resolve time: the bar must already be gone.
                      showToastWithParams({ title: 'hidden', icon: 'success', duration: 800 });
                    }}
                    className="flex-1 bg-red-500 hover:bg-red-600 text-white py-2 px-4 rounded-lg text-sm font-medium transition-colors"
                  >
                    Hide TabBar
                  </button>
                </div>

                {/* Item Text Control */}
                <div className="pt-2 border-t border-line-100">
                  <label className="block text-sm font-medium text-gray-700 mb-2">
                    Update Tab 1 Text
                  </label>
                  <div className="flex space-x-2">
                    <input
                      type="text"
                      value={itemText}
                      onChange={(e) => setItemText(e.target.value)}
                      className="flex-1 px-3 py-2 border border-line-300 rounded-lg focus:outline-hidden focus:ring-2 focus:ring-blue-500"
                      placeholder="Enter new text"
                    />
                    <button
                      onClick={() => {
                        const result = updateTabBarItem({ index: 1, text: itemText });
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
            <div className="mx-1 mb-4 bg-surface rounded-xl shadow-sm border border-line-200 overflow-hidden">
              <div className="px-4 py-3 border-b border-line-100">
                <h3 className="text-base font-medium text-gray-900">Red Dot Controls</h3>
                <p className="text-sm text-gray-500 mt-1">Show/hide red dot on tab 1</p>
              </div>
              <div className="p-4">
                <div className="flex space-x-3">
                  <button
                    onClick={() => {
                      const result = enableTabBarRedDot({ index: 1 });
                      console.log('Show red dot on tab 1:', result);
                    }}
                    className="flex-1 bg-red-500 hover:bg-red-600 text-white py-2 px-4 rounded-lg text-sm font-medium transition-colors"
                  >
                    Show Red Dot
                  </button>
                  <button
                    onClick={() => {
                      const result = disableTabBarRedDot({ index: 1 });
                      console.log('Hide red dot on tab 1:', result);
                    }}
                    className="flex-1 bg-surface-500 hover:bg-surface-600 text-white py-2 px-4 rounded-lg text-sm font-medium transition-colors"
                  >
                    Hide Red Dot
                  </button>
                </div>
              </div>
            </div>

            {/* Badge Controls */}
            <div className="mx-1 mb-4 bg-surface rounded-xl shadow-sm border border-line-200 overflow-hidden">
              <div className="px-4 py-3 border-b border-line-100">
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
                    className="w-full px-3 py-2 border border-line-300 rounded-lg text-sm focus:outline-hidden focus:ring-2 focus:ring-blue-500"
                    placeholder="Enter badge text"
                  />
                </div>
                <div className="flex space-x-3">
                  <button
                    onClick={() => {
                      const result = updateTabBarBadge({ index: 1, text: badgeText });
                      console.log(`Set badge "${badgeText}" on tab 1:`, result);
                    }}
                    className="flex-1 bg-orange-500 hover:bg-orange-600 text-white py-2 px-4 rounded-lg text-sm font-medium transition-colors"
                  >
                    Set Badge
                  </button>
                  <button
                    onClick={() => {
                      const result = clearTabBarBadge({ index: 1 });
                      console.log('Remove badge on tab 1:', result);
                    }}
                    className="flex-1 bg-surface-500 hover:bg-surface-600 text-white py-2 px-4 rounded-lg text-sm font-medium transition-colors"
                  >
                    Remove Badge
                  </button>
                </div>
              </div>
            </div>



            {/* Style Controls */}
            <div className="mx-1 mb-4 bg-surface rounded-xl shadow-sm border border-line-200 overflow-hidden">
              <div className="px-4 py-3 border-b border-line-100">
                <h3 className="text-base font-medium text-gray-900">Style Controls</h3>
                <p className="text-sm text-gray-500 mt-1">Customize TabBar appearance</p>
              </div>
              <div className="p-4 space-y-3">
                <div className="grid grid-cols-2 gap-3">
                  <div>
                    <label className="block text-sm font-medium text-gray-700 mb-1">Text Color</label>
                    <div className="flex items-center space-x-2">
                      <div
                        className="w-8 h-8 border border-line-300 rounded cursor-pointer"
                        style={{ backgroundColor: color }}
                      ></div>
                      <input
                        type="text"
                        value={color}
                        onChange={(e) => setColor(e.target.value)}
                        className="flex-1 px-2 py-1 border border-line-300 rounded text-sm"
                        placeholder="#666666"
                      />
                    </div>
                  </div>
                  <div>
                    <label className="block text-sm font-medium text-gray-700 mb-1">Selected Color</label>
                    <div className="flex items-center space-x-2">
                      <div
                        className="w-8 h-8 border border-line-300 rounded cursor-pointer"
                        style={{ backgroundColor: selectedColor }}
                      ></div>
                      <input
                        type="text"
                        value={selectedColor}
                        onChange={(e) => setSelectedColor(e.target.value)}
                        className="flex-1 px-2 py-1 border border-line-300 rounded text-sm"
                        placeholder="#007AFF"
                      />
                    </div>
                  </div>
                </div>

                <button
                  onClick={() => {
                    const result = updateTabBarForegrounds({
                      color,
                      selectedColor
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
                        const theme = { color: '#666666', selectedColor: '#007AFF' };
                        setColor(theme.color);
                        setSelectedColor(theme.selectedColor);
                        const result = updateTabBarForegrounds(theme);
                        console.log('Applied Default theme:', result);
                      }}
                      className="px-3 py-2 bg-surface-100 hover:bg-surface-200 text-gray-700 rounded-lg text-sm font-medium transition-colors"
                    >
                      Default
                    </button>
                    <button
                      onClick={() => {
                        const theme = { color: '#CCCCCC', selectedColor: '#0A84FF' };
                        setColor(theme.color);
                        setSelectedColor(theme.selectedColor);
                        const result = updateTabBarForegrounds(theme);
                        console.log('Applied Dark theme:', result);
                      }}
                      className="px-3 py-2 bg-surface-800 hover:bg-surface-900 text-white rounded-lg text-sm font-medium transition-colors"
                    >
                      Dark
                    </button>
                    <button
                      onClick={() => {
                        const theme = { color: '#8E8E93', selectedColor: '#34C759' };
                        setColor(theme.color);
                        setSelectedColor(theme.selectedColor);
                        const result = updateTabBarForegrounds(theme);
                        console.log('Applied Green theme:', result);
                      }}
                      className="px-3 py-2 bg-green-100 hover:bg-green-200 text-green-700 dark:text-green-400 rounded-lg text-sm font-medium transition-colors"
                    >
                      Green
                    </button>
                    <button
                      onClick={() => {
                        const theme = { color: '#8E8E93', selectedColor: '#AF52DE' };
                        setColor(theme.color);
                        setSelectedColor(theme.selectedColor);
                        const result = updateTabBarForegrounds(theme);
                        console.log('Applied Purple theme:', result);
                      }}
                      className="px-3 py-2 bg-purple-100 hover:bg-purple-200 text-purple-700 dark:text-purple-400 rounded-lg text-sm font-medium transition-colors"
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
