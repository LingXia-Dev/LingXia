import { t, useLocal, type View } from '@lingxia/view';
import '../../theme/index.css';
import { said } from '../../shared/lib/said';
import { Button, Page, Section } from '../../shared/ui';

export function render({ data, actions }: View) {
  const draft = useLocal('clipboard-draft', t('clipboard.draft'));
  const busy = useLocal('clipboard-busy', false);
  const failure = useLocal('clipboard-error', '');
  const run = async (action: () => Promise<unknown>) => {
    if (busy.value) return;
    busy.set(true);
    failure.set('');
    try {
      await action();
    } catch {
      failure.set('clipboard.failed');
    } finally {
      busy.set(false);
    }
  };
  const status = failure.value ? t(failure.value) : busy.value ? t('clipboard.working') : said(String(data.statusText || 'clipboard.state.ready'), data.statusArgs as Record<string, unknown>);
  return (
    <Page intro={t('clipboard.lxClipboardReadsAnd')}>
      <Section title={t('clipboard.status')}>
        <text data-testid="clipboard-status" className={failure.value ? 'text-sm text-danger' : 'text-sm'}>
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
          <Button testId="clipboard-write-text" disabled={busy.value} onClick={() => run(() => actions.writeText({ text: draft.value }))}>
            {t('clipboard.writeText')}
          </Button>
          <Button variant="secondary" testId="clipboard-read-text" disabled={busy.value} onClick={() => run(() => actions.readText())}>
            {t('clipboard.readText')}
          </Button>
        </view>
        <Button
          variant="secondary"
          testId="clipboard-write-typed"
          disabled={busy.value}
          onClick={() => run(() => actions.writeTypedText({ text: draft.value }))}
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
          <Button testId="clipboard-write-sample-image" disabled={busy.value} onClick={() => run(() => actions.writeSampleImage())}>
            {t('clipboard.writeSamplePng')}
          </Button>
          <Button variant="secondary" testId="clipboard-choose-image" disabled={busy.value} onClick={() => run(() => actions.chooseAndWriteImage())}>
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
          <Button variant="secondary" testId="clipboard-types" disabled={busy.value} onClick={() => run(() => actions.peekTypes())}>
            {t('clipboard.types')}
          </Button>
          <Button testId="clipboard-read-all" disabled={busy.value} onClick={() => run(() => actions.readAll())}>
            {t('clipboard.read')}
          </Button>
          <Button variant="destructive" testId="clipboard-clear" disabled={busy.value} onClick={() => run(() => actions.clearClipboard())}>
            {t('clipboard.clear')}
          </Button>
        </view>
      </Section>
    </Page>
  );
}
