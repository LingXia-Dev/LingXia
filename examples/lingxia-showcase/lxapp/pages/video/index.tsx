import React from 'react';
import { useLxPage } from '@lingxia/react';
import {
  LxNativeButton,
  LxNativeCover,
  LxNativeRoot,
  LxNativeText,
  LxNativeView,
  LxVideo,
} from '@lingxia/react';
import '../../tailwind.css';

type VideoConfig = {
  id: string;
  src: string;
  poster?: string;
  autoplay?: boolean;
  qualities?: Array<{ label: string; url?: string }>;
  playbackRates?: number[];
};

type PageData = {
  videos?: VideoConfig[];
  eventLog?: string;
  currentTime?: number;
  duration?: number;
};

type PageActions = {
  data: PageData;
  play(): void;
  pause(): void;
  onError(event: Event): void;
  stop(): void;
  seek(position: number): void;
  requestFullScreen(): void;
  onPlaying(event: Event): void;
  onPause(event: Event): void;
  onStop(event: Event): void;
  onEnded(event: Event): void;
  onWaiting(event: Event): void;
  onTimeUpdate(event: Event): void;
  onFullscreenChange(event: Event): void;
  onQualityChange(event: Event): void;
  onRateChange(event: Event): void;
};

const SEEK_STEP_SECONDS = 10;

export default function App() {
  const { data, actions } = useLxPage();
  const {
    play,
    pause,
    stop,
    seek,
    requestFullScreen,
    onPlaying,
    onPause,
    onStop,
    onEnded,
    onWaiting,
    onTimeUpdate,
    onFullscreenChange,
    onQualityChange,
    onRateChange,
  } = actions;
  const video = data?.videos?.[0];
  const eventLog = data?.eventLog || 'Ready';
  const [islandPlaying, setIslandPlaying] = React.useState(false);
  const [nativePressSource, setNativePressSource] = React.useState('none');
  const [nativeMenuOpen, setNativeMenuOpen] = React.useState(false);
  const [nativeMenuResult, setNativeMenuResult] = React.useState('Tap Menu to mount native actions above the video.');
  const currentTime = typeof data?.currentTime === 'number' ? data.currentTime : 0;
  const duration = typeof data?.duration === 'number' ? data.duration : 0;

  // Relative seek helpers
  const seekBackward = React.useCallback(
    (seconds: number) => {
      const newTime = Math.max(0, currentTime - seconds);
      seek(newTime);
    },
    [currentTime, seek],
  );

  const seekForward = React.useCallback(
    (seconds: number) => {
      const maxTime = duration > 0 ? duration : Number.POSITIVE_INFINITY;
      const newTime = Math.min(maxTime, currentTime + seconds);
      seek(newTime);
    },
    [currentTime, duration, seek],
  );

  const toggleNativeMenu = () => {
    const open = !nativeMenuOpen;
    setNativeMenuOpen(open);
    setNativeMenuResult(open ? 'H5 mounted the native menu.' : 'H5 removed the native menu.');
  };

  const handleNativeMenuMore = ({ source }: { source?: string }) => {
    setNativePressSource(source || 'unknown');
    setNativeMenuOpen(false);
    setNativeMenuResult('More handled by View JS.');
  };

  const handleNativeMenuClose = ({ source }: { source?: string }) => {
    setNativePressSource(source || 'unknown');
    setNativeMenuOpen(false);
    setNativeMenuResult('Close handled by View JS.');
  };

  if (!video) {
    return (
      <div className="bg-surface-100 min-h-screen flex items-center justify-center">
        <div className="text-gray-500">Loading video...</div>
      </div>
    );
  }

  return (
    <div className="bg-surface-100 min-h-screen" data-testid="video-page">
      <div className="px-4 py-4 space-y-3 pb-6">
        {/* Header */}
        <div className="flex items-center justify-between">
          <div className="flex items-center gap-2">
            <div className="w-8 h-8 bg-linear-to-br from-blue-500 to-purple-600 rounded-lg flex items-center justify-center">
              <svg viewBox="0 0 24 24" fill="white" className="w-4 h-4">
                <polygon points="5 3 19 12 5 21 5 3" />
              </svg>
            </div>
            <div>
              <div className="text-base font-semibold text-gray-900">Native Video</div>
            </div>
          </div>
          <div data-testid="video-event" className="bg-surface-900 text-green-400 font-mono text-xs px-3 py-1.5 rounded-lg w-[180px] truncate">
            {eventLog}
          </div>
          <div data-testid="island-playing" className="sr-only">{islandPlaying ? 'yes' : 'no'}</div>
          <div data-testid="native-press-source" className="sr-only">{nativePressSource}</div>
        </div>

        <div className="flex items-center justify-between gap-3 rounded-xl border border-blue-200 bg-blue-50 px-3 py-2.5">
          <div className="min-w-0">
            <div className="text-xs font-semibold text-blue-900">H5 → native video menu</div>
            <div data-testid="native-menu-js-result" className="text-[11px] leading-4 text-blue-700">{nativeMenuResult}</div>
          </div>
          <button
            type="button"
            data-testid="native-menu-toggle"
            aria-label={nativeMenuOpen ? 'Close native menu' : 'Open native menu'}
            aria-expanded={nativeMenuOpen}
            onClick={toggleNativeMenu}
            className="flex shrink-0 items-center gap-2 rounded-lg bg-blue-600 px-3 py-2 text-xs font-semibold text-white active:scale-95"
          >
            <svg viewBox="0 0 20 20" fill="none" className="h-4 w-4" aria-hidden="true">
              <path d="M3 5h14M3 10h14M3 15h14" stroke="currentColor" strokeWidth="2" strokeLinecap="round" />
            </svg>
            {nativeMenuOpen ? 'Close' : 'Menu'}
          </button>
          <span data-testid="native-menu-state" className="sr-only">{nativeMenuOpen ? 'open' : 'closed'}</span>
        </div>

        <div className="bg-black rounded-xl overflow-hidden">
          <LxNativeRoot id="video-native-root" className="block w-full" style={{ aspectRatio: '16 / 9' }}>
            <LxVideo
              id={video.id}
              src={video.src}
              poster={video.poster}
              qualities={video.qualities}
              playbackRates={video.playbackRates}
              autoplay={video.autoplay ?? Boolean(video.src)}
              controls
              volume="0.8"
              className="block w-full rounded-lg bg-black"
              style={{ aspectRatio: '16 / 9', borderRadius: 12 }}
              onPlaying={(event) => {
                setIslandPlaying(true);
                onPlaying(event);
              }}
              onError={actions.onError}
              onPause={(event) => {
                setIslandPlaying(false);
                onPause(event);
              }}
              onStop={onStop}
              onEnded={onEnded}
              onWaiting={onWaiting}
              onTimeUpdate={onTimeUpdate}
              onFullscreenChange={onFullscreenChange}
              onQualityChange={onQualityChange}
              onRateChange={onRateChange}
            />
            {nativeMenuOpen ? (
              <LxNativeCover
                id="video-native-cover"
                automationId="video-native-cover"
                scrim="none"
                role="presentation"
              >
                <LxNativeView
                  id="video-native-menu"
                  automationId="video-native-menu"
                  role="menu"
                  aria-label="Native video menu"
                  className="absolute right-3 top-3 border border-slate-500 bg-slate-900"
                  style={{ width: 240, height: 144, borderRadius: 14, backgroundColor: '#0f172a', borderColor: '#64748b', borderWidth: 1 }}
                >
                  <LxNativeText
                    id="video-native-menu-title"
                    className="absolute left-4 right-4 top-4 text-sm font-semibold text-white"
                    fontSize={14}
                    fontWeight={600}
                    color="#ffffff"
                    maxLines={1}
                  >
                    Native menu
                  </LxNativeText>
                  <LxNativeText
                    id="video-native-menu-detail"
                    className="absolute left-4 right-4 top-11 text-xs text-slate-300"
                    fontSize={11}
                    color="#cbd5e1"
                    maxLines={1}
                  >
                    NativeView above native video
                  </LxNativeText>
                  <LxNativeButton
                    id="video-native-menu-more"
                    automationId="video-native-menu-more"
                    label="More"
                    icon="more"
                    intent="accent"
                    emphasis="primary"
                    size="compact"
                    aria-label="More native menu actions"
                    className="absolute bottom-4 left-4"
                    style={{ width: 96, height: 40, borderRadius: 10 }}
                    onPress={handleNativeMenuMore}
                  />
                  <LxNativeButton
                    id="video-native-menu-close"
                    automationId="video-native-menu-close"
                    label="Close"
                    icon="close"
                    emphasis="secondary"
                    size="compact"
                    aria-label="Close native menu"
                    className="absolute bottom-4 right-4"
                    style={{ width: 96, height: 40, borderRadius: 10 }}
                    onPress={handleNativeMenuClose}
                  />
                </LxNativeView>
              </LxNativeCover>
            ) : null}
          </LxNativeRoot>
        </div>

        {/* Controls */}
        <div className="bg-surface/80 backdrop-blur-xl rounded-2xl shadow-lg border border-white/20 p-5">
          <div className="text-xs text-gray-400 uppercase tracking-wider mb-4 font-semibold">Playback Controls</div>

          <div className="flex items-center justify-center gap-4 mb-5">
            <button
              onClick={() => seekBackward(SEEK_STEP_SECONDS)}
              className="w-12 h-12 rounded-full bg-surface-100 hover:bg-surface-200 active:scale-95 transition-all flex items-center justify-center"
            >
              <svg viewBox="0 0 24 24" fill="none" className="w-5 h-5 text-gray-600">
                <path d="M12 5V1L7 6l5 5V7c3.31 0 6 2.69 6 6s-2.69 6-6 6-6-2.69-6-6H4c0 4.42 3.58 8 8 8s8-3.58 8-8-3.58-8-8-8z" fill="currentColor" />
                <text x="12" y="14" textAnchor="middle" fontSize="5" fill="currentColor" fontWeight="bold">{SEEK_STEP_SECONDS}</text>
              </svg>
            </button>

            <button
              data-testid="video-play"
              onClick={() => play()}
              className="w-16 h-16 rounded-full bg-linear-to-b from-green-400 to-green-600 hover:from-green-500 hover:to-green-700 active:scale-95 transition-all flex items-center justify-center shadow-lg shadow-green-500/30"
            >
              <svg viewBox="0 0 24 24" fill="white" className="w-7 h-7 ml-1">
                <polygon points="5 3 19 12 5 21 5 3" />
              </svg>
            </button>

            <button
              data-testid="video-pause"
              onClick={() => pause()}
              className="w-14 h-14 rounded-full bg-linear-to-b from-surface-700 to-surface-900 hover:from-surface-600 hover:to-surface-800 active:scale-95 transition-all flex items-center justify-center shadow-lg shadow-gray-900/30"
            >
              <svg viewBox="0 0 24 24" fill="white" className="w-6 h-6">
                <rect x="6" y="4" width="4" height="16" rx="1" />
                <rect x="14" y="4" width="4" height="16" rx="1" />
              </svg>
            </button>

            <button
              onClick={() => seekForward(SEEK_STEP_SECONDS)}
              className="w-12 h-12 rounded-full bg-surface-100 hover:bg-surface-200 active:scale-95 transition-all flex items-center justify-center"
            >
              <svg viewBox="0 0 24 24" fill="none" className="w-5 h-5 text-gray-600">
                <path d="M12 5V1l5 5-5 5V7c-3.31 0-6 2.69-6 6s2.69 6 6 6 6-2.69 6-6h2c0 4.42-3.58 8-8 8s-8-3.58-8-8 3.58-8 8-8z" fill="currentColor" />
                <text x="12" y="14" textAnchor="middle" fontSize="5" fill="currentColor" fontWeight="bold">{SEEK_STEP_SECONDS}</text>
              </svg>
            </button>
          </div>

          <div className="flex items-center justify-center gap-3">
            <button
              data-testid="video-stop"
              onClick={() => stop()}
              className="flex items-center gap-2 px-4 py-2.5 rounded-xl bg-red-50 hover:bg-red-100 active:scale-98 transition-all"
            >
              <svg viewBox="0 0 24 24" fill="currentColor" className="w-4 h-4 text-red-500 dark:text-red-400">
                <rect x="6" y="6" width="12" height="12" rx="2" />
              </svg>
              <span className="text-sm font-medium text-red-600 dark:text-red-400">Stop</span>
            </button>

            <button
              onClick={() => requestFullScreen()}
              className="flex items-center gap-2 px-4 py-2.5 rounded-xl bg-indigo-50 hover:bg-indigo-100 active:scale-98 transition-all"
            >
              <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" className="w-4 h-4 text-indigo-500 dark:text-indigo-400">
                <path d="M8 3H5a2 2 0 0 0-2 2v3m18 0V5a2 2 0 0 0-2-2h-3m0 18h3a2 2 0 0 0 2-2v-3M3 16v3a2 2 0 0 0 2 2h3" />
              </svg>
              <span className="text-sm font-medium text-indigo-600 dark:text-indigo-400">Fullscreen</span>
            </button>
          </div>
        </div>

        {/* Info Card */}
        <div className="bg-blue-50 border border-blue-200 rounded-xl p-3">
          <div className="flex gap-2">
            <div className="text-blue-500 dark:text-blue-400 mt-0.5 shrink-0">
              <svg viewBox="0 0 24 24" fill="currentColor" className="w-4 h-4">
                <circle cx="12" cy="12" r="10" opacity="0.2" />
                <path d="M12 16v-4m0-4h.01M12 2a10 10 0 1 0 0 20 10 10 0 0 0 0-20z" fill="none" stroke="currentColor" strokeWidth="2" />
              </svg>
            </div>
            <div className="text-xs text-blue-700 dark:text-blue-400 leading-relaxed">
              Video config comes from <code className="bg-blue-100 px-1 py-0.5 rounded text-blue-800 dark:text-blue-400">data.videos</code> in <code className="bg-blue-100 px-1 py-0.5 rounded text-blue-800 dark:text-blue-400">pages/video/index.js</code>.
              Quality and playbackRate are passed to the native player.
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}
