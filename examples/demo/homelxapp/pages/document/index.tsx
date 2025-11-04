import React from 'react';
import '../../tailwind.css';

type PageData = {
  pdfUrl?: string;
  officeUrl?: string;
  officeFileType?: string;
  showMenu?: boolean;
  isDownloading?: boolean;
};

type PageActions = {
  data: PageData;
  onPdfUrlInput(event: any): void;
  onOfficeUrlInput(event: any): void;
  onOfficeFileTypeInput(event: any): void;
  toggleShowMenu(): void;
  openPdf(): void;
  openOffice(): void;
};

declare function useLingXia(): PageActions;

export default function DocumentPage() {
  const {
    data,
    onPdfUrlInput,
    onOfficeUrlInput,
    onOfficeFileTypeInput,
    toggleShowMenu,
    openPdf,
    openOffice,
  } = useLingXia();

  const pdfUrl = data?.pdfUrl || '';
  const officeUrl = data?.officeUrl || '';
  const officeFileType = data?.officeFileType || '';
  const showMenu = Boolean(data?.showMenu);
  const isDownloading = Boolean(data?.isDownloading);

  return (
    <div className="min-h-screen bg-gray-100">
      <div className="px-3 py-2 pb-12 space-y-3">

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
                onChange={toggleShowMenu}
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
          </div>

          <div className="px-4 py-4 space-y-3">
            {/* PDF URL Input */}
            <div>
              <div className="text-sm text-gray-600 mb-2">PDF URL:</div>
              <input
                type="text"
                value={pdfUrl}
                onChange={onPdfUrlInput}
                placeholder="Enter PDF URL"
                className="w-full px-3 py-2 border border-gray-300 rounded-lg text-sm focus:outline-none focus:ring-2 focus:ring-blue-500"
              />
            </div>

            {/* Open PDF Button */}
            <button
              onClick={openPdf}
              disabled={isDownloading}
              className={`w-full py-3 rounded-lg text-white font-medium ${
                isDownloading
                  ? 'bg-gray-400 cursor-not-allowed'
                  : 'bg-blue-500 hover:bg-blue-600 active:bg-blue-700'
              }`}
            >
              {isDownloading ? 'Downloading...' : 'Open PDF'}
            </button>
          </div>
        </div>

        {/* Office Document Section */}
        <div className="bg-white rounded-lg shadow-sm">
          <div className="px-4 py-4 border-b border-gray-100">
            <div className="text-base text-gray-900 font-medium">Office Document</div>
            <div className="text-xs text-gray-500 mt-1">Supports: doc, docx, xls, xlsx, ppt, pptx</div>
          </div>

          <div className="px-4 py-4 space-y-3">
            {/* Office URL Input */}
            <div>
              <div className="text-sm text-gray-600 mb-2">Document URL:</div>
              <input
                type="text"
                value={officeUrl}
                onChange={onOfficeUrlInput}
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
                onChange={onOfficeFileTypeInput}
                placeholder="e.g., docx, xlsx, pptx"
                className="w-full px-3 py-2 border border-gray-300 rounded-lg text-sm focus:outline-none focus:ring-2 focus:ring-blue-500"
              />
              <div className="text-xs text-gray-500 mt-1">Auto-detected from URL or enter manually</div>
            </div>

            {/* Open Office Button */}
            <button
              onClick={openOffice}
              disabled={isDownloading}
              className={`w-full py-3 rounded-lg text-white font-medium ${
                isDownloading
                  ? 'bg-gray-400 cursor-not-allowed'
                  : 'bg-blue-500 hover:bg-blue-600 active:bg-blue-700'
              }`}
            >
              {isDownloading ? 'Downloading...' : 'Open Document'}
            </button>
          </div>
        </div>

      </div>
    </div>
  );
}

