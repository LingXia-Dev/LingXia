import { useAction } from '@lingxia/view';
import '../../theme/index.css';
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
  const failure = calls.some(call => call.error) ? 'Unable to open sharing. Please try again.' : '';
  const path = (value: unknown) => (
    <text className="px-3 py-2 rounded-lg bg-surface-sunken text-xs text-muted break-all">{String(value || 'No selection')}</text>
  );
  return (
    <Page intro="lx.share opens the native system share sheet.">
      <text data-testid="share-status" className={failure ? 'text-sm text-center text-danger' : 'text-sm text-center'}>
        {failure || String(data.statusText || 'Ready')}
      </text>
      <Section title="Text" description="Some receivers reject text-only shares.">
        <Button testId="share-text" disabled={busy} onClick={() => shareText.run()}>
          Share Text
        </Button>
      </Section>
      <Section title="Page" description="Share the current page as an AppLink URL.">
        <Button testId="share-page" disabled={busy} onClick={() => shareCurrentPage.run()}>
          Share Current Page
        </Button>
      </Section>
      <Section title="Image" description="Choose or capture an image, then share it as an attachment.">
        {path(data.selectedImagePath)}
        <view className="grid gap-3 grid-cols-2">
          <Button variant="secondary" testId="share-choose-image" disabled={busy} onClick={() => chooseImage.run()}>
            Choose Image
          </Button>
          <Button testId="share-image" disabled={busy} onClick={() => shareSelectedImage.run()}>
            Share Image
          </Button>
        </view>
      </Section>
      <Section title="File" description="Pick an image, PDF, or other document before sharing it.">
        {path(data.selectedFilePath)}
        <view className="grid gap-3 grid-cols-2">
          <Button variant="secondary" testId="share-choose-file" disabled={busy} onClick={() => chooseFile.run()}>
            Choose File
          </Button>
          <Button testId="share-file" disabled={busy} onClick={() => shareSelectedFile.run()}>
            Share File
          </Button>
        </view>
      </Section>
    </Page>
  );
}
