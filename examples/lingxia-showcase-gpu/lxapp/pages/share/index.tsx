import { t, useLocal, type View } from '@lingxia/view';
import '../../theme/index.css';
import { said } from '../../shared/lib/said';
import { Button, Page, Section } from '../../shared/ui';

export function render({ data, actions }: View) {
  const busy = useLocal('share-busy', false);
  const failure = useLocal('share-error', '');
  const run = async (action: () => Promise<unknown>) => {
    if (busy.value) return;
    busy.set(true);
    failure.set('');
    try {
      await action();
    } catch {
      failure.set('share.failed');
    } finally {
      busy.set(false);
    }
  };
  const path = (value: unknown) => (
    <text className="px-3 py-2 rounded-lg bg-surface-sunken text-xs text-muted break-all">{String(value || t('share.noSelection'))}</text>
  );
  return (
    <Page intro={t('share.lxShareOpensThe')}>
      <text data-testid="share-status" className={failure.value ? 'text-sm text-center text-danger' : 'text-sm text-center'}>
        {failure.value ? t(failure.value) : said(String(data.statusText || 'share.state.ready'))}
      </text>
      <Section title={t('share.text')} description={t('share.someReceiversRejectText')}>
        <Button testId="share-text" disabled={busy.value} onClick={() => run(() => actions.shareText())}>
          {t('share.shareText')}
        </Button>
      </Section>
      <Section title={t('share.page')} description={t('share.shareTheCurrentPage')}>
        <Button testId="share-page" disabled={busy.value} onClick={() => run(() => actions.shareCurrentPage())}>
          {t('share.shareCurrentPage')}
        </Button>
      </Section>
      <Section title={t('share.image')} description={t('share.chooseOrCaptureAn')}>
        {path(data.selectedImagePath)}
        <view className="grid gap-3 grid-cols-2">
          <Button variant="secondary" testId="share-choose-image" disabled={busy.value} onClick={() => run(() => actions.chooseImage())}>
            {t('share.chooseImage')}
          </Button>
          <Button testId="share-image" disabled={busy.value} onClick={() => run(() => actions.shareSelectedImage())}>
            {t('share.shareImage')}
          </Button>
        </view>
      </Section>
      <Section title={t('share.file')} description={t('share.pickAnImagePdf')}>
        {path(data.selectedFilePath)}
        <view className="grid gap-3 grid-cols-2">
          <Button variant="secondary" testId="share-choose-file" disabled={busy.value} onClick={() => run(() => actions.chooseFile())}>
            {t('share.chooseFile')}
          </Button>
          <Button testId="share-file" disabled={busy.value} onClick={() => run(() => actions.shareSelectedFile())}>
            {t('share.shareFile')}
          </Button>
        </view>
      </Section>
    </Page>
  );
}
