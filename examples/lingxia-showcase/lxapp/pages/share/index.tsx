import { t, useAction, useLocal, type MessageKey } from '@lingxia/view';
import '../../theme/index.css';
import { said } from '../../shared/lib/said';
import { Button, Page, Section } from '../../shared/ui';
import type { View } from './$types';

export function render({ data, actions }: View) {
  const shareText = useAction(actions.shareText);
  const shareCurrentPage = useAction(actions.shareCurrentPage);
  const chooseImage = useAction(actions.chooseImage);
  const shareSelectedImage = useAction(actions.shareSelectedImage);
  const chooseFile = useAction(actions.chooseFile);
  const shareSelectedFile = useAction(actions.shareSelectedFile);
  const calls = [shareText, shareCurrentPage, chooseImage, shareSelectedImage, chooseFile, shareSelectedFile];
  const busy = calls.some(call => call.pending);
  const failure = calls.some(call => call.error) ? t('share.failed') : '';
  const path = (value: unknown) => (
    <text className="px-3 py-2 rounded-lg bg-surface-sunken text-xs text-muted break-all">{String(value || t('share.noSelection'))}</text>
  );
  return (
    <Page intro={t('share.lxShareOpensThe')}>
      <text data-testid="share-status" className={failure ? 'text-sm text-center text-danger' : 'text-sm text-center'}>
        {failure || said(String(data.statusText || 'share.state.ready'))}
      </text>
      <Section title={t('share.text')} description={t('share.someReceiversRejectText')}>
        <Button testId="share-text" disabled={busy} onClick={() => shareText.run()}>
          {t('share.shareText')}
        </Button>
      </Section>
      <Section title={t('share.page')} description={t('share.shareTheCurrentPage')}>
        <Button testId="share-page" disabled={busy} onClick={() => shareCurrentPage.run()}>
          {t('share.shareCurrentPage')}
        </Button>
      </Section>
      <Section title={t('share.image')} description={t('share.chooseOrCaptureAn')}>
        {path(data.selectedImagePath)}
        <view className="grid gap-3 grid-cols-2">
          <Button variant="secondary" testId="share-choose-image" disabled={busy} onClick={() => chooseImage.run()}>
            {t('share.chooseImage')}
          </Button>
          <Button testId="share-image" disabled={busy} onClick={() => shareSelectedImage.run()}>
            {t('share.shareImage')}
          </Button>
        </view>
      </Section>
      <Section title={t('share.file')} description={t('share.pickAnImagePdf')}>
        {path(data.selectedFilePath)}
        <view className="grid gap-3 grid-cols-2">
          <Button variant="secondary" testId="share-choose-file" disabled={busy} onClick={() => chooseFile.run()}>
            {t('share.chooseFile')}
          </Button>
          <Button testId="share-file" disabled={busy} onClick={() => shareSelectedFile.run()}>
            {t('share.shareFile')}
          </Button>
        </view>
      </Section>
    </Page>
  );
}
