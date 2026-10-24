import { useLocal, type Props, type View } from '@lingxia/ui';
import '../../theme/index.css';

function Card({ title, description, path, children }: Pick<Props, 'children'> & { title: string; description: string; path?: string }) {
  return <column className="bg-surface rounded-lg shadow-sm">
    <column className="px-4 py-4 border-b border-line-100 gap-1">
      <text className="text-sm font-medium text-gray-900">{title}</text>
      <text className="text-xs text-gray-500">{description}</text>
      {path !== undefined && <column className="pt-2"><text className="rounded bg-surface-50 px-3 py-2 text-xs text-gray-500 break-all">{path || 'No selection'}</text></column>}
    </column>
    {children}
  </column>;
}

export function render({ data, actions, env }: View) {
  const busy = useLocal('share-busy', false);
  const error = useLocal('share-error', '');
  const run = async (action: () => Promise<unknown>) => {
    if (busy.value) return;
    busy.set(true); error.set('');
    try { await action(); } catch { error.set('Unable to open sharing. Please try again.'); }
    finally { busy.set(false); }
  };
  const height = Math.max(1, env.viewport.height - env.insets.top - Math.max(env.insets.bottom, env.keyboard.height));
  const primary = 'min-h-11 px-2 py-3 rounded-lg bg-blue-500 text-white font-medium text-base';
  const secondary = 'min-h-11 px-2 py-3 rounded-lg bg-surface-900 text-white font-medium text-base';
  return <column className="bg-surface-100 text-gray-900">
    <spacer style={{ height: env.insets.top }} />
    <row style={{ height }}>
      <spacer style={{ width: env.insets.left }} />
      <scroll testId="page-scroll" className="grow basis-0 min-w-0" style={{ height }}>
        <column className="px-3 pt-3 pb-12 gap-3">
          <column className="bg-surface rounded-lg shadow-sm">
            <column className="px-4 py-4 border-b border-line-100 gap-1">
              <text className="text-base font-medium">Share</text>
              <text className="text-xs text-gray-500">lx.share opens the native system share sheet.</text>
            </column>
            <text testId="share-status" className="px-4 py-3 text-sm text-gray-700">{error.value || String(data.statusText || 'Ready')}</text>
          </column>
          <Card title="Text" description="Some receivers reject text-only shares.">
            <column className="p-4"><button testId="share-text" enabled={!busy.value} onClick={() => run(() => actions.shareText())} className={primary}>Share Text</button></column>
          </Card>
          <Card title="Page" description="Share the current page as an AppLink URL.">
            <column className="p-4"><button testId="share-page" enabled={!busy.value} onClick={() => run(() => actions.shareCurrentPage())} className={primary}>Share Current Page</button></column>
          </Card>
          <Card title="Image" description="Choose or capture an image, then share it as an attachment." path={String(data.selectedImagePath || '')}>
            <grid columns={2} className="p-4 gap-3">
              <button testId="share-choose-image" enabled={!busy.value} onClick={() => run(() => actions.chooseImage())} className={secondary}>Choose Image</button>
              <button testId="share-image" enabled={!busy.value} onClick={() => run(() => actions.shareSelectedImage())} className={primary}>Share Image</button>
            </grid>
          </Card>
          <Card title="File" description="Pick an image, PDF, or other document before sharing it." path={String(data.selectedFilePath || '')}>
            <grid columns={2} className="p-4 gap-3">
              <button testId="share-choose-file" enabled={!busy.value} onClick={() => run(() => actions.chooseFile())} className={secondary}>Choose File</button>
              <button testId="share-file" enabled={!busy.value} onClick={() => run(() => actions.shareSelectedFile())} className={primary}>Share File</button>
            </grid>
          </Card>
        </column>
      </scroll>
      <spacer style={{ width: env.insets.right }} />
    </row>
    <spacer style={{ height: Math.max(env.insets.bottom, env.keyboard.height) }} />
  </column>;
}
