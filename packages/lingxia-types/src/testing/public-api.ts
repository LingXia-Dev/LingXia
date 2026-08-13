import type {
  Automation,
  BrowserCookies,
  BrowserDriver,
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
  NavDriver,
  PageDriver,
  PageKey,
  PagePointer,
  ShellDriver,
  TerminalDriver,
} from '../automation/index.js';
import type {
  AppearanceApi,
  AutostartApi,
  CompressVideoTask,
  DownloadTask,
  FileSystemApi,
  HostAppApi,
  HostAppUpdateInfo,
  HostAppUpdateTask,
  LxFile,
  NavigationBarApi,
  PageMessagePort,
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
  UpdateManager,
  UploadTask,
  VideoContext,
} from '../generated/logic.js';

export const LX_API_NAMES = [
  'app',
  'appearance',
  'automation',
  'chooseDirectory',
  'chooseFile',
  'chooseMedia',
  'compressImage',
  'compressVideo',
  'connectWifi',
  'createVideoContext',
  'downloadFile',
  'env',
  'extractVideoThumbnail',
  'fs',
  'getConnectedWifi',
  'getDeviceInfo',
  'getImageInfo',
  'getLocation',
  'getLxAppInfo',
  'getNetworkInfo',
  'getScreenInfo',
  'getStorage',
  'getSystemSetting',
  'getUpdateManager',
  'getVideoInfo',
  'getWifiList',
  'hideToast',
  'makePhoneCall',
  'navigateBack',
  'navigateBackApp',
  'navigateTo',
  'navigateToApp',
  'navigationBar',
  'onDeviceOrientationChange',
  'onKeyDown',
  'onKeyUp',
  'onNetworkChange',
  'onWifiConnected',
  'openExternal',
  'openFile',
  'previewMedia',
  'reLaunch',
  'redirectTo',
  'saveImageToPhotosAlbum',
  'saveVideoToPhotosAlbum',
  'scanCode',
  'setDeviceOrientation',
  'setMoreActions',
  'share',
  'shell',
  'showActionSheet',
  'showModal',
  'showToast',
  'startPullDownRefresh',
  'startWifi',
  'stopPullDownRefresh',
  'stopWifi',
  'supports',
  'surface',
  'switchTab',
  'tabBar',
  'terminal',
  'tray',
  'uploadFile',
  'vibrateLong',
  'vibrateShort',
] as const;

const HOST_APP_API = [
  'autostart',
  'checkUpdate',
  'envVersion',
  'exit',
  'getBaseInfo',
  'screenshot',
  'setBadge',
] as const;
const HOST_APP_RUNTIME_API = HOST_APP_API.filter((name) => name !== 'autostart');
const AUTOSTART_API = ['isEnabled', 'setEnabled'] as const;
const APPEARANCE_API = ['get', 'set'] as const;
const NAVIGATION_BAR_API = ['update'] as const;
const TAB_BAR_API = ['update'] as const;
const TERMINAL_API = ['colorSchemes', 'fonts', 'settings', 'windows'] as const;
const TERMINAL_SETTINGS_API = ['get', 'onChange', 'reset', 'update'] as const;
const TERMINAL_COLOR_SCHEMES_API = ['createPreview', 'import', 'list'] as const;
const TERMINAL_FONTS_API = ['list'] as const;
const TERMINAL_PREVIEW_API = ['clear', 'close', 'show'] as const;
const WINDOWS_TERMINAL_API = ['install', 'setEnabled', 'status'] as const;
const ENV_API = ['USER_CACHE_PATH', 'USER_DATA_PATH'] as const;
const SHELL_API = ['openApp', 'openBuiltin', 'openDeclared', 'reconfigure', 'sidebarActions'] as const;
const SHELL_SIDEBAR_ACTIONS_API = ['clear', 'remove', 'replace', 'update'] as const;
const TRAY_API = ['hide', 'onClick', 'setBadge', 'setIcon', 'setMenu', 'setTitle', 'show'] as const;
const FILE_SYSTEM_API = [
  'copy',
  'exists',
  'file',
  'mkdir',
  'readDir',
  'remove',
  'rename',
  'stat',
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
const STORAGE_API = ['clear', 'delete', 'get', 'info', 'list', 'set'] as const;
const UPDATE_MANAGER_API = ['applyUpdate', 'onUpdateFailed', 'onUpdateReady'] as const;
const VIDEO_CONTEXT_API = [
  'exitFullScreen',
  'pause',
  'play',
  'requestFullScreen',
  'seek',
  'setStreamSource',
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
const SHELL_DRIVER_API = ['pins', 'setPin'] as const;
const TERMINAL_DRIVER_API = ['input', 'newTab', 'setMaximized', 'snapshot', 'split'] as const;
const LXAPP_DRIVER_API = ['eval', 'info', 'nav', 'page', 'pages', 'surfaceLayout'] as const;
const LXAPP_MANAGER_API = [
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
  'click',
  'eval',
  'fill',
  'key',
  'pointer',
  'press',
  'query',
  'screenshot',
  'scroll',
  'scrollTo',
  'type',
  'waitFor',
] as const;
const PAGE_POINTER_API = ['click', 'down', 'drag', 'move', 'scroll', 'up'] as const;
const PAGE_KEY_API = ['press', 'type'] as const;
const NAV_DRIVER_API = [
  'back',
  'current',
  'info',
  'redirect',
  'relaunch',
  'stack',
  'switchTab',
  'to',
] as const;
const DEVICE_DRIVER_API = ['get', 'list', 'set'] as const;
const BROWSER_DRIVER_API = [
  'activate',
  'back',
  'click',
  'close',
  'cookies',
  'current',
  'eval',
  'fill',
  'forward',
  'open',
  'press',
  'query',
  'reload',
  'screenshot',
  'scroll',
  'scrollTo',
  'tabs',
  'type',
  'wait',
] as const;
const BROWSER_COOKIES_API = ['clear', 'delete', 'list', 'set'] as const;
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
    optionalMembers: ['terminal'],
    properties: [
      'app',
      'appearance',
      'env',
      'fs',
      'navigationBar',
      'shell',
      'tabBar',
      'terminal',
      'tray',
    ],
  },
  {
    name: 'lx.app',
    layer: 'logic',
    expression: 'lx.app',
    members: HOST_APP_RUNTIME_API,
    properties: ['envVersion'],
  },
  {
    name: 'lx.app.autostart',
    layer: 'logic',
    expression: 'lx.app.autostart',
    members: AUTOSTART_API,
    optional: true,
  },
  {
    name: 'lx.appearance',
    layer: 'logic',
    expression: 'lx.appearance',
    members: APPEARANCE_API,
  },
  {
    name: 'lx.navigationBar',
    layer: 'logic',
    expression: 'lx.navigationBar',
    members: NAVIGATION_BAR_API,
  },
  {
    name: 'lx.tabBar',
    layer: 'logic',
    expression: 'lx.tabBar',
    members: TAB_BAR_API,
  },
  {
    name: 'lx.terminal',
    layer: 'logic',
    expression: 'lx.terminal',
    members: TERMINAL_API,
    properties: TERMINAL_API,
    optional: true,
  },
  {
    name: 'lx.terminal.settings',
    layer: 'logic',
    expression: 'lx.terminal?.settings',
    members: TERMINAL_SETTINGS_API,
    optional: true,
  },
  {
    name: 'lx.terminal.colorSchemes',
    layer: 'logic',
    expression: 'lx.terminal?.colorSchemes',
    members: TERMINAL_COLOR_SCHEMES_API,
    optional: true,
  },
  {
    name: 'lx.terminal.fonts',
    layer: 'logic',
    expression: 'lx.terminal?.fonts',
    members: TERMINAL_FONTS_API,
    optional: true,
  },
  {
    name: 'lx.terminal.windows',
    layer: 'logic',
    expression: 'lx.terminal?.windows',
    members: WINDOWS_TERMINAL_API,
    optional: true,
  },
  {
    name: 'lx.env',
    layer: 'logic',
    expression: 'lx.env',
    members: ENV_API,
    properties: ENV_API,
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
  { name: 'lx.tray', layer: 'logic', expression: 'lx.tray', members: TRAY_API },
  { name: 'lx.fs', layer: 'logic', expression: 'lx.fs', members: FILE_SYSTEM_API },
  { name: 'Storage', layer: 'logic', expression: 'lx.getStorage()', members: STORAGE_API },
  { name: 'UpdateManager', layer: 'logic', expression: 'lx.getUpdateManager()', members: UPDATE_MANAGER_API },
  {
    name: 'Automation',
    layer: 'automation',
    expression: 'lx.automation()',
    members: AUTOMATION_API,
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
    properties: ['nav', 'page'],
  },
  {
    name: 'PageDriver',
    layer: 'automation',
    expression: 'lx.automation().lxapp().page',
    members: PAGE_DRIVER_API,
    properties: ['key', 'pointer'],
  },
  { name: 'PagePointer', layer: 'automation', expression: 'lx.automation().lxapp().page.pointer', members: PAGE_POINTER_API },
  { name: 'PageKey', layer: 'automation', expression: 'lx.automation().lxapp().page.key', members: PAGE_KEY_API },
  { name: 'NavDriver', layer: 'automation', expression: 'lx.automation().lxapp().nav', members: NAV_DRIVER_API },
  { name: 'LxAppManager', layer: 'automation', expression: 'lx.automation().lxapps', members: LXAPP_MANAGER_API },
  { name: 'DeviceDriver', layer: 'automation', expression: 'lx.automation().device', members: DEVICE_DRIVER_API },
  {
    name: 'BrowserDriver',
    layer: 'automation',
    expression: 'lx.automation().browser',
    members: BROWSER_DRIVER_API,
    properties: ['cookies'],
  },
  { name: 'BrowserCookies', layer: 'automation', expression: 'lx.automation().browser.cookies', members: BROWSER_COOKIES_API },
  {
    name: 'DesktopDriver',
    layer: 'automation',
    expression: 'lx.automation().desktop',
    members: DESKTOP_DRIVER_API,
    properties: ['app', 'ax', 'clipboard', 'key', 'pointer', 'process', 'wait', 'window'],
  },
  { name: 'DesktopPointer', layer: 'automation', expression: 'lx.automation().desktop.pointer', members: DESKTOP_POINTER_API },
  { name: 'DesktopKey', layer: 'automation', expression: 'lx.automation().desktop.key', members: DESKTOP_KEY_API },
  { name: 'DesktopWindow', layer: 'automation', expression: 'lx.automation().desktop.window', members: DESKTOP_WINDOW_API },
  { name: 'DesktopClipboard', layer: 'automation', expression: 'lx.automation().desktop.clipboard', members: DESKTOP_CLIPBOARD_API },
  { name: 'DesktopAx', layer: 'automation', expression: 'lx.automation().desktop.ax', members: DESKTOP_AX_API },
  { name: 'DesktopWait', layer: 'automation', expression: 'lx.automation().desktop.wait', members: DESKTOP_WAIT_API },
  { name: 'DesktopApp', layer: 'automation', expression: 'lx.automation().desktop.app', members: DESKTOP_APP_API },
  { name: 'DesktopProcess', layer: 'automation', expression: 'lx.automation().desktop.process', members: DESKTOP_PROCESS_API },
] as const;

/** Canonical identifiers used by behavioral automation coverage. */
export const LX_RUNTIME_CAPABILITY_NAMES = LX_RUNTIME_SURFACES.flatMap(({ name, members }) => (
  members.map((member) => `${name}.${member}`)
));

/** Canonical identifiers used by runtime shape automation coverage. */
export const LX_RUNTIME_SHAPE_NAMES = LX_RUNTIME_CAPABILITY_NAMES.map((name) => `shape:${name}`);

const DOWNLOAD_TASK_API = [
  'abort',
  'cancel',
  'catch',
  'finally',
  'next',
  'pause',
  'resume',
  'return',
  'then',
  'wait',
] as const;
const UPLOAD_TASK_API = ['cancel', 'catch', 'finally', 'next', 'return', 'then', 'wait'] as const;
const COMPRESS_VIDEO_TASK_API = ['cancel', 'catch', 'finally', 'next', 'return', 'then', 'wait'] as const;
const HOST_UPDATE_INFO_API = ['apply', 'isForceUpdate', 'releaseNotes', 'size', 'version'] as const;
const HOST_UPDATE_TASK_API = ['catch', 'finally', 'next', 'return', 'then', 'wait'] as const;
const PREVIEW_MEDIA_API = ['completed', 'current', 'onChange', 'presented'] as const;
const SURFACE_NAMESPACE_API = ['get', 'onContext', 'openDeclared', 'openPage', 'openUrl'] as const;
const PAGE_SURFACE_API = [
  'alive',
  'close',
  'hide',
  'id',
  'key',
  'kind',
  'onClose',
  'onHide',
  'onMessage',
  'onShow',
  'postMessage',
  'realized',
  'show',
  'visible',
] as const;
const TAB_SURFACE_API = [
  'activate',
  'alive',
  'close',
  'id',
  'key',
  'kind',
  'onClose',
  'realized',
  'scope',
  'visible',
] as const;
const PAGE_MESSAGE_PORT_API = ['onMessage', 'postMessage'] as const;

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
    name: 'VideoContext',
    members: VIDEO_CONTEXT_API,
    properties: [],
    optionalProperties: [],
    fixture: 'runtime-safe',
    factory: 'lx.createVideoContext',
  },
  {
    name: 'DownloadTask',
    members: DOWNLOAD_TASK_API,
    properties: [],
    optionalProperties: [],
    fixture: 'external-service',
    factory: 'lx.downloadFile',
  },
  {
    name: 'UploadTask',
    members: UPLOAD_TASK_API,
    properties: [],
    optionalProperties: [],
    fixture: 'external-service',
    factory: 'lx.uploadFile',
  },
  {
    name: 'CompressVideoTask',
    members: COMPRESS_VIDEO_TASK_API,
    properties: [],
    optionalProperties: [],
    fixture: 'external-media',
    factory: 'lx.compressVideo',
  },
  {
    name: 'HostAppUpdateInfo',
    members: HOST_UPDATE_INFO_API,
    properties: ['isForceUpdate', 'releaseNotes', 'size', 'version'],
    optionalProperties: ['releaseNotes', 'size'],
    fixture: 'external-service',
    factory: 'lx.app.checkUpdate().update',
  },
  {
    name: 'HostAppUpdateTask',
    members: HOST_UPDATE_TASK_API,
    properties: [],
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
    factory: 'lx.previewMedia',
  },
  {
    name: 'PageSurface',
    members: PAGE_SURFACE_API,
    properties: ['alive', 'id', 'key', 'kind', 'realized', 'visible'],
    optionalProperties: ['key'],
    fixture: 'surface',
    factory: 'lx.surface.openPage',
  },
  {
    name: 'TabSurface',
    members: TAB_SURFACE_API,
    properties: ['alive', 'id', 'key', 'kind', 'realized', 'scope', 'visible'],
    optionalProperties: ['key'],
    fixture: 'surface',
    factory: 'lx.surface.openUrl',
  },
  {
    name: 'PageMessagePort',
    members: PAGE_MESSAGE_PORT_API,
    properties: [],
    optionalProperties: [],
    fixture: 'navigation',
    factory: 'lx.navigateTo',
  },
] as const;

/** Canonical identifiers for behavior exercised through objects returned by lx APIs. */
export const LX_RETURNED_OBJECT_CAPABILITY_NAMES = LX_RETURNED_OBJECT_SURFACES.flatMap(({ name, members }) => (
  members.map((member) => `${name}.${member}`)
));

export const LX_RETURNED_OBJECT_SHAPE_NAMES = LX_RETURNED_OBJECT_CAPABILITY_NAMES.map((name) => `shape:${name}`);

export const LX_REQUIRED_RUNTIME_SHAPE_NAMES = [
  ...LX_RUNTIME_SHAPE_NAMES,
  ...LX_RETURNED_OBJECT_SURFACES
    .filter(({ fixture }) => fixture === 'runtime-safe')
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
  AssertTrue<Exact<AppearanceApi, typeof APPEARANCE_API>>,
  AssertTrue<Exact<NavigationBarApi, typeof NAVIGATION_BAR_API>>,
  AssertTrue<Exact<TabBarApi, typeof TAB_BAR_API>>,
  AssertTrue<Exact<TerminalApi, typeof TERMINAL_API>>,
  AssertTrue<Exact<TerminalSettingsApi, typeof TERMINAL_SETTINGS_API>>,
  AssertTrue<Exact<TerminalColorSchemesApi, typeof TERMINAL_COLOR_SCHEMES_API>>,
  AssertTrue<Exact<TerminalFontsApi, typeof TERMINAL_FONTS_API>>,
  AssertTrue<Exact<TerminalPreviewController, typeof TERMINAL_PREVIEW_API>>,
  AssertTrue<Exact<LxEnv, typeof ENV_API>>,
  AssertTrue<Exact<ShellApi, typeof SHELL_API>>,
  AssertTrue<Exact<ShellApi['sidebarActions'], typeof SHELL_SIDEBAR_ACTIONS_API>>,
  AssertTrue<Exact<TrayApi, typeof TRAY_API>>,
  AssertTrue<Exact<FileSystemApi, typeof FILE_SYSTEM_API>>,
  AssertTrue<Exact<LxFile, typeof LX_FILE_API>>,
  AssertTrue<Exact<Storage, typeof STORAGE_API>>,
  AssertTrue<Exact<UpdateManager, typeof UPDATE_MANAGER_API>>,
  AssertTrue<Exact<VideoContext, typeof VIDEO_CONTEXT_API>>,
  AssertTrue<Exact<Automation, typeof AUTOMATION_API>>,
  AssertTrue<Exact<ShellDriver, typeof SHELL_DRIVER_API>>,
  AssertTrue<Exact<TerminalDriver, typeof TERMINAL_DRIVER_API>>,
  AssertTrue<Exact<LxAppDriver, typeof LXAPP_DRIVER_API>>,
  AssertTrue<Exact<LxAppManager, typeof LXAPP_MANAGER_API>>,
  AssertTrue<Exact<PageDriver, typeof PAGE_DRIVER_API>>,
  AssertTrue<Exact<PagePointer, typeof PAGE_POINTER_API>>,
  AssertTrue<Exact<PageKey, typeof PAGE_KEY_API>>,
  AssertTrue<Exact<NavDriver, typeof NAV_DRIVER_API>>,
  AssertTrue<Exact<DeviceDriver, typeof DEVICE_DRIVER_API>>,
  AssertTrue<Exact<BrowserDriver, typeof BROWSER_DRIVER_API>>,
  AssertTrue<Exact<BrowserCookies, typeof BROWSER_COOKIES_API>>,
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
  AssertTrue<Exact<TabSurface, typeof TAB_SURFACE_API>>,
  AssertTrue<Exact<PageMessagePort, typeof PAGE_MESSAGE_PORT_API>>,
];
