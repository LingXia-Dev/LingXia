import React from 'react';
import { useLingXia } from '@lingxia/web-runtime/react';
import '../../tailwind.css';

export default function PopupPage() {
  const { data, sendPopupMessage } = useLingXia();
  const queryString = data.queryString ?? '';
  const [message, setMessage] = React.useState('');

  const handleSend = React.useCallback(() => {
    const text = message.trim();
    if (!text) {
      return;
    }

    try {
      sendPopupMessage({ message: text });
      setMessage('');
    } catch (error) {
      console.error('sendPopupMessage failed:', error);
    }
  }, [message, sendPopupMessage]);

  return (
    <div className="min-h-screen bg-gray-100 text-gray-900 flex flex-col items-center px-4 py-6">
      <div className="w-full max-w-md space-y-6">
        <header>
          <h1 className="text-lg font-semibold tracking-wide text-gray-900">Popup Overlay</h1>
          <p className="text-sm text-gray-500 mt-1">
            Inspect the query string and send a message back to the opener.
          </p>
        </header>

        <section className="bg-white rounded-xl border border-gray-200 p-4 space-y-2 shadow-sm">
          <div className="text-xs uppercase text-gray-500 tracking-wide">Query String</div>
          <div className="font-mono text-sm text-gray-800 break-words">
            {queryString || '(none)'}
          </div>
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
            Send message
          </button>
        </section>
      </div>
    </div>
  );
}
