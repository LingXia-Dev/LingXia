import { t, useLeaveGuard, useLocal } from '@lingxia/view';
import '../../theme/index.css';
import { Button, ErrorState, Field, Page, Section } from '../../shared/ui';

export function render() {
  const saved = useLocal('saved', '');
  const draft = useLocal('draft', '');
  const dirty = draft.value !== saved.value;
  const guard = useLeaveGuard('draft', { enabled: dirty });
  return (
    <Page intro={t('leave.intro')}>
      <Section title={t('leave.draft')}>
        <Field testId="leave-draft" label={t('leave.draft')} bind={draft} placeholder={t('leave.placeholder')} />
        <view className="flex-row items-center justify-between gap-2">
          <text data-testid="leave-status" className="text-xs text-muted">
            {dirty ? t('leave.unsaved') : t('leave.saved')}
          </text>
          <text data-testid="leave-requests" className="text-xs text-muted">{t('leave.requests', { count: guard.requests })}</text>
        </view>
        <Button testId="leave-save" disabled={!dirty} onClick={() => saved.set(draft.value)}>
          {t('leave.save')}
        </Button>
      </Section>
      {guard.pending && (
        <Section testId="leave-confirm" title={t('leave.confirmTitle')}>
          <text data-testid="leave-question" className="text-sm">
            {t('leave.confirmBody')}
          </text>
          <view className="grid gap-2 grid-cols-2">
            <Button variant="secondary" testId="leave-keep" onClick={() => guard.answer(false)}>
              {t('leave.keep')}
            </Button>
            <Button variant="destructive" testId="leave-discard" onClick={() => guard.answer(true)}>
              {t('leave.discard')}
            </Button>
          </view>
        </Section>
      )}
      <ErrorState testId="leave-error" error={guard.error} />
    </Page>
  );
}
