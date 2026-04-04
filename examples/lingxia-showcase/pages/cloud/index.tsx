import { useLxPage } from '@lingxia/react';
import '../../tailwind.css';

type TenantLike = {
  tenantId?: string;
  tenantName?: string;
  displayName?: string;
  logoUrl?: string;
};

type CloudPageType = 'auth' | 'mqtt' | 'functions';

type PageData = {
  type?: CloudPageType;
  status?: string;
  tenant?: TenantLike | null;
  tenants?: TenantLike[];

  mqttStatus?: string;
  mqttRuntimeState?: string;
  mqttLastError?: string;
  mqttSubscribed?: boolean;
  mqttTopicFilter?: string;
  mqttMessageCount?: number;
  mqttLastTopic?: string;
  mqttLastPayload?: string;
  mqttLastReceivedAt?: string;

  functionsStatus?: string;
  functionsAvailable?: string[];
  functionsLastCall?: string;
  functionsLastResult?: string;
};

type PageActions = {
  loginInteractive: () => void | Promise<void>;
  logoutCurrentTenant: () => void | Promise<void>;
  switchTenant: (params: { tenantId: string }) => void | Promise<void>;
  startMqttDemo: () => void | Promise<void>;
  stopMqttDemo: () => void | Promise<void>;
  callNamedFunction: (params: { name: string }) => void | Promise<void>;
};

function getTenantId(tenant: TenantLike | null | undefined): string {
  return tenant?.tenantId || '';
}

function getTenantName(tenant: TenantLike | null | undefined): string {
  return tenant?.displayName || tenant?.tenantName || tenant?.tenantId || 'Unknown tenant';
}

function getTenantLogoUrl(tenant: TenantLike | null | undefined): string {
  return tenant?.logoUrl || '';
}

function mqttStateColor(state: string): string {
  switch (state) {
    case 'connected': return 'text-emerald-600';
    case 'connecting':
    case 'reconnecting': return 'text-amber-600';
    case 'disconnected':
    case 'error': return 'text-red-600';
    default: return 'text-gray-600';
  }
}

function mqttStateDot(state: string): string {
  switch (state) {
    case 'connected': return 'bg-emerald-500';
    case 'connecting':
    case 'reconnecting': return 'bg-amber-500';
    case 'disconnected':
    case 'error': return 'bg-red-500';
    default: return 'bg-gray-400';
  }
}

function authStatusColor(status: string): string {
  const lower = status.toLowerCase();
  if (lower.includes('failed') || lower.includes('error')) return 'text-red-600 bg-red-50';
  if (lower.includes('succeeded') || lower === 'ready') return 'text-emerald-700 bg-emerald-50';
  if (lower.includes('...') || lower.includes('starting') || lower.includes('switching') || lower.includes('logging')) return 'text-amber-700 bg-amber-50';
  return 'text-gray-800 bg-blue-50';
}

function CloudAuthView({
  status,
  tenant,
  tenants,
  loginInteractive,
  logoutCurrentTenant,
  switchTenant,
}: {
  status: string;
  tenant: TenantLike | null;
  tenants: TenantLike[];
  loginInteractive: () => void | Promise<void>;
  logoutCurrentTenant: () => void | Promise<void>;
  switchTenant: (params: { tenantId: string }) => void | Promise<void>;
}) {
  const activeTenantId = getTenantId(tenant);

  return (
    <>
      <div className="mb-5 bg-white rounded-2xl shadow-sm border border-gray-100 overflow-hidden">
        <div className="p-6">
          <div className="flex items-center gap-3 mb-4">
            <div className="flex items-center justify-center w-10 h-10 rounded-xl bg-gradient-to-br from-sky-50 to-cyan-50">
              <span className="text-xl">C</span>
            </div>
            <div>
              <div className="text-sm text-gray-800 font-semibold">Cloud Authentication</div>
                <div className="text-xs text-gray-500 mt-0.5">
                  Any lxapp may start interactive login and inspect the shared auth state.
                </div>
            </div>
          </div>
          <div className="grid grid-cols-2 gap-3">
            <button
              onClick={loginInteractive}
              className="py-3 text-sm font-medium transition-all duration-200 rounded-xl shadow-sm active:scale-[0.98] bg-gradient-to-r from-sky-600 to-sky-500 hover:from-sky-500 hover:to-sky-600 text-white"
            >
              Interactive Login
            </button>
            <button
              onClick={logoutCurrentTenant}
              className="py-3 text-sm font-medium transition-all duration-200 rounded-xl shadow-sm active:scale-[0.98] bg-gradient-to-r from-rose-600 to-rose-500 hover:from-rose-500 hover:to-rose-600 text-white"
            >
              Logout Current Tenant
            </button>
          </div>
        </div>
      </div>

      <div className="mb-5 bg-white rounded-2xl shadow-sm border border-gray-100 overflow-hidden">
        <div className="flex items-center gap-4 px-5 py-5 border-b border-gray-100">
          <div className="flex items-center justify-center w-12 h-12 rounded-xl bg-gradient-to-br from-blue-50 to-indigo-50">
            <span className="text-2xl">S</span>
          </div>
          <div className="flex-1">
            <div className="text-sm text-gray-800 font-semibold">Current State</div>
            <div className="text-xs text-gray-500 mt-0.5">Latest cloud auth snapshot from the runtime</div>
          </div>
        </div>
        <div className="p-5">
          <div className="rounded-xl border border-gray-200 bg-gradient-to-br from-gray-50 to-white p-4">
            <div className="flex items-center gap-2 mb-4">
              <span className="w-1 h-4 bg-blue-500 rounded-full"></span>
              <h4 className="text-sm font-semibold text-gray-700">Status</h4>
            </div>
            <div className="flex justify-between items-center py-3 border-b border-gray-200 gap-4">
              <span className="text-sm text-gray-600">Status</span>
              <span className={`text-sm font-semibold px-3 py-1 rounded-lg text-right ${authStatusColor(status)}`}>{status}</span>
            </div>
            <div className="flex justify-between items-center py-3 border-b border-gray-200 gap-4">
              <span className="text-sm text-gray-600">Active Tenant</span>
              <div className="flex items-center gap-3 px-3 py-2 bg-blue-50 rounded-lg">
                {getTenantLogoUrl(tenant) ? (
                  <img
                    src={getTenantLogoUrl(tenant)}
                    alt={getTenantName(tenant)}
                    className="w-8 h-8 rounded-full border border-white shadow-sm bg-white object-cover"
                  />
                ) : null}
                <span className="text-sm font-semibold text-gray-800 text-right">
                  {tenant ? getTenantName(tenant) : 'No active tenant'}
                </span>
              </div>
            </div>
          </div>
        </div>
      </div>

      <div className="bg-white rounded-2xl shadow-sm border border-gray-100 overflow-hidden">
        <div className="flex items-center gap-4 px-5 py-5 border-b border-gray-100">
          <div className="flex items-center justify-center w-12 h-12 rounded-xl bg-gradient-to-br from-purple-50 to-pink-50">
            <span className="text-2xl">T</span>
          </div>
          <div className="flex-1">
            <div className="text-sm text-gray-800 font-semibold">Tenants</div>
            <div className="text-xs text-gray-500 mt-0.5">Switch the active tenant after login</div>
          </div>
          <div className="text-xs text-gray-500">{tenants.length}</div>
        </div>
        <div className="p-5">
          {tenants.length === 0 ? (
            <div className="text-sm text-gray-500">No tenants yet. Run interactive login first.</div>
          ) : (
            <div className="space-y-3">
              {tenants.map((item, index) => {
                const tenantId = getTenantId(item);
                const isActive = tenantId !== '' && tenantId === activeTenantId;
                return (
                  <button
                    key={`${tenantId || 'tenant'}-${index}`}
                    disabled={!tenantId || isActive}
                    onClick={() => tenantId && switchTenant({ tenantId })}
                    className={`w-full p-4 rounded-xl border text-left transition-colors ${
                      isActive
                        ? 'border-sky-200 bg-sky-50'
                        : 'border-gray-200 bg-white hover:border-emerald-200 hover:bg-emerald-50/30'
                    }`}
                  >
                    <div className="flex items-center justify-between gap-3">
                      <div className="flex items-center gap-3 min-w-0">
                        {getTenantLogoUrl(item) ? (
                          <img
                            src={getTenantLogoUrl(item)}
                            alt={getTenantName(item)}
                            className="w-10 h-10 rounded-full border border-gray-200 bg-white object-cover shrink-0"
                          />
                        ) : null}
                        <div className="min-w-0">
                          <div className="text-sm font-semibold text-gray-800">{getTenantName(item)}</div>
                          <div className="mt-1 text-xs text-gray-500 break-all">{tenantId || 'Missing tenantId'}</div>
                        </div>
                      </div>
                      <div className="text-xs px-2 py-1 rounded-full bg-gray-100 text-gray-600">
                        {isActive ? 'Active' : tenantId ? 'Switch' : 'Unavailable'}
                      </div>
                    </div>
                  </button>
                );
              })}
            </div>
          )}
        </div>
      </div>
    </>
  );
}

function CloudMqttView({
  mqttStatus,
  mqttRuntimeState,
  mqttLastError,
  mqttSubscribed,
  mqttTopicFilter,
  mqttMessageCount,
  mqttLastTopic,
  mqttLastPayload,
  mqttLastReceivedAt,
  startMqttDemo,
  stopMqttDemo,
}: {
  mqttStatus: string;
  mqttRuntimeState: string;
  mqttLastError: string;
  mqttSubscribed: boolean;
  mqttTopicFilter: string;
  mqttMessageCount: number;
  mqttLastTopic: string;
  mqttLastPayload: string;
  mqttLastReceivedAt: string;
  startMqttDemo: () => void | Promise<void>;
  stopMqttDemo: () => void | Promise<void>;
}) {
  return (
    <>
      <div className="mb-5 bg-white rounded-2xl shadow-sm border border-gray-100 overflow-hidden">
        <div className="px-5 py-5 border-b border-gray-100">
          <div className="text-sm text-gray-800 font-semibold">Runtime Connection</div>
          <div className="text-xs text-gray-500 mt-0.5">
            MQTT runtime status for the shared cloud session.
          </div>
        </div>
        <div className="p-5">
          <div className="grid gap-2 rounded-xl bg-sky-50/70 p-4 text-xs text-gray-600">
            <div className="flex items-center justify-between gap-3">
              <span>Connection state</span>
              <span className="flex items-center gap-1.5">
                <span className={`inline-block w-2 h-2 rounded-full ${mqttStateDot(mqttRuntimeState)}`} />
                <span className={`font-mono font-semibold ${mqttStateColor(mqttRuntimeState)}`}>{mqttRuntimeState}</span>
              </span>
            </div>
            <div className="flex items-center justify-between gap-3">
              <span>Last error</span>
              <span className={`font-mono text-right ${mqttLastError ? 'text-red-600' : 'text-gray-800'}`}>{mqttLastError || '-'}</span>
            </div>
          </div>
        </div>
      </div>

      <div className="mb-5 bg-white rounded-2xl shadow-sm border border-gray-100 overflow-hidden">
        <div className="px-5 py-5 border-b border-gray-100">
          <div className="text-sm text-gray-800 font-semibold">Topic Subscription</div>
          <div className="text-xs text-gray-500 mt-0.5">
            Manage the demo topic subscription and inspect how many messages this page has received.
          </div>
        </div>
        <div className="p-5 space-y-4">
          <div className="rounded-xl border border-gray-200 bg-emerald-50/60 p-4">
            <div className="flex items-start justify-between gap-3">
              <div>
                <div className="text-xs uppercase tracking-wide text-emerald-700">Topic</div>
                <div className="mt-2 font-mono text-sm text-gray-900 break-all">{mqttTopicFilter}</div>
              </div>
              <span className={`shrink-0 rounded-full px-2.5 py-1 text-xs font-semibold ${mqttSubscribed ? 'bg-emerald-100 text-emerald-700' : 'bg-gray-100 text-gray-500'}`}>
                {mqttSubscribed ? 'active' : 'inactive'}
              </span>
            </div>
            <div className="mt-3 text-sm font-semibold text-gray-800">{mqttStatus}</div>
            <div className="mt-4 grid grid-cols-2 gap-3">
              <button
                onClick={startMqttDemo}
                disabled={mqttSubscribed}
                className={`py-3 text-sm font-medium rounded-xl transition-all duration-200 shadow-sm active:scale-[0.98] ${
                  mqttSubscribed
                    ? 'bg-gray-100 text-gray-400'
                    : 'bg-gradient-to-r from-emerald-600 to-emerald-500 text-white'
                }`}
              >
                Subscribe
              </button>
              <button
                onClick={stopMqttDemo}
                disabled={!mqttSubscribed}
                className={`py-3 text-sm font-medium rounded-xl transition-all duration-200 shadow-sm active:scale-[0.98] ${
                  mqttSubscribed
                    ? 'bg-gradient-to-r from-rose-600 to-rose-500 text-white'
                    : 'bg-gray-100 text-gray-400'
                }`}
              >
                Unsubscribe
              </button>
            </div>
          </div>

          <div className="grid gap-2 rounded-xl bg-white p-4 text-xs text-gray-600 border border-gray-200">
            <div className="flex items-center justify-between gap-3">
              <span>Subscription state</span>
              <span className={`font-mono font-semibold ${mqttSubscribed ? 'text-emerald-600' : 'text-gray-500'}`}>
                {mqttSubscribed ? 'active' : 'inactive'}
              </span>
            </div>
            <div className="flex items-center justify-between gap-3">
              <span>Messages received</span>
              <span className="font-mono text-gray-800">{mqttMessageCount}</span>
            </div>
          </div>
        </div>
      </div>

      <div className="bg-white rounded-2xl shadow-sm border border-gray-100 overflow-hidden">
        <div className="px-5 py-5 border-b border-gray-100">
          <div className="text-sm text-gray-800 font-semibold">Latest Message</div>
          <div className="text-xs text-gray-500 mt-0.5">Incoming message data is written back through setData.</div>
        </div>
        <div className="p-5">
          {mqttMessageCount === 0 ? (
            <div className="rounded-xl border border-dashed border-gray-300 bg-gray-50 p-6 text-sm text-gray-500">
              No message yet. Publish one to the topic above.
            </div>
          ) : (
            <div className="rounded-xl border border-gray-200 bg-gradient-to-br from-gray-50 to-white p-4">
              <div className="grid gap-3 text-sm">
                <div>
                  <div className="text-xs uppercase tracking-wide text-gray-500">Topic</div>
                  <div className="mt-1 font-mono text-gray-800 break-all">{mqttLastTopic}</div>
                </div>
                <div>
                  <div className="text-xs uppercase tracking-wide text-gray-500">Received at</div>
                  <div className="mt-1 text-gray-800">{mqttLastReceivedAt}</div>
                </div>
                <div>
                  <div className="text-xs uppercase tracking-wide text-gray-500">Payload</div>
                  <pre className="mt-1 rounded-lg bg-slate-900 text-slate-100 p-3 text-xs whitespace-pre-wrap break-all">
                    {mqttLastPayload || '(empty payload)'}
                  </pre>
                </div>
              </div>
            </div>
          )}
        </div>
      </div>
    </>
  );
}

function CloudFunctionsView({
  functionsStatus,
  functionsAvailable,
  functionsLastCall,
  functionsLastResult,
  callNamedFunction,
}: {
  functionsStatus: string;
  functionsAvailable: string[];
  functionsLastCall: string;
  functionsLastResult: string;
  callNamedFunction: (params: { name: string }) => void | Promise<void>;
}) {
  return (
    <>
      <div className="mb-5 bg-white rounded-2xl shadow-sm border border-gray-100 overflow-hidden">
        <div className="px-5 py-5 border-b border-gray-100">
          <div className="text-sm text-gray-800 font-semibold">Current LxApp Functions</div>
          <div className="text-xs text-gray-500 mt-0.5">
            These sample calls use `lx.cloud.invoke(...)` for the current lxapp. They are demo actions, not a runtime-discovered manifest list.
          </div>
        </div>
        <div className="p-5 space-y-4">
          <div className="rounded-xl border border-gray-200 bg-indigo-50/60 p-4">
            <div className="text-xs uppercase tracking-wide text-indigo-700">Status</div>
            <div className="mt-2 text-sm font-semibold text-gray-800">{functionsStatus}</div>
            {functionsAvailable.length === 0 ? null : (
              <div className="mt-4 flex flex-wrap gap-2">
                {functionsAvailable.map((name) => (
                  <button
                    key={name}
                    onClick={() => callNamedFunction({ name })}
                    className="px-4 py-2 rounded-xl text-sm font-medium bg-gradient-to-r from-sky-600 to-sky-500 text-white"
                  >
                    Call {name}
                  </button>
                ))}
              </div>
            )}
          </div>
          <div className="rounded-xl border border-gray-200 bg-gray-50 p-4">
            <div className="text-xs uppercase tracking-wide text-gray-500">Demo Functions</div>
            {functionsAvailable.length === 0 ? (
              <div className="mt-2 text-sm text-gray-500">
                No demo functions are configured for this page.
              </div>
            ) : (
              <div className="mt-3 flex flex-wrap gap-2">
                {functionsAvailable.map((name) => (
                  <span
                    key={name}
                    className="inline-flex items-center rounded-full bg-white border border-gray-200 px-3 py-1 text-xs font-mono text-gray-700"
                  >
                    {name}
                  </span>
                ))}
              </div>
            )}
          </div>
        </div>
      </div>

      <div className="bg-white rounded-2xl shadow-sm border border-gray-100 overflow-hidden">
        <div className="px-5 py-5 border-b border-gray-100">
          <div className="text-sm text-gray-800 font-semibold">Last Result</div>
          <div className="text-xs text-gray-500 mt-0.5">Most recent cloud function invocation output.</div>
        </div>
        <div className="p-5">
          <div className="rounded-xl border border-gray-200 bg-gradient-to-br from-gray-50 to-white p-4">
            <div className="text-xs uppercase tracking-wide text-gray-500">Function</div>
            <div className="mt-2 text-sm font-semibold text-gray-800">{functionsLastCall || '-'}</div>
            <div className="mt-4 text-xs uppercase tracking-wide text-gray-500">Result</div>
            <pre className="mt-2 rounded-lg bg-slate-900 text-slate-100 p-3 text-xs whitespace-pre-wrap break-all min-h-[96px]">
              {functionsLastResult || '(no result yet)'}
            </pre>
          </div>
        </div>
      </div>
    </>
  );
}

export default function CloudPage() {
  const { data, actions } = useLxPage<PageData, PageActions>();
  const {
    loginInteractive,
    logoutCurrentTenant,
    switchTenant,
    startMqttDemo,
    stopMqttDemo,
    callNamedFunction,
  } = actions;
  const {
    type = 'auth',
    status = 'Idle',
    tenant = null,
    tenants = [],
    mqttStatus = 'Idle',
    mqttRuntimeState = 'idle',
    mqttLastError = '',
    mqttSubscribed = false,
    mqttTopicFilter = 'demo/mqtt',
    mqttMessageCount = 0,
    mqttLastTopic = '',
    mqttLastPayload = '',
    mqttLastReceivedAt = '',
    functionsStatus = 'Idle',
    functionsAvailable = [],
    functionsLastCall = '',
    functionsLastResult = '',
  } = data;

  return (
    <div className="min-h-screen bg-gradient-to-br from-gray-50 to-gray-100">
      <div className="px-4 py-6">
        {type === 'mqtt' ? (
          <CloudMqttView
            mqttStatus={mqttStatus}
            mqttRuntimeState={mqttRuntimeState}
            mqttLastError={mqttLastError}
            mqttSubscribed={mqttSubscribed}
            mqttTopicFilter={mqttTopicFilter}
            mqttMessageCount={mqttMessageCount}
            mqttLastTopic={mqttLastTopic}
            mqttLastPayload={mqttLastPayload}
            mqttLastReceivedAt={mqttLastReceivedAt}
            startMqttDemo={startMqttDemo}
            stopMqttDemo={stopMqttDemo}
          />
        ) : type === 'functions' ? (
          <CloudFunctionsView
            functionsStatus={functionsStatus}
            functionsAvailable={functionsAvailable}
            functionsLastCall={functionsLastCall}
            functionsLastResult={functionsLastResult}
            callNamedFunction={callNamedFunction}
          />
        ) : (
          <CloudAuthView
            status={status}
            tenant={tenant}
            tenants={tenants}
            loginInteractive={loginInteractive}
            logoutCurrentTenant={logoutCurrentTenant}
            switchTenant={switchTenant}
          />
        )}
      </div>
    </div>
  );
}
