import { t, useAction, useLocal, type MessageKey } from '@lingxia/view';
import '../../theme/index.css';
import { said } from '../../shared/lib/said';
import { Button, Page, Section } from '../../shared/ui';
import type { View } from './$types';

export function render({ data, actions }: View) {
  const draft = useLocal('clipboard-draft', t('clipboard.draft'));
  const writeText = useAction(actions.writeText);
  const readText = useAction(actions.readText);
  const writeTypedText = useAction(actions.writeTypedText);
  const writeSampleImage = useAction(actions.writeSampleImage);
  const chooseAndWriteImage = useAction(actions.chooseAndWriteImage);
  const peekTypes = useAction(actions.peekTypes);
  const readAll = useAction(actions.readAll);
  const clearClipboard = useAction(actions.clearClipboard);
  const calls = [writeText, readText, writeTypedText, writeSampleImage, chooseAndWriteImage, peekTypes, readAll, clearClipboard];
  const busy = calls.some(call => call.pending);
  const failure = calls.some(call => call.error) ? t('clipboard.failed') : '';
  const status = failure || (busy ? t('clipboard.working') : said(String(data.statusText || 'clipboard.state.ready'), data.statusArgs as Record<string, unknown>));
  return (
    <Page intro={t('clipboard.lxClipboardReadsAnd')}>
      <Section title={t('clipboard.status')}>
        <text data-testid="clipboard-status" className={failure ? 'text-sm text-danger' : 'text-sm'}>
          {status}
        </text>
        <text data-testid="clipboard-types-value" className="text-xs text-muted">{t('clipboard.typesLine', { types: said(String(data.typesText || 'clipboard.state.notPeeked')) })}</text>
      </Section>
      <Section title={t('clipboard.text')} description={t('clipboard.writetextReadtextAnEmpty')}>
        <textarea
          data-testid="clipboard-draft"
          aria-label={t('clipboard.textToCopy')}
          bind={draft}
          placeholder={t('clipboard.textToCopy')}
          className="h-20 px-3 py-2 rounded-lg border border-line-strong bg-surface text-sm"
        />
        <view className="grid gap-3 grid-cols-2">
          <Button testId="clipboard-write-text" disabled={busy} onClick={() => writeText.run({ text: draft.value })}>
            {t('clipboard.writeText')}
          </Button>
          <Button variant="secondary" testId="clipboard-read-text" disabled={busy} onClick={() => readText.run()}>
            {t('clipboard.readText')}
          </Button>
        </view>
        <Button
          variant="secondary"
          testId="clipboard-write-typed"
          disabled={busy}
          onClick={() => writeTypedText.run({ text: draft.value })}
        >
          {t('clipboard.writeTypedItem')}
        </Button>
        {!!data.readTextValue && (
          <text data-testid="clipboard-read-value" className="px-3 py-2 rounded-lg bg-surface-sunken text-xs text-muted break-all">
            {String(data.readTextValue)}
          </text>
        )}
      </Section>
      <Section title={t('clipboard.image')} description={t('clipboard.writeA11')}>
        <view className="grid gap-3 grid-cols-2">
          <Button testId="clipboard-write-sample-image" disabled={busy} onClick={() => writeSampleImage.run()}>
            {t('clipboard.writeSamplePng')}
          </Button>
          <Button variant="secondary" testId="clipboard-choose-image" disabled={busy} onClick={() => chooseAndWriteImage.run()}>
            {t('clipboard.chooseImage')}
          </Button>
        </view>
        {!!data.imagePath && (
          <view className="items-center gap-2">
            <image
              data-testid="clipboard-image"
              src={String(data.imagePath)}
              alt={t('clipboard.imageAlt')}
              className="w-40 h-40 rounded border border-line object-contain"
            />
            <text className="text-xs text-muted break-all">{String(data.imagePath)}</text>
          </view>
        )}
      </Section>
      <Section title={t('clipboard.inspect')} description={t('clipboard.peekTypesWithoutThe')}>
        <view className="grid gap-3 grid-cols-3">
          <Button variant="secondary" testId="clipboard-types" disabled={busy} onClick={() => peekTypes.run()}>
            {t('clipboard.types')}
          </Button>
          <Button testId="clipboard-read-all" disabled={busy} onClick={() => readAll.run()}>
            {t('clipboard.read')}
          </Button>
          <Button variant="destructive" testId="clipboard-clear" disabled={busy} onClick={() => clearClipboard.run()}>
            {t('clipboard.clear')}
          </Button>
        </view>
      </Section>
    </Page>
  );
}
