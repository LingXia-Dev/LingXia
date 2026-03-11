import { useLingXia } from '@lingxia/core/react';
import '../../tailwind.css';

type PageData = {
  pdfUrl?: string;
  officeUrl?: string;
  officeFileType?: string;
  showMenu?: boolean;
  isPdfDownloading?: boolean;
  isOfficeDownloading?: boolean;
  pdfDownloadPaused?: boolean;
  pdfDownloadProgress?: number;
  officeDownloadProgress?: number;
  officeCached?: boolean;
};

type PageActions = {
  data: PageData;
  onPdfUrlInput(event: any): void;
  onOfficeUrlInput(event: any): void;
  onOfficeFileTypeInput(event: any): void;
  toggleShowMenu(): void;
  openPdf(): void;
  pausePdfDownload(): void;
  resumePdfDownload(): void;
  cancelPdfDownload(): void;
  openOffice(): void;
  cancelOfficeDownload(): void;
};

export default function DocumentPage() {
  const {
    data,
    onPdfUrlInput,
    onOfficeUrlInput,
    onOfficeFileTypeInput,
    toggleShowMenu,
    openPdf,
    pausePdfDownload,
    resumePdfDownload,
    cancelPdfDownload,
    openOffice,
    cancelOfficeDownload,
  } = useLingXia();

  const pdfUrl = data?.pdfUrl || '';
  const officeUrl = data?.officeUrl || '';
  const officeFileType = data?.officeFileType || '';
  const showMenu = Boolean(data?.showMenu);
  const isPdfDownloading = Boolean(data?.isPdfDownloading);
  const pdfDownloadPaused = Boolean(data?.pdfDownloadPaused);
  const isOfficeDownloading = Boolean(data?.isOfficeDownloading);
  const pdfDownloadProgress = Number(data?.pdfDownloadProgress || 0);
  const officeDownloadProgress = Number(data?.officeDownloadProgress || 0);
  const officeCached = Boolean(data?.officeCached);

  return (
    <div className="min-h-screen bg-gray-100">
      <div className="px-3 pt-6 pb-12 space-y-3">

        {/* Options Section */}
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
                <div className="text-sm text-gray-900 font-medium">
                  Show Share Button
                </div>
                <div className="text-xs text-gray-500 mt-1">
                  Only applies to PDF documents. Office documents (Word, Excel, PowerPoint) and other files always open with system default viewer.
                </div>
              </div>
            </label>
          </div>
        </div>

        {/* PDF Section */}
        <div className="bg-white rounded-lg shadow-sm">
          <div className="px-4 py-4 border-b border-gray-100">
            <div className="text-base text-gray-900 font-medium">PDF Document</div>
            <div className="text-xs text-gray-500 mt-1">Path: `lx.downloadFile` (runtime managed)</div>
          </div>

          <div className="px-4 py-4 space-y-3">
            {/* PDF URL Input */}
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

            {/* Open PDF Button */}
            <button
              onClick={openPdf}
              disabled={isPdfDownloading}
              className={`w-full py-3 rounded-lg text-white font-medium ${
                isPdfDownloading
                  ? 'bg-gray-400 cursor-not-allowed'
                  : 'bg-blue-500 hover:bg-blue-600 active:bg-blue-700'
              }`}
            >
              {isPdfDownloading ? 'Downloading...' : 'Open PDF'}
            </button>

            {isPdfDownloading && (
              <div className="space-y-1">
                <div className="flex gap-2">
                  {!pdfDownloadPaused ? (
                    <button
                      onClick={pausePdfDownload}
                      className="flex-1 rounded-md bg-amber-500 px-3 py-2 text-sm font-medium text-white"
                    >
                      Pause
                    </button>
                  ) : (
                    <button
                      onClick={resumePdfDownload}
                      className="flex-1 rounded-md bg-emerald-600 px-3 py-2 text-sm font-medium text-white"
                    >
                      Resume
                    </button>
                  )}
                  <button
                    onClick={cancelPdfDownload}
                    className="flex-1 rounded-md bg-red-600 px-3 py-2 text-sm font-medium text-white"
                  >
                    Cancel
                  </button>
                </div>
                <div className="h-2 w-full overflow-hidden rounded bg-gray-200">
                  <div
                    className="h-full bg-blue-500 transition-all duration-200"
                    style={{ width: `${Math.max(0, Math.min(100, pdfDownloadProgress))}%` }}
                  />
                </div>
                <div className="text-right text-xs text-gray-500">
                  {Math.max(0, Math.min(100, Math.floor(pdfDownloadProgress)))}%
                </div>
              </div>
            )}
          </div>
        </div>

        {/* Office Document Section */}
        <div className="bg-white rounded-lg shadow-sm">
          <div className="px-4 py-4 border-b border-gray-100">
            <div className="text-base text-gray-900 font-medium">Office Document</div>
            <div className="text-xs text-gray-500 mt-1">Supports: doc, docx, xls, xlsx, ppt, pptx</div>
            <div className="text-xs text-gray-500">Path: `fetch` stream to local file (manual flow)</div>
          </div>

          <div className="px-4 py-4 space-y-3">
            {/* Office URL Input */}
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

            {/* File Type Input */}
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

            {/* Open Office Button */}
            <button
              onClick={openOffice}
              disabled={isOfficeDownloading}
              className={`w-full py-3 rounded-lg text-white font-medium ${
                isOfficeDownloading
                  ? 'bg-gray-400 cursor-not-allowed'
                  : 'bg-blue-500 hover:bg-blue-600 active:bg-blue-700'
              }`}
            >
              {isOfficeDownloading ? 'Downloading...' : officeCached ? 'Open Cached Document' : 'Open Document'}
            </button>

            {isOfficeDownloading && (
              <div className="space-y-1">
                <button
                  onClick={cancelOfficeDownload}
                  className="w-full rounded-md bg-red-600 px-3 py-2 text-sm font-medium text-white"
                >
                  Cancel
                </button>
                <div className="h-2 w-full overflow-hidden rounded bg-gray-200">
                  <div
                    className="h-full bg-blue-500 transition-all duration-200"
                    style={{ width: `${Math.max(0, Math.min(100, officeDownloadProgress))}%` }}
                  />
                </div>
                <div className="text-right text-xs text-gray-500">
                  {Math.max(0, Math.min(100, Math.floor(officeDownloadProgress)))}%
                </div>
              </div>
            )}
          </div>
        </div>

      </div>
    </div>
  );
}
