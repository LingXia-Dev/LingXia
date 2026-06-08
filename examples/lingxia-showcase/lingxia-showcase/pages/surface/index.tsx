import React from 'react';
import { useLxPage } from '@lingxia/react';
import '../../tailwind.css';

export default function SurfacePage() {
  const { data, actions } = useLxPage();
  const { logSurfaceMessage, hideSelf, closeSelf } = actions;
  const queryString = data.queryString ?? '';
  const showCount = data.showCount ?? 0;
  const hideCount = data.hideCount ?? 0;
  const lastLifecycle = data.lastLifecycle ?? 'onLoad';
  const [message, setMessage] = React.useState('');
  // The counter survives hide() → show() round-trips because the page mount
  // is preserved. After close() the page is destroyed and the counter resets,
  // which is the visible difference between hide and close.
  const [counter, setCounter] = React.useState(0);

  const handleSend = React.useCallback(() => {
    const text = message.trim();
    if (!text) {
      return;
    }

    try {
      logSurfaceMessage({ message: text });
      setMessage('');
      closeSelf?.();
    } catch (error) {
      console.error('logSurfaceMessage failed:', error);
    }
  }, [message, logSurfaceMessage, closeSelf]);

  return (
    <div className="min-h-screen bg-gray-100 text-gray-900 flex flex-col items-center px-4 py-6">
      <div className="w-full max-w-md space-y-6">
        <header>
          <h1 className="text-lg font-semibold tracking-wide text-gray-900">Surface Page</h1>
          <p className="text-sm text-gray-500 mt-1">
            Inspect the query string and send a message to the opener.
          </p>
        </header>

        <section className="bg-white rounded-xl border border-gray-200 p-4 space-y-2 shadow-sm">
          <div className="text-xs uppercase text-gray-500 tracking-wide">Query String</div>
          <div className="font-mono text-sm text-gray-800 break-words">
            {queryString || '(none)'}
          </div>
        </section>

        <section className="bg-white rounded-xl border border-gray-200 p-4 space-y-3 shadow-sm">
          <div className="text-xs uppercase text-gray-500 tracking-wide">Page lifecycle</div>
          <div className="grid grid-cols-2 gap-2 text-sm">
            <div className="bg-gray-50 rounded-md px-3 py-2">
              <div className="text-xs text-gray-500">onShow</div>
              <div className="font-mono text-base text-gray-900">{showCount}</div>
            </div>
            <div className="bg-gray-50 rounded-md px-3 py-2">
              <div className="text-xs text-gray-500">onHide</div>
              <div className="font-mono text-base text-gray-900">{hideCount}</div>
            </div>
          </div>
          <div className="text-xs text-gray-500">
            Last event: <span className="font-mono text-gray-800">{lastLifecycle}</span>
          </div>
        </section>

        <section className="bg-white rounded-xl border border-gray-200 p-4 space-y-3 shadow-sm">
          <div className="text-xs uppercase text-gray-500 tracking-wide">In-page counter</div>
          <div className="font-mono text-2xl text-gray-900">{counter}</div>
          <div className="text-xs text-gray-500">
            Hide preserves this counter; close resets it on re-open.
          </div>
          <button
            type="button"
            onClick={() => setCounter((value) => value + 1)}
            className="w-full h-10 text-sm font-medium rounded-md bg-gray-200 hover:bg-gray-300 text-gray-900 transition-colors"
          >
            Increment
          </button>
        </section>

        <section className="bg-white rounded-xl border border-gray-200 p-4 space-y-3 shadow-sm">
          <div className="text-xs uppercase text-gray-500 tracking-wide">Message</div>
          <input
            className="w-full px-3 py-2 rounded-md bg-white border border-gray-300 text-sm text-gray-900 focus:outline-none focus:ring-2 focus:ring-blue-500"
            placeholder="Message to parent page"
            value={message}
            onChange={(event) => setMessage(event.target.value)}
          />
          <button
            type="button"
            onClick={handleSend}
            className="w-full h-10 text-sm font-medium rounded-md bg-blue-500 hover:bg-blue-600 text-white transition-colors"
          >
            Send then close
          </button>
        </section>

        <section className="bg-white rounded-xl border border-gray-200 p-4 space-y-3 shadow-sm">
          <div className="text-xs uppercase text-gray-500 tracking-wide">Self actions</div>
          <button
            type="button"
            onClick={() => hideSelf?.()}
            className="w-full h-10 text-sm font-medium rounded-md bg-amber-500 hover:bg-amber-600 text-white transition-colors"
          >
            Hide (parent can show again)
          </button>
          <button
            type="button"
            onClick={() => closeSelf?.()}
            className="w-full h-10 text-sm font-medium rounded-md bg-rose-500 hover:bg-rose-600 text-white transition-colors"
          >
            Close (destroys this page)
          </button>
        </section>
      </div>
    </div>
  );
}
