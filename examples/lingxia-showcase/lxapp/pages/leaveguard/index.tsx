import { useState } from 'react';
import { leavePage } from '@lingxia/bridge';
import { useLxLeaveGuard } from '@lingxia/react';
import '../../tailwind.css';

export default function LeaveGuardPage() {
  const [saved, setSaved] = useState('');
  const [draft, setDraft] = useState('');
  const [asking, setAsking] = useState(false);
  const [requests, setRequests] = useState(0);
  const dirty = draft !== saved;

  // While the draft differs from what was saved, back (navigation bar,
  // system back, edge swipe) stays here and asks instead of discarding.
  useLxLeaveGuard(dirty, () => {
    setRequests((count) => count + 1);
    setAsking(true);
  });

  return (
    <div className="min-h-screen bg-surface-100 overflow-y-auto">
      <div className="px-3 py-3 pb-12 space-y-3">
        <div className="bg-surface rounded-lg shadow-sm">
          <div className="px-4 py-4 border-b border-line-100">
            <div className="text-base text-gray-900 font-medium">Leave guard</div>
            <div className="text-xs text-gray-500 mt-1">
              Edit the draft, then go back: the page stays and asks. Save, and back leaves at once.
            </div>
          </div>
          <div className="px-4 py-3 space-y-3">
            <input
              data-testid="leave-draft"
              className="w-full rounded border border-line-200 px-3 py-2 text-sm"
              value={draft}
              placeholder="Type something"
              onChange={(event) => setDraft(event.target.value)}
            />
            <div className="flex items-center justify-between text-xs text-gray-500">
              <span data-testid="leave-status">{dirty ? 'Unsaved changes' : 'Saved'}</span>
              <span data-testid="leave-requests">back requests: {requests}</span>
            </div>
            <button
              data-testid="leave-save"
              className="w-full rounded bg-blue-500 py-2 text-sm text-white disabled:opacity-50"
              disabled={!dirty}
              onClick={() => setSaved(draft)}
            >
              Save
            </button>
          </div>
        </div>

        {asking ? (
          <div className="bg-surface rounded-lg shadow-sm px-4 py-4 space-y-3" data-testid="leave-confirm">
            <div className="text-sm text-gray-900 font-medium">Discard unsaved changes?</div>
            <div className="flex gap-2">
              <button
                data-testid="leave-keep"
                className="flex-1 rounded border border-line-200 py-2 text-sm text-gray-700"
                onClick={() => setAsking(false)}
              >
                Keep editing
              </button>
              <button
                data-testid="leave-discard"
                className="flex-1 rounded bg-red-500 py-2 text-sm text-white"
                onClick={() => {
                  setAsking(false);
                  void leavePage();
                }}
              >
                Discard
              </button>
            </div>
          </div>
        ) : null}
      </div>
    </div>
  );
}
