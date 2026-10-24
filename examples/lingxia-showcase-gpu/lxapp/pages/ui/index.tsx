import { useLocal, type View, type Local } from '@lingxia/ui';
import '../../theme/index.css';

export function render({ data, actions, env }: View) {
  if (["toast", "actionsheet", "modal"].includes(String(data.currentType))) return <DialogControls data={data} actions={actions} env={env} />;
  if (data.currentType === 'appearance') return <AppearanceControls data={data} actions={actions} env={env} />;
  if (data.currentType === 'navbar') return <NavbarControls data={data} actions={actions} env={env} />;
  if (data.currentType === 'tabbar') return <TabbarControls data={data} actions={actions} env={env} />;
  if (!data.currentType || data.currentType === 'navigation') return <NavigationControls data={data} actions={actions} env={env} />;
  return <SurfaceControls data={data} actions={actions} env={env} />;
}


function DialogControls({ data: d, actions: a, env }: Pick<View, 'data' | 'actions' | 'env'>) {
  const type = String(d.currentType);
  const toastTitle = useLocal('toastTitle', 'Hello Toast!');
  const toastDuration = useLocal('toastDuration', '2000');
  const toastMask = useLocal('toastMask', false);
  const modalTitle = useLocal('modalTitle', 'Alert');
  const modalContent = useLocal('modalContent', 'This is a modal dialog');
  const modalCancel = useLocal('modalCancel', true);
  const cancelText = useLocal('cancelText', 'Cancel');
  const confirmText = useLocal('confirmText', 'OK');
  const card = 'bg-surface rounded-xl shadow-sm border border-line-200';
  const input = 'w-full px-3 py-2 border border-line-300 rounded-md bg-surface text-gray-800 text-base';
  const field = (id: string, label: string, value: Local<string>, placeholder: string, keyboard: 'text' | 'number' = 'text') => <column className="gap-2"><text className="text-sm font-medium text-gray-700">{label}</text><input testId={id} bind={value} placeholder={placeholder} keyboard={keyboard} className={input} /></column>;
  const choice = (id: string, label: string, value: string, onClick: () => unknown) => <column className="gap-2"><text className="text-sm font-medium text-gray-700">{label}</text><button testId={id} accessibilityLabel={`${label}: ${value}`} onClick={onClick} className="w-full px-3 py-2 border border-line-300 rounded-md bg-surface"><row className="items-center justify-between gap-3"><text className="text-base text-gray-800">{value}</text><text className="text-xs text-blue-500 dark:text-blue-400">Change</text></row></button></column>;
  const action = (id: string, label: string, onClick: () => unknown, danger = false) => <button testId={id} onClick={onClick} className={`px-4 py-4 bg-surface text-base font-medium ${danger ? 'text-red-600 dark:text-red-400' : 'text-blue-600 dark:text-blue-400'}`}>{label}</button>;
  return <column className="bg-linear-to-br from-surface-50 to-surface-100"><spacer style={{ height: env.insets.top }} /><row className="grow basis-0"><spacer style={{ width: env.insets.left }} /><scroll testId="page-scroll" className="grow basis-0 px-4 pt-6 pb-6 gap-4">
    {!!d.chromeError && <text testId="ui-chrome-error" className="rounded-lg border border-red-200 bg-red-50 px-3 py-2 text-sm text-red-700 dark:text-red-400 break-all">{String(d.chromeError)}</text>}
    {type === 'toast' && <column className="gap-4">
      <text className="pt-4 px-4 text-sm text-gray-500 font-medium">Toast Parameters</text>
      <column className={`${card} px-3 py-3 gap-3`}>
        {field('toast-title', 'Title', toastTitle, 'Enter toast title')}
        {choice('toast-icon', 'Icon', String(d.toastIconLabel || 'Success'), () => a.chooseToastIcon())}
        {field('toast-duration', 'Duration (ms)', toastDuration, '', 'number')}
        {choice('toast-position', 'Position', String(d.toastPositionLabel || 'Center'), () => a.chooseToastPosition())}
        <checkbox testId="toast-mask" bind={toastMask} className="text-sm text-gray-700">Show mask (prevents interaction)</checkbox>
      </column>
      <column className={card}>{action('toast-show', 'Show Toast', () => a.showToastWithParams({title: toastTitle.value, icon: d.toastIcon, durationMs: parseInt(toastDuration.value, 10) || 2000, position: d.toastPosition, mask: toastMask.value}))}<spacer className="h-px bg-line-100" />{action('toast-hide', 'Hide Toast', () => a.hideToast(), true)}</column>
    </column>}
    {type === 'actionsheet' && <column><spacer className="h-8" /><column className={card}><button testId="actionsheet-show" onClick={() => a.showDemoActionSheet()} className="px-4 py-10 bg-surface text-base text-blue-600 dark:text-blue-400 font-medium">Show Action Sheet</button>{!!d.actionSheetResult && <text testId="actionsheet-result" className="p-4 text-xs font-mono text-gray-800 break-all">{JSON.stringify(d.actionSheetResult, null, 2)}</text>}</column></column>}
    {type === 'modal' && <column className="gap-4">
      <text className="pt-4 px-4 text-sm text-gray-500 font-medium">Modal Parameters</text>
      <column className={`${card} px-3 py-3 gap-3`}>
        {field('modal-title', 'Title (optional)', modalTitle, 'Leave empty for no title')}
        <column className="gap-2"><text className="text-sm font-medium text-gray-700">Content</text><textarea testId="modal-content" bind={modalContent} placeholder="Enter modal content" className={`${input} h-24`} /></column>
        <checkbox testId="modal-show-cancel" bind={modalCancel} className="text-sm text-gray-700">Show cancel button</checkbox>
        {field('modal-cancel-text', 'Cancel Button Text', cancelText, 'Cancel button text')}
        {field('modal-confirm-text', 'Confirm Button Text', confirmText, 'Confirm button text')}
      </column>
      <column className={card}>{action('modal-show', 'Show Modal', () => a.showModalWithParams({title: modalTitle.value, content: modalContent.value, showCancel: modalCancel.value, cancelText: cancelText.value, confirmText: confirmText.value}))}</column>
      {!!d.modalResult && <column className={`${card} px-3 py-3 gap-3`}><text className="text-sm font-medium text-gray-700">Modal Result</text><column className="bg-surface-50 rounded-lg p-3"><text testId="modal-result" className="text-xs font-mono text-gray-600 break-all">{JSON.stringify(d.modalResult, null, 2)}</text></column><button testId="modal-clear" onClick={() => a.clearModalResult()} className="bg-surface text-center text-sm text-red-600 dark:text-red-400">Clear Result</button></column>}
    </column>}
  </scroll><spacer style={{ width: env.insets.right }} /></row><spacer style={{ height: Math.max(env.insets.bottom, env.keyboard.height) }} /></column>;
}

function AppearanceControls({ data, actions, env }: Pick<View, 'data' | 'actions' | 'env'>) {
  const appearance = data.appearance as { preference: string; resolved: string } | undefined;
  const preference = appearance?.preference || 'auto';
  return <column className="bg-linear-to-br from-surface-50 to-surface-100"><spacer style={{ height: env.insets.top }} /><row className="grow basis-0"><spacer style={{ width: env.insets.left }} /><scroll testId="page-scroll" className="grow basis-0 px-4 pt-6 pb-6 gap-3">
    {!!data.chromeError && <text testId="ui-chrome-error" className="text-sm text-red-600 dark:text-red-400 break-all">{String(data.chromeError)}</text>}
    <text className="pt-4 px-4 text-sm text-gray-500 font-medium">Appearance APIs</text>
    <column className="bg-surface rounded-xl shadow-sm border border-line-200">
      <column className="px-4 py-3 gap-1">
        <text className="text-base font-medium text-gray-900">Light / Dark</text>
        <text className="text-sm text-gray-500 leading-relaxed">Choose how this app looks. Auto follows the host appearance; Light and Dark keep your chosen theme. Your preference is saved for this app.</text>
      </column>
      <spacer className="h-px bg-line-100" />
      <column className="p-4 gap-4">
        <grid testId="ui-appearance" columns={3} className="gap-1.5">
          {['auto', 'light', 'dark'].map(option => <button testId={`ui-appearance-${option}`} onClick={() => actions.setAppearance({ preference: option })} className={`px-2 py-1.5 rounded text-xs font-medium border ${preference === option ? 'bg-blue-500 text-white border-blue-500' : 'bg-surface text-gray-700 border-line-300'}`}>{option === 'auto' ? 'Auto' : option === 'light' ? 'Light' : 'Dark'}</button>)}
        </grid>
        <column className="rounded-lg bg-surface-50 border border-line-100">
          <row className="items-center justify-between px-3 py-2"><text className="text-sm text-gray-500">Preference</text><text testId="ui-appearance-preference" className="text-sm font-medium text-gray-900">{preference}</text></row>
          <spacer className="h-px bg-line-100" />
          <row className="items-center justify-between px-3 py-2"><text className="text-sm text-gray-500">Resolved</text><text testId="ui-appearance-resolved" className="text-sm font-medium text-gray-900">{appearance?.resolved || env.theme}</text></row>
        </column>
        <text className="text-xs text-gray-400 leading-relaxed">The resolved theme is the appearance currently in use. Pages share the same theme palette, so their colors update together.</text>
      </column>
    </column>
  </scroll><spacer style={{ width: env.insets.right }} /></row><spacer style={{ height: env.insets.bottom }} /></column>;
}

function NavbarControls({ data, actions, env }: Pick<View, 'data' | 'actions' | 'env'>) {
  const title = useLocal('navTitle', '');
  const background = useLocal('navBackground', '');
  const foreground = useLocal('navForeground', '');
  const field = 'grow basis-0 px-2 py-1.5 text-sm border border-line-300 rounded bg-surface text-gray-900';
  const preset = (name: string, color: string, text: string, className: string) => <button testId={`navbar-preset-${name.toLowerCase()}`} className={`px-2 py-1.5 rounded text-xs ${className}`} onClick={() => { actions.updateNavigationBarTitle({ title: `${name} Theme` }); actions.updateNavigationBarColors({ backgroundColor: color, frontColor: text }); }}>{name}</button>;
  return <column className="bg-linear-to-br from-surface-50 to-surface-100"><spacer style={{ height: env.insets.top }} /><row className="grow basis-0"><spacer style={{ width: env.insets.left }} /><scroll testId="page-scroll" className="grow basis-0 px-4 pt-6 pb-6 gap-3">
    {!!data.chromeError && <text testId="ui-chrome-error" className="text-sm text-red-600 dark:text-red-400 break-all">{String(data.chromeError)}</text>}
    <text className="pt-4 px-4 text-sm text-gray-500 font-medium">NavigationBar APIs</text>
    <column className="p-4 gap-4 bg-surface rounded-xl shadow-sm border border-line-200">
      <column className="gap-1"><text className="text-sm font-medium text-gray-700">Title</text><row className="gap-2"><input testId="navbar-title-input" bind={title} placeholder="Enter title" className={field} /><button testId="navbar-set-title" onClick={() => { if (title.value) actions.updateNavigationBarTitle({ title: title.value }); }} className="px-3 py-1.5 text-sm bg-blue-500 text-white rounded">Set</button></row></column>
      <column className="gap-2"><text className="text-sm font-medium text-gray-700">Colors</text><grid columns={2} className="gap-2"><input testId="navbar-bg-input" bind={background} placeholder="Background #ffffff" className={field} /><input testId="navbar-fg-input" bind={foreground} placeholder="Text #000000" className={field} /></grid><button testId="navbar-set-colors" onClick={() => actions.updateNavigationBarColors({ backgroundColor: background.value || '#ffffff', frontColor: foreground.value || '#000000' })} className="px-3 py-1.5 text-sm bg-green-500 text-white rounded">Set Colors</button></column>
      <column className="gap-1"><text className="text-sm font-medium text-gray-700">Presets</text><grid columns={2} className="gap-1.5">
        {preset('Dark', '#1f2937', '#ffffff', 'bg-[#1f2937] text-white')}
        {preset('Blue', '#3b82f6', '#ffffff', 'bg-blue-500 text-white')}
        {preset('Light', '#ffffff', '#000000', 'bg-surface text-gray-900 border border-line-300')}
        {preset('Green', '#10b981', '#ffffff', 'bg-green-500 text-white')}
      </grid></column>
      <grid columns={3} className="gap-2"><button testId="navbar-home-hide" onClick={() => actions.updateNavigationBarHomeButton({ homeButton: 'hidden' })} className="px-2 py-1.5 bg-surface text-gray-900 border border-line-300 rounded text-xs">Hide home</button><button testId="navbar-home-auto" onClick={() => actions.updateNavigationBarHomeButton({ homeButton: 'auto' })} className="px-2 py-1.5 bg-surface text-gray-900 border border-line-300 rounded text-xs">Auto home</button><button testId="navbar-reset" onClick={() => actions.resetNavigationBar()} className="px-2 py-1.5 bg-surface-100 text-gray-700 rounded text-xs">Reset</button></grid>
    </column>
  </scroll><spacer style={{ width: env.insets.right }} /></row><spacer style={{ height: Math.max(env.insets.bottom, env.keyboard.height) }} /></column>;
}

function TabbarControls({ data, actions: a, env }: Pick<View, 'data' | 'actions' | 'env'>) {
  const itemText = useLocal('itemText', 'New Tab');
  const badge = useLocal('badge', '99');
  const card = 'bg-surface rounded-xl shadow-sm border border-line-200';
  const header = (title: string, description: string) => <column><column className="px-4 py-3 gap-1"><text className="text-base font-medium text-gray-900">{title}</text><text className="text-sm text-gray-500">{description}</text></column><spacer className="h-px bg-line-100" /></column>;
  const button = (id: string, label: string, action: () => unknown, color: string) => <button testId={id} onClick={action} className={`grow basis-0 py-2 px-4 rounded-lg text-sm font-medium text-white ${color}`}>{label}</button>;
  return <column className="bg-linear-to-br from-surface-50 to-surface-100"><spacer style={{ height: env.insets.top }} /><row className="grow basis-0"><spacer style={{ width: env.insets.left }} /><scroll testId="page-scroll" className="grow basis-0 px-4 pt-6 pb-6 gap-4">
    {!!data.chromeError && <text testId="ui-chrome-error" className="text-sm text-red-600 dark:text-red-400 break-all">{String(data.chromeError)}</text>}
    <text className="pt-4 px-4 text-sm text-gray-500 font-medium">TabBar APIs</text>
    <column className={card}>{header('Visibility Controls', 'Show/hide TabBar and update item text')}<column className="p-4 gap-4">
      <grid columns={2} className="gap-3">{button('tabbar-show', 'Show TabBar', () => a.revealTabBar(), 'bg-green-500')}{button('tabbar-hide', 'Hide TabBar', () => a.concealTabBar(), 'bg-red-500')}</grid>
      <spacer className="h-px bg-line-100" />
      <column className="gap-2"><text className="text-sm font-medium text-gray-700">Update Tab 1 Text</text><row className="gap-2"><input testId="tabbar-item-text" bind={itemText} placeholder="Enter new text" className="grow basis-0 px-3 py-2 border border-line-300 rounded-lg bg-surface text-base text-gray-900" /><button testId="tabbar-item-update" onClick={() => a.updateTabBarItem({ index: 1, text: itemText.value })} className="px-4 py-2 bg-blue-500 text-white rounded-lg text-base">Update</button></row></column>
    </column></column>
    <column className={card}>{header('Red Dot Controls', 'Show/hide red dot on tab 1')}<grid columns={2} className="p-4 gap-3">{button('tabbar-reddot-show', 'Show Red Dot', () => a.enableTabBarRedDot({ index: 1 }), 'bg-red-500')}{button('tabbar-reddot-hide', 'Hide Red Dot', () => a.disableTabBarRedDot({ index: 1 }), 'bg-surface-500')}</grid></column>
    <column className={card}>{header('Badge Controls', 'Set/remove badge on tab 1')}<column className="p-4 gap-3"><column className="gap-1"><text className="text-sm font-medium text-gray-700">Badge Text</text><input testId="tabbar-badge-input" bind={badge} placeholder="Enter badge text" className="w-full px-3 py-2 border border-line-300 rounded-lg text-sm bg-surface text-gray-900" /></column><grid columns={2} className="gap-3">{button('tabbar-badge-set', 'Set Badge', () => a.updateTabBarBadge({ index: 1, text: badge.value }), 'bg-orange-500')}{button('tabbar-badge-clear', 'Remove Badge', () => a.clearTabBarBadge({ index: 1 }), 'bg-surface-500')}</grid></column></column>
  </scroll><spacer style={{ width: env.insets.right }} /></row><spacer style={{ height: Math.max(env.insets.bottom, env.keyboard.height) }} /></column>;
}

function NavigationControls({ data: d, actions: a, env }: Pick<View, 'data' | 'actions' | 'env'>) {
  const count = useLocal('viewCounter', 0);
  const popup = useLocal('popup', false);
  const pages = (d.pageStack || []) as { index: number; name: string; current: boolean }[];
  const events = (d.events || []) as string[];
  const card = 'bg-surface rounded-xl shadow-sm border border-line-100';
  const header = (title: string, description: string) => <column><column className="px-4 py-3 gap-1"><text className="text-base font-medium text-gray-900">{title}</text><text className="text-xs text-gray-500">{description}</text></column><spacer className="h-px bg-line-100" /></column>;
  const nav = (id: string, label: string, action: () => unknown, description = '') => <button testId={id} accessibilityLabel={label} onClick={action} className="px-5 py-4 bg-surface text-left"><row className="items-center justify-between gap-3"><column className="grow basis-0 gap-1"><text className="text-sm text-gray-800 font-medium">{label}</text>{!!description && <text className="text-xs text-gray-500">{description}</text>}</column><text className="text-lg text-gray-400">›</text></row></button>;
  const counter = (key: string, label: string, value: number, hint: string, action: () => unknown, color: string) => <button testId={`lifecycle-bump-${key}`} accessibilityLabel={`${label}: ${value}`} onClick={action} className={`py-4 px-2 rounded-xl ${color}`}><column className="items-center gap-1"><text testId={`lifecycle-${key}-counter`} className="text-2xl font-bold">{String(value)}</text><text className="text-sm font-medium">{label}</text><text className="text-[10px] opacity-70">{hint}</text></column></button>;
  return <stack style={{ width: env.viewport.width, height: env.viewport.height }}><column style={{ width: env.viewport.width, height: env.viewport.height }} className="bg-linear-to-br from-surface-50 to-surface-100"><spacer style={{ height: env.insets.top }} /><row className="grow basis-0"><spacer style={{ width: env.insets.left }} /><scroll testId="page-scroll" className="grow basis-0 px-4 pt-6 pb-6 gap-4">
    {!!d.chromeError && <text className="text-sm text-red-600" testId="ui-chrome-error">{String(d.chromeError)}</text>}
    <text className="text-sm text-gray-600 font-semibold">navigateTo/Back, redirectTo</text>
    <column className={card}>{nav('ui-navigate-to', 'Push this page again', () => a.demoNavigateTo(), 'Each entry creates a fresh instance. Watch the stack grow.')}<spacer className="h-px bg-line-100" />{nav('ui-navigate-back', 'Back to previous page', () => a.demoNavigateBack())}<spacer className="h-px bg-line-100" />{nav('ui-redirect-to', 'Replace this page (redirectTo)', () => a.demoRedirectTo())}<spacer className="h-px bg-line-100" />{nav('ui-switch-tab', 'Jump to Tab page', () => a.demoSwitchTab())}</column>
    <column className={card}><row className="px-4 py-3 items-center justify-between gap-2"><column className="grow basis-0 gap-1"><text className="text-base font-medium text-gray-900">Page stack</text><text className="text-xs text-gray-500">getCurrentPages(), top is where you are</text></column><text testId="ui-page-stack-depth" className="text-xs font-mono text-violet-600 dark:text-violet-400 bg-violet-50 dark:bg-violet-950 rounded-full px-2.5 py-1">{`${pages.length}/10`}</text></row><spacer className="h-px bg-line-100" /><column testId="ui-page-stack-list" className="p-3 gap-1.5">{[...pages].reverse().map(page => <row className={`items-center gap-3 rounded-lg px-3 py-2 border ${page.current ? 'bg-violet-50 dark:bg-violet-950 text-violet-700 dark:text-violet-300 border-violet-200 dark:border-violet-800' : 'bg-surface-50 text-gray-600 border-line-100'}`}><text className="w-5 text-right text-xs font-mono opacity-60">{String(page.index + 1)}</text><text className="grow basis-0 text-xs font-mono">{page.name}</text>{page.current && <text className="text-[10px] font-medium">you are here</text>}</row>)}{pages.length === 0 && <text className="text-xs text-gray-400 text-center py-4">No page stack available</text>}</column></column>
    <column className={card}>{header('Logic instance', 'Push this page again to create a new Page instance with a new tag.')}<column className="px-4 py-4 gap-3"><row className="justify-between items-center"><text className="text-sm text-gray-600">This instance</text><text testId="lifecycle-instance-tag" className="text-sm font-mono font-medium text-violet-600 dark:text-violet-400">{`#${d.instanceTag || '-'}`}</text></row><row className="justify-between items-center"><text className="text-sm text-gray-600">Previous instance</text><text testId="lifecycle-previous-tag" className="text-sm font-mono text-gray-500">{`#${d.previousInstanceTag || '-'}`}</text></row><text className="text-[11px] text-gray-400 leading-relaxed">The previous tag is saved in app storage and survives a page reset.</text></column></column>
    <column className={card}>{header('State that resets and state that stays', 'Logic and View reset per entry; the module counter is shared by every instance.')}<grid columns={3} className="p-4 gap-3">{counter('logic','Logic +1',Number(d.logicCounter || 0),'this.setData',() => a.bumpLogicCounter(),'bg-violet-50 text-violet-700 dark:text-violet-400')}{counter('view','View +1',count.value,'useLocal',() => count.set(count.value + 1),'bg-cyan-50 text-cyan-700 dark:text-cyan-400')}{counter('module','Module +1',Number(d.moduleCounter || 0),'module scope',() => a.bumpModuleCounter(),'bg-amber-50 text-amber-700 dark:text-amber-400')}</grid><column className="px-4 pb-4"><button testId="lifecycle-open-popup" onClick={() => popup.set(true)} className="py-2.5 px-4 bg-linear-to-r from-violet-500 to-fuchsia-500 text-white rounded-lg text-sm font-medium shadow-sm">Open View popup</button></column></column>
    <column className={card}>{header('Lifecycle of this instance', 'Stored in page data, so it starts over with every entry.')}<column className="p-4 gap-3"><column testId="lifecycle-events" className="gap-2">{events.map(event => <text className="text-xs font-mono text-gray-700 bg-surface-50 border border-line-100 rounded-lg px-3 py-2 break-all">{event}</text>)}{events.length === 0 && <text className="text-xs text-gray-400">No events yet</text>}</column><text className="text-[11px] text-gray-400 leading-relaxed">Backgrounding the app fires onHide / onShow on the same instance. Leaving the page fires onUnload and ends it.</text></column></column>
  </scroll><spacer style={{ width: env.insets.right }} /></row><spacer style={{ height: env.insets.bottom }} /></column>
  {popup.value && <stack testId="lifecycle-popup" className="w-full h-full"><button testId="lifecycle-popup-backdrop" onClick={() => popup.set(false)} className="w-full h-full bg-black/50 rounded-none" /><column className="w-full h-full justify-center items-center px-8"><column className="w-full max-w-xs bg-surface rounded-2xl shadow-xl px-5 py-6 gap-4"><text className="text-base font-semibold text-gray-900 text-center">View popup</text><text className="text-xs text-gray-500 leading-relaxed text-center">Leave this page with the popup open. A fresh visit starts with it closed, because the View state belongs to each page instance.</text><button testId="lifecycle-popup-close" onClick={() => popup.set(false)} className="w-full py-2 rounded-lg bg-surface-100 text-sm text-gray-700">Close</button></column></column></stack>}
  </stack>;
}

function SurfaceControls({ data, actions: a, env }: Pick<View, 'data' | 'actions' | 'env'>) {
  const kind = useLocal('surfaceKind', 'aside');
  const edge = useLocal('surfaceEdge', 'right');
  const position = useLocal('surfacePosition', 'center');
  const chrome = useLocal('surfaceChrome', 'full');
  const width = useLocal('surfaceWidth', '');
  const height = useLocal('surfaceHeight', '');
  const attempted = useLocal('surfaceSizeAttempted', false);
  const state = data.surfaceDemo as { active?: boolean; visible?: boolean; message?: string } | undefined;
  const active = state?.active === true;
  const kinds = [
    { id: 'aside', label: 'Aside', hint: 'Docks beside the main page. On compact screens, switch between tabs to view it.' },
    { id: 'float', label: 'Float', hint: 'A popup above the main page, like a dialog. It does not take layout space.' },
    { id: 'window', label: 'Window', hint: 'A standalone window with no sidebar or shell. Desktop only.' },
    { id: 'lxapp', label: 'Lxapp', hint: 'Opens the chat app as an aside. Requires home-app privilege.' },
  ];
  const parseSize = (raw: string, label: string): number | string | undefined => {
    const value = raw.trim();
    if (!value) return undefined;
    const percent = value.endsWith('%');
    const n = Number(percent ? value.slice(0, -1).trim() : value);
    if (!Number.isFinite(n) || n <= 0 || (percent && n > 100)) throw new Error(`${label} must be ${percent ? 'a percentage greater than 0% and at most 100%' : 'a positive px value or a percentage like 80%'}.`);
    return percent ? `${n}%` : n;
  };
  let sizeError = '';
  let size: { width?: number | string; height?: number | string } = {};
  try { const w = parseSize(width.value, 'Width'); const h = parseSize(height.value, 'Height'); size = { ...(w === undefined ? {} : { width: w }), ...(h === undefined ? {} : { height: h }) }; } catch (error) { sizeError = error instanceof Error ? error.message : String(error); }
  const choice = (prefix: string, value: string, label: string, local: Local<string>, enabled = true) => <button testId={`${prefix}-${value}`} enabled={enabled} onClick={() => local.set(value)} className={`py-2 text-sm rounded-lg border ${local.value === value ? 'bg-blue-500 border-blue-500 text-white' : 'bg-surface border-line-200 text-gray-600'} ${enabled ? '' : 'opacity-50'}`}>{label}</button>;
  const label = (text: string) => <text className="text-xs text-gray-500">{text}</text>;
  const hint = (text: string) => <column className="bg-surface-50 rounded-lg px-3 py-2"><text className="text-xs text-gray-500 leading-5">{text}</text></column>;
  return <column className="bg-linear-to-br from-surface-50 to-surface-100"><spacer style={{ height: env.insets.top }} /><row className="grow basis-0"><spacer style={{ width: env.insets.left }} /><scroll testId="page-scroll" className="grow basis-0 px-4 pt-6 pb-6 gap-6">
    <column className="pt-4 px-4 items-center gap-2"><text className="text-2xl font-light text-gray-800">lx.surface</text><spacer className="w-16 h-0.5 bg-surface-400" /></column>
    <column className="px-4 py-4 gap-4 bg-surface rounded-xl shadow-sm border border-line-200">
      <column className="gap-2">{label('KIND')}<grid columns={3} className="gap-2">{kinds.map(item => choice('surface-kind', item.id, item.label, kind, !active))}</grid>{hint(kinds.find(item => item.id === kind.value)?.hint || '')}</column>
      {kind.value === 'aside' && <column className="gap-2">{label('EDGE')}<grid columns={2} className="gap-2">{['left','right','top','bottom'].map(value => choice('surface-edge', value, value[0].toUpperCase() + value.slice(1), edge))}</grid></column>}
      {kind.value === 'float' && <column className="gap-2">{label('POSITION')}<grid columns={2} className="gap-2">{['center','top','bottom','left','right'].map(value => choice('surface-position', value, value[0].toUpperCase() + value.slice(1), position))}</grid></column>}
      {kind.value === 'window' && <column className="gap-2">{label('CHROME')}<grid columns={2} className="gap-2">{choice('surface-chrome','system','System',chrome)}{choice('surface-chrome','full','Full',chrome)}</grid>{hint(chrome.value === 'system' ? 'Standard title bar with system close, minimize, and drag.' : 'Page runs to the window edge; system close, minimize, maximize, and drag stay on top.')}</column>}
      <column className="gap-2">{label('SIZE (OPTIONAL - PX OR %)')}<grid columns={2} className="gap-2"><input testId="surface-width" bind={width} placeholder="width (px or %)" className="py-2 px-3 text-sm rounded-lg border border-line-200 bg-surface text-gray-900" /><input testId="surface-height" bind={height} placeholder="height (px or %)" className="py-2 px-3 text-sm rounded-lg border border-line-200 bg-surface text-gray-900" /></grid>{attempted.value && !!sizeError && <text testId="size-error" className="text-xs text-rose-600 dark:text-rose-400">{sizeError}</text>}</column>
      <button testId="open-surface" enabled={!active} onClick={() => { attempted.set(true); if (!sizeError) a.openSurfaceDemo({ verb: kind.value, edge: edge.value, position: position.value, ...(kind.value === 'window' ? { chrome: chrome.value } : {}), ...size }); }} className={`py-2 px-4 rounded-lg text-sm font-medium text-white ${active ? 'bg-surface-300' : 'bg-surface-800'}`}>{`Open ${kind.value}${active ? ' (already open)' : ''}`}</button>
      <text className="text-xs text-gray-500">Edge and position apply when the surface opens. Close and reopen to apply a new value; hiding and showing keeps the current placement.</text>
      {active && <grid columns={3} className="gap-2"><button testId="surface-parent-show" enabled={!state?.visible} onClick={() => a.showActiveSurface()} className={`py-2 px-3 rounded-lg text-sm font-medium ${state?.visible ? 'bg-surface-200 text-gray-500' : 'bg-emerald-500 text-white'}`}>Show</button><button testId="surface-parent-hide" enabled={!!state?.visible} onClick={() => a.hideActiveSurface()} className={`py-2 px-3 rounded-lg text-sm font-medium ${state?.visible ? 'bg-amber-500 text-white' : 'bg-surface-200 text-gray-500'}`}>Hide</button><button testId="surface-parent-close" onClick={() => a.closeActiveSurface()} className="py-2 px-3 rounded-lg text-sm font-medium bg-rose-500 text-white">Close</button></grid>}
      {label('SURFACE STATUS')}<text testId="surface-status" className="text-sm text-gray-800 bg-surface-50 rounded-lg px-3 py-2 font-mono break-all">{state?.message || 'No message received yet.'}</text>
    </column>
  </scroll><spacer style={{ width: env.insets.right }} /></row><spacer style={{ height: Math.max(env.insets.bottom, env.keyboard.height) }} /></column>;
}
