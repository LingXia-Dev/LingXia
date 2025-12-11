import React from 'react';
import '../../tailwind.css';

type PageData = {
  greeting?: string;
  imageUrl?: string;
  ipAddr?: string;
};

type PageActions = {
  data: PageData;
  greet(payload: { name: string }): void;
};

declare function useLingXia(): PageActions;

export default function HomePage() {
  const { data, greet } = useLingXia();
  const [name, setName] = React.useState('');
  const [isSending, setIsSending] = React.useState(false);

  const greetingMessage = typeof data?.greeting === 'string' ? data.greeting : '';
  const ipAddress = typeof data?.ipAddr === 'string' ? data.ipAddr : '';
  const imageUrl = typeof data?.imageUrl === 'string' ? data.imageUrl : '';

  React.useEffect(() => {
    if (isSending && greetingMessage) {
      setIsSending(false);
    }
  }, [greetingMessage, isSending]);

  const handleGreet = React.useCallback(() => {
    const trimmed = name.trim();
    if (!trimmed) return;
    setIsSending(true);
    greet({ name: trimmed });
  }, [name, greet]);

  const handleKeyDown = (event: React.KeyboardEvent<HTMLInputElement>) => {
    if (event.key === 'Enter') {
      event.preventDefault();
      handleGreet();
    }
  };

  return (
    <div className="relative min-h-screen w-full">
      {/* Background Image - Full Screen */}
      {imageUrl && (
        <img
          src={imageUrl}
          alt=""
          className="absolute inset-0 w-full h-full object-cover"
        />
      )}

      {/* Gradient Overlay */}
      <div className="absolute inset-0 bg-gradient-to-b from-black/10 via-transparent to-black/40" />

      {/* Content Container - Centered */}
      <div className="relative z-10 min-h-screen flex flex-col justify-center items-center px-5 py-16">
        {/* Main Card - Apple Style Frosted Glass */}
        <div className="bg-white/80 backdrop-blur-xl rounded-2xl shadow-lg border border-white/20 p-6">
          <div className="text-center mb-6">
            <div className="w-14 h-14 mx-auto mb-3 bg-gradient-to-br from-blue-500 to-indigo-600 rounded-[16px] flex items-center justify-center shadow-lg">
              <svg viewBox="0 0 24 24" fill="white" className="w-7 h-7">
                <path d="M13 2L3 14h9l-1 8 10-12h-9l1-8z" />
              </svg>
            </div>
            <div className="text-[17px] font-semibold text-gray-900">LingXia</div>
            <div className="text-[13px] text-gray-500 mt-0.5">Lightweight Application Framework</div>
          </div>

          <div className="space-y-3">
            <input
              type="text"
              placeholder="Enter your name"
              value={name}
              onChange={e => setName(e.target.value)}
              onKeyDown={handleKeyDown}
              className="w-full h-[44px] px-4 bg-gray-100/80 border-0 rounded-[10px] text-[17px] text-gray-900 placeholder-gray-400 focus:outline-none focus:ring-2 focus:ring-blue-500/30 transition-all"
            />

            <button
              type="button"
              onClick={handleGreet}
              disabled={!name.trim() || isSending}
              className="w-full h-[50px] bg-[#007AFF] hover:bg-[#0066CC] active:bg-[#0055B3] disabled:bg-[#007AFF]/50 disabled:cursor-not-allowed rounded-[12px] text-[17px] text-white font-semibold transition-colors"
            >
              {isSending ? 'Sending...' : 'Say Hello'}
            </button>
          </div>

          {/* Result Message */}
          {greetingMessage && (
            <div className="mt-4 p-4 bg-green-50 border border-green-200 rounded-xl">
              <div className="flex items-start gap-3">
                <div className="w-5 h-5 text-green-500 flex-shrink-0 mt-0.5">
                  <svg viewBox="0 0 24 24" fill="currentColor">
                    <path d="M12 2C6.48 2 2 6.48 2 12s4.48 10 10 10 10-4.48 10-10S17.52 2 12 2zm-2 15l-5-5 1.41-1.41L10 14.17l7.59-7.59L19 8l-9 9z" />
                  </svg>
                </div>
                <p className="text-sm text-green-700 leading-relaxed">
                  {greetingMessage}
                </p>
              </div>
            </div>
          )}
        </div>

        {/* IP Address Badge - Below Card */}
        {ipAddress && (
          <div className="mt-4 flex justify-center">
            <span className="inline-flex items-center gap-2 px-3 py-1.5 bg-black/20 backdrop-blur-md rounded-full text-xs text-white/90 font-mono">
              <span className="w-1.5 h-1.5 bg-green-400 rounded-full animate-pulse" />
              {ipAddress}
            </span>
          </div>
        )}
      </div>
    </div>
  );
}
