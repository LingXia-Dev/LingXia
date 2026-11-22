import { useLocal, useStream } from '@lingxia/view';
import '../../theme/index.css';
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
  if (!progress) return 'Starting transfer...';
  const loaded = formatBytes(progress.loaded);
  if (progress.total === undefined) return `Streaming · ${loaded} downloaded`;
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
    download.status === 'completed' ? `Opened ${download.result?.path ?? ''}`
      : download.status === 'failed' ? download.error?.message ?? 'Download failed'
        : download.status === 'canceled' ? 'Download canceled'
          : paused.value ? 'Download paused'
            : progressText(progress.value);
  const pdfLabel = paused.value ? 'Download Paused' : streaming ? 'Downloading...' : 'Download and Preview PDF';
  const togglePause = async () => {
    if (paused.value) await actions.resumePdf();
    else await actions.pausePdf();
    paused.set(!paused.value);
  };
  return (
    <Page>
      {data.activeDemo === 'chooseFile' ? (
        <Section title="Choose File" description="Open the host chooser in a predefined folder instead of the recent-files picker.">
          <InfoRow label="Default folder" value={data.chooseFileDefaultPath} testId="file-default-path" />
          <Button testId="file-choose" onClick={() => actions.chooseFileFromUserCache()}>
            Open File Chooser
          </Button>
          <InfoList>
            <InfoRow label="Status" value={data.chooseFileStatusText || 'Choose a file'} testId="file-choose-status" />
            <InfoRow label="Selected Path" value={data.chooseFileSelectedPath || 'None'} testId="file-selected-path" />
            <InfoRow label="Detected Type" value={data.chooseFileSelectedType || 'Unknown'} testId="file-selected-type" />
          </InfoList>
          <Button variant="secondary" testId="file-open-selected" disabled={!data.chooseFileSelectedPath} onClick={() => actions.openChosenFile()}>
            Open Selected File
          </Button>
        </Section>
      ) : (
        <view className="gap-4">
          <Section
            title="PDF via lx.network.download()"
            description="Download to a temporary file with progress and pause/continue, then open the native PDF viewer."
          >
            <UrlField
              testId="file-pdf-url"
              label="PDF URL"
              value={data.pdfUrl}
              placeholder="Enter PDF URL"
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
                  <text className="text-xs text-accent">PDF Transfer</text>
                  <text data-testid="file-pdf-progress" className="text-xs text-accent">
                    {fraction === undefined ? 'Streaming' : `${Math.round(fraction * 100)}%`}
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
                      {paused.value ? 'Continue Download' : 'Pause Download'}
                    </Button>
                    <Button size="sm" variant="secondary" testId="file-pdf-cancel" onClick={() => download.cancel()}>
                      Cancel
                    </Button>
                  </view>
                )}
              </view>
            )}
          </Section>
          <Section
            title="Office via fetch()"
            description="Use web-standard fetch in page logic, save into usercache, then open with the native file API."
          >
            <UrlField
              testId="file-office-url"
              label="File URL"
              value={data.officeUrl}
              placeholder="Enter file URL"
              disabled={data.isOfficeFetching}
              onInput={url => actions.setOfficeUrl({ url })}
            />
            <UrlField
              testId="file-office-type"
              label="File Type"
              value={data.officeFileType}
              placeholder="e.g., docx, xlsx, pptx"
              disabled={data.isOfficeFetching}
              onInput={type => actions.setOfficeFileType({ type })}
            />
            <text className="text-xs text-muted">Auto-detected from URL or enter manually</text>
            <Button
              testId="file-office-open"
              disabled={data.isOfficeFetching || !data.officeUrl.trim() || !data.officeFileType.trim()}
              onClick={() => actions.openOffice()}
            >
              {data.isOfficeFetching ? 'Fetching and Opening File...' : 'Fetch and Open File'}
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
