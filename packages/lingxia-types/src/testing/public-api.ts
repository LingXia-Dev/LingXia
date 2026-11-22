import type {
  Automation,
  BrowserConsole,
  BrowserCookies,
  BrowserDriver,
  ClockDriver,
  DialogDriver,
  DesktopApp,
  DesktopAx,
  DesktopClipboard,
  DesktopDriver,
  DesktopKey,
  DesktopPointer,
  DesktopProcess,
  DesktopWait,
  DesktopWindowDriver,
  DeviceDriver,
  LxAppDriver,
  LxAppManager,
  MockDriver,
  NavDriver,
  NetworkDriver,
  NetworkRoute,
  Scenario,
  PageDriver,
  ProfileDriver,
  PageKey,
  PageGesture,
  PagePointer,
  ShellDriver,
  TerminalDriver,
} from '../automation/index.js';
import type {
  AppearanceApi,
  AppCacheApi,
  AutostartApi,
  BannerApi,
  CompressVideoTask,
  DisplayLanguageApi,
  DownloadTask,
  ClipboardApi,
  FileSystemApi,
  HostAppApi,
  HostAppUpdateInfo,
  HostAppUpdateTask,
  LxFile,
  UiApi,
  NavApi,
  PickApi,
  MediaApi,
  NetworkApi,
  DeviceApi,
  OrientationApi,
  WifiApi,
  NotificationApi,
  PageHandle,
  PreviewMediaHandle,
  TabSurface,
  PageSurface,
  ShellApi,
  Storage,
  SurfaceApi,
  TabBarApi,
  TerminalApi,
  TerminalColorSchemesApi,
  TerminalFontsApi,
  TerminalPreviewController,
  TerminalSettingsApi,
  TrayApi,
  TrayMenuApi,
  HostUpdateApi,
  LxAppUpdateApi,
  MoreActionsApi,
  LxAppApi,
  UploadTask,
  PagePlayer,
} from '../generated/logic.js';

export const LX_API_NAMES = [
  'host',
  'app',
  'automation',
  'clipboard',
  'fs',
  'nav',
  'openExternal',
  'device',
  'media',
  'network',
  'openFile',
  'pick',
  'share',
  'storage',
  'shell',
  'supports',
  'surface',
  'ui',
] as const;

const HOST_APP_API = [
  'appearance',
  'autostart',
  'banner',
  'cache',
  'displayLanguage',
  'env',
  'exit',
  'info',
  'notification',
  'screenshot',
  'serviceEnv',
  'setBadge',
  'setServiceEnv',
  'terminal',
  'tray',
  'update',
] as const;
// `autostart`, `banner`, `cache`, `notification`, and `terminal` are injected
// only where they apply, so a runtime walk of `lx.host` must not require them.
type HostAppOptionalMember = 'autostart' | 'banner' | 'cache' | 'notification' | 'terminal';
const HOST_APP_RUNTIME_API = HOST_APP_API.filter(
  (name): name is Exclude<(typeof HOST_APP_API)[number], HostAppOptionalMember> =>
    name !== 'autostart' &&
    name !== 'banner' &&
    name !== 'cache' &&
    name !== 'notification' &&
    name !== 'terminal',
);
const AUTOSTART_API = ['disable', 'enable', 'isEnabled'] as const;
const BANNER_API = ['dismiss', 'show'] as const;
const NOTIFICATION_API = ['cancel', 'cancelAll', 'show'] as const;
const APP_CACHE_API = ['clear', 'size'] as const;
/** `Live<T>`: every value that changes. */
const LIVE_API = ['get', 'watch'] as const;
const DISPLAY_LANGUAGE_API = ['get', 'preference', 'watch'] as const;
const APPEARANCE_API = DISPLAY_LANGUAGE_API;
const PREFERENCE_API = ['get', 'set', 'watch'] as const;
const HOST_UPDATE_API = ['check', 'claim'] as const;
const TAB_BAR_API = ['update'] as const;
const UI_API = ['actionSheet', 'alert', 'confirm', 'toast'] as const;
const PICK_API = ['capture', 'code', 'directory', 'file', 'files', 'media'] as const;
const DEVICE_API = [
  'haptics',
  'info',
  'location',
  'onKeyDown',
  'onKeyUp',
  'orientation',
  'permission',
  'requestPermission',
  'screen',
  'systemSettings',
] as const;
const ORIENTATION_API = LIVE_API;
const NETWORK_API = ['download', 'status', 'upload', 'wifi'] as const;
const WIFI_API = ['connect', 'current', 'list', 'onConnected'] as const;
const MEDIA_API = ['compressImage', 'compressVideo', 'imageInfo', 'preview', 'saveToLibrary', 'videoInfo', 'videoThumbnail'] as const;
const NAV_API = ['back', 'go', 'leaveApp', 'reset'] as const;
const TERMINAL_API = ['colorSchemes', 'fonts', 'settings', 'windows'] as const;
const TERMINAL_SETTINGS_API = ['get', 'onChange', 'reset', 'update'] as const;
const TERMINAL_COLOR_SCHEMES_API = ['createPreview', 'import', 'list'] as const;
const TERMINAL_FONTS_API = ['list'] as const;
const TERMINAL_PREVIEW_API = ['clear', 'close', 'show'] as const;
const WINDOWS_TERMINAL_API = ['install', 'setEnabled', 'status'] as const;
const SURFACE_NAMESPACE_API = ['app', 'declared', 'get', 'openApp', 'openDeclared', 'openPage', 'openUrl'] as const;
const SHELL_API = ['activate', 'openBuiltin', 'setAppDisplay', 'sidebarActions'] as const;
const SHELL_SIDEBAR_ACTIONS_API = ['remove', 'replace', 'update'] as const;
const TRAY_API = ['menu', 'onClick', 'update'] as const;
const TRAY_MENU_API = ['replace'] as const;
const CLIPBOARD_API = ['clear', 'read', 'readText', 'types', 'write', 'writeText'] as const;
const FILE_SYSTEM_API = [
  'copy',
  'exists',
  'file',
  'mkdir',
  'readDir',
  'remove',
  'rename',
  'stat',
  'userCache',
  'userData',
  'write',
] as const;
const LX_FILE_API = [
  'arrayBuffer',
  'base64',
  'bytes',
  'exists',
  'json',
  'path',
  'stat',
  'text',
] as const;
const STORAGE_API = ['clear', 'delete', 'get', 'has', 'info', 'list', 'set'] as const;
const APP_API = ['info', 'moreActions', 'tabBar', 'update'] as const;
const APP_UPDATE_API = ['apply', 'onFailed', 'onReady'] as const;
const MORE_ACTIONS_API = ['replace'] as const;
const PAGE_PLAYER_API = [
  'exitFullscreen',
  'pause',
  'play',
  'requestFullscreen',
  'seek',
  'setMuted',
  'setPlaybackRate',
  'setQuality',
  'setStreamSource',
  'setVolume',
  'stop',
] as const;

const AUTOMATION_API = [
  'browser',
  'desktop',
  'device',
  'lxapp',
  'lxapps',
  'shell',
  'terminal',
] as const;
const SHELL_DRIVER_API = ['pins', 'setPin', 'reorderPins'] as const;
const TERMINAL_DRIVER_API = ['input', 'newTab', 'setMaximized', 'snapshot', 'split'] as const;
const LXAPP_DRIVER_API = ['clock', 'dialogs', 'eval', 'info', 'mock', 'nav', 'network', 'page', 'pages', 'profile', 'surfaceLayout'] as const;
const MOCK_DRIVER_API = ['reset', 'use'] as const;
const CLOCK_DRIVER_API = ['install', 'runAll', 'setSystemTime', 'tick', 'uninstall'] as const;
const DIALOG_DRIVER_API = [
  'actionSheets',
  'answerNextActionSheet',
  'answerNextModal',
  'modals',
  'setAnswerMode',
  'toasts',
  'unanswered',
  'unwatch',
  'watch',
] as const;
const PROFILE_DRIVER_API = ['checkpoint', 'drop', 'restore'] as const;
const NETWORK_DRIVER_API = ['captureResponses', 'requests', 'responses', 'route', 'unrouteAll'] as const;
const NETWORK_ROUTE_API = ['id', 'pattern', 'requests', 'requestsAfter', 'unroute'] as const;
const SCENARIO_API = ['calls', 'callsAfter', 'name', 'rules', 'unroute', 'variant'] as const;
const LXAPP_MANAGER_API = [
  'applink',
  'close',
  'current',
  'list',
  'open',
  'restart',
  'screenshot',
  'uninstall',
  'windows',
] as const;
const PAGE_DRIVER_API = [
  'action',
  'blur',
  'click',
  'dblclick',
  'environment',
  'eval',
  'fill',
  'gesture',
  'hud',
  'key',
  'pinState',
  'pointer',
  'semanticAction',
  'press',
  'query',
  'screenshot',
  'scroll',
  'scrollTo',
  'selectOption',
  'tree',
  'type',
  'uiFrame',
  'waitFor',
] as const;
const PAGE_POINTER_API = ['click', 'down', 'drag', 'move', 'scroll', 'up'] as const;
const PAGE_KEY_API = ['press', 'type'] as const;
const PAGE_GESTURE_API = ['capabilities', 'chrome', 'edgeBack', 'inputs', 'orientWindow', 'perform', 'refreshIndicator', 'viewport'] as const;
const NAV_DRIVER_API = [
  'back',
  'current',
  'go',
  'info',
  'press',
  'reset',
  'stack',
] as const;
const DEVICE_DRIVER_API = ['get', 'list', 'set'] as const;
const BROWSER_DRIVER_API = [
  'activate',
  'back',
  'check',
  'click',
  'close',
  'console',
  'cookies',
  'current',
  'drag',
  'eval',
  'fill',
  'forward',
  'hover',
  'open',
  'press',
  'query',
  'reload',
  'screenshot',
  'scroll',
  'scrollTo',
  'selectOption',
  'setInputFiles',
  'snapshot',
  'tabs',
  'text',
  'type',
  'uncheck',
  'wait',
] as const;
const BROWSER_COOKIES_API = ['clear', 'delete', 'list', 'set'] as const;
const BROWSER_CONSOLE_API = ['clear', 'list'] as const;
const DESKTOP_DRIVER_API = [
  'app',
  'ax',
  'clipboard',
  'displays',
  'doctor',
  'key',
  'permissions',
  'pixel',
  'pointer',
  'process',
  'screenshot',
  'snapshot',
  'wait',
  'window',
  'windows',
] as const;
const DESKTOP_POINTER_API = ['click', 'down', 'drag', 'move', 'scroll', 'up'] as const;
const DESKTOP_KEY_API = ['down', 'press', 'type', 'up'] as const;
const DESKTOP_WINDOW_API = [
  'activate',
  'close',
  'focus',
  'maximize',
  'minimize',
  'moveTo',
  'moveToDisplay',
  'raise',
  'resize',
  'restore',
  'setAlwaysOnTop',
  'status',
] as const;
const DESKTOP_CLIPBOARD_API = ['clear', 'get', 'paste', 'set'] as const;
const DESKTOP_AX_API = [
  'collapse',
  'expand',
  'focus',
  'hitTest',
  'invoke',
  'query',
  'scrollIntoView',
  'select',
  'setValue',
  'tree',
] as const;
const DESKTOP_WAIT_API = ['ax', 'pixel', 'window'] as const;
const DESKTOP_APP_API = ['launch', 'quit'] as const;
const DESKTOP_PROCESS_API = ['kill', 'list'] as const;

export const LX_RUNTIME_SURFACES = [
  {
    name: 'lx',
    layer: 'logic',
    expression: 'lx',
    members: LX_API_NAMES,
    properties: [
      'app',
      'clipboard',
      'fs',
      'device',
      'host',
      'media',
      'nav',
      'network',
      'pick',
      'shell',
      'storage',
      'surface',
      'ui',
    ],
  },
  {
    name: 'lx.host',
    layer: 'logic',
    expression: 'lx.host',
    members: HOST_APP_RUNTIME_API,
    properties: ['appearance', 'displayLanguage', 'env', 'info', 'tray', 'update'],
  },
  {
    name: 'lx.host.update',
    layer: 'logic',
    expression: 'lx.host.update',
    members: HOST_UPDATE_API,
  },
  {
    name: 'lx.host.autostart',
    layer: 'logic',
    expression: 'lx.host.autostart',
    members: AUTOSTART_API,
  },
  {
    name: 'lx.host.banner',
    layer: 'logic',
    expression: 'lx.host.banner',
    members: BANNER_API,
  },
  {
    name: 'lx.host.notification',
    layer: 'logic',
    expression: 'lx.host.notification',
    members: NOTIFICATION_API,
  },
  {
    name: 'lx.host.cache',
    layer: 'logic',
    expression: 'lx.host.cache',
    members: APP_CACHE_API,
  },
  {
    name: 'lx.host.displayLanguage',
    layer: 'logic',
    expression: 'lx.host.displayLanguage',
    members: DISPLAY_LANGUAGE_API,
    properties: ['preference'],
  },
  {
    name: 'lx.host.displayLanguage.preference',
    layer: 'logic',
    expression: 'lx.host.displayLanguage.preference',
    members: PREFERENCE_API,
  },

  {
    name: 'lx.host.appearance',
    layer: 'logic',
    expression: 'lx.host.appearance',
    members: APPEARANCE_API,
    properties: ['preference'],
  },
  {
    name: 'lx.host.appearance.preference',
    layer: 'logic',
    expression: 'lx.host.appearance.preference',
    members: PREFERENCE_API,
  },


  {
    name: 'lx.app.tabBar',
    layer: 'logic',
    expression: 'lx.app.tabBar',
    members: TAB_BAR_API,
  },
  {
    name: 'lx.ui',
    layer: 'logic',
    expression: 'lx.ui',
    members: UI_API,
  },
  {
    name: 'lx.pick',
    layer: 'logic',
    expression: 'lx.pick',
    members: PICK_API,
  },
  {
    name: 'lx.device',
    layer: 'logic',
    expression: 'lx.device',
    members: DEVICE_API,
    properties: ['info', 'orientation'],
  },
  {
    name: 'lx.device.orientation',
    layer: 'logic',
    expression: 'lx.device.orientation',
    members: ORIENTATION_API,
  },
  {
    name: 'lx.network',
    layer: 'logic',
    expression: 'lx.network',
    members: NETWORK_API,
    properties: ['status', 'wifi'],
  },
  {
    name: 'lx.network.status',
    layer: 'logic',
    expression: 'lx.network.status',
    members: LIVE_API,
  },
  {
    name: 'lx.network.wifi',
    layer: 'logic',
    expression: 'lx.network.wifi',
    members: WIFI_API,
  },
  {
    name: 'lx.media',
    layer: 'logic',
    expression: 'lx.media',
    members: MEDIA_API,
  },
  {
    name: 'lx.nav',
    layer: 'logic',
    expression: 'lx.nav',
    members: NAV_API,
  },
  {
    name: 'lx.host.terminal',
    layer: 'logic',
    expression: 'lx.host.terminal',
    members: TERMINAL_API,
    properties: TERMINAL_API,
    optionalMembers: ['windows'],
  },
  {
    name: 'lx.host.terminal.settings',
    layer: 'logic',
    expression: 'lx.host.terminal.settings',
    members: TERMINAL_SETTINGS_API,
  },
  {
    name: 'lx.host.terminal.colorSchemes',
    layer: 'logic',
    expression: 'lx.host.terminal.colorSchemes',
    members: TERMINAL_COLOR_SCHEMES_API,
  },
  {
    name: 'lx.host.terminal.fonts',
    layer: 'logic',
    expression: 'lx.host.terminal.fonts',
    members: TERMINAL_FONTS_API,
  },
  {
    name: 'lx.host.terminal.windows',
    layer: 'logic',
    expression: 'lx.host.terminal.windows',
    members: WINDOWS_TERMINAL_API,
    optional: true,
  },

  {
    name: 'lx.surface',
    layer: 'logic',
    expression: 'lx.surface',
    members: SURFACE_NAMESPACE_API,
  },
  {
    name: 'lx.shell',
    layer: 'logic',
    expression: 'lx.shell',
    members: SHELL_API,
    properties: SHELL_API,
  },
  {
    name: 'lx.shell.sidebarActions',
    layer: 'logic',
    expression: 'lx.shell.sidebarActions',
    members: SHELL_SIDEBAR_ACTIONS_API,
  },
  { name: 'lx.host.tray', layer: 'logic', expression: 'lx.host.tray', members: TRAY_API, properties: ['menu'] },
  { name: 'lx.host.tray.menu', layer: 'logic', expression: 'lx.host.tray.menu', members: TRAY_MENU_API },
  {
    name: 'lx.fs',
    layer: 'logic',
    expression: 'lx.fs',
    members: FILE_SYSTEM_API,
    properties: ['userCache', 'userData'],
  },
  { name: 'lx.clipboard', layer: 'logic', expression: 'lx.clipboard', members: CLIPBOARD_API },
  { name: 'lx.storage', layer: 'logic', expression: 'lx.storage', members: STORAGE_API },
  { name: 'lx.app', layer: 'logic', expression: 'lx.app', members: APP_API, properties: ['info', 'moreActions', 'tabBar', 'update'] },
  { name: 'lx.app.update', layer: 'logic', expression: 'lx.app.update', members: APP_UPDATE_API },
  { name: 'lx.app.moreActions', layer: 'logic', expression: 'lx.app.moreActions', members: MORE_ACTIONS_API },
  {
    name: 'Automation',
    layer: 'automation',
    expression: 'lx.automation()',
    members: AUTOMATION_API,
    // The desktop tier is deliberately absent on mobile hosts. Keep the
    // portable shape contract honest while the desktop entry covers it on
    // Windows and macOS.
    optionalMembers: ['desktop'],
    properties: ['browser', 'desktop', 'device', 'lxapps', 'shell', 'terminal'],
  },
  { name: 'ShellDriver', layer: 'automation', expression: 'lx.automation().shell', members: SHELL_DRIVER_API },
  {
    name: 'TerminalDriver',
    layer: 'automation',
    expression: 'lx.automation().terminal',
    members: TERMINAL_DRIVER_API,
  },
  {
    name: 'LxAppDriver',
    layer: 'automation',
    expression: 'lx.automation().lxapp()',
    members: LXAPP_DRIVER_API,
    properties: ['clock', 'dialogs', 'mock', 'nav', 'network', 'page', 'profile'],
  },
  { name: 'MockDriver', layer: 'automation', expression: 'lx.automation().lxapp().mock', members: MOCK_DRIVER_API },
  {
    name: 'PageDriver',
    layer: 'automation',
    expression: 'lx.automation().lxapp().page',
    members: PAGE_DRIVER_API,
    properties: ['gesture', 'key', 'pointer'],
  },
  { name: 'PageGesture', layer: 'automation', expression: 'lx.automation().lxapp().page.gesture', members: PAGE_GESTURE_API },
  { name: 'PagePointer', layer: 'automation', expression: 'lx.automation().lxapp().page.pointer', members: PAGE_POINTER_API },
  { name: 'PageKey', layer: 'automation', expression: 'lx.automation().lxapp().page.key', members: PAGE_KEY_API },
  { name: 'NavDriver', layer: 'automation', expression: 'lx.automation().lxapp().nav', members: NAV_DRIVER_API },
  {
    name: 'NetworkDriver',
    layer: 'automation',
    expression: 'lx.automation().lxapp().network',
    members: NETWORK_DRIVER_API,
  },
  {
    name: 'ProfileDriver',
    layer: 'automation',
    expression: 'lx.automation().lxapp().profile',
    members: PROFILE_DRIVER_API,
  },
  {
    name: 'ClockDriver',
    layer: 'automation',
    expression: 'lx.automation().lxapp().clock',
    members: CLOCK_DRIVER_API,
  },
  {
    name: 'DialogDriver',
    layer: 'automation',
    expression: 'lx.automation().lxapp().dialogs',
    members: DIALOG_DRIVER_API,
  },
  {
    name: 'NetworkRoute',
    layer: 'automation',
    expression: 'lx.automation().lxapp().network.route()',
    members: NETWORK_ROUTE_API,
    properties: ['id', 'pattern'],
  },
  {
    name: 'Scenario',
    layer: 'automation',
    expression: 'lx.automation().lxapp().mock.use()',
    members: SCENARIO_API,
    properties: ['name', 'rules', 'variant'],
  },
  { name: 'LxAppManager', layer: 'automation', expression: 'lx.automation().lxapps', members: LXAPP_MANAGER_API },
  { name: 'DeviceDriver', layer: 'automation', expression: 'lx.automation().device', members: DEVICE_DRIVER_API },
  {
    name: 'BrowserDriver',
    layer: 'automation',
    expression: 'lx.automation().browser',
    members: BROWSER_DRIVER_API,
    properties: ['console', 'cookies'],
  },
  { name: 'BrowserConsole', layer: 'automation', expression: 'lx.automation().browser.console', members: BROWSER_CONSOLE_API },
  { name: 'BrowserCookies', layer: 'automation', expression: 'lx.automation().browser.cookies', members: BROWSER_COOKIES_API },
  {
    name: 'DesktopDriver',
    layer: 'automation',
    expression: 'lx.automation().desktop',
    members: DESKTOP_DRIVER_API,
    optional: true,
    properties: ['app', 'ax', 'clipboard', 'key', 'pointer', 'process', 'wait', 'window'],
  },
  { name: 'DesktopPointer', layer: 'automation', expression: 'lx.automation().desktop.pointer', members: DESKTOP_POINTER_API, optional: true },
  { name: 'DesktopKey', layer: 'automation', expression: 'lx.automation().desktop.key', members: DESKTOP_KEY_API, optional: true },
  { name: 'DesktopWindow', layer: 'automation', expression: 'lx.automation().desktop.window', members: DESKTOP_WINDOW_API, optional: true },
  { name: 'DesktopClipboard', layer: 'automation', expression: 'lx.automation().desktop.clipboard', members: DESKTOP_CLIPBOARD_API, optional: true },
  { name: 'DesktopAx', layer: 'automation', expression: 'lx.automation().desktop.ax', members: DESKTOP_AX_API, optional: true },
  { name: 'DesktopWait', layer: 'automation', expression: 'lx.automation().desktop.wait', members: DESKTOP_WAIT_API, optional: true },
  { name: 'DesktopApp', layer: 'automation', expression: 'lx.automation().desktop.app', members: DESKTOP_APP_API, optional: true },
  { name: 'DesktopProcess', layer: 'automation', expression: 'lx.automation().desktop.process', members: DESKTOP_PROCESS_API, optional: true },
] as const;

/** Canonical identifiers used by behavioral automation coverage. */
export const LX_RUNTIME_CAPABILITY_NAMES = LX_RUNTIME_SURFACES.flatMap(({ name, members }) => (
  members.map((member) => `${name}.${member}`)
));

/** Canonical identifiers used by runtime shape automation coverage. */
export const LX_RUNTIME_SHAPE_NAMES = LX_RUNTIME_CAPABILITY_NAMES.map((name) => `shape:${name}`);

const DOWNLOAD_TASK_API = ["result", "pause", "resume", "cancel"] as const;
const UPLOAD_TASK_API = ["result", "cancel"] as const;
const COMPRESS_VIDEO_TASK_API = ["result", "cancel"] as const;
const HOST_UPDATE_INFO_API = ['apply', 'releaseNotes', 'size', 'version', 'via'] as const;
const HOST_UPDATE_TASK_API = ["result"] as const;
const PREVIEW_MEDIA_API = ['completed', 'current', 'onChange', 'presented'] as const;
const PAGE_SURFACE_API = [
  'alive',
  'close',
  'hide',
  'key',
  'kind',
  'onClose',
  'onHide',
  'onMessage',
  'onShow',
  'opened',
  'realized',
  'result',
  'send',
  'show',
  'visible',
] as const;
const TAB_SURFACE_API = [
  'activate',
  'alive',
  'close',
  'key',
  'kind',
  'onClose',
  'realized',
  'scope',
  'visible',
] as const;
const PAGE_HANDLE_API = ['close', 'onMessage', 'opened', 'result', 'send'] as const;

export const LX_RETURNED_OBJECT_SURFACES = [
  {
    name: 'LxFile',
    members: LX_FILE_API,
    properties: ['path'],
    optionalProperties: [],
    fixture: 'runtime-safe',
    factory: 'lx.fs.file',
  },
  {
    name: 'PagePlayer',
    members: PAGE_PLAYER_API,
    properties: [],
    optionalProperties: [],
    fixture: 'native-component',
    factory: 'page.video',
  },
  {
    name: 'DownloadTask',
    members: DOWNLOAD_TASK_API,
    properties: ['result'],
    optionalProperties: [],
    fixture: 'external-service',
    factory: 'lx.network.download',
  },
  {
    name: 'UploadTask',
    members: UPLOAD_TASK_API,
    properties: ['result'],
    optionalProperties: [],
    fixture: 'external-service',
    factory: 'lx.network.upload',
  },
  {
    name: 'CompressVideoTask',
    members: COMPRESS_VIDEO_TASK_API,
    properties: ['result'],
    optionalProperties: [],
    fixture: 'external-media',
    factory: 'lx.media.compressVideo',
  },
  {
    name: 'HostAppUpdateInfo',
    members: HOST_UPDATE_INFO_API,
    properties: ['channel', 'releaseNotes', 'size', 'version'],
    optionalProperties: ['releaseNotes', 'size'],
    fixture: 'external-service',
    factory: 'lx.host.update.check()',
  },
  {
    name: 'HostAppUpdateTask',
    members: HOST_UPDATE_TASK_API,
    properties: ['result'],
    optionalProperties: [],
    fixture: 'external-service',
    factory: 'HostAppUpdateInfo.apply',
  },
  {
    name: 'PreviewMediaHandle',
    members: PREVIEW_MEDIA_API,
    properties: ['completed', 'current', 'presented'],
    optionalProperties: [],
    fixture: 'external-ui',
    factory: 'lx.media.preview',
  },
  {
    name: 'PageSurface',
    members: PAGE_SURFACE_API,
    properties: ['alive', 'key', 'kind', 'opened', 'realized', 'result', 'visible'],
    optionalProperties: ['key'],
    fixture: 'surface',
    factory: 'lx.surface.openPage',
  },
  {
    name: 'TabSurface',
    members: TAB_SURFACE_API,
    properties: ['alive', 'key', 'kind', 'realized', 'scope', 'visible'],
    optionalProperties: ['key'],
    fixture: 'surface',
    factory: 'lx.surface.openUrl',
  },
  {
    name: 'PageHandle',
    members: PAGE_HANDLE_API,
    properties: ['opened', 'result'],
    fixture: 'navigation',
    factory: 'lx.nav.go',
  },
] as const;

/** Canonical identifiers for behavior exercised through objects returned by lx APIs. */
export const LX_RETURNED_OBJECT_CAPABILITY_NAMES = LX_RETURNED_OBJECT_SURFACES.flatMap(({ name, members }) => (
  members.map((member) => `${name}.${member}`)
));

export const LX_RETURNED_OBJECT_SHAPE_NAMES = LX_RETURNED_OBJECT_CAPABILITY_NAMES.map((name) => `shape:${name}`);

/**
 * Shape coverage a run on any host must prove. A surface marked `optional`, or
 * a member listed in `optionalMembers`, is absent on hosts that do not build
 * it — `lx.host.terminal` ships only inside the bundled Terminal Settings lxapp —
 * so a suite that skips it has not covered it. Requiring it here would let the
 * covers gate report a surface nobody touched as proven.
 */
export const LX_REQUIRED_RUNTIME_SHAPE_NAMES = [
  ...LX_RUNTIME_SURFACES.flatMap((surface) => {
    if ('optional' in surface && surface.optional) return [];
    const optionalMembers: readonly string[] =
      'optionalMembers' in surface ? surface.optionalMembers : [];
    return surface.members
      .filter((member) => !optionalMembers.includes(member))
      .map((member) => `shape:${surface.name}.${member}`);
  }),
  ...LX_RETURNED_OBJECT_SURFACES
    .filter((surface) => surface.fixture === 'runtime-safe'
      && !('optional' in surface && surface.optional))
    .flatMap(({ name, members }) => members.map((member) => `shape:${name}.${member}`)),
];

type StringKey<T> = Extract<keyof T, string>;
type Members<T extends readonly string[]> = T[number];
type Exact<T, TMembers extends readonly string[]> = [
  Exclude<StringKey<T>, Members<TMembers>>,
  Exclude<Members<TMembers>, StringKey<T>>,
] extends [never, never]
  ? true
  : false;
type AssertTrue<T extends true> = T;
type PublishedLx = Lx & { automation(): Automation };

export type LxApiManifestGate = [
  AssertTrue<Exact<PublishedLx, typeof LX_API_NAMES>>,
  AssertTrue<Exact<HostAppApi, typeof HOST_APP_API>>,
  AssertTrue<Exact<AutostartApi, typeof AUTOSTART_API>>,
  AssertTrue<Exact<BannerApi, typeof BANNER_API>>,
  AssertTrue<Exact<NotificationApi, typeof NOTIFICATION_API>>,
  AssertTrue<Exact<AppCacheApi, typeof APP_CACHE_API>>,
  AssertTrue<Exact<DisplayLanguageApi, typeof DISPLAY_LANGUAGE_API>>,
  AssertTrue<Exact<AppearanceApi, typeof APPEARANCE_API>>,
  AssertTrue<Exact<TabBarApi, typeof TAB_BAR_API>>,
  AssertTrue<Exact<UiApi, typeof UI_API>>,
  AssertTrue<Exact<PickApi, typeof PICK_API>>,
  AssertTrue<Exact<MediaApi, typeof MEDIA_API>>,
  AssertTrue<Exact<NetworkApi, typeof NETWORK_API>>,
  AssertTrue<Exact<DeviceApi, typeof DEVICE_API>>,
  AssertTrue<Exact<OrientationApi, typeof ORIENTATION_API>>,
  AssertTrue<Exact<WifiApi, typeof WIFI_API>>,
  AssertTrue<Exact<NavApi, typeof NAV_API>>,
  AssertTrue<Exact<TerminalApi, typeof TERMINAL_API>>,
  AssertTrue<Exact<TerminalSettingsApi, typeof TERMINAL_SETTINGS_API>>,
  AssertTrue<Exact<TerminalColorSchemesApi, typeof TERMINAL_COLOR_SCHEMES_API>>,
  AssertTrue<Exact<TerminalFontsApi, typeof TERMINAL_FONTS_API>>,
  AssertTrue<Exact<TerminalPreviewController, typeof TERMINAL_PREVIEW_API>>,
  AssertTrue<Exact<ShellApi, typeof SHELL_API>>,
  AssertTrue<Exact<ShellApi['sidebarActions'], typeof SHELL_SIDEBAR_ACTIONS_API>>,
  AssertTrue<Exact<TrayApi, typeof TRAY_API>>,
  AssertTrue<Exact<TrayMenuApi, typeof TRAY_MENU_API>>,
  AssertTrue<Exact<HostUpdateApi, typeof HOST_UPDATE_API>>,
  AssertTrue<Exact<LxAppUpdateApi, typeof APP_UPDATE_API>>,
  AssertTrue<Exact<MoreActionsApi, typeof MORE_ACTIONS_API>>,
  AssertTrue<Exact<AppearanceApi['preference'], typeof PREFERENCE_API>>,
  AssertTrue<Exact<DisplayLanguageApi['preference'], typeof PREFERENCE_API>>,
  AssertTrue<Exact<NetworkApi['status'], typeof LIVE_API>>,
  AssertTrue<Exact<FileSystemApi, typeof FILE_SYSTEM_API>>,
  AssertTrue<Exact<ClipboardApi, typeof CLIPBOARD_API>>,
  AssertTrue<Exact<LxFile, typeof LX_FILE_API>>,
  AssertTrue<Exact<Storage, typeof STORAGE_API>>,
  AssertTrue<Exact<LxAppApi, typeof APP_API>>,
  AssertTrue<Exact<PagePlayer, typeof PAGE_PLAYER_API>>,
  AssertTrue<Exact<Automation, typeof AUTOMATION_API>>,
  AssertTrue<Exact<ShellDriver, typeof SHELL_DRIVER_API>>,
  AssertTrue<Exact<TerminalDriver, typeof TERMINAL_DRIVER_API>>,
  AssertTrue<Exact<LxAppDriver, typeof LXAPP_DRIVER_API>>,
  AssertTrue<Exact<LxAppManager, typeof LXAPP_MANAGER_API>>,
  AssertTrue<Exact<PageDriver, typeof PAGE_DRIVER_API>>,
  AssertTrue<Exact<PagePointer, typeof PAGE_POINTER_API>>,
  AssertTrue<Exact<PageGesture, typeof PAGE_GESTURE_API>>,
  AssertTrue<Exact<PageKey, typeof PAGE_KEY_API>>,
  AssertTrue<Exact<NavDriver, typeof NAV_DRIVER_API>>,
  AssertTrue<Exact<NetworkDriver, typeof NETWORK_DRIVER_API>>,
  AssertTrue<Exact<NetworkRoute, typeof NETWORK_ROUTE_API>>,
  AssertTrue<Exact<Scenario, typeof SCENARIO_API>>,
  AssertTrue<Exact<MockDriver, typeof MOCK_DRIVER_API>>,
  AssertTrue<Exact<ProfileDriver, typeof PROFILE_DRIVER_API>>,
  AssertTrue<Exact<ClockDriver, typeof CLOCK_DRIVER_API>>,
  AssertTrue<Exact<DialogDriver, typeof DIALOG_DRIVER_API>>,
  AssertTrue<Exact<DeviceDriver, typeof DEVICE_DRIVER_API>>,
  AssertTrue<Exact<BrowserDriver, typeof BROWSER_DRIVER_API>>,
  AssertTrue<Exact<BrowserCookies, typeof BROWSER_COOKIES_API>>,
  AssertTrue<Exact<BrowserConsole, typeof BROWSER_CONSOLE_API>>,
  AssertTrue<Exact<DesktopDriver, typeof DESKTOP_DRIVER_API>>,
  AssertTrue<Exact<DesktopPointer, typeof DESKTOP_POINTER_API>>,
  AssertTrue<Exact<DesktopKey, typeof DESKTOP_KEY_API>>,
  AssertTrue<Exact<DesktopWindowDriver, typeof DESKTOP_WINDOW_API>>,
  AssertTrue<Exact<DesktopClipboard, typeof DESKTOP_CLIPBOARD_API>>,
  AssertTrue<Exact<DesktopAx, typeof DESKTOP_AX_API>>,
  AssertTrue<Exact<DesktopWait, typeof DESKTOP_WAIT_API>>,
  AssertTrue<Exact<DesktopApp, typeof DESKTOP_APP_API>>,
  AssertTrue<Exact<DesktopProcess, typeof DESKTOP_PROCESS_API>>,
  AssertTrue<Exact<DownloadTask, typeof DOWNLOAD_TASK_API>>,
  AssertTrue<Exact<UploadTask, typeof UPLOAD_TASK_API>>,
  AssertTrue<Exact<CompressVideoTask, typeof COMPRESS_VIDEO_TASK_API>>,
  AssertTrue<Exact<HostAppUpdateInfo, typeof HOST_UPDATE_INFO_API>>,
  AssertTrue<Exact<HostAppUpdateTask, typeof HOST_UPDATE_TASK_API>>,
  AssertTrue<Exact<PreviewMediaHandle, typeof PREVIEW_MEDIA_API>>,
  AssertTrue<Exact<SurfaceApi, typeof SURFACE_NAMESPACE_API>>,
  AssertTrue<Exact<PageSurface, typeof PAGE_SURFACE_API>>,
  AssertTrue<Exact<Extract<TabSurface, { scope: 'tab' }>, typeof TAB_SURFACE_API>>,
  AssertTrue<Exact<PageHandle, typeof PAGE_HANDLE_API>>,
];

type FunctionKeys<T> = {
  [K in StringKey<T>]-?: NonNullable<T[K]> extends (...args: never[]) => unknown ? K : never;
}[StringKey<T>];

/** The declared type each Logic runtime surface walks. */
type RuntimeSurfaceTypes = {
  lx: PublishedLx;
  'lx.host': HostAppApi;
  'lx.host.update': HostUpdateApi;
  'lx.host.autostart': AutostartApi;
  'lx.host.banner': BannerApi;
  'lx.host.notification': NotificationApi;
  'lx.host.cache': AppCacheApi;
  'lx.host.displayLanguage': DisplayLanguageApi;
  'lx.host.displayLanguage.preference': DisplayLanguageApi['preference'];
  'lx.host.appearance': AppearanceApi;
  'lx.host.appearance.preference': AppearanceApi['preference'];
  'lx.app.tabBar': TabBarApi;
  'lx.ui': UiApi;
  'lx.pick': PickApi;
  'lx.device': DeviceApi;
  'lx.device.orientation': OrientationApi;
  'lx.network': NetworkApi;
  'lx.network.status': NetworkApi['status'];
  'lx.network.wifi': WifiApi;
  'lx.media': MediaApi;
  'lx.nav': NavApi;
  'lx.host.terminal': TerminalApi;
  'lx.host.terminal.settings': TerminalSettingsApi;
  'lx.host.terminal.colorSchemes': TerminalColorSchemesApi;
  'lx.host.terminal.fonts': TerminalFontsApi;
  'lx.host.terminal.windows': NonNullable<TerminalApi['windows']>;
  'lx.surface': SurfaceApi;
  'lx.shell': ShellApi;
  'lx.shell.sidebarActions': ShellApi['sidebarActions'];
  'lx.host.tray': TrayApi;
  'lx.host.tray.menu': TrayMenuApi;
  'lx.fs': FileSystemApi;
  'lx.clipboard': ClipboardApi;
  'lx.storage': Storage;
  'lx.app': LxAppApi;
  'lx.app.update': LxAppUpdateApi;
  'lx.app.moreActions': MoreActionsApi;
};

/**
 * `surface.member` for each runtime member the shape check would require to be a
 * function while its declared type is not one (an object such as `preference`
 * belongs in `properties`).
 */
type RuntimeKindMismatch<S> = S extends { name: infer N; members: infer M extends readonly string[] }
  ? N extends keyof RuntimeSurfaceTypes
    ? `${N}.${Exclude<
        Members<M>,
        (S extends { properties: infer P extends readonly string[] } ? Members<P> : never)
        | FunctionKeys<RuntimeSurfaceTypes[N]>
      >}`
    : never
  : never;
type AssertNever<T extends never> = T;
type RuntimeSurfaceName = (typeof LX_RUNTIME_SURFACES)[number]['name'];

export type LxRuntimeKindGate = [
  AssertNever<RuntimeKindMismatch<(typeof LX_RUNTIME_SURFACES)[number]>>,
  // Every Logic surface has a declared type to check against.
  AssertNever<Exclude<Extract<(typeof LX_RUNTIME_SURFACES)[number], { layer: 'logic' }>['name'], keyof RuntimeSurfaceTypes>>,
  AssertNever<Exclude<keyof RuntimeSurfaceTypes, RuntimeSurfaceName>>,
];
