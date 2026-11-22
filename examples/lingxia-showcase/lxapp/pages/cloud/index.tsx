import { t } from '@lingxia/view';
import '../../theme/index.css';
import { Button, Card, EmptyState, InfoList, InfoRow, Page, Section } from '../../shared/ui';
import type { View } from './$types';

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

const tenantName = (tenant: Tenant) => tenant.displayName || tenant.tenantName || tenant.tenantId || t('cloud.unknownTenant');
const tone = (status: string) => {
  const lower = status.toLowerCase();
  if (lower.includes('failed') || lower.includes('error')) return 'text-danger';
  if (lower.includes('succeeded') || lower === 'ready') return 'text-success';
  return ['...', 'starting', 'switching', 'logging'].some(word => lower.includes(word)) ? 'text-warning' : 'text-text';
};

function Avatar({ src }: { src?: string }) {
  return src ? <image src={src} alt="" className="w-8 h-8 shrink-0 rounded-full border border-line bg-surface object-cover" /> : null;
}

function Code({ testId, children }: { testId: string; children: string }) {
  return (
    <text data-testid={testId} className="p-3 rounded-lg bg-surface-sunken text-text text-xs font-mono break-all whitespace-pre-wrap">
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
      data={{ 'data-mode': 'auth' }}
      intro={t('cloud.loginReturnsTheActive')}
    >
      <view className="grid gap-3 grid-cols-2">
        {!tenant && (
          <Button testId="cloud-login" onClick={() => actions.loginInteractive()}>
            {t('cloud.interactiveLogin')}
          </Button>
        )}
        <Button variant="secondary" testId="cloud-add-identity" onClick={() => actions.addTenant()}>
          {t('cloud.addIdentity')}
        </Button>
        <Button variant="destructive" testId="cloud-logout" onClick={() => actions.logoutCurrentTenant()}>
          {t('cloud.logoutCurrentTenant')}
        </Button>
      </view>
      <Section title={t('cloud.currentState')} description={t('cloud.latestCloudAuthSnapshot')}>
        <InfoList>
          <view className="flex-row items-center justify-between gap-4 py-2.5">
            <text className="text-sm text-muted">{t('cloud.status')}</text>
            <text data-testid="cloud-auth-status" className={`text-sm font-semibold text-right ${tone(status)}`}>
              {status}
            </text>
          </view>
          <view className="flex-row items-center gap-3 py-2.5">
            <text className="grow basis-0 text-sm text-muted">{t('cloud.activeTenant')}</text>
            <Avatar src={tenant?.logoUrl} />
            <view className="items-end gap-1">
              <text data-testid="cloud-current-tenant" className="text-sm font-semibold text-right">
                {tenant ? tenantName(tenant) : t('cloud.noActiveTenant')}
              </text>
              {!!tenant?.shortName && (
                <text data-testid="cloud-tenant-short-name" className="px-2 py-0.5 rounded-full bg-accent-soft text-xs font-semibold text-accent">
                  {tenant.shortName}
                </text>
              )}
            </view>
          </view>
          <view className="flex-row items-center gap-3 py-2.5">
            <text className="grow basis-0 text-sm text-muted">{t('cloud.activeUser')}</text>
            <Avatar src={user?.avatar} />
            <view className="items-end gap-1">
              <text data-testid="cloud-current-user" className="text-sm font-semibold text-right">
                {user?.name || t('cloud.notSignedIn')}
              </text>
              {!!user?.id && (
                <text data-testid="cloud-user-id" className="text-xs text-muted break-all">
                  {user.id}
                </text>
              )}
            </view>
          </view>
        </InfoList>
      </Section>
      <Section
        title={t('cloud.tenants')}
        description={t('cloud.activateOrRemoveIdentities')}
        action={
          <text data-testid="cloud-tenants-count" className="text-xs text-muted">
            {String(tenants.length)}
          </text>
        }
      >
        {!tenants.length && <EmptyState testId="cloud-identities-empty" icon="cloud" title={t('cloud.noTenantsYet')} message={t('cloud.runInteractiveLoginFirst')} />}
        {tenants.map((item, index) => {
          const active = !!item.tenantId && item.tenantId === tenant?.tenantId;
          const state = active ? t('cloud.tenantActive') : item.tenantId ? t('cloud.tenantSwitch') : t('cloud.tenantUnavailable');
          return (
            <button
              key={`${item.tenantId || 'tenant'}-${index}`}
              data-testid={`cloud-activate-${item.tenantId || index}`}
              disabled={!item.tenantId || active}
              onClick={() => actions.activateTenant({ tenantId: item.tenantId })}
              className={
                active
                  ? 'p-4 rounded-xl border border-line bg-accent-soft text-text text-left'
                  : 'p-4 rounded-xl border border-line bg-surface text-text text-left active:bg-surface-sunken'
              }
            >
              <view className="flex-row items-center gap-3">
                <Avatar src={item.logoUrl} />
                <view className="grow basis-0 min-w-0 gap-1">
                  <text className="text-sm font-semibold">{tenantName(item)}</text>
                  {!!item.shortName && <text className="text-xs font-semibold text-muted">{item.shortName}</text>}
                  <text className="text-xs text-muted break-all">{item.tenantId || 'Missing tenantId'}</text>
                </view>
                <text className="px-2 py-1 rounded-full bg-surface-sunken text-xs text-muted">{state}</text>
              </view>
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
  const color = connected ? 'text-success' : waiting ? 'text-warning' : failed ? 'text-danger' : 'text-muted';
  const dot = connected ? 'bg-success' : waiting ? 'bg-warning' : failed ? 'bg-danger' : 'bg-line-strong';
  const active = !!data.mqttSubscribed;
  return (
    <Page testId="cloud-page" data={{ 'data-mode': 'mqtt' }}>
      <Section title={t('cloud.runtimeConnection')} description={t('cloud.mqttRuntimeStatusFor')}>
        <view className="flex-row items-center justify-between gap-3">
          <text className="text-xs text-muted">{t('cloud.connectionState')}</text>
          <view className="flex-row items-center gap-1.5">
            <view className={`w-2 h-2 rounded-full ${dot}`} />
            <text data-testid="cloud-mqtt-state" className={`font-mono text-xs font-semibold ${color}`}>
              {state}
            </text>
          </view>
        </view>
        <view className="flex-row items-start justify-between gap-3">
          <text className="shrink-0 text-xs text-muted">{t('cloud.lastError')}</text>
          <text
            data-testid="cloud-mqtt-error"
            className={
              data.mqttLastError
                ? 'grow basis-0 min-w-0 text-right font-mono text-xs break-all text-danger'
                : 'grow basis-0 min-w-0 text-right font-mono text-xs break-all'
            }
          >
            {String(data.mqttLastError || '-')}
          </text>
        </view>
      </Section>
      <Section
        title={t('cloud.topicSubscription')}
        description={t('cloud.manageTheDemoTopic')}
        action={
          <text
            className={
              active
                ? 'px-2.5 py-1 rounded-full bg-success-soft text-xs font-semibold text-success'
                : 'px-2.5 py-1 rounded-full bg-surface-sunken text-xs font-semibold text-muted'
            }
          >
            {active ? 'active' : 'inactive'}
          </text>
        }
      >
        <text className="text-xs text-muted uppercase">{t('cloud.topic')}</text>
        <text data-testid="cloud-mqtt-topic" className="font-mono text-sm break-all">
          {String(data.mqttTopicFilter || 'demo/mqtt')}
        </text>
        <text data-testid="cloud-mqtt-status" className="text-sm font-semibold">
          {String(data.mqttStatus || 'Idle')}
        </text>
        <view className="grid gap-3 grid-cols-2">
          <Button testId="cloud-subscribe" disabled={active} onClick={() => actions.startMqttDemo()}>
            {t('cloud.subscribe')}
          </Button>
          <Button variant="destructive" testId="cloud-unsubscribe" disabled={!active} onClick={() => actions.stopMqttDemo()}>
            {t('cloud.unsubscribe')}
          </Button>
        </view>
        <InfoList>
          <InfoRow label={t('cloud.subscriptionState')} value={active ? 'active' : 'inactive'} testId="cloud-mqtt-active" />
          <InfoRow label={t('cloud.messagesReceived')} value={String(data.mqttMessageCount || 0)} testId="cloud-mqtt-count" />
        </InfoList>
      </Section>
      <Section title={t('cloud.latestMessage')} description={t('cloud.incomingMessageDataIs')}>
        {!data.mqttMessageCount ? (
          <EmptyState testId="cloud-mqtt-empty" icon="cloud" title={t('cloud.noMessageYet')} message={t('cloud.publishOneToThe')} />
        ) : (
          <view className="gap-3">
            <InfoList>
              <InfoRow label={t('cloud.topic')} value={data.mqttLastTopic} testId="cloud-mqtt-last-topic" />
              <InfoRow label={t('cloud.receivedAt')} value={data.mqttLastReceivedAt} testId="cloud-mqtt-received-at" />
            </InfoList>
            <Code testId="cloud-mqtt-payload">{String(data.mqttLastPayload || '(empty payload)')}</Code>
          </view>
        )}
      </Section>
    </Page>
  );
}

function Functions({ data, actions }: Pane) {
  const names = (data.functionsAvailable || []) as string[];
  return (
    <Page testId="cloud-page" data={{ 'data-mode': 'functions' }}>
      <Section
        title={t('cloud.currentLxappFunctions')}
        description={t('cloud.theseSampleCallsUse')}
      >
        <InfoRow label={t('cloud.status')} value={data.functionsStatus || 'Idle'} testId="cloud-functions-status" />
        {!names.length ? (
          <EmptyState testId="cloud-functions-empty" icon="cloud" title={t('cloud.noDemoFunctions')} message={t('cloud.noneAreConfiguredFor')} />
        ) : (
          <view className="flex-row flex-wrap gap-2">
            {names.map(name => (
              <Button key={name} testId={`cloud-function-${name}`} onClick={() => actions.callNamedFunction({ name })}>{`Call ${name}`}</Button>
            ))}
          </view>
        )}
      </Section>
      <Card>
        <InfoRow label={t('cloud.function')} value={data.functionsLastCall || '-'} testId="cloud-functions-last-call" />
        <Code testId="cloud-functions-result">{String(data.functionsLastResult || '(no result yet)')}</Code>
      </Card>
    </Page>
  );
}
