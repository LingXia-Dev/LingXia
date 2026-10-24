import { type View } from '@lingxia/ui';
import '../../theme/index.css';
import { Button, Card, EmptyState, InfoList, InfoRow, Page, Section } from '../../shared/ui';

type Data = Record<string, any>;
type Actions = Record<string, (payload?: unknown) => Promise<unknown>>;
type Pane = { data: Data; actions: Actions };
type Tenant = { tenantId?: string; tenantName?: string; displayName?: string; shortName?: string; logoUrl?: string };
type User = { id?: string; name?: string; avatar?: string };

export function render({ data, actions }: View) {
  const pane = { data: data as Data, actions: actions as Actions };
  if (data.type === 'mqtt') return <Mqtt {...pane} />;
  if (data.type === 'functions') return <Functions {...pane} />;
  return <Auth {...pane} />;
}

const tenantName = (tenant: Tenant) => tenant.displayName || tenant.tenantName || tenant.tenantId || 'Unknown tenant';
const tone = (status: string) => {
  const lower = status.toLowerCase();
  if (lower.includes('failed') || lower.includes('error')) return 'text-error';
  if (lower.includes('succeeded') || lower === 'ready') return 'text-success';
  return ['...', 'starting', 'switching', 'logging'].some(word => lower.includes(word)) ? 'text-warning' : 'text-foreground';
};

function Avatar({ src }: { src?: string }) {
  return src ? <image src={src} alt="" className="w-8 h-8 shrink-0 rounded-full border border-border bg-card object-cover" /> : null;
}

function Code({ testId, children }: { testId: string; children: string }) {
  return (
    <text testId={testId} className="p-3 rounded-lg bg-surface-950 text-surface-50 text-xs font-mono break-all">
      {children}
    </text>
  );
}

function Auth({ data, actions }: Pane) {
  const tenant = data.tenant as Tenant | null;
  const user = data.user as User | null;
  const tenants = (data.tenants || []) as Tenant[];
  const status = String(data.status || 'Idle');
  return (
    <Page
      testId="cloud-page"
      title="Cloud Authentication"
      subtitle="Login returns the active identity; the home lxapp manages identity list, activation and logout."
    >
      <grid className="gap-3 grid-cols-2">
        {!tenant && (
          <Button testId="cloud-login" onClick={() => actions.loginInteractive()}>
            Interactive Login
          </Button>
        )}
        <Button variant="secondary" testId="cloud-add-identity" onClick={() => actions.addTenant()}>
          Add Identity
        </Button>
        <Button variant="destructive" testId="cloud-logout" onClick={() => actions.logoutCurrentTenant()}>
          Logout Current Tenant
        </Button>
      </grid>
      <Section title="Current State" description="Latest cloud auth snapshot from the runtime">
        <InfoList>
          <row className="items-center justify-between gap-4 py-2.5">
            <text className="text-sm text-muted-foreground">Status</text>
            <text testId="cloud-auth-status" className={`text-sm font-semibold text-right ${tone(status)}`}>
              {status}
            </text>
          </row>
          <row className="items-center gap-3 py-2.5">
            <text className="grow basis-0 text-sm text-muted-foreground">Active Tenant</text>
            <Avatar src={tenant?.logoUrl} />
            <column className="items-end gap-1">
              <text testId="cloud-current-tenant" className="text-sm font-semibold text-right">
                {tenant ? tenantName(tenant) : 'No active tenant'}
              </text>
              {!!tenant?.shortName && (
                <text testId="cloud-tenant-short-name" className="px-2 py-0.5 rounded-full bg-blue-100 text-xs font-semibold text-primary">
                  {tenant.shortName}
                </text>
              )}
            </column>
          </row>
          <row className="items-center gap-3 py-2.5">
            <text className="grow basis-0 text-sm text-muted-foreground">Active User</text>
            <Avatar src={user?.avatar} />
            <column className="items-end gap-1">
              <text testId="cloud-current-user" className="text-sm font-semibold text-right">
                {user?.name || 'Not signed in'}
              </text>
              {!!user?.id && (
                <text testId="cloud-user-id" className="text-xs text-muted-foreground break-all">
                  {user.id}
                </text>
              )}
            </column>
          </row>
        </InfoList>
      </Section>
      <Section
        title="Tenants"
        description="Activate or remove identities returned by lx.auth.list()"
        action={
          <text testId="cloud-tenants-count" className="text-xs text-muted-foreground">
            {String(tenants.length)}
          </text>
        }
      >
        {!tenants.length && <EmptyState testId="cloud-identities-empty" icon="cloud" title="No tenants yet" message="Run interactive login first." />}
        {tenants.map((item, index) => {
          const active = !!item.tenantId && item.tenantId === tenant?.tenantId;
          const state = active ? 'Active' : item.tenantId ? 'Switch' : 'Unavailable';
          return (
            <button
              key={`${item.tenantId || 'tenant'}-${index}`}
              testId={`cloud-activate-${item.tenantId || index}`}
              disabled={!item.tenantId || active}
              onClick={() => actions.activateTenant({ tenantId: item.tenantId })}
              className={
                active
                  ? 'p-4 rounded-xl border border-sky-200 bg-sky-50 text-foreground text-left'
                  : 'p-4 rounded-xl border border-border bg-card text-foreground text-left active:bg-muted'
              }
            >
              <row className="items-center gap-3">
                <Avatar src={item.logoUrl} />
                <column className="grow basis-0 min-w-0 gap-1">
                  <text className="text-sm font-semibold">{tenantName(item)}</text>
                  {!!item.shortName && <text className="text-xs font-semibold text-muted-foreground">{item.shortName}</text>}
                  <text className="text-xs text-muted-foreground break-all">{item.tenantId || 'Missing tenantId'}</text>
                </column>
                <text className="px-2 py-1 rounded-full bg-muted text-xs text-muted-foreground">{state}</text>
              </row>
            </button>
          );
        })}
      </Section>
    </Page>
  );
}

function Mqtt({ data, actions }: Pane) {
  const state = String(data.mqttRuntimeState || 'idle');
  const connected = state === 'connected';
  const waiting = state === 'connecting' || state === 'reconnecting';
  const failed = state === 'disconnected' || state === 'error';
  const color = connected ? 'text-success' : waiting ? 'text-warning' : failed ? 'text-error' : 'text-muted-foreground';
  const dot = connected ? 'bg-success' : waiting ? 'bg-warning' : failed ? 'bg-error' : 'bg-surface-400';
  const active = !!data.mqttSubscribed;
  return (
    <Page testId="cloud-page" title="Cloud MQTT">
      <Section title="Runtime Connection" description="MQTT runtime status for the shared cloud session.">
        <row className="items-center justify-between gap-3">
          <text className="text-xs text-muted-foreground">Connection state</text>
          <row className="items-center gap-1.5">
            <spacer className={`w-2 h-2 rounded-full ${dot}`} />
            <text testId="cloud-mqtt-state" className={`font-mono text-xs font-semibold ${color}`}>
              {state}
            </text>
          </row>
        </row>
        <row className="items-start justify-between gap-3">
          <text className="shrink-0 text-xs text-muted-foreground">Last error</text>
          <text
            testId="cloud-mqtt-error"
            className={
              data.mqttLastError
                ? 'grow basis-0 min-w-0 text-right font-mono text-xs break-all text-error'
                : 'grow basis-0 min-w-0 text-right font-mono text-xs break-all'
            }
          >
            {String(data.mqttLastError || '-')}
          </text>
        </row>
      </Section>
      <Section
        title="Topic Subscription"
        description="Manage the demo topic subscription and inspect how many messages this page has received."
        action={
          <text
            className={
              active
                ? 'px-2.5 py-1 rounded-full bg-green-100 text-xs font-semibold text-success'
                : 'px-2.5 py-1 rounded-full bg-muted text-xs font-semibold text-muted-foreground'
            }
          >
            {active ? 'active' : 'inactive'}
          </text>
        }
      >
        <text className="text-xs text-muted-foreground uppercase">Topic</text>
        <text testId="cloud-mqtt-topic" className="font-mono text-sm break-all">
          {String(data.mqttTopicFilter || 'demo/mqtt')}
        </text>
        <text testId="cloud-mqtt-status" className="text-sm font-semibold">
          {String(data.mqttStatus || 'Idle')}
        </text>
        <grid className="gap-3 grid-cols-2">
          <Button testId="cloud-mqtt-start" disabled={active} onClick={() => actions.startMqttDemo()}>
            Subscribe
          </Button>
          <Button variant="destructive" testId="cloud-mqtt-stop" disabled={!active} onClick={() => actions.stopMqttDemo()}>
            Unsubscribe
          </Button>
        </grid>
        <InfoList>
          <InfoRow label="Subscription state" value={active ? 'active' : 'inactive'} testId="cloud-mqtt-active" />
          <InfoRow label="Messages received" value={String(data.mqttMessageCount || 0)} testId="cloud-mqtt-count" />
        </InfoList>
      </Section>
      <Section title="Latest Message" description="Incoming message data is written back through setData.">
        {!data.mqttMessageCount ? (
          <EmptyState testId="cloud-mqtt-empty" icon="cloud" title="No message yet" message="Publish one to the topic above." />
        ) : (
          <column className="gap-3">
            <InfoList>
              <InfoRow label="Topic" value={data.mqttLastTopic} testId="cloud-mqtt-last-topic" />
              <InfoRow label="Received at" value={data.mqttLastReceivedAt} testId="cloud-mqtt-received-at" />
            </InfoList>
            <Code testId="cloud-mqtt-payload">{String(data.mqttLastPayload || '(empty payload)')}</Code>
          </column>
        )}
      </Section>
    </Page>
  );
}

function Functions({ data, actions }: Pane) {
  const names = (data.functionsAvailable || []) as string[];
  return (
    <Page testId="cloud-page" title="Cloud Functions">
      <Section
        title="Current LxApp Functions"
        description="These sample calls use lx.cloud.invoke(...) for the current lxapp. They are demo actions, not a runtime-discovered manifest list."
      >
        <InfoRow label="Status" value={data.functionsStatus || 'Idle'} testId="cloud-functions-status" />
        {!names.length ? (
          <EmptyState testId="cloud-functions-empty" icon="cloud" title="No demo functions" message="None are configured for this page." />
        ) : (
          <row className="flex-wrap gap-2">
            {names.map(name => (
              <Button key={name} testId={`cloud-function-${name}`} onClick={() => actions.callNamedFunction({ name })}>{`Call ${name}`}</Button>
            ))}
          </row>
        )}
      </Section>
      <Card>
        <InfoRow label="Function" value={data.functionsLastCall || '-'} testId="cloud-functions-last-call" />
        <Code testId="cloud-functions-result">{String(data.functionsLastResult || '(no result yet)')}</Code>
      </Card>
    </Page>
  );
}
