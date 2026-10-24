import { useLocal, useLeaveGuard, type View } from '@lingxia/ui';
import '../../theme/index.css';

export function render({ env }: View) {
  const saved = useLocal('saved', '');
  const draft = useLocal('draft', '');
  const dirty = draft.value !== saved.value;
  const guard = useLeaveGuard('draft', dirty);
  const height = Math.max(1, env.viewport.height - env.insets.top - Math.max(env.insets.bottom, env.keyboard.height));
  return <column className="bg-surface-100 text-gray-900">
    <spacer style={{ height: env.insets.top }} />
    <row style={{ height }}><spacer style={{ width: env.insets.left }} />
      <scroll testId="page-scroll" className="grow basis-0 min-w-0" style={{ height }}>
        <column className="px-3 pt-3 pb-12 gap-3">
          <column className="bg-surface rounded-lg shadow-sm">
            <column className="px-4 py-4 border-b border-line-100 gap-1">
              <text className="text-base font-medium">Leave guard</text>
              <text className="text-xs text-gray-500">Edit the draft, then go back or home: the page stays and asks. Save, and it leaves at once.</text>
            </column>
            <column className="px-4 py-3 gap-3">
              <input testId="leave-draft" bind={draft} placeholder="Type something" className="rounded border border-line-200 px-3 py-2 text-sm bg-surface text-gray-900" />
              <row className="items-center justify-between gap-2 text-xs text-gray-500">
                <text testId="leave-status">{dirty ? 'Unsaved changes' : 'Saved'}</text>
                <text testId="leave-requests">{`leave requests: ${guard.requests}`}</text>
              </row>
              <button testId="leave-save" enabled={dirty} onClick={() => saved.set(draft.value)} className={dirty ? 'rounded bg-blue-500 py-2 text-sm text-white' : 'rounded bg-blue-500 py-2 text-sm text-white opacity-50'}>Save</button>
            </column>
          </column>
          {guard.pending ? <column testId="leave-confirm" className="bg-surface rounded-lg shadow-sm px-4 py-4 gap-3">
            <text testId="leave-question" className="text-sm font-medium">Discard unsaved changes?</text>
            <row className="gap-2">
              <button testId="leave-keep" onClick={() => guard.answer(false)} className="grow basis-0 w-0 rounded bg-surface border border-line-200 py-2 text-sm text-gray-700">Keep editing</button>
              <button testId="leave-discard" onClick={() => guard.answer(true)} className="grow basis-0 w-0 rounded bg-red-500 py-2 text-sm text-white">Discard</button>
            </row>
          </column> : null}
          {guard.error ? <text testId="leave-error" className="text-sm text-red-500">{guard.error}</text> : null}
        </column>
      </scroll><spacer style={{ width: env.insets.right }} />
    </row><spacer style={{ height: Math.max(env.insets.bottom, env.keyboard.height) }} />
  </column>;
}
