import { useState } from 'react';
import { useLxPage } from '@lingxia/react';
import '../../tailwind.css';

type PageData = {
  statusText?: string;
  typesText?: string;
  readTextValue?: string;
  imagePath?: string;
};

type PageActions = {
  writeText(params: { text: string }): void;
  readText(): void;
  writeTypedText(params: { text: string }): void;
  writeSampleImage(): void;
  chooseAndWriteImage(): void;
  readAll(): void;
  peekTypes(): void;
  clearClipboard(): void;
};

export default function ClipboardPage() {
  const { data, actions } = useLxPage<PageData, PageActions>();
  const {
    writeText,
    readText,
    writeTypedText,
    writeSampleImage,
    chooseAndWriteImage,
    readAll,
    peekTypes,
    clearClipboard,
  } = actions;
  const [draft, setDraft] = useState('Hello from lx.clipboard');

  return (
    <div className="min-h-screen bg-surface-100 overflow-y-auto">
      <div className="px-3 py-3 pb-12 space-y-3">
        <div className="bg-surface rounded-lg shadow-sm">
          <div className="px-4 py-4 border-b border-line-100">
            <div className="text-base text-gray-900 font-medium">Clipboard</div>
            <div className="text-xs text-gray-500 mt-1">
              lx.clipboard reads and writes the system clipboard. Write never toasts.
            </div>
          </div>
          <div className="px-4 py-3 text-sm text-gray-700" data-testid="clipboard-status">
            {data?.statusText || 'Ready'}
          </div>
          <div className="px-4 pb-3 text-xs text-gray-500" data-testid="clipboard-types">
            types: {data?.typesText || 'Not peeked'}
          </div>
        </div>

        <div className="bg-surface rounded-lg shadow-sm">
          <div className="px-4 py-4 border-b border-line-100">
            <div className="text-sm text-gray-900 font-medium">Text</div>
            <div className="text-xs text-gray-500 mt-1">
              writeText / readText. An empty string is a valid write, not clear().
            </div>
          </div>
          <div className="px-4 py-4 space-y-3">
            <textarea
              data-testid="clipboard-draft"
              value={draft}
              onChange={(event) => setDraft(event.target.value)}
              rows={3}
              className="w-full px-3 py-2 border border-line-300 rounded-md text-sm"
            />
            <div className="grid grid-cols-2 gap-3">
              <button
                data-testid="clipboard-write-text"
                onClick={() => writeText({ text: draft })}
                className="py-3 rounded-lg bg-blue-500 text-white font-medium"
              >
                Write text
              </button>
              <button
                data-testid="clipboard-read-text"
                onClick={readText}
                className="py-3 rounded-lg bg-surface-900 text-white font-medium"
              >
                Read text
              </button>
            </div>
            <button
              data-testid="clipboard-write-typed"
              onClick={() => writeTypedText({ text: draft })}
              className="w-full py-3 rounded-lg bg-surface-200 text-gray-900 font-medium"
            >
              Write typed item
            </button>
            {data?.readTextValue !== undefined && data.readTextValue !== '' && (
              <div className="rounded bg-surface-50 px-3 py-2 text-xs text-gray-500 break-all">
                {data.readTextValue}
              </div>
            )}
          </div>
        </div>

        <div className="bg-surface rounded-lg shadow-sm">
          <div className="px-4 py-4 border-b border-line-100">
            <div className="text-sm text-gray-900 font-medium">Image</div>
            <div className="text-xs text-gray-500 mt-1">
              Write a 1×1 sample PNG, or pick one with lx.chooseMedia.
            </div>
          </div>
          <div className="px-4 py-4 space-y-3">
            <div className="grid grid-cols-2 gap-3">
              <button
                data-testid="clipboard-write-sample-image"
                onClick={writeSampleImage}
                className="py-3 rounded-lg bg-blue-500 text-white font-medium"
              >
                Write sample PNG
              </button>
              <button
                onClick={chooseAndWriteImage}
                className="py-3 rounded-lg bg-surface-900 text-white font-medium"
              >
                Choose image
              </button>
            </div>
            {data?.imagePath ? (
              <div className="space-y-2">
                <img
                  src={data.imagePath}
                  alt="Clipboard image"
                  className="max-h-40 rounded border border-line-200"
                />
                <div className="text-xs text-gray-500 break-all">{data.imagePath}</div>
              </div>
            ) : null}
          </div>
        </div>

        <div className="bg-surface rounded-lg shadow-sm">
          <div className="px-4 py-4 border-b border-line-100">
            <div className="text-sm text-gray-900 font-medium">Inspect</div>
            <div className="text-xs text-gray-500 mt-1">
              Peek types without the payload, read every representation, or clear.
            </div>
          </div>
          <div className="px-4 py-4 grid grid-cols-3 gap-3">
            <button
              data-testid="clipboard-types"
              onClick={peekTypes}
              className="py-3 rounded-lg bg-surface-200 text-gray-900 font-medium"
            >
              Types
            </button>
            <button
              data-testid="clipboard-read"
              onClick={readAll}
              className="py-3 rounded-lg bg-blue-500 text-white font-medium"
            >
              Read
            </button>
            <button
              data-testid="clipboard-clear"
              onClick={clearClipboard}
              className="py-3 rounded-lg bg-surface-900 text-white font-medium"
            >
              Clear
            </button>
          </div>
        </div>
      </div>
    </div>
  );
}
