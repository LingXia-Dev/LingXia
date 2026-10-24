import { type View, type Props } from '@lingxia/ui';
import '../../theme/index.css';
const small = 'px-4 py-2 text-xs font-medium bg-surface-100 text-gray-700 rounded-lg';
const titles: Record<string, string> = { appBaseInfo: 'app.getBaseInfo', systemSetting: 'getSystemSetting', autostart: 'app.autostart', notification: 'app.notification', banner: 'app.banner', cache: 'app.cache' };
const yesNo = (v: unknown) => v == null ? '--' : v ? 'Yes' : 'No';
function bytes(v: unknown): string {
  if (typeof v !== 'number') return '--';
  if (v < 1024) return `${v} B`;
  let size = v / 1024, unit = 0;
  const units = ['KB', 'MB', 'GB'];
  while (size >= 1024 && unit < 2) { size /= 1024; unit++; }
  return `${size.toFixed(1)} ${units[unit]}`;
}
function InfoRow({ label, value, testId, last = false }: { label: string; value: unknown; testId?: string; last?: boolean }) {
  const display = value === undefined || value === null || value === '' || (Array.isArray(value) && !value.length) ? '--' : Array.isArray(value) ? value.join('\n') : String(value);
  return <row className={`items-start justify-between gap-3 py-3 ${last ? '' : 'border-b border-line-200'}`}>
    <text className="text-sm text-gray-600 shrink-0">{label}</text>
    <column className="grow basis-0 items-end max-w-[72%]" style={{ minWidth: 0 }}><text testId={testId} className="text-right break-all text-sm font-semibold text-gray-800 bg-blue-50 rounded-lg px-3 py-1">{display}</text></column>
  </row>;
}
function Results({ children, testId, title, accent = 'bg-blue-500', spacing = 'p-5' }: Pick<Props, 'children' | 'testId'> & { title?: string; accent?: string; spacing?: string }) {
  return <column className={spacing}><column testId={testId} className="bg-linear-to-br from-surface-50 to-surface rounded-xl border border-line-200 p-4">
    {title && <row className="items-center gap-2 pb-4"><spacer className={`w-1 h-4 rounded-full ${accent}`} /><text className="text-sm font-semibold text-gray-700">{title}</text></row>}
    {children}
  </column></column>;
}
function Header({ title, description, icon, children, divided = false, compact = false, tone = 'bg-linear-to-br from-blue-50 to-indigo-50' }: Pick<Props, 'children'> & { title: string; description: string; icon: string; divided?: boolean; compact?: boolean; tone?: string }) {
  return <row className={`items-center ${compact ? 'gap-3' : 'gap-4 p-5'} ${divided ? 'border-b border-line-100' : ''}`}>
    <column className={`shrink-0 rounded-xl items-center justify-center ${compact ? 'w-10 h-10' : 'w-12 h-12'} ${tone}`}><text className={compact ? 'text-xl' : 'text-2xl'}>{icon}</text></column>
    <column className="grow basis-0 gap-0.5" style={{ minWidth: 0 }}><text lines={3} className="text-sm font-semibold text-gray-800">{title}</text><text lines={4} className="text-xs text-gray-500">{description}</text></column>
    {children}
  </row>;
}

export function render({ data: d, actions: a, env }: View) {
  const type = String(d.currentType || 'appBaseInfo');
  const base = (d.appBaseInfo || {}) as Record<string, unknown>;
  const setting = (d.systemSetting || {}) as Record<string, unknown>;
  const targets = [['activate','activate'],['native','route \u2192 native'],['page','page \u2192 this page']];
  const targetRows = env.viewport.width - env.insets.left - env.insets.right < 440 ? [targets.slice(0, 2), targets.slice(2)] : [targets];
  const desktop = env.os === 'macOS' || env.os === 'Windows';
  return <column testId="system-page" className="bg-linear-to-br from-surface-50 to-surface-100">
    <spacer style={{ height: env.insets.top }} />
    <row className="grow basis-0"><spacer style={{ width: env.insets.left }} /><scroll testId="page-scroll" className="grow basis-0 px-4 py-6 gap-6">
    {(type !== 'banner' || desktop) && <column className="gap-6">
      <column className="items-center gap-2"><text testId="system-mode-title" className="text-2xl font-light text-gray-800">{titles[type]}</text><spacer className="w-16 h-0.5 bg-surface-400" /></column>
      <column className="bg-surface rounded-2xl shadow-sm border border-line-100">
{type === 'appBaseInfo' && <column><Header divided title="Fetch App Base Info" description="Get app identity (OS, product, versions) plus the language it renders in" icon={'\u{1f4cb}'} tone="bg-linear-to-br from-blue-50 to-indigo-50"><button testId="system-get-base" onClick={() => a.getBaseInfo()} className="px-5 py-2.5 text-sm font-medium bg-linear-to-r from-blue-600 to-blue-500 text-white rounded-xl shadow-sm">Get Info</button></Header>
{!!d.appBaseInfo && <Results title="Result" accent="bg-blue-500"><InfoRow label="Display Language" testId="system-base-language" value={d.displayLanguage} last={false} /><InfoRow label="OS" testId="system-base-os" value={base.os} last={false} /><InfoRow label="Product Name" testId="system-base-productName" value={base.productName} last={false} /><InfoRow label="Product Version" testId="system-base-version" value={base.version} last={false} /><InfoRow label="SDK Version" testId="system-base-sdkVersion" value={base.sdkVersion} last={true} /></Results>}</column>}
{type === 'systemSetting' && <column><Header divided title="Fetch System Setting" description="WiFi, location, and Bluetooth toggles" icon={'\u2699'} tone="bg-linear-to-br from-emerald-50 to-teal-50"><button testId="system-get-settings" onClick={() => a.getSystemSetting()} className="px-5 py-2.5 text-sm font-medium bg-linear-to-r from-emerald-600 to-emerald-500 text-white rounded-xl shadow-sm">Get Info</button></Header>
{!!d.systemSetting && <Results title="Result" accent="bg-emerald-500"><InfoRow label="WiFi Enabled" testId="system-settings-wifiEnabled" value={yesNo(setting.wifiEnabled)} last={false} /><InfoRow label="Location Enabled" testId="system-settings-locationEnabled" value={yesNo(setting.locationEnabled)} last={false} /><InfoRow label="Bluetooth Enabled" testId="system-settings-bluetoothEnabled" value={yesNo(setting.bluetoothEnabled)} last={true} /></Results>}</column>}

{type === 'autostart' && <column>
<Header divided title="Launch at Startup" description={d.autostartSupported ? 'Register this app as a login / startup item' : 'Not available on this platform'} icon={'\u{1f680}'} tone="bg-linear-to-br from-amber-50 to-orange-50">
{!!d.autostartSupported && <switch testId="system-autostart-toggle" value={!!d.autostartEnabled} onChange={() => a.toggleAutostart()}>Launch at startup</switch>}</Header>
<Results title="State" accent="bg-amber-500"><InfoRow label="Supported" testId="system-autostart-status" value={yesNo(d.autostartSupported)} /><InfoRow label="Enabled (OS)" value={yesNo(d.autostartEnabled)} last={!d.autostartError} />{!!d.autostartError && <InfoRow last label="Error" value={d.autostartError} />}<column className="pt-3 items-start"><button testId="system-autostart-refresh" onClick={() => a.refreshAutostart()} className={small}>Re-read OS State</button></column></Results>
</column>}
{type === 'cache' && <column>
<Header divided title="Product Cache" description="Reclaimable product cache - home lxapp only" icon={'\u{1f9f9}'} tone="bg-linear-to-br from-sky-50 to-blue-50"><button testId="system-cache-clear" enabled={!d.cacheBusy} onClick={() => a.clearCache()} className="px-4 py-2 text-xs font-medium bg-sky-500 text-white rounded-lg">{d.cacheBusy ? 'Clearing...' : 'Clear'}</button></Header>
<Results title="State" accent="bg-sky-500"><InfoRow label="Reclaimable (estimate)" testId="system-cache-size" value={bytes(d.cacheBytes)} /><InfoRow label="Last Freed (estimate)" value={bytes(d.cacheFreedBytes)} last={!d.cacheNotice && !d.cacheError} />{!!d.cacheNotice && <InfoRow label="Result" testId="system-cache-status" value={d.cacheNotice} last={!d.cacheError} />}{!!d.cacheError && <InfoRow last label="Error" testId="system-cache-error" value={d.cacheError} />}<text className="pt-3 text-xs text-gray-500">Running apps retain their private caches. Use their menu to clear cache and restart. Estimates exclude WebView cache.</text><column className="pt-3 items-start"><button testId="system-cache-refresh" enabled={!d.cacheBusy} onClick={() => a.refreshCacheSize()} className={small}>Re-read Size</button></column></Results>
</column>}
{type === 'notification' && <column>
<Header divided title="Local Notifications" description={d.notificationSupported ? 'Post and cancel a local banner - Control app only' : 'Not available on this host'} icon={'\u{1f514}'} tone="bg-linear-to-br from-violet-50 to-purple-50">{!!d.notificationSupported && <button testId="system-notification-show" onClick={() => a.showNotification()} className="px-4 py-2 text-xs font-medium bg-violet-500 text-white rounded-lg">Show</button>}</Header>
<Results title="Tap target" accent="bg-violet-500" spacing="px-5 pt-5 pb-4"><text className="text-xs text-gray-500 pb-3">page is navigateTo. route is a host-registered native location. Neither needs a product URL.</text><column className="gap-2">{targetRows.map((options, index) => <row key={String(index)} className="gap-2">{options.map(([value,label]) => <button key={value} testId={`system-notification-target-${value}`} onClick={() => a.setNotificationTarget(value)} className={`px-3 py-1.5 text-xs font-medium rounded-lg ${d.notificationTarget === value ? 'bg-violet-500 text-white' : 'bg-surface-100 text-gray-700'}`}>{label}</button>)}</row>)}</column></Results>
<Results title="State" accent="bg-violet-500" spacing="px-5 pb-5"><InfoRow label="Supported" value={yesNo(d.notificationSupported)} /><InfoRow label="Permission" testId="system-notification-status" value={d.notificationPermission} /><InfoRow label="Last id" testId="system-notification-result" value={d.notificationLastId} last={!d.notificationError} />{!!d.notificationError && <InfoRow last label="Error" value={d.notificationError} />}<row className="pt-3 gap-2"><button testId="system-notification-refresh" onClick={() => a.refreshNotification()} className={small}>Re-read Permission</button><button testId="system-notification-cancel" onClick={() => a.cancelNotification()} className={small}>Cancel</button></row></Results>
</column>}
{type === 'banner' && desktop && <column>
<Header divided title="Desktop Banner" description={d.bannerSupported ? 'Toast and Prompt show a native card at the window top-right' : 'Not available on this host'} icon={'\u{1faa7}'} tone="bg-linear-to-br from-rose-50 to-orange-50" />
<Results title="State" accent="bg-rose-500"><InfoRow label="Supported" value={yesNo(d.bannerSupported)} /><InfoRow label="Last result" testId="system-banner-last" value={d.bannerLast} last={!d.bannerError} />{!!d.bannerError && <InfoRow last label="Error" value={d.bannerError} />}<column className="pt-3 items-start"><button testId="system-banner-reread" onClick={() => a.refreshBanner()} className={small}>Re-read support</button></column></Results>
<Results title="Show" accent="bg-rose-500"><text className="text-xs text-gray-500 pb-3">Toast auto-dismisses. Prompt is a gate - Allow / Deny are on the card. Dismiss is off until a card is up.</text><row className="gap-2 pb-3">{['system','light','dark','#f4f5f7','#1c1c1e'].map(value => <button key={value} onClick={() => a.setBannerBackground(value)} className={`px-3 py-1.5 text-xs font-medium rounded-lg ${d.bannerBackground === value ? 'bg-rose-500 text-white' : 'bg-surface-100 text-gray-700'}`}>{value}</button>)}</row>{!!d.bannerSupported && <row className="gap-2"><button testId="system-banner-toast" enabled={!d.bannerBusy} onClick={() => a.showBannerToast()} className="px-4 py-2 text-xs font-medium bg-rose-500 text-white rounded-lg">Toast</button><button testId="system-banner-prompt" enabled={!d.bannerBusy} onClick={() => a.showBannerPrompt()} className="px-4 py-2 text-xs font-medium bg-rose-500 text-white rounded-lg">Prompt</button><button testId="system-banner-dismiss" enabled={!!d.bannerBusy} onClick={() => a.dismissBanner()} className={small}>Dismiss</button></row>}</Results>
</column>}
</column></column>}
</scroll><spacer style={{ width: env.insets.right }} /></row><spacer style={{ height: Math.max(env.insets.bottom, env.keyboard.height) }} />
</column>;
}
