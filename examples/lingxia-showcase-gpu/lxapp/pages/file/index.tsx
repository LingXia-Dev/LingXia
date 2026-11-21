import { t, type View } from '@lingxia/view';
import '../../theme/index.css';
import { said } from '../../shared/lib/said';
import { Button, InfoList, InfoRow, Page, Section } from '../../shared/ui';

type Actions = Record<string, (payload?: unknown) => Promise<unknown>>;

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
  value: unknown;
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
        value={String(value || '')}
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

export function render({ data, actions: raw }: View) {
  const actions = raw as Actions;
  const state = String(data.pdfDownloadState || 'idle');
  const paused = state === 'paused';
  const transferEnabled = !!data.pdfSupportsTransferControl && (!!data.isPdfDownloading || paused);
  const progress = Math.max(0, Math.min(100, Number(data.pdfDownloadProgress) || 0));
  const pdfLabel = paused
    ? t('file.downloadPaused')
    : state === 'opening'
      ? t('file.openingFile')
      : data.isPdfDownloading
        ? t('file.downloading')
        : t('file.downloadPdf');
  return (
    <Page>
      <Section title={t('file.options')}>
        <checkbox
          data-testid="file-show-menu"
          value={!!data.showMenu}
          onChange={() => actions.toggleShowMenu()}
          className="bg-transparent text-text"
        >
          <view className="gap-1">
            <text className="text-sm font-medium">{t('file.showShareButton')}</text>
            <text className="text-xs text-muted">
              {t('file.onlyAppliesToPdf')}
            </text>
          </view>
        </checkbox>
      </Section>
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
              disabled={!!data.isPdfDownloading}
              onInput={value => actions.onPdfUrlInput({ detail: { value } })}
            />
            <Button
              testId="file-pdf-open"
              disabled={!!data.isPdfDownloading || paused || !String(data.pdfUrl || '').trim()}
              onClick={() => actions.openPdf()}
            >
              {pdfLabel}
            </Button>
            {(state !== 'idle' || !!data.pdfProgressText) && (
              <view className="p-3 gap-2 rounded-xl border border-accent/20 bg-accent-soft">
                <view className="flex-row items-center justify-between">
                  <text className="text-xs text-accent">{t('file.pdfTransfer')}</text>
                  <text data-testid="file-pdf-progress" className="text-xs text-accent">
                    {data.pdfProgressKnown ? `${Math.round(progress)}%` : t('file.streaming')}
                  </text>
                </view>
                {data.pdfProgressKnown ? (
                  <view className="flex-row h-2 rounded-full bg-accent-soft">
                    <view data-testid="file-progress-bar" className="h-2 rounded-full bg-accent" style={{ width: `${progress}%` }} />
                  </view>
                ) : (
                  <text className="text-[11px] text-accent">{t('file.waitingForPreciseProgress')}</text>
                )}
                <text data-testid="file-pdf-status" className="text-xs">
                  {String(data.pdfProgressText || '')}
                </text>
                <Button size="sm" testId="file-pdf-transfer" disabled={!transferEnabled} onClick={() => actions.togglePdfTransfer()}>
                  {t(String(data.pdfTransferButtonText || 'file.pauseDownload'))}
                </Button>
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
              disabled={!!data.isOfficeFetching}
              onInput={value => actions.onOfficeUrlInput({ detail: { value } })}
            />
            <UrlField
              testId="file-office-type"
              label={t('file.fileType')}
              value={data.officeFileType}
              placeholder={t('file.eGDocxXlsx')}
              disabled={!!data.isOfficeFetching}
              onInput={value => actions.onOfficeFileTypeInput({ detail: { value } })}
            />
            <text className="text-xs text-muted">{t('file.autoDetectedFromUrl')}</text>
            <Button
              testId="file-office-open"
              disabled={!!data.isOfficeFetching || !String(data.officeUrl || '').trim() || !String(data.officeFileType || '').trim()}
              onClick={() => actions.openOffice()}
            >
              {data.isOfficeFetching ? t('file.fetching') : t('file.fetchOpen')}
            </Button>
            {!!data.officeStatusText && (
              <text data-testid="file-office-status" className="p-3 rounded-xl border border-accent/20 bg-accent-soft text-xs">
                {String(data.officeStatusText)}
              </text>
            )}
          </Section>
        </view>
      )}
    </Page>
  );
}
