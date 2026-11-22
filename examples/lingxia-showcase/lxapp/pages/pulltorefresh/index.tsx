import { t } from '@lingxia/view';
import '../../theme/index.css';
import { Button, InfoList, InfoRow, Page, Section } from '../../shared/ui';
import type { View } from './$types';

// Enough rows to scroll on a phone, so a pull below the top scrolls instead.
const ROWS = Array.from({ length: 24 }, (_, i) => i + 1);

export function render({ data, actions }: View) {
  const refreshing = Boolean(data.isRefreshing);
  return (
    <Page
      testId="pull-refresh-page"
      intro={t('pulltorefresh.pullDownOnThis')}
    >
      <text data-testid="pull-refresh-title" className="text-xs text-muted text-center">
        {t('pulltorefresh.pullDownToRefresh')}
      </text>
      <Section title={t('pulltorefresh.refreshStatus')}>
        <InfoList>
          <view className="flex-row items-center justify-between gap-3 py-2.5">
            <text className="text-sm text-muted">{t('pulltorefresh.status')}</text>
            <text data-testid="pull-refresh-status" className={refreshing ? 'text-sm font-medium text-accent' : 'text-sm font-medium text-success'}>
              {refreshing ? t('refresh.refreshing') : t('refresh.idle')}
            </text>
          </view>
          <InfoRow label={t('pulltorefresh.refreshCount')} value={String(data.refreshCount ?? 0)} testId="pull-refresh-count" />
          <InfoRow label={t('pulltorefresh.lastRefresh')} value={data.lastRefreshTime || t('pulltorefresh.never')} testId="pull-refresh-time" />
        </InfoList>
      </Section>
      <Section title={t('pulltorefresh.apiControls')} description={t('pulltorefresh.controlRefreshProgrammatically')}>
        <Button testId="pull-refresh-start" disabled={refreshing} onClick={() => actions.startRefresh()}>
          page.refresh()
        </Button>
        <Button variant="secondary" testId="pull-refresh-stop" onClick={() => actions.stopRefresh()}>
          {t('pulltorefresh.finishRefresh')}
        </Button>
      </Section>
      <Section title={t('pulltorefresh.sampleRows')} description={t('pulltorefresh.scrollThePageA')}>
        <InfoList>
          {ROWS.map((row) => (
            <text key={String(row)} data-testid="pull-refresh-row" className="text-sm py-2.5">{t('video.row', { id: row })}</text>
          ))}
        </InfoList>
      </Section>
    </Page>
  );
}
