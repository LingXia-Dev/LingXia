import { useLocal, type View } from '@lingxia/ui';
import '../../theme/index.css';
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
      failure.set('Unable to open sharing. Please try again.');
    } finally {
      busy.set(false);
    }
  };
  const path = (value: unknown) => (
    <text className="px-3 py-2 rounded-lg bg-surface-sunken text-xs text-muted break-all">{String(value || 'No selection')}</text>
  );
  return (
    <Page title="Share" subtitle="lx.share opens the native system share sheet.">
      <text testId="share-status" className={failure.value ? 'text-sm text-center text-danger' : 'text-sm text-center'}>
        {failure.value || String(data.statusText || 'Ready')}
      </text>
      <Section title="Text" description="Some receivers reject text-only shares.">
        <Button testId="share-text" disabled={busy.value} onClick={() => run(() => actions.shareText())}>
          Share Text
        </Button>
      </Section>
      <Section title="Page" description="Share the current page as an AppLink URL.">
        <Button testId="share-page" disabled={busy.value} onClick={() => run(() => actions.shareCurrentPage())}>
          Share Current Page
        </Button>
      </Section>
      <Section title="Image" description="Choose or capture an image, then share it as an attachment.">
        {path(data.selectedImagePath)}
        <grid className="gap-3 grid-cols-2">
          <Button variant="secondary" testId="share-choose-image" disabled={busy.value} onClick={() => run(() => actions.chooseImage())}>
            Choose Image
          </Button>
          <Button testId="share-image" disabled={busy.value} onClick={() => run(() => actions.shareSelectedImage())}>
            Share Image
          </Button>
        </grid>
      </Section>
      <Section title="File" description="Pick an image, PDF, or other document before sharing it.">
        {path(data.selectedFilePath)}
        <grid className="gap-3 grid-cols-2">
          <Button variant="secondary" testId="share-choose-file" disabled={busy.value} onClick={() => run(() => actions.chooseFile())}>
            Choose File
          </Button>
          <Button testId="share-file" disabled={busy.value} onClick={() => run(() => actions.shareSelectedFile())}>
            Share File
          </Button>
        </grid>
      </Section>
    </Page>
  );
}
