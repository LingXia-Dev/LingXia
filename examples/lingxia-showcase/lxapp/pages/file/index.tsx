import { t, useLocal, useStream, type Stream } from '@lingxia/view';
import '../../theme/index.css';
import { said } from '../../shared/lib/said';
import { Button, InfoList, InfoRow, Page, Section } from '../../shared/ui';
import type { View } from './$types';

type Progress = { readonly loaded: number; readonly total?: number; readonly fraction?: number };

function formatBytes(bytes: number) {
  if (!Number.isFinite(bytes) || bytes <= 0) return '0 B';
  const units = ['B', 'KB', 'MB', 'GB'];
  let value = bytes;
  let unit = 0;
  while (value >= 1024 && unit < units.length - 1) {
    value /= 1024;
    unit += 1;
  }
  return `${value.toFixed(unit === 0 ? 0 : value >= 10 ? 1 : 2)} ${units[unit]}`;
}

function progressText(progress: Progress | null) {
  if (!progress) return t('file.startingTransfer');
  const loaded = formatBytes(progress.loaded);
  if (progress.total === undefined) return t('file.streamed', { downloaded: loaded });
  return `${Math.round((progress.fraction ?? 0) * 100)}% · ${loaded} / ${formatBytes(progress.total)}`;
}

function UrlField({
  testId,
  label,
  value,
  placeholder,
  disabled,
  onInput,
}: {
  testId: string;
  label: string;
  value: string;
  placeholder: string;
  disabled: boolean;
  onInput: (value: string) => unknown;
}) {
  return (
    <view className="gap-1.5">
      <text className="text-sm text-muted">{label}</text>
      <input
        data-testid={testId}
        aria-label={label}
        value={value}
        disabled={disabled}
        placeholder={placeholder}
        onInput={value => {
          void onInput(value);
        }}
        className="px-3 py-2 rounded-lg border border-line-strong bg-surface text-sm"
      />
    </view>
  );
}

export function render({ data, actions }: View) {
  const progress = useLocal<Progress | null>('pdf-progress', null);
  const paused = useLocal('pdf-paused', false);
  const download = useStream('pdf', actions.downloadPdf, { onData: value => progress.set(value) });
  const streaming = download.status === 'streaming';
  const fraction = progress.value?.fraction;
  const status =
    download.status === 'completed' ? t('file.opened', { path: download.result?.path ?? '' })
      : download.status === 'failed' ? download.error?.message ?? t('file.downloadFailed')
        : download.status === 'canceled' ? t('file.downloadCanceled')
          : paused.value ? t('file.paused')
            : progressText(progress.value);
  const pdfLabel = paused.value ? t('file.downloadPaused') : streaming ? t('file.downloading') : t('file.downloadPdf');
  const togglePause = async () => {
    if (paused.value) await actions.resumePdf();
    else await actions.pausePdf();
    paused.set(!paused.value);
  };
  return (
    <Page>
      {data.activeDemo === 'chooseFile' ? (
        <Section title={t('file.chooseFile')} description={t('file.openTheHostChooser')}>
          <InfoRow label={t('file.defaultFolder')} value={data.chooseFileDefaultPath} testId="file-default-path" />
          <Button testId="file-choose" onClick={() => actions.chooseFileFromUserCache()}>
            {t('file.openFileChooser')}
          </Button>
          <InfoList>
            <InfoRow label={t('file.status')} value={said(String(data.chooseFileStatusText || 'file.state.choose'))} testId="file-choose-status" />
            <InfoRow label={t('file.selectedPath')} value={data.chooseFileSelectedPath || 'None'} testId="file-selected-path" />
            <InfoRow label={t('file.detectedType')} value={data.chooseFileSelectedType || 'Unknown'} testId="file-selected-type" />
          </InfoList>
          <Button variant="secondary" testId="file-open-selected" disabled={!data.chooseFileSelectedPath} onClick={() => actions.openChosenFile()}>
            {t('file.openSelectedFile')}
          </Button>
        </Section>
      ) : (
        <view className="gap-4">
          <Section
            title={t('file.pdfViaLxNetwork')}
            description={t('file.downloadToATemporary')}
          >
            <UrlField
              testId="file-pdf-url"
              label={t('file.pdfUrl')}
              value={data.pdfUrl}
              placeholder={t('file.enterPdfUrl')}
              disabled={streaming}
              onInput={url => actions.setPdfUrl({ url })}
            />
            <Button
              testId="file-pdf-open"
              disabled={streaming || !data.pdfUrl.trim()}
              onClick={() => {
                progress.set(null);
                paused.set(false);
                download.start(data.pdfUrl);
              }}
            >
              {pdfLabel}
            </Button>
            {download.status !== 'idle' && (
              <view className="p-3 gap-2 rounded-xl border border-accent/20 bg-accent-soft">
                <view className="flex-row items-center justify-between">
                  <text className="text-xs text-accent">{t('file.pdfTransfer')}</text>
                  <text data-testid="file-pdf-progress" className="text-xs text-accent">
                    {fraction === undefined ? t('file.streaming') : `${Math.round(fraction * 100)}%`}
                  </text>
                </view>
                {fraction !== undefined && (
                  <view className="flex-row h-2 rounded-full bg-accent-soft">
                    <view data-testid="file-progress-bar" className="h-2 rounded-full bg-accent" style={{ width: `${fraction * 100}%` }} />
                  </view>
                )}
                <text data-testid="file-pdf-status" className="text-xs">{status}</text>
                {streaming && (
                  <view className="flex-row gap-2">
                    <Button size="sm" testId="file-pdf-transfer" onClick={() => void togglePause()}>
                      {paused.value ? t('file.continueDownload') : t('file.pauseDownload')}
                    </Button>
                    <Button size="sm" variant="secondary" testId="file-pdf-cancel" onClick={() => download.cancel()}>
                      {t('common.cancel')}
                    </Button>
                  </view>
                )}
              </view>
            )}
          </Section>
          <Section
            title={t('file.officeViaFetch')}
            description={t('file.useWebStandardFetch')}
          >
            <UrlField
              testId="file-office-url"
              label={t('file.fileUrl')}
              value={data.officeUrl}
              placeholder={t('file.enterFileUrl')}
              disabled={data.isOfficeFetching}
              onInput={url => actions.setOfficeUrl({ url })}
            />
            <UrlField
              testId="file-office-type"
              label={t('file.fileType')}
              value={data.officeFileType}
              placeholder={t('file.eGDocxXlsx')}
              disabled={data.isOfficeFetching}
              onInput={type => actions.setOfficeFileType({ type })}
            />
            <text className="text-xs text-muted">{t('file.autoDetectedFromUrl')}</text>
            <Button
              testId="file-office-open"
              disabled={data.isOfficeFetching || !data.officeUrl.trim() || !data.officeFileType.trim()}
              onClick={() => actions.openOffice()}
            >
              {data.isOfficeFetching ? t('file.fetching') : t('file.fetchOpen')}
            </Button>
            {!!data.officeStatusText && (
              <text data-testid="file-office-status" className="p-3 rounded-xl border border-accent/20 bg-accent-soft text-xs">
                {data.officeStatusText}
              </text>
            )}
          </Section>
        </view>
      )}
    </Page>
  );
}
