import { type View } from '@lingxia/ui';
import '../../theme/index.css';
import { Button, InfoList, InfoRow, Page, Section } from '../../shared/ui';

// Enough rows to scroll on a phone, so a pull below the top scrolls instead.
const ROWS = Array.from({ length: 24 }, (_, i) => i + 1);

export function render({ data, actions }: View) {
  const refreshing = Boolean(data.isRefreshing);
  return (
    <Page
      testId="pull-refresh-page"
      title="Pull Down Refresh Demo"
      subtitle="Pull down on this page to trigger refresh, or use the buttons below to control it programmatically."
    >
      <text testId="pull-refresh-title" className="text-xs text-muted text-center">
        Pull down to refresh
      </text>
      <Section title="Refresh Status">
        <InfoList>
          <row className="items-center justify-between gap-3 py-2.5">
            <text className="text-sm text-muted">Status</text>
            <text testId="pull-refresh-status" className={refreshing ? 'text-sm font-medium text-accent' : 'text-sm font-medium text-success'}>
              {refreshing ? 'Refreshing...' : 'Idle'}
            </text>
          </row>
          <InfoRow label="Refresh Count" value={String(data.refreshCount ?? 0)} testId="pull-refresh-count" />
          <InfoRow label="Last Refresh" value={data.lastRefreshTime || 'Never'} testId="pull-refresh-time" />
        </InfoList>
      </Section>
      <Section title="API Controls" description="Control refresh programmatically">
        <Button testId="pull-refresh-start" disabled={refreshing} onClick={() => actions.startRefresh()}>
          lx.startPullDownRefresh()
        </Button>
        <Button variant="secondary" testId="pull-refresh-stop" onClick={() => actions.stopRefresh()}>
          lx.stopPullDownRefresh()
        </Button>
      </Section>
      <Section title="Sample Rows" description="Scroll the page: a pull refreshes only from the top">
        <InfoList>
          {ROWS.map((row) => (
            <text key={String(row)} testId="pull-refresh-row" className="text-sm py-2.5">Row {row}</text>
          ))}
        </InfoList>
      </Section>
    </Page>
  );
}
