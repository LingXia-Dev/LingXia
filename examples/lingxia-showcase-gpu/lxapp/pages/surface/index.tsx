import { useLocal, type View } from '@lingxia/ui';
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
    <Page testId="surface-page" scrollId="surface-scroll" title="Surface Page" subtitle="Inspect the query string and send a message to the opener.">
      <Section title="Query string">
        <text testId="surface-query" className="font-mono text-sm">
          {String(data.queryString || '(none)')}
        </text>
      </Section>
      <Section title="Page lifecycle">
        <InfoList>
          <InfoRow label="onShow" value={String(data.showCount || 0)} testId="surface-show-count" />
          <InfoRow label="onHide" value={String(data.hideCount || 0)} testId="surface-hide-count" />
          <InfoRow label="Last event" value={data.lastLifecycle || 'onLoad'} testId="surface-lifecycle" />
        </InfoList>
      </Section>
      <Section title="In-page counter" description="Hide preserves this counter; close resets it on re-open.">
        <text testId="surface-counter" className="font-mono text-2xl">
          {String(count.value)}
        </text>
        <Button variant="secondary" testId="surface-increment" onClick={() => count.set(count.value + 1)}>
          Increment
        </Button>
      </Section>
      <Section title="From opener">
        <InfoRow label="Received" value={String(data.inboundCount || 0)} testId="surface-inbound-count" />
        <text testId="surface-inbound" className="font-mono text-sm">
          {String(data.lastInbound || '(none)')}
        </text>
      </Section>
      <Section title="Message">
        <Field testId="surface-message" data={{ 'data-controlled-value': message.value }} label="Message to parent page" bind={message} placeholder="Message to parent page" />
        <Button testId="surface-send-message" disabled={!message.value.trim()} onClick={send}>
          Send then close
        </Button>
      </Section>
      <Section title="Self actions">
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
