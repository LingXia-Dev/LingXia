import { t, useLocal, type View } from '@lingxia/view';
import '../../theme/index.css';
import { Button, Field, InfoList, InfoRow, Page, Section } from '../../shared/ui';

export function render({ data, actions }: View) {
  const count = useLocal('counter', 0);
  const message = useLocal('message', '');
  const send = async () => {
    await actions.logSurfaceMessage({ message: message.value.trim() });
    message.set('');
  };
  return (
    <Page testId="surface-page" scrollId="surface-scroll" intro={t('surface.inspectTheQueryString')}>
      <Section title={t('surface.queryString')}>
        <text data-testid="surface-query" className="font-mono text-sm">
          {String(data.queryString || '(none)')}
        </text>
      </Section>
      <Section title={t('surface.pageLifecycle')}>
        <InfoList>
          <InfoRow label={t('surface.onshow')} value={String(data.showCount || 0)} testId="surface-show-count" />
          <InfoRow label={t('surface.onhide')} value={String(data.hideCount || 0)} testId="surface-hide-count" />
          <InfoRow label={t('surface.lastEvent')} value={data.lastLifecycle || 'onLoad'} testId="surface-lifecycle" />
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
      <Section title={t('surface.fromOpener')}>
        <InfoRow label={t('surface.received')} value={String(data.inboundCount || 0)} testId="surface-inbound-count" />
        <text data-testid="surface-inbound" className="font-mono text-sm">
          {String(data.lastInbound || '(none)')}
        </text>
      </Section>
      <Section title={t('surface.message')}>
        <Field testId="surface-message" data={{ 'data-controlled-value': message.value }} label={t('surface.messageToParentPage')} bind={message} placeholder={t('surface.messageToParentPage')} />
        <Button testId="surface-send-message" disabled={!message.value.trim()} onClick={send}>
          {t('surface.sendThenClose')}
        </Button>
      </Section>
      <Section title={t('surface.selfActions')}>
        <Button variant="secondary" testId="surface-hide" onClick={() => actions.hideSelf()}>
          Hide (parent can show again)
        </Button>
        <Button variant="destructive" testId="surface-close" onClick={() => actions.closeSelf()}>
          Close (destroys this page)
        </Button>
      </Section>
    </Page>
  );
}
