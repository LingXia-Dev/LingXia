import { useLocal, type View } from '@lingxia/ui';
import '../../theme/index.css';

export function render({ data, actions, env }: View) {
  const draft = useLocal('clipboard-draft', 'Hello from lx.clipboard');
  const busy = useLocal('clipboard-busy', false);
  const error = useLocal('clipboard-error', '');

  const run = async (action: () => Promise<unknown>) => {
    if (busy.value) return;
    busy.set(true); error.set('');
    try { await action(); } catch { error.set('The action failed. Please try again.'); }
    finally { busy.set(false); }
  };
  const height = Math.max(1, env.viewport.height - env.insets.top - Math.max(env.insets.bottom, env.keyboard.height));
  const card = 'bg-surface rounded-lg shadow-sm';
  const heading = 'px-4 py-4 border-b border-line-100 gap-1';
  const primary = 'min-h-11 py-3 px-2 rounded-lg bg-blue-500 text-white font-medium text-base';
  const secondary = 'min-h-11 py-3 px-2 rounded-lg bg-surface-900 text-white font-medium text-base';
  const neutral = 'min-h-11 py-3 px-2 rounded-lg bg-surface-200 text-gray-900 font-medium text-base';
  return <column className="bg-surface-100 text-gray-900">
    <spacer style={{ height: env.insets.top }} />
    <row style={{ height }}>
      <spacer style={{ width: env.insets.left }} />
      <scroll testId="page-scroll" className="grow basis-0 min-w-0" style={{ height }}>
        <column className="px-3 pt-3 pb-12 gap-3">
          <column className={card}>
            <column className={heading}>
              <text className="text-base font-medium">Clipboard</text>
              <text className="text-xs text-gray-500">lx.clipboard reads and writes the system clipboard. Write never toasts.</text>
            </column>
            <text testId="clipboard-status" className="px-4 py-3 text-sm text-gray-700">{error.value || (busy.value ? 'Working...' : String(data.statusText || 'Ready'))}</text>
            <text testId="clipboard-types-value" className="px-4 pb-3 text-xs text-gray-500">{`types: ${String(data.typesText || 'Not peeked')}`}</text>
          </column>
          <column className={card}>
            <column className={heading}>
              <text className="text-sm font-medium">Text</text>
              <text className="text-xs text-gray-500">writeText / readText. An empty string is a valid write, not clear().</text>
            </column>
            <column className="p-4 gap-3">
              <textarea testId="clipboard-draft" bind={draft} placeholder="Text to copy" className="w-full h-[78px] px-3 py-2 border border-line-300 rounded-md text-sm bg-surface text-gray-900" />
              <grid columns={2} className="gap-3">
                <button testId="clipboard-write-text" enabled={!busy.value} onClick={() => run(() => actions.writeText({ text: draft.value }))} className={primary}>Write text</button>
                <button testId="clipboard-read-text" enabled={!busy.value} onClick={() => run(() => actions.readText())} className={secondary}>Read text</button>
              </grid>
              <button testId="clipboard-write-typed" enabled={!busy.value} onClick={() => run(() => actions.writeTypedText({ text: draft.value }))} className={neutral}>Write typed item</button>
              {data.readTextValue ? <text testId="clipboard-read-value" className="rounded bg-surface-50 px-3 py-2 text-xs text-gray-500 break-all">{String(data.readTextValue)}</text> : null}
            </column>
          </column>
          <column className={card}>
            <column className={heading}>
              <text className="text-sm font-medium">Image</text>
              <text className="text-xs text-gray-500">Write a 1×1 sample PNG, or pick one with lx.chooseMedia.</text>
            </column>
            <column className="p-4 gap-3">
              <grid columns={2} className="gap-3">
                <button testId="clipboard-write-sample-image" enabled={!busy.value} onClick={() => run(() => actions.writeSampleImage())} className={primary}>Write sample PNG</button>
                <button testId="clipboard-choose-image" enabled={!busy.value} onClick={() => run(() => actions.chooseAndWriteImage())} className={secondary}>Choose image</button>
              </grid>
              {data.imagePath ? <column className="gap-2">
                <row className="justify-center">
                  <image testId="clipboard-image" src={String(data.imagePath)} alt="Image from clipboard" fit="contain" className="w-40 h-40 rounded border border-line-200" />
                </row>
                <text className="text-xs text-gray-500 break-all">{String(data.imagePath)}</text>
              </column> : null}
            </column>
          </column>
          <column className={card}>
            <column className={heading}>
              <text className="text-sm font-medium">Inspect</text>
              <text className="text-xs text-gray-500">Peek types without the payload, read every representation, or clear.</text>
            </column>
            <grid columns={3} className="p-4 gap-3">
              <button testId="clipboard-types" enabled={!busy.value} onClick={() => run(() => actions.peekTypes())} className={neutral}>Types</button>
              <button testId="clipboard-read-all" enabled={!busy.value} onClick={() => run(() => actions.readAll())} className={primary}>Read</button>
              <button testId="clipboard-clear" enabled={!busy.value} onClick={() => run(() => actions.clearClipboard())} className={secondary}>Clear</button>
            </grid>
          </column>
        </column>
      </scroll>
      <spacer style={{ width: env.insets.right }} />
    </row>
    <spacer style={{ height: Math.max(env.insets.bottom, env.keyboard.height) }} />
  </column>;
}
