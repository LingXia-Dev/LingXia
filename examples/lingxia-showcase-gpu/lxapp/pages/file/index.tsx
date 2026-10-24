import { type View } from '@lingxia/ui';
import '../../theme/index.css';

export function render({ data, actions, env }: View) {
  const state = String(data.pdfDownloadState || 'idle');
  const transferEnabled = !!data.pdfSupportsTransferControl && (!!data.isPdfDownloading || state === 'paused');
  const progress = Math.max(0, Math.min(100, Number(data.pdfDownloadProgress) || 0));
  const height = Math.max(1, env.viewport.height - env.insets.top - Math.max(env.insets.bottom, env.keyboard.height));
  const field = 'px-3 py-2 border border-line-300 rounded-lg text-sm bg-surface text-gray-900';
  const primary = 'py-3 rounded-lg text-white font-medium bg-blue-500';
  const disabled = 'py-3 rounded-lg text-white font-medium bg-surface-400';
  const card = 'bg-surface rounded-lg shadow-sm';
  return <column className="bg-surface-100 text-gray-900">
    <spacer style={{ height: env.insets.top }} />
    <row style={{ height }}><spacer style={{ width: env.insets.left }} />
      <scroll testId="page-scroll" className="grow basis-0 min-w-0" style={{ height }}>
        <column className="px-3 pt-6 pb-12 gap-3">
          <column className={card}>
            <column className="px-4 py-3 border-b border-line-100"><text className="text-base font-medium">Options</text></column>
            <checkbox testId="file-show-menu" accessibilityLabel="Show Share Button. Only applies to PDF documents. Office documents always open with system default viewer." value={!!data.showMenu} onChange={() => actions.toggleShowMenu()} className="px-4 py-3">
              <row className="items-start gap-3">
                <column className="pt-0.5 shrink-0"><column className={data.showMenu ? 'w-5 h-5 rounded bg-blue-500 dark:bg-blue-400 items-center justify-center' : 'w-5 h-5 rounded border border-line-300 bg-surface'}>{data.showMenu ? <vector className="w-4 h-4 text-white" paths={[{ d: 'M5 12l4 4L19 6', fill: 'none', stroke: 'currentColor', strokeWidth: 2 }]} /> : null}</column></column>
                <column className="grow basis-0 min-w-0 gap-1"><text className="text-sm font-medium">Show Share Button</text><text className="text-xs text-gray-500">Only applies to PDF documents. Office documents always open with system default viewer.</text></column>
              </row>
            </checkbox>
          </column>
          {data.activeDemo === 'chooseFile' ? <column className={card}>
            <column className="px-4 py-4 border-b border-line-100 gap-1"><text className="text-base font-medium">Choose File</text><text className="text-xs text-gray-500">Open the host chooser in a predefined folder instead of the system recent-files picker.</text></column>
            <column className="px-4 py-4 gap-3">
              <text className="text-sm text-gray-600">Default folder:</text>
              <text testId="file-default-path" className="rounded-lg bg-surface-50 border border-line-200 px-3 py-2 text-xs text-gray-700">{String(data.chooseFileDefaultPath || '')}</text>
              <button testId="file-choose" onClick={() => actions.chooseFileFromUserCache()} className={primary}>Open File Chooser</button>
              <column className="rounded-xl border border-line-200 bg-surface-50 p-3 gap-2">
                <text className="text-xs text-gray-500">Status</text><text testId="file-choose-status" className="text-sm">{String(data.chooseFileStatusText || 'Choose a file')}</text>
                <text className="text-xs text-gray-500">Selected Path</text><text testId="file-selected-path" className="text-xs text-gray-700">{String(data.chooseFileSelectedPath || 'None')}</text>
                <text className="text-xs text-gray-500">Detected Type</text><text testId="file-selected-type" className="text-xs text-gray-700">{String(data.chooseFileSelectedType || 'Unknown')}</text>
              </column>
              <button testId="file-open-selected" enabled={!!data.chooseFileSelectedPath} onClick={() => actions.openChosenFile()} className={data.chooseFileSelectedPath ? 'py-3 rounded-lg text-white font-medium bg-surface-900' : disabled}>Open Selected File</button>
            </column>
          </column> : <column className="gap-3">
            <column className={card}>
              <column className="px-4 py-4 border-b border-line-100 gap-1"><text className="text-base font-medium">PDF via lx.downloadFile()</text><text className="text-xs text-gray-500">Download to a temporary file with progress and pause/continue, then open with the native PDF viewer.</text></column>
              <column className="px-4 py-4 gap-3">
                <column className="gap-2"><text className="text-sm text-gray-600">PDF URL:</text><input testId="file-pdf-url" value={String(data.pdfUrl || '')} onInput={value => actions.onPdfUrlInput({ detail: { value } })} enabled={!data.isPdfDownloading} placeholder="Enter PDF URL" className={field} /></column>
                <button testId="file-pdf-open" enabled={!data.isPdfDownloading && state !== 'paused' && !!String(data.pdfUrl || '').trim()} onClick={() => actions.openPdf()} className={data.isPdfDownloading || state === 'paused' ? disabled : primary}>{state === 'paused' ? 'Download Paused' : state === 'opening' ? 'Opening File...' : data.isPdfDownloading ? 'Downloading...' : 'Download and Preview PDF'}</button>
                {state !== 'idle' || data.pdfProgressText ? <column className="rounded-xl border border-blue-100 bg-blue-50/70 p-3 gap-2">
                  <row className="items-center justify-between text-xs text-blue-700 dark:text-blue-400"><text>PDF Transfer</text><text testId="file-pdf-progress">{data.pdfProgressKnown ? `${Math.round(progress)}%` : 'Streaming'}</text></row>
                  {data.pdfProgressKnown ? <stack className="h-2 rounded-full bg-blue-100"><column testId="file-progress-bar" className="h-2 rounded-full bg-blue-500" style={{ width: `${progress}%` }} /></stack> : <row className="items-center gap-2"><column className="w-2.5 h-2.5 rounded-full bg-blue-500" /><text className="text-[11px] text-blue-700 dark:text-blue-400">Waiting for precise progress from runtime...</text></row>}
                  <text testId="file-pdf-status" className="text-xs text-blue-900 dark:text-blue-400">{String(data.pdfProgressText || '')}</text>
                  <button testId="file-pdf-transfer" enabled={transferEnabled} onClick={() => actions.togglePdfTransfer()} className={transferEnabled ? 'rounded-lg py-2 text-sm font-medium bg-blue-600 text-white' : 'rounded-lg py-2 text-sm font-medium bg-blue-100 text-blue-400'}>{String(data.pdfTransferButtonText || 'Pause Download')}</button>
                </column> : null}
              </column>
            </column>
            <column className={card}>
              <column className="px-4 py-4 border-b border-line-100 gap-1"><text className="text-base font-medium">Office via fetch()</text><text className="text-xs text-gray-500">Use web-standard fetch in page logic, save into usercache, then open with the native file API.</text></column>
              <column className="px-4 py-4 gap-3">
                <column className="gap-2"><text className="text-sm text-gray-600">File URL:</text><input testId="file-office-url" value={String(data.officeUrl || '')} onInput={value => actions.onOfficeUrlInput({ detail: { value } })} enabled={!data.isOfficeFetching} placeholder="Enter file URL" className={field} /></column>
                <column className="gap-2"><text className="text-sm text-gray-600">File Type:</text><input testId="file-office-type" value={String(data.officeFileType || '')} onInput={value => actions.onOfficeFileTypeInput({ detail: { value } })} enabled={!data.isOfficeFetching} placeholder="e.g., docx, xlsx, pptx" className={field} /><text className="text-xs text-gray-500">Auto-detected from URL or enter manually</text></column>
                <button testId="file-office-open" enabled={!data.isOfficeFetching && !!String(data.officeUrl || '').trim() && !!String(data.officeFileType || '').trim()} onClick={() => actions.openOffice()} className={data.isOfficeFetching ? disabled : primary}>{data.isOfficeFetching ? 'Fetching and Opening File...' : 'Fetch and Open File'}</button>
                {data.officeStatusText ? <text testId="file-office-status" className="rounded-xl border border-blue-100 bg-blue-50/70 p-3 text-xs text-blue-900 dark:text-blue-400">{String(data.officeStatusText)}</text> : null}
              </column>
            </column>
          </column>}
        </column>
      </scroll><spacer style={{ width: env.insets.right }} />
    </row><spacer style={{ height: Math.max(env.insets.bottom, env.keyboard.height) }} />
  </column>;
}
