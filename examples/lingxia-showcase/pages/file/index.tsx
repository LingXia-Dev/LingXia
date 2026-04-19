import { useLxPage } from '@lingxia/react';
import '../../tailwind.css';

type ActiveDemo = 'openFile' | 'chooseFile';

type PageData = {
  activeDemo?: ActiveDemo;
  pdfUrl?: string;
  officeUrl?: string;
  officeFileType?: string;
  showMenu?: boolean;
  chooseFileDefaultPath?: string;
  chooseFileStatusText?: string;
  chooseFileSelectedPath?: string;
  chooseFileSelectedType?: string;
  isPdfDownloading?: boolean;
  pdfDownloadState?: string;
  pdfProgressKnown?: boolean;
  pdfDownloadProgress?: number;
  pdfProgressText?: string;
  pdfSupportsTransferControl?: boolean;
  pdfTransferButtonText?: string;
  isOfficeFetching?: boolean;
  officeStatusText?: string;
};

type PageActions = {
  onPdfUrlInput(event: any): void;
  onOfficeUrlInput(event: any): void;
  onOfficeFileTypeInput(event: any): void;
  toggleShowMenu(): void;
  chooseFileFromUserCache(): void;
  openChosenFile(): void;
  openPdf(): void;
  openOffice(): void;
  togglePdfTransfer(): void;
};

export default function FilePage() {
  const { data, actions } = useLxPage<PageData, PageActions>();
  const {
    onPdfUrlInput,
    onOfficeUrlInput,
    onOfficeFileTypeInput,
    toggleShowMenu,
    chooseFileFromUserCache,
    openChosenFile,
    openPdf,
    openOffice,
    togglePdfTransfer,
  } = actions;

  const activeDemo: ActiveDemo = data?.activeDemo || 'openFile';
  const pdfUrl = data?.pdfUrl || '';
  const officeUrl = data?.officeUrl || '';
  const officeFileType = data?.officeFileType || '';
  const showMenu = Boolean(data?.showMenu);
  const chooseFileDefaultPath = data?.chooseFileDefaultPath || '';
  const chooseFileStatusText = data?.chooseFileStatusText || 'Choose a file';
  const chooseFileSelectedPath = data?.chooseFileSelectedPath || '';
  const chooseFileSelectedType = data?.chooseFileSelectedType || '';
  const isPdfDownloading = Boolean(data?.isPdfDownloading);
  const pdfDownloadState = data?.pdfDownloadState || 'idle';
  const pdfProgressKnown = Boolean(data?.pdfProgressKnown);
  const pdfDownloadProgress = data?.pdfDownloadProgress || 0;
  const pdfProgressText = data?.pdfProgressText || '';
  const pdfSupportsTransferControl = Boolean(data?.pdfSupportsTransferControl);
  const pdfTransferButtonText = data?.pdfTransferButtonText || 'Pause Download';
  const showPdfProgress =
    pdfDownloadState !== 'idle' || Boolean(data?.pdfProgressText);
  const pdfPrimaryButtonText =
    pdfDownloadState === 'paused'
      ? 'Download Paused'
      : pdfDownloadState === 'opening'
        ? 'Opening File...'
      : isPdfDownloading
        ? 'Downloading...'
        : 'Download and Preview PDF';
  const isOfficeFetching = Boolean(data?.isOfficeFetching);
  const officeStatusText = data?.officeStatusText || '';
  const officePrimaryButtonText = isOfficeFetching
    ? 'Fetching and Opening File...'
    : 'Fetch and Open File';

  return (
    <div className="min-h-screen bg-gray-100">
      <div className="px-3 pt-6 pb-12 space-y-3">
        <div className="bg-white rounded-lg shadow-sm">
          <div className="px-4 py-3 border-b border-gray-100">
            <div className="text-base text-gray-900 font-medium">Options</div>
          </div>
          <div className="px-4 py-3">
            <label className="flex items-start cursor-pointer">
              <input
                type="checkbox"
                checked={showMenu}
                onChange={() => toggleShowMenu()}
                className="w-5 h-5 text-blue-500 border-gray-300 rounded focus:ring-2 focus:ring-blue-500 mt-0.5"
              />
              <div className="ml-3 flex-1">
                <div className="text-sm text-gray-900 font-medium">Show Share Button</div>
                <div className="text-xs text-gray-500 mt-1">
                  Only applies to PDF documents. Office documents always open with system default viewer.
                </div>
              </div>
            </label>
          </div>
        </div>

        {activeDemo === 'openFile' ? (
          <>
            <div className="bg-white rounded-lg shadow-sm">
              <div className="px-4 py-4 border-b border-gray-100">
                <div className="text-base text-gray-900 font-medium">PDF via lx.downloadFile()</div>
                <div className="text-xs text-gray-500 mt-1">Download to a temporary file with progress and pause/continue, then open with the native PDF viewer.</div>
              </div>
              <div className="px-4 py-4 space-y-3">
                <div>
                  <div className="text-sm text-gray-600 mb-2">PDF URL:</div>
                  <input
                    type="text"
                    value={pdfUrl}
                    onChange={(e) => onPdfUrlInput({ detail: { value: e.target.value } })}
                    placeholder="Enter PDF URL"
                    className="w-full px-3 py-2 border border-gray-300 rounded-lg text-sm focus:outline-none focus:ring-2 focus:ring-blue-500"
                  />
                </div>
                <button
                  onClick={openPdf}
                  disabled={isPdfDownloading}
                  className={`w-full py-3 rounded-lg text-white font-medium ${
                    isPdfDownloading
                      ? 'bg-gray-400 cursor-not-allowed'
                      : 'bg-blue-500 hover:bg-blue-600 active:bg-blue-700'
                  }`}
                >
                  {pdfPrimaryButtonText}
                </button>
                {showPdfProgress ? (
                  <div className="rounded-xl border border-blue-100 bg-blue-50/70 p-3">
                    <div className="flex items-center justify-between text-xs text-blue-700">
                      <span>PDF Transfer</span>
                      <span>{pdfProgressKnown ? `${Math.round(pdfDownloadProgress)}%` : 'Streaming'}</span>
                    </div>
                    {pdfProgressKnown ? (
                      <div className="mt-2 h-2 overflow-hidden rounded-full bg-blue-100">
                        <div
                          className="h-full rounded-full bg-blue-500 transition-all duration-300"
                          style={{ width: `${pdfDownloadProgress}%` }}
                        />
                      </div>
                    ) : (
                      <div className="mt-2 flex items-center gap-2 text-[11px] text-blue-700">
                        <span className="inline-flex h-2.5 w-2.5 rounded-full bg-blue-500 animate-pulse" />
                        <span>Waiting for precise progress from runtime…</span>
                      </div>
                    )}
                    <div className="mt-2 text-xs text-blue-900">{pdfProgressText}</div>
                    <button
                      onClick={togglePdfTransfer}
                      disabled={!isPdfDownloading || !pdfSupportsTransferControl}
                      className={`mt-3 w-full rounded-lg py-2 text-sm font-medium ${
                        isPdfDownloading && pdfSupportsTransferControl
                          ? 'bg-blue-600 text-white hover:bg-blue-700 active:bg-blue-800'
                          : 'bg-blue-100 text-blue-300 cursor-not-allowed'
                      }`}
                    >
                      {pdfTransferButtonText}
                    </button>
                  </div>
                ) : null}
              </div>
            </div>

            <div className="bg-white rounded-lg shadow-sm">
              <div className="px-4 py-4 border-b border-gray-100">
                <div className="text-base text-gray-900 font-medium">Office via fetch()</div>
                <div className="text-xs text-gray-500 mt-1">Use web-standard fetch in page logic, save into usercache, then open with the native file API.</div>
              </div>
              <div className="px-4 py-4 space-y-3">
                <div>
                  <div className="text-sm text-gray-600 mb-2">File URL:</div>
                  <input
                    type="text"
                    value={officeUrl}
                    onChange={(e) => onOfficeUrlInput({ detail: { value: e.target.value } })}
                    placeholder="Enter file URL"
                    className="w-full px-3 py-2 border border-gray-300 rounded-lg text-sm focus:outline-none focus:ring-2 focus:ring-blue-500"
                  />
                </div>
                <div>
                  <div className="text-sm text-gray-600 mb-2">File Type:</div>
                  <input
                    type="text"
                    value={officeFileType}
                    onChange={(e) => onOfficeFileTypeInput({ detail: { value: e.target.value } })}
                    placeholder="e.g., docx, xlsx, pptx"
                    className="w-full px-3 py-2 border border-gray-300 rounded-lg text-sm focus:outline-none focus:ring-2 focus:ring-blue-500"
                  />
                  <div className="text-xs text-gray-500 mt-1">Auto-detected from URL or enter manually</div>
                </div>
                <button
                  onClick={openOffice}
                  disabled={isOfficeFetching}
                  className={`w-full py-3 rounded-lg text-white font-medium ${
                    isOfficeFetching
                      ? 'bg-gray-400 cursor-not-allowed'
                      : 'bg-blue-500 hover:bg-blue-600 active:bg-blue-700'
                  }`}
                >
                  {officePrimaryButtonText}
                </button>
                {officeStatusText ? (
                  <div className="rounded-xl border border-blue-100 bg-blue-50/70 p-3 text-xs text-blue-900">
                    {officeStatusText}
                  </div>
                ) : null}
              </div>
            </div>
          </>
        ) : (
          <div className="bg-white rounded-lg shadow-sm">
            <div className="px-4 py-4 border-b border-gray-100">
              <div className="text-base text-gray-900 font-medium">Choose File</div>
              <div className="text-xs text-gray-500 mt-1">Open the host chooser in a predefined folder instead of the system recent-files picker.</div>
            </div>
            <div className="px-4 py-4 space-y-3">
              <div className="text-sm text-gray-600">Default folder:</div>
              <div className="rounded-lg bg-gray-50 border border-gray-200 px-3 py-2 text-xs text-gray-700 break-all">
                {chooseFileDefaultPath}
              </div>
              <button
                onClick={chooseFileFromUserCache}
                className="w-full py-3 rounded-lg bg-blue-500 hover:bg-blue-600 active:bg-blue-700 text-white font-medium"
              >
                Open File Chooser
              </button>
              <div className="rounded-xl border border-gray-200 bg-gray-50 p-3 space-y-2">
                <div className="text-xs text-gray-500">Status</div>
                <div className="text-sm text-gray-900">{chooseFileStatusText}</div>
                <div className="text-xs text-gray-500">Selected Path</div>
                <div className="text-xs text-gray-700 break-all">{chooseFileSelectedPath || 'None'}</div>
                <div className="text-xs text-gray-500">Detected Type</div>
                <div className="text-xs text-gray-700">{chooseFileSelectedType || 'Unknown'}</div>
              </div>
              <button
                onClick={openChosenFile}
                disabled={!chooseFileSelectedPath}
                className={`w-full py-3 rounded-lg text-white font-medium ${
                  chooseFileSelectedPath
                    ? 'bg-gray-900 hover:bg-black active:bg-gray-800'
                    : 'bg-gray-400 cursor-not-allowed'
                }`}
              >
                Open Selected File
              </button>
            </div>
          </div>
        )}
      </div>
    </div>
  );
}
