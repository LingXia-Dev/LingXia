import { t, useEnv, useLocal } from '@lingxia/view';
import '../../theme/index.css';
import { Button, Field, InfoList, InfoRow, Page, Section } from '../../shared/ui';
import type { View } from './$types';

export function render({ data, actions }: View) {
  const count = useLocal('counter', 0);
  const env = useEnv();
  const message = useLocal('message', '');
  const send = async () => {
    await actions.logSurfaceMessage({ message: message.value.trim() });
    message.set('');
  };
  const intro = data.placement === 'window'
    ? t('surface.introWindow')
    : data.placement === 'float'
      ? t('surface.introFloat')
      : t('surface.introPage');
  return (
    <Page testId="surface-page" scrollId="surface-scroll" intro={intro}>
      <Section title={t('surface.queryString')} description={t('surface.queryStringLine')}>
        <text data-testid="surface-query" className="font-mono text-sm">
          {data.queryString || t('surface.none')}
        </text>
      </Section>
      <Section title={t('surface.pageLifecycle')} description={t('surface.pageLifecycleLine')}>
        <InfoList>
          <InfoRow label={t('surface.onshow')} value={String(data.showCount || 0)} testId="surface-show-count" />
          <InfoRow label={t('surface.onhide')} value={String(data.hideCount || 0)} testId="surface-hide-count" />
          <InfoRow label={t('surface.lastEvent')} value={data.lastLifecycle || 'onLoad'} testId="surface-lifecycle" />
          <InfoRow label={t('surface.topInset')} value={String(env.insets.top)} testId="surface-top-inset" />
          <InfoRow label={t('surface.appearance')} value={env.appearance === 'dark' ? t('surface.dark') : t('surface.light')} testId="surface-appearance" />
        </InfoList>
      </Section>
      <Section title={t('surface.inPageCounter')} description={t('surface.hidePreservesThisCounter')}>
        <text data-testid="surface-counter" className="font-mono text-2xl">
          {String(count.value)}
        </text>
        <Button variant="secondary" testId="surface-increment" onClick={() => count.set(count.value + 1)}>
          {t('surface.increment')}
        </Button>
      </Section>
      {data.hasOpener ? (
        <Section title={t('surface.fromOpener')} description={t('surface.fromOpenerLine')}>
          <InfoRow label={t('surface.received')} value={String(data.inboundCount || 0)} testId="surface-inbound-count" />
          <text data-testid="surface-inbound" className="font-mono text-sm">
            {data.lastInbound || t('surface.none')}
          </text>
        </Section>
      ) : null}
      {data.hasOpener ? (
        <Section title={t('surface.message')} description={t('surface.messageLine')}>
          <Field testId="surface-message" data={{ 'data-controlled-value': message.value }} label={t('surface.messageToParentPage')} bind={message} placeholder={t('surface.messageToParentPage')} />
          <Button testId="surface-send-message" disabled={!message.value.trim()} onClick={send}>
            {t('surface.sendThenClose')}
          </Button>
        </Section>
      ) : null}
      {data.placement !== 'page' ? (
        <Section title={t('surface.selfActions')} description={t('surface.selfActionsLine')}>
          <Button variant="secondary" testId="surface-hide" onClick={() => actions.hideSelf()}>
            {t('surface.hide')}
          </Button>
          <Button variant="destructive" testId="surface-close" onClick={() => actions.closeSelf()}>
            {t('surface.close')}
          </Button>
        </Section>
      ) : null}
    </Page>
  );
}
