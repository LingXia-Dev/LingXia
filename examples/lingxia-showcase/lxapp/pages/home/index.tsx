import React from 'react';
import { useLxPage } from '@lingxia/react';
import '../../tailwind.css';

type AppearancePreference = 'auto' | 'light' | 'dark';

type PageData = {
  greeting?: string;
  imageUrl?: string;
  ipAddr?: string;
  appVersion?: string;
  appearance?: { preference: AppearancePreference; resolved: 'light' | 'dark' };
};

type PageActions = {
  greet(payload: { name: string }): void;
  setAppearance(payload: { preference: AppearancePreference }): void;
};

const APPEARANCE_OPTIONS: AppearancePreference[] = ['auto', 'light', 'dark'];

export default function HomePage() {
  const { data, actions } = useLxPage<PageData, PageActions>();
  const { greet, setAppearance } = actions;
  const [name, setName] = React.useState('');
  const [isSending, setIsSending] = React.useState(false);

  const greetingMessage = typeof data?.greeting === 'string' ? data.greeting : '';
  const ipAddress = typeof data?.ipAddr === 'string' ? data.ipAddr : '';
  const imageUrl = typeof data?.imageUrl === 'string' ? data.imageUrl : '';
  const appVersion = typeof data?.appVersion === 'string' ? data.appVersion : '';
  const preference = data?.appearance?.preference ?? 'auto';
  const resolvedAppearance = data?.appearance?.resolved ?? 'light';

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
    <div className="fixed inset-0 w-screen h-screen overflow-hidden" data-testid="home-page">
      {/* Background Image - Full Screen */}
      {imageUrl && (
        <img
          src={imageUrl}
          alt=""
          className="absolute inset-0 w-full h-full object-cover"
        />
      )}

      {/* Gradient Overlay */}
      <div className="absolute inset-0 bg-linear-to-b from-black/10 via-transparent to-black/40" />

      {/* Content Container - Centered */}
      <div className="relative z-10 w-full h-full flex flex-col justify-center items-center px-5 py-16">
        {/* Main Card - Apple Style Frosted Glass */}
        <div className="max-w-full bg-surface/80 backdrop-blur-xl rounded-2xl shadow-lg border border-white/20 p-6">
          <div className="text-center mb-6">
            <img src="/public/AppIcon.png" alt="Logo" className="w-16 h-16 mx-auto mb-3 rounded-[16px]" />
            <div className="text-[17px] font-semibold text-gray-900">LingXia</div>
            <div className="text-[13px] text-gray-500 mt-0.5">Lightweight Application Framework</div>
          </div>

          <div className="space-y-3">
            <input
              data-testid="home-name"
              data-controlled-value={name}
              type="text"
              placeholder="Enter your name"
              value={name}
              onChange={e => setName(e.target.value)}
              onKeyDown={handleKeyDown}
              className="w-full h-[44px] px-4 bg-surface-100/80 border-0 rounded-[10px] text-[17px] text-gray-900 placeholder:text-gray-400 focus:outline-hidden focus:ring-2 focus:ring-blue-500/30 transition-all"
            />

            <button
              data-testid="home-greet"
              type="button"
              onClick={handleGreet}
              disabled={!name.trim() || isSending}
              className="w-full h-[50px] bg-primary hover:bg-primary-dark active:bg-primary-dark disabled:bg-primary/50 disabled:cursor-not-allowed rounded-[12px] text-[17px] text-white font-semibold transition-colors"
            >
              {isSending ? 'Sending...' : 'Say Hello'}
            </button>
          </div>

          {/* Result Message */}
          {greetingMessage && (
            <div className="mt-4 p-4 bg-green-50 border border-green-200 rounded-xl">
              <div className="flex items-start gap-3">
                <div className="w-5 h-5 text-green-500 dark:text-green-400 shrink-0 mt-0.5">
                  <svg viewBox="0 0 24 24" fill="currentColor">
                    <path d="M12 2C6.48 2 2 6.48 2 12s4.48 10 10 10 10-4.48 10-10S17.52 2 12 2zm-2 15l-5-5 1.41-1.41L10 14.17l7.59-7.59L19 8l-9 9z" />
                  </svg>
                </div>
                <p className="min-w-0 whitespace-pre-wrap break-words text-sm text-green-700 dark:text-green-400 leading-relaxed" data-testid="home-greeting">
                  {greetingMessage}
                </p>
              </div>
            </div>
          )}

          {/* lxapp-owned light/dark branch — `auto` follows the host shell. */}
          <div className="mt-4" data-testid="home-appearance">
            <div className="flex items-center justify-between mb-1.5">
              <span className="text-[11px] font-medium text-gray-500">Appearance</span>
              <span className="text-[11px] text-gray-400" data-testid="home-appearance-resolved">
                {resolvedAppearance}
              </span>
            </div>
            <div className="flex gap-1 p-1 bg-surface-100 rounded-[10px]">
              {APPEARANCE_OPTIONS.map((option) => (
                <button
                  key={option}
                  type="button"
                  data-testid={`home-appearance-${option}`}
                  data-selected={preference === option}
                  onClick={() => setAppearance({ preference: option })}
                  className={`flex-1 h-8 rounded-[8px] text-[13px] font-medium capitalize transition-colors ${
                    preference === option
                      ? 'bg-surface text-gray-900 shadow-sm'
                      : 'text-gray-500'
                  }`}
                >
                  {option}
                </button>
              ))}
            </div>
          </div>

          {appVersion && (
            <div className="mt-1 text-left leading-none">
              <span className="text-[10px] text-gray-500 font-medium">{appVersion}</span>
            </div>
          )}
        </div>

        {/* IP Address Badge - Below Card */}
        {ipAddress && (
          <div className="mt-4 flex justify-center">
            <div className="inline-flex items-center gap-2 px-4 py-2 bg-black/20 backdrop-blur-md rounded-full text-white/90">
              <span className="w-1.5 h-1.5 bg-green-400 rounded-full animate-pulse" />
              <span className="text-xs font-medium tracking-wide">My IP </span>
              <span className="text-xs font-mono">{ipAddress}</span>
            </div>
          </div>
        )}
      </div>

    </div>
  );
}
