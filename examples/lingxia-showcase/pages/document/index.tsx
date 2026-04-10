import { useLxPage } from '@lingxia/react';
import '../../tailwind.css';

type PageData = {
  pdfUrl?: string;
  officeUrl?: string;
  officeFileType?: string;
  showMenu?: boolean;
  isPdfDownloading?: boolean;
  isOfficeDownloading?: boolean;
  officeDownloadState?: string;
  officeProgressKnown?: boolean;
  officeDownloadProgress?: number;
  officeProgressText?: string;
  officeSupportsTransferControl?: boolean;
  officeTransferButtonText?: string;
};

type PageActions = {
  onPdfUrlInput(event: any): void;
  onOfficeUrlInput(event: any): void;
  onOfficeFileTypeInput(event: any): void;
  toggleShowMenu(): void;
  openPdf(): void;
  openOffice(): void;
  toggleOfficeTransfer(): void;
};

export default function DocumentPage() {
  const { data, actions } = useLxPage<PageData, PageActions>();
  const {
    onPdfUrlInput,
    onOfficeUrlInput,
    onOfficeFileTypeInput,
    toggleShowMenu,
    openPdf,
    openOffice,
    toggleOfficeTransfer,
  } = actions;

  const pdfUrl = data?.pdfUrl || '';
  const officeUrl = data?.officeUrl || '';
  const officeFileType = data?.officeFileType || '';
  const showMenu = Boolean(data?.showMenu);
  const isPdfDownloading = Boolean(data?.isPdfDownloading);
  const isOfficeDownloading = Boolean(data?.isOfficeDownloading);
  const officeProgressKnown = Boolean(data?.officeProgressKnown);
  const officeDownloadProgress = data?.officeDownloadProgress || 0;
  const officeProgressText = data?.officeProgressText || 'Not started yet';
  const officeSupportsTransferControl = Boolean(data?.officeSupportsTransferControl);
  const officeTransferButtonText = data?.officeTransferButtonText || 'Pause Download';
  const officePrimaryButtonText =
    data?.officeDownloadState === 'paused'
      ? 'Download Paused'
      : isOfficeDownloading
        ? 'Downloading...'
        : 'Download and Open Document';

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

        <div className="bg-white rounded-lg shadow-sm">
          <div className="px-4 py-4 border-b border-gray-100">
            <div className="text-base text-gray-900 font-medium">PDF via fetch()</div>
            <div className="text-xs text-gray-500 mt-1">Standard fetch validation, then open the resolved PDF URL in-app.</div>
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
              {isPdfDownloading ? 'Fetching PDF...' : 'Fetch and Preview PDF'}
            </button>
          </div>
        </div>

        <div className="bg-white rounded-lg shadow-sm">
          <div className="px-4 py-4 border-b border-gray-100">
            <div className="text-base text-gray-900 font-medium">Office via lx.downloadFile()</div>
            <div className="text-xs text-gray-500 mt-1">Supports: doc, docx, xls, xlsx, ppt, pptx. Promise-like task with progress and pause/continue.</div>
          </div>
          <div className="px-4 py-4 space-y-3">
            <div>
              <div className="text-sm text-gray-600 mb-2">Document URL:</div>
              <input
                type="text"
                value={officeUrl}
                onChange={(e) => onOfficeUrlInput({ detail: { value: e.target.value } })}
                placeholder="Enter document URL"
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
              disabled={isOfficeDownloading}
              className={`w-full py-3 rounded-lg text-white font-medium ${
                isOfficeDownloading
                  ? 'bg-gray-400 cursor-not-allowed'
                  : 'bg-blue-500 hover:bg-blue-600 active:bg-blue-700'
              }`}
            >
              {officePrimaryButtonText}
            </button>
            <div className="rounded-xl border border-blue-100 bg-blue-50/70 p-3">
              <div className="flex items-center justify-between text-xs text-blue-700">
                <span>Transfer Progress</span>
                <span>{officeProgressKnown ? `${Math.round(officeDownloadProgress)}%` : 'Streaming'}</span>
              </div>
              <div className="mt-2 h-2 overflow-hidden rounded-full bg-blue-100">
                <div
                  className={`h-full rounded-full bg-blue-500 transition-all duration-300 ${
                    officeProgressKnown ? '' : 'animate-pulse'
                  }`}
                  style={{ width: officeProgressKnown ? `${officeDownloadProgress}%` : '42%' }}
                />
              </div>
              <div className="mt-2 text-xs text-blue-900">{officeProgressText}</div>
              <button
                onClick={toggleOfficeTransfer}
                disabled={!isOfficeDownloading || !officeSupportsTransferControl}
                className={`mt-3 w-full rounded-lg py-2 text-sm font-medium ${
                  isOfficeDownloading && officeSupportsTransferControl
                    ? 'bg-blue-600 text-white hover:bg-blue-700 active:bg-blue-800'
                    : 'bg-blue-100 text-blue-300 cursor-not-allowed'
                }`}
              >
                {officeTransferButtonText}
              </button>
            </div>
          </div>
        </div>

      </div>
    </div>
  );
}
