import { useLxPage } from '@lingxia/react';
import '../../tailwind.css';

type PageData = {
  statusText?: string;
  selectedImagePath?: string;
  selectedFilePath?: string;
};

type PageActions = {
  shareText(): void;
  shareCurrentPage(): void;
  chooseImage(): void;
  shareSelectedImage(): void;
  chooseFile(): void;
  shareSelectedFile(): void;
};

function PathText({ value }: { value?: string }) {
  return (
    <div className="mt-2 rounded bg-gray-50 px-3 py-2 text-xs text-gray-500 break-all">
      {value || 'No selection'}
    </div>
  );
}

export default function SharePage() {
  const { data, actions } = useLxPage<PageData, PageActions>();
  const {
    shareText,
    shareCurrentPage,
    chooseImage,
    shareSelectedImage,
    chooseFile,
    shareSelectedFile,
  } = actions;

  return (
    <div className="min-h-screen bg-gray-100 overflow-y-auto">
      <div className="px-3 py-3 pb-12 space-y-3">
        <div className="bg-white rounded-lg shadow-sm">
          <div className="px-4 py-4 border-b border-gray-100">
            <div className="text-base text-gray-900 font-medium">Share</div>
            <div className="text-xs text-gray-500 mt-1">lx.share opens the native system share sheet.</div>
          </div>
          <div className="px-4 py-3 text-sm text-gray-700">{data?.statusText || 'Ready'}</div>
        </div>

        <div className="bg-white rounded-lg shadow-sm">
          <div className="px-4 py-4 border-b border-gray-100">
            <div className="text-sm text-gray-900 font-medium">Text</div>
            <div className="text-xs text-gray-500 mt-1">Some receivers reject text-only shares.</div>
          </div>
          <div className="px-4 py-4">
            <button onClick={shareText} className="w-full py-3 rounded-lg bg-blue-500 text-white font-medium">
              Share Text
            </button>
          </div>
        </div>

        <div className="bg-white rounded-lg shadow-sm">
          <div className="px-4 py-4 border-b border-gray-100">
            <div className="text-sm text-gray-900 font-medium">Page</div>
            <div className="text-xs text-gray-500 mt-1">Share the current page as an AppLink URL.</div>
          </div>
          <div className="px-4 py-4">
            <button onClick={shareCurrentPage} className="w-full py-3 rounded-lg bg-blue-500 text-white font-medium">
              Share Current Page
            </button>
          </div>
        </div>

        <div className="bg-white rounded-lg shadow-sm">
          <div className="px-4 py-4 border-b border-gray-100">
            <div className="text-sm text-gray-900 font-medium">Image</div>
            <div className="text-xs text-gray-500 mt-1">Choose or capture an image, then share it as an attachment.</div>
            <PathText value={data?.selectedImagePath} />
          </div>
          <div className="px-4 py-4 grid grid-cols-2 gap-3">
            <button onClick={chooseImage} className="py-3 rounded-lg bg-gray-900 text-white font-medium">
              Choose Image
            </button>
            <button onClick={shareSelectedImage} className="py-3 rounded-lg bg-blue-500 text-white font-medium">
              Share Image
            </button>
          </div>
        </div>

        <div className="bg-white rounded-lg shadow-sm">
          <div className="px-4 py-4 border-b border-gray-100">
            <div className="text-sm text-gray-900 font-medium">File</div>
            <div className="text-xs text-gray-500 mt-1">Pick an image, PDF, or other document before sharing it.</div>
            <PathText value={data?.selectedFilePath} />
          </div>
          <div className="px-4 py-4 grid grid-cols-2 gap-3">
            <button onClick={chooseFile} className="py-3 rounded-lg bg-gray-900 text-white font-medium">
              Choose File
            </button>
            <button onClick={shareSelectedFile} className="py-3 rounded-lg bg-blue-500 text-white font-medium">
              Share File
            </button>
          </div>
        </div>
      </div>
    </div>
  );
}
