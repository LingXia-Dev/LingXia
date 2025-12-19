import React from 'react';
import '../../tailwind.css';
import { LxVideo } from 'lingxia-ui/react';

type LxVideoEvent<TDetail> = { detail?: TDetail };

type VideoConfig = {
  id: string;
  src: string;
  poster?: string;
  qualities?: Array<{ label: string; url?: string }>;
  playbackRates?: number[];
};

type PageData = {
  videos?: VideoConfig[];
};

type PageActions = {
  data: PageData;
  play(): void;
  pause(): void;
  stop(): void;
  seek(position: number): void;
  requestFullScreen(): void;
  onQualityChange(payload: { videoId: string; detail: unknown }): void;
  onPlaybackRateChange(payload: { videoId: string; detail: unknown }): void;
};

declare function useLingXia(): PageActions;

const SEEK_STEP_SECONDS = 10;

export default function App() {
  const {
    data,
    play,
    pause,
    stop,
    seek,
    requestFullScreen,
    onQualityChange,
    onPlaybackRateChange,
  } = useLingXia();
  const video = data?.videos?.[0];
  const [eventLog, setEventLog] = React.useState('Ready');
  const currentTimeRef = React.useRef(0);
  const durationRef = React.useRef(0);

  React.useEffect(() => {
    document.body.classList.add('api-page');
    return () => document.body.classList.remove('api-page');
  }, []);

  const onPlayHandler = React.useCallback(() => {
    setEventLog('Playing');
  }, []);

  const onPauseHandler = React.useCallback(() => {
    setEventLog('Paused');
  }, []);

  const onWaitingHandler = React.useCallback(() => {
    setEventLog('Buffering...');
  }, []);

  const onTimeUpdateHandler = React.useCallback(
    (e: LxVideoEvent<{ currentTime?: number; duration?: number }>) => {
      const currentTime = e.detail?.currentTime;
      const duration = e.detail?.duration;
      if (typeof currentTime === 'number') currentTimeRef.current = currentTime;
      if (typeof duration === 'number') durationRef.current = duration;
    },
    [],
  );

  const onFullscreenChangeHandler = React.useCallback(
    (e: LxVideoEvent<{ fullScreen?: boolean }>) => {
      setEventLog(`Fullscreen: ${e.detail?.fullScreen ? 'on' : 'off'}`);
    },
    [],
  );

  const onQualityChangeHandler = React.useCallback(
    (e: LxVideoEvent<{ quality?: string }>) => {
      if (!video) return;
      setEventLog(`Quality: ${e.detail?.quality ?? ''}`);
      onQualityChange({ videoId: video.id, detail: e.detail });
    },
    [onQualityChange, video],
  );

  const onPlaybackRateChangeHandler = React.useCallback(
    (e: LxVideoEvent<{ rate?: number }>) => {
      if (!video) return;
      setEventLog(`Rate: ${e.detail?.rate ?? ''}`);
      onPlaybackRateChange({ videoId: video.id, detail: e.detail });
    },
    [onPlaybackRateChange, video],
  );

  // Relative seek helpers
  const seekBackward = React.useCallback(
    (seconds: number) => {
      const newTime = Math.max(0, currentTimeRef.current - seconds);
      currentTimeRef.current = newTime; // Optimistic update
      seek(newTime);
    },
    [seek],
  );

  const seekForward = React.useCallback(
    (seconds: number) => {
      const maxTime =
        durationRef.current > 0 ? durationRef.current : Number.POSITIVE_INFINITY;
      const newTime = Math.min(maxTime, currentTimeRef.current + seconds);
      currentTimeRef.current = newTime; // Optimistic update
      seek(newTime);
    },
    [seek],
  );

  if (!video) {
    return (
      <div className="bg-gray-100 min-h-screen flex items-center justify-center">
        <div className="text-gray-500">Loading video...</div>
      </div>
    );
  }

  return (
    <div className="bg-gray-100 min-h-screen">
      <div className="px-4 py-4 space-y-3 pb-6">
        {/* Header */}
        <div className="flex items-center justify-between">
          <div className="flex items-center gap-2">
            <div className="w-8 h-8 bg-gradient-to-br from-blue-500 to-purple-600 rounded-lg flex items-center justify-center">
              <svg viewBox="0 0 24 24" fill="white" className="w-4 h-4">
                <polygon points="5 3 19 12 5 21 5 3" />
              </svg>
            </div>
            <div>
              <div className="text-base font-semibold text-gray-900">Native Video</div>
            </div>
          </div>
          <div className="bg-gray-900 text-green-400 font-mono text-xs px-3 py-1.5 rounded-lg w-[180px] truncate">
            {eventLog}
          </div>
        </div>

        <div className="bg-black rounded-xl overflow-hidden">
          <LxVideo
            id={video.id}
            src={video.src}
            poster={video.poster}
            qualities={video.qualities}
            playbackRates={video.playbackRates}
            autoplay
            controls
            volume="0.8"
            className="block w-full rounded-lg bg-black"
            style={{ aspectRatio: '16 / 9', borderRadius: 12 }}
            onPlay={onPlayHandler}
            onPause={onPauseHandler}
            onWaiting={onWaitingHandler}
            onTimeUpdate={onTimeUpdateHandler}
            onFullscreenChange={onFullscreenChangeHandler}
            onQualityChange={onQualityChangeHandler}
            onPlaybackRateChange={onPlaybackRateChangeHandler}
          />
        </div>

        {/* Controls */}
        <div className="bg-white/80 backdrop-blur-xl rounded-2xl shadow-lg border border-white/20 p-5">
          <div className="text-xs text-gray-400 uppercase tracking-wider mb-4 font-semibold">Playback Controls</div>

          <div className="flex items-center justify-center gap-4 mb-5">
            <button
              onClick={() => seekBackward(SEEK_STEP_SECONDS)}
              className="w-12 h-12 rounded-full bg-gray-100 hover:bg-gray-200 active:scale-95 transition-all flex items-center justify-center"
            >
              <svg viewBox="0 0 24 24" fill="none" className="w-5 h-5 text-gray-600">
                <path d="M12 5V1L7 6l5 5V7c3.31 0 6 2.69 6 6s-2.69 6-6 6-6-2.69-6-6H4c0 4.42 3.58 8 8 8s8-3.58 8-8-3.58-8-8-8z" fill="currentColor" />
                <text x="12" y="14" textAnchor="middle" fontSize="5" fill="currentColor" fontWeight="bold">{SEEK_STEP_SECONDS}</text>
              </svg>
            </button>

            <button
              onClick={() => play()}
              className="w-16 h-16 rounded-full bg-gradient-to-b from-green-400 to-green-600 hover:from-green-500 hover:to-green-700 active:scale-95 transition-all flex items-center justify-center shadow-lg shadow-green-500/30"
            >
              <svg viewBox="0 0 24 24" fill="white" className="w-7 h-7 ml-1">
                <polygon points="5 3 19 12 5 21 5 3" />
              </svg>
            </button>

            <button
              onClick={() => pause()}
              className="w-14 h-14 rounded-full bg-gradient-to-b from-gray-700 to-gray-900 hover:from-gray-600 hover:to-gray-800 active:scale-95 transition-all flex items-center justify-center shadow-lg shadow-gray-900/30"
            >
              <svg viewBox="0 0 24 24" fill="white" className="w-6 h-6">
                <rect x="6" y="4" width="4" height="16" rx="1" />
                <rect x="14" y="4" width="4" height="16" rx="1" />
              </svg>
            </button>

            <button
              onClick={() => seekForward(SEEK_STEP_SECONDS)}
              className="w-12 h-12 rounded-full bg-gray-100 hover:bg-gray-200 active:scale-95 transition-all flex items-center justify-center"
            >
              <svg viewBox="0 0 24 24" fill="none" className="w-5 h-5 text-gray-600">
                <path d="M12 5V1l5 5-5 5V7c-3.31 0-6 2.69-6 6s2.69 6 6 6 6-2.69 6-6h2c0 4.42-3.58 8-8 8s-8-3.58-8-8 3.58-8 8-8z" fill="currentColor" />
                <text x="12" y="14" textAnchor="middle" fontSize="5" fill="currentColor" fontWeight="bold">{SEEK_STEP_SECONDS}</text>
              </svg>
            </button>
          </div>

          <div className="flex items-center justify-center gap-3">
            <button
              onClick={() => stop()}
              className="flex items-center gap-2 px-4 py-2.5 rounded-xl bg-red-50 hover:bg-red-100 active:scale-98 transition-all"
            >
              <svg viewBox="0 0 24 24" fill="currentColor" className="w-4 h-4 text-red-500">
                <rect x="6" y="6" width="12" height="12" rx="2" />
              </svg>
              <span className="text-sm font-medium text-red-600">Stop</span>
            </button>

            <button
              onClick={() => requestFullScreen()}
              className="flex items-center gap-2 px-4 py-2.5 rounded-xl bg-indigo-50 hover:bg-indigo-100 active:scale-98 transition-all"
            >
              <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" className="w-4 h-4 text-indigo-500">
                <path d="M8 3H5a2 2 0 0 0-2 2v3m18 0V5a2 2 0 0 0-2-2h-3m0 18h3a2 2 0 0 0 2-2v-3M3 16v3a2 2 0 0 0 2 2h3" />
              </svg>
              <span className="text-sm font-medium text-indigo-600">Fullscreen</span>
            </button>
          </div>
        </div>

        {/* Info Card */}
        <div className="bg-blue-50 border border-blue-200 rounded-xl p-3">
          <div className="flex gap-2">
            <div className="text-blue-500 mt-0.5 flex-shrink-0">
              <svg viewBox="0 0 24 24" fill="currentColor" className="w-4 h-4">
                <circle cx="12" cy="12" r="10" opacity="0.2" />
                <path d="M12 16v-4m0-4h.01M12 2a10 10 0 1 0 0 20 10 10 0 0 0 0-20z" fill="none" stroke="currentColor" strokeWidth="2" />
              </svg>
            </div>
            <div className="text-xs text-blue-700 leading-relaxed">
              Video config comes from <code className="bg-blue-100 px-1 py-0.5 rounded text-blue-800">data.videos</code> in <code className="bg-blue-100 px-1 py-0.5 rounded text-blue-800">pages/video/index.js</code>.
              Quality and playbackRate are passed to the native player.
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}
