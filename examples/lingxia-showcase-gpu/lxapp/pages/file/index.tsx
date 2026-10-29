import { type View } from '@lingxia/ui';
import '../../theme/index.css';
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
    <column className="gap-1.5">
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
    </column>
  );
}

export function render({ data, actions: raw }: View) {
  const actions = raw as Actions;
  const state = String(data.pdfDownloadState || 'idle');
  const paused = state === 'paused';
  const transferEnabled = !!data.pdfSupportsTransferControl && (!!data.isPdfDownloading || paused);
  const progress = Math.max(0, Math.min(100, Number(data.pdfDownloadProgress) || 0));
  const pdfLabel = paused
    ? 'Download Paused'
    : state === 'opening'
      ? 'Opening File...'
      : data.isPdfDownloading
        ? 'Downloading...'
        : 'Download and Preview PDF';
  return (
    <Page>
      <Section title="Options">
        <checkbox
          data-testid="file-show-menu"
          value={!!data.showMenu}
          onChange={() => actions.toggleShowMenu()}
          className="bg-transparent text-text"
        >
          <column className="gap-1">
            <text className="text-sm font-medium">Show Share Button</text>
            <text className="text-xs text-muted">
              Only applies to PDF documents. Office documents always open with the system default viewer.
            </text>
          </column>
        </checkbox>
      </Section>
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
        <column className="gap-4">
          <Section
            title="PDF via lx.downloadFile()"
            description="Download to a temporary file with progress and pause/continue, then open the native PDF viewer."
          >
            <UrlField
              testId="file-pdf-url"
              label="PDF URL"
              value={data.pdfUrl}
              placeholder="Enter PDF URL"
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
              <column className="p-3 gap-2 rounded-xl border border-accent/20 bg-accent-soft">
                <row className="items-center justify-between">
                  <text className="text-xs text-accent">PDF Transfer</text>
                  <text data-testid="file-pdf-progress" className="text-xs text-accent">
                    {data.pdfProgressKnown ? `${Math.round(progress)}%` : 'Streaming'}
                  </text>
                </row>
                {data.pdfProgressKnown ? (
                  <row className="h-2 rounded-full bg-accent-soft">
                    <spacer data-testid="file-progress-bar" className="h-2 rounded-full bg-accent" style={{ width: `${progress}%` }} />
                  </row>
                ) : (
                  <text className="text-[11px] text-accent">Waiting for precise progress from runtime...</text>
                )}
                <text data-testid="file-pdf-status" className="text-xs">
                  {String(data.pdfProgressText || '')}
                </text>
                <Button size="sm" testId="file-pdf-transfer" disabled={!transferEnabled} onClick={() => actions.togglePdfTransfer()}>
                  {String(data.pdfTransferButtonText || 'Pause Download')}
                </Button>
              </column>
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
              disabled={!!data.isOfficeFetching}
              onInput={value => actions.onOfficeUrlInput({ detail: { value } })}
            />
            <UrlField
              testId="file-office-type"
              label="File Type"
              value={data.officeFileType}
              placeholder="e.g., docx, xlsx, pptx"
              disabled={!!data.isOfficeFetching}
              onInput={value => actions.onOfficeFileTypeInput({ detail: { value } })}
            />
            <text className="text-xs text-muted">Auto-detected from URL or enter manually</text>
            <Button
              testId="file-office-open"
              disabled={!!data.isOfficeFetching || !String(data.officeUrl || '').trim() || !String(data.officeFileType || '').trim()}
              onClick={() => actions.openOffice()}
            >
              {data.isOfficeFetching ? 'Fetching and Opening File...' : 'Fetch and Open File'}
            </Button>
            {!!data.officeStatusText && (
              <text data-testid="file-office-status" className="p-3 rounded-xl border border-accent/20 bg-accent-soft text-xs">
                {String(data.officeStatusText)}
              </text>
            )}
          </Section>
        </column>
      )}
    </Page>
  );
}
