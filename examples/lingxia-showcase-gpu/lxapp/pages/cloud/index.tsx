import { type View } from '@lingxia/ui';
import '../../theme/index.css';

type Tenant = { tenantId?: string; tenantName?: string; displayName?: string; shortName?: string; logoUrl?: string };
type User = { id?: string; name?: string; avatar?: string };

export function render({ data, actions, env }: View) {
  if (data.type === 'mqtt') return <Mqtt data={data} actions={actions} env={env} />;
  if (data.type === 'functions') return <Functions data={data} actions={actions} env={env} />;
  return <Auth data={data} actions={actions} env={env} />;
}

const tenantName = (tenant: Tenant) => tenant.displayName || tenant.tenantName || tenant.tenantId || 'Unknown tenant';
function AuthHeading({ title, description, icon, tone, compact = false, count }: { title: string; description: string; icon: string; tone: string; compact?: boolean; count?: number }) {
  return <row className={`items-center gap-3 ${compact ? '' : 'p-5 border-b border-line-100'}`}>
    <column className={`shrink-0 items-center justify-center rounded-xl ${compact ? 'w-10 h-10' : 'w-12 h-12'} ${tone}`}><text className={compact ? 'text-xl text-gray-900' : 'text-2xl text-gray-900'}>{icon}</text></column>
    <column className="grow basis-0 min-w-0 gap-0.5"><text className="text-sm text-gray-800 font-semibold">{title}</text><text className="text-xs text-gray-500">{description}</text></column>{count !== undefined && <text testId="cloud-tenants-count" className="text-xs text-gray-500">{String(count)}</text>}
  </row>;
}
function Auth({ data: d, actions: a, env }: Pick<View, 'data' | 'actions' | 'env'>) {
  const tenant = d.tenant as Tenant | null;
  const user = d.user as User | null;
  const tenants = (d.tenants || []) as Tenant[];
  const status = String(d.status || 'Idle');
  const lower = status.toLowerCase();
  const statusColor = lower.includes('failed') || lower.includes('error') ? 'text-red-600 dark:text-red-400 bg-red-50' : lower.includes('succeeded') || lower === 'ready' ? 'text-emerald-700 dark:text-emerald-400 bg-emerald-50' : ['...', 'starting', 'switching', 'logging'].some(word => lower.includes(word)) ? 'text-amber-700 dark:text-amber-400 bg-amber-50' : 'text-gray-800 bg-blue-50';
  const card = 'bg-surface rounded-2xl shadow-sm border border-line-100';
  return <column testId="cloud-page" className="bg-linear-to-br from-surface-50 to-surface-100">
    <spacer style={{ height: env.insets.top }} /><row className="grow basis-0"><spacer style={{ width: env.insets.left }} /><scroll testId="page-scroll" className="grow basis-0 px-4 py-6 gap-5">
      <column className={card}><column className="p-6 gap-4"><AuthHeading compact icon="C" tone="bg-linear-to-br from-sky-50 to-cyan-50" title="Cloud Authentication" description="Login returns the active identity; the home lxapp manages identity list, activation and logout." />
        <grid columns={2} className="gap-3">
          {!tenant && <button testId="cloud-login" onClick={() => a.loginInteractive()} className="py-3 text-sm font-medium rounded-xl shadow-sm bg-linear-to-r from-sky-600 to-sky-500 text-white">Interactive Login</button>}
          <button testId="cloud-add-identity" onClick={() => a.addTenant()} className="py-3 text-sm font-medium rounded-xl shadow-sm bg-linear-to-r from-emerald-600 to-emerald-500 text-white">Add Identity</button>
          <button testId="cloud-logout" onClick={() => a.logoutCurrentTenant()} className="py-3 text-sm font-medium rounded-xl shadow-sm bg-linear-to-r from-rose-600 to-rose-500 text-white">Logout Current Tenant</button>
        </grid>
      </column></column>
      <column className={card}><AuthHeading icon="S" tone="bg-linear-to-br from-blue-50 to-indigo-50" title="Current State" description="Latest cloud auth snapshot from the runtime" /><column className="p-5"><column className="rounded-xl border border-line-200 bg-linear-to-br from-surface-50 to-surface p-4">
        <row className="items-center gap-2 pb-4"><spacer className="w-1 h-4 bg-blue-500 rounded-full" /><text className="text-sm font-semibold text-gray-700">Status</text></row>
        <row className="items-center justify-between py-3 border-b border-line-200 gap-4"><text className="text-sm text-gray-600 shrink-0">Status</text><column className="grow basis-0 min-w-0 items-end"><text testId="cloud-auth-status" className={`text-sm font-semibold px-3 py-1 rounded-lg text-right ${statusColor}`}>{status}</text></column></row>
        <row className="items-center justify-between py-3 border-b border-line-200 gap-4"><text className="text-sm text-gray-600 shrink-0">Active Tenant</text><row className="grow basis-0 min-w-0 items-center gap-3 px-3 py-2 bg-blue-50 rounded-lg">
          {!!tenant?.logoUrl && <image src={tenant.logoUrl} alt="" fit="cover" className="w-8 h-8 shrink-0 rounded-full border border-white bg-surface shadow-sm" />}
          <column className="grow basis-0 min-w-0 items-end gap-1"><text testId="cloud-current-tenant" className="text-sm font-semibold text-gray-800 text-right">{tenant ? tenantName(tenant) : 'No active tenant'}</text>{!!tenant?.shortName && <text testId="cloud-tenant-short-name" className="text-xs font-semibold px-2 py-0.5 rounded-full bg-blue-100 text-blue-700 dark:text-blue-400">{tenant.shortName}</text>}</column>
        </row></row>
        <row className="items-center justify-between py-3 border-b border-line-200 gap-4"><text className="text-sm text-gray-600 shrink-0">Active User</text><row className="grow basis-0 min-w-0 items-center gap-3 px-3 py-2 bg-emerald-50 rounded-lg">
          {!!user?.avatar && <image src={user.avatar} alt="" fit="cover" className="w-8 h-8 shrink-0 rounded-full border border-white bg-surface shadow-sm" />}
          <column className="grow basis-0 min-w-0 items-end gap-1"><text testId="cloud-current-user" className="text-sm font-semibold text-gray-800 text-right">{user?.name || 'Not signed in'}</text>{!!user?.id && <text testId="cloud-user-id" className="text-xs font-semibold px-2 py-0.5 rounded-full bg-emerald-100 text-emerald-700 dark:text-emerald-400 break-all">{user.id}</text>}</column>
        </row></row>
      </column></column></column>
      <column className={card}><AuthHeading icon="T" tone="bg-linear-to-br from-purple-50 to-pink-50" title="Tenants" description="Activate or remove identities returned by lx.auth.list()" count={tenants.length} /><column className="p-5 gap-3">
        {!tenants.length && <text testId="cloud-identities-empty" className="text-sm text-gray-500">No tenants yet. Run interactive login first.</text>}
        {tenants.map((item, index) => {
          const active = !!item.tenantId && item.tenantId === tenant?.tenantId;
          return <button key={`${item.tenantId || 'tenant'}-${index}`} testId={`cloud-activate-${item.tenantId || index}`} accessibilityLabel={`${tenantName(item)}, ${active ? 'Active' : item.tenantId ? 'Switch' : 'Unavailable'}`} enabled={!!item.tenantId && !active} onClick={() => a.activateTenant({ tenantId: item.tenantId })} className={`p-4 rounded-xl border text-left ${active ? 'border-sky-200 bg-sky-50' : 'border-line-200 bg-surface'}`}>
            <row className="items-center justify-between gap-3">{!!item.logoUrl && <image src={item.logoUrl} alt="" fit="cover" className="w-10 h-10 shrink-0 rounded-full border border-line-200 bg-surface" />}<column className="grow basis-0 min-w-0 items-start gap-1"><text className="text-left text-sm font-semibold text-gray-800">{tenantName(item)}</text>{!!item.shortName && <text className="text-left text-xs font-semibold px-2 py-0.5 rounded-full bg-surface-100 text-gray-600">{item.shortName}</text>}<text className="text-left text-xs text-gray-500 break-all">{item.tenantId || 'Missing tenantId'}</text></column><text className="text-xs px-2 py-1 rounded-full bg-surface-100 text-gray-600">{active ? 'Active' : item.tenantId ? 'Switch' : 'Unavailable'}</text></row>
          </button>;
        })}
      </column></column>
    </scroll><spacer style={{ width: env.insets.right }} /></row><spacer style={{ height: Math.max(env.insets.bottom, env.keyboard.height) }} />
  </column>;
}

function Header({ title, description }: { title: string; description: string }) {
  return <column className="p-5 gap-0.5 border-b border-line-100"><text className="text-sm text-gray-800 font-semibold">{title}</text><text className="text-xs text-gray-500">{description}</text></column>;
}
function Mqtt({ data: d, actions: a, env }: Pick<View, 'data' | 'actions' | 'env'>) {
  const card = 'bg-surface rounded-2xl shadow-sm border border-line-100';
  const state = String(d.mqttRuntimeState || 'idle');
  const connected = state === 'connected';
  const waiting = state === 'connecting' || state === 'reconnecting';
  const failed = state === 'disconnected' || state === 'error';
  const dot = connected ? 'bg-emerald-500' : waiting ? 'bg-amber-500' : failed ? 'bg-red-500' : 'bg-surface-400';
  const stateColor = connected ? 'text-emerald-600 dark:text-emerald-400' : waiting ? 'text-amber-600 dark:text-amber-400' : failed ? 'text-red-600 dark:text-red-400' : 'text-gray-600';
  const active = !!d.mqttSubscribed;
  return <column testId="cloud-page" className="bg-linear-to-br from-surface-50 to-surface-100">
    <spacer style={{ height: env.insets.top }} /><row className="grow basis-0"><spacer style={{ width: env.insets.left }} /><scroll testId="page-scroll" className="grow basis-0 px-4 py-6 gap-5">
      <column className={card}><Header title="Runtime Connection" description="MQTT runtime status for the shared cloud session." /><column className="p-5"><column className="bg-sky-50/70 rounded-xl p-4 gap-2">
        <row className="items-center justify-between gap-3"><text className="text-xs text-gray-600">Connection state</text><row className="items-center gap-1.5"><spacer className={`w-2 h-2 rounded-full ${dot}`} /><text testId="cloud-mqtt-state" className={`font-mono text-xs font-semibold ${stateColor}`}>{state}</text></row></row>
        <row className="items-start justify-between gap-3"><text className="text-xs text-gray-600 shrink-0">Last error</text><text testId="cloud-mqtt-error" className={`grow basis-0 min-w-0 font-mono text-xs text-right break-all ${d.mqttLastError ? 'text-red-600 dark:text-red-400' : 'text-gray-800'}`}>{String(d.mqttLastError || '-')}</text></row>
      </column></column></column>
      <column className={card}><Header title="Topic Subscription" description="Manage the demo topic subscription and inspect how many messages this page has received." /><column className="p-5 gap-4">
        <column className="rounded-xl border border-line-200 bg-emerald-50/60 p-4 gap-3">
          <row className="items-start justify-between gap-3"><column className="grow basis-0 min-w-0 gap-2"><text className="text-xs text-emerald-700 dark:text-emerald-400">TOPIC</text><text testId="cloud-mqtt-topic" className="font-mono text-sm text-gray-900 break-all">{String(d.mqttTopicFilter || 'demo/mqtt')}</text></column><text className={`shrink-0 rounded-full px-2.5 py-1 text-xs font-semibold ${active ? 'bg-emerald-100 text-emerald-700 dark:text-emerald-400' : 'bg-surface-100 text-gray-500'}`}>{active ? 'active' : 'inactive'}</text></row>
          <text testId="cloud-mqtt-status" className="text-sm font-semibold text-gray-800">{String(d.mqttStatus || 'Idle')}</text>
          <grid columns={2} className="gap-3 pt-1"><button testId="cloud-mqtt-start" enabled={!active} onClick={() => a.startMqttDemo()} className={`py-3 text-sm font-medium rounded-xl shadow-sm ${active ? 'bg-surface-100 text-gray-400' : 'bg-linear-to-r from-emerald-600 to-emerald-500 text-white'}`}>Subscribe</button><button testId="cloud-mqtt-stop" enabled={active} onClick={() => a.stopMqttDemo()} className={`py-3 text-sm font-medium rounded-xl shadow-sm ${active ? 'bg-linear-to-r from-rose-600 to-rose-500 text-white' : 'bg-surface-100 text-gray-400'}`}>Unsubscribe</button></grid>
        </column>
        <column className="gap-2 rounded-xl bg-surface p-4 border border-line-200"><row className="items-center justify-between gap-3"><text className="text-xs text-gray-600">Subscription state</text><text testId="cloud-mqtt-active" className={`font-mono text-xs font-semibold ${active ? 'text-emerald-600 dark:text-emerald-400' : 'text-gray-500'}`}>{active ? 'active' : 'inactive'}</text></row><row className="items-center justify-between gap-3"><text className="text-xs text-gray-600">Messages received</text><text testId="cloud-mqtt-count" className="font-mono text-xs text-gray-800">{String(d.mqttMessageCount || 0)}</text></row></column>
      </column></column>
      <column className={card}><Header title="Latest Message" description="Incoming message data is written back through setData." /><column className="p-5">
        {!d.mqttMessageCount ? <text testId="cloud-mqtt-empty" className="rounded-xl border border-line-300 bg-surface-50 p-6 text-sm text-gray-500">No message yet. Publish one to the topic above.</text> : <column className="rounded-xl border border-line-200 bg-linear-to-br from-surface-50 to-surface p-4 gap-3">
          <column className="gap-1"><text className="text-xs text-gray-500">TOPIC</text><text testId="cloud-mqtt-last-topic" className="font-mono text-sm text-gray-800 break-all">{String(d.mqttLastTopic || '')}</text></column>
          <column className="gap-1"><text className="text-xs text-gray-500">RECEIVED AT</text><text testId="cloud-mqtt-received-at" className="text-sm text-gray-800">{String(d.mqttLastReceivedAt || '')}</text></column>
          <column className="gap-1"><text className="text-xs text-gray-500">PAYLOAD</text><text testId="cloud-mqtt-payload" className="rounded-lg bg-slate-900 text-[#f1f5f9] p-3 text-xs font-mono break-all">{String(d.mqttLastPayload || '(empty payload)')}</text></column>
        </column>}
      </column></column>
    </scroll><spacer style={{ width: env.insets.right }} /></row><spacer style={{ height: Math.max(env.insets.bottom, env.keyboard.height) }} />
  </column>;
}



function Functions({ data: d, actions: a, env }: Pick<View, 'data' | 'actions' | 'env'>) {
  const names = (d.functionsAvailable || []) as string[];
  const narrow = env.viewport.width - env.insets.left - env.insets.right < 440;
  const rows = narrow ? Array.from({ length: Math.ceil(names.length / 2) }, (_, i) => names.slice(i * 2, i * 2 + 2)) : [names];
  const card = 'bg-surface rounded-2xl shadow-sm border border-line-100';
  return <column testId="cloud-page" className="bg-linear-to-br from-surface-50 to-surface-100">
    <spacer style={{ height: env.insets.top }} /><row className="grow basis-0"><spacer style={{ width: env.insets.left }} /><scroll testId="page-scroll" className="grow basis-0 px-4 py-6 gap-5">
      <column className={card}><Header title="Current LxApp Functions" description="These sample calls use lx.cloud.invoke(...) for the current lxapp. They are demo actions, not a runtime-discovered manifest list." /><column className="p-5 gap-4">
        <column className="rounded-xl border border-line-200 bg-indigo-50/60 p-4 gap-2"><text className="text-xs text-indigo-700 dark:text-indigo-400">STATUS</text><text testId="cloud-functions-status" className="text-sm font-semibold text-gray-800">{String(d.functionsStatus || 'Idle')}</text>
          {!!names.length && <column className="pt-2 gap-2">{rows.map((items, index) => <row key={String(index)} className="gap-2">{items.map(name => <button key={name} testId={`cloud-function-${name}`} onClick={() => a.callNamedFunction({ name })} className="px-4 py-2 rounded-xl text-sm font-medium bg-linear-to-r from-sky-600 to-sky-500 text-white">{`Call ${name}`}</button>)}</row>)}</column>}
        </column>
        <column className="rounded-xl border border-line-200 bg-surface-50 p-4 gap-3"><text className="text-xs text-gray-500">DEMO FUNCTIONS</text>{!names.length ? <text testId="cloud-functions-empty" className="text-sm text-gray-500">No demo functions are configured for this page.</text> : <column className="gap-2">{[names].map((items, index) => <row key={String(index)} className="gap-2">{items.map(name => <text key={name} className="rounded-full bg-surface border border-line-200 px-3 py-1 text-xs font-mono text-gray-700">{name}</text>)}</row>)}</column>}</column>
      </column></column>
      <column className={card}><Header title="Last Result" description="Most recent cloud function invocation output." /><column className="p-5"><column className="rounded-xl border border-line-200 bg-linear-to-br from-surface-50 to-surface p-4 gap-2">
        <text className="text-xs text-gray-500">FUNCTION</text><text testId="cloud-functions-last-call" className="text-sm font-semibold text-gray-800">{String(d.functionsLastCall || '-')}</text>
        <text className="pt-2 text-xs text-gray-500">RESULT</text><text testId="cloud-functions-result" className="rounded-lg bg-slate-900 text-[#f1f5f9] p-3 text-xs font-mono break-all min-h-24">{String(d.functionsLastResult || '(no result yet)')}</text>
      </column></column></column>
    </scroll><spacer style={{ width: env.insets.right }} /></row><spacer style={{ height: Math.max(env.insets.bottom, env.keyboard.height) }} />
  </column>;
}
