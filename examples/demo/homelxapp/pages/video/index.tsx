import React from 'react';
import '../../tailwind.css';
import { LxVideo } from 'lingxia-ui/react';

const VIDEO_SRC =
  'https://cdn.plyr.io/static/demo/View_From_A_Blue_Moon_Trailer-576p.mp4';
const VIDEO_POSTER =
  'https://cdn.plyr.io/static/demo/View_From_A_Blue_Moon_Trailer-HD.jpg';

type PageActions = {
  data: Record<string, unknown>;
  play(): void;
  pause(): void;
  stop(): void;
  seek(position: number): void;
  requestFullScreen(): void;
  exitFullScreen(): void;
};

declare function useLingXia(): PageActions;

// Memoized video component to prevent re-renders when parent state changes
const MemoizedVideo = React.memo(function MemoizedVideo({
  onPlay,
  onPause,
  onEnded,
  onWaiting,
  onTimeUpdate,
  onFullscreenChange,
  onLoadedMetadata,
}: {
  onPlay: () => void;
  onPause: () => void;
  onEnded: () => void;
  onWaiting: () => void;
  onTimeUpdate: (e: any) => void;
  onFullscreenChange: (e: any) => void;
  onLoadedMetadata: (e: any) => void;
}) {
  return (
    <LxVideo
      id="lx-video"
      src={VIDEO_SRC}
      poster={VIDEO_POSTER}
      autoplay
      controls
      volume="0.8"
      className="block w-full rounded-lg bg-black"
      style={{ aspectRatio: '16 / 9', borderRadius: 12 }}
      onPlay={onPlay}
      onPause={onPause}
      onEnded={onEnded}
      onWaiting={onWaiting}
      onTimeUpdate={onTimeUpdate}
      onFullscreenChange={onFullscreenChange}
      onLoadedMetadata={onLoadedMetadata}
    />
  );
});

export default function App() {
  const { play, pause, stop, seek, requestFullScreen } = useLingXia();

  const [eventLog, setEventLog] = React.useState('Waiting for events...');
  const [metadata, setMetadata] = React.useState('');
  const currentTimeRef = React.useRef(0);
  const durationRef = React.useRef(0);

  React.useEffect(() => {
    document.body.classList.add('api-page');
    return () => document.body.classList.remove('api-page');
  }, []);

  // Use useCallback to prevent creating new function references on each render
  const onPlay = React.useCallback(() => {
    console.log('[VideoPage] Play event');
    setEventLog('▶️ Playing');
  }, []);

  const onPause = React.useCallback(() => {
    console.log('[VideoPage] Pause event');
    setEventLog('⏸️ Paused');
  }, []);

  const onEnded = React.useCallback(() => {
    console.log('[VideoPage] Ended event');
    setEventLog('🏁 Ended');
  }, []);

  const onWaiting = React.useCallback(() => {
    console.log('[VideoPage] Waiting event');
    setEventLog('⏳ Buffering...');
  }, []);

  const onTimeUpdate = React.useCallback((e: any) => {
    const detail = e.detail || {};
    if (typeof detail.currentTime === 'number') {
      currentTimeRef.current = detail.currentTime;
    }
    if (typeof detail.duration === 'number') {
      durationRef.current = detail.duration;
    }
  }, []);

  const onFullscreenChange = React.useCallback((e: any) => {
    console.log('[VideoPage] FullscreenChange detail:', e.detail);
    setEventLog(`📺 Fullscreen: ${e.detail?.fullScreen} (${e.detail?.direction})`);
  }, []);

  const onLoadedMetadata = React.useCallback((e: any) => {
    console.log('[VideoPage] LoadedMetadata detail:', e.detail);
    const detail = e.detail || {};
    const width = detail.width || '?';
    const height = detail.height || '?';
    const duration = detail.duration ? Number(detail.duration).toFixed(1) : '?';
    if (typeof detail.duration === 'number') {
      durationRef.current = detail.duration;
    }
    const metadataText = `${width}×${height}, ${duration}s`;
    setMetadata(metadataText);
    setEventLog(`ℹ️ Metadata loaded: ${metadataText}`);
  }, []);

  // Relative seek helpers
  const seekBackward = React.useCallback((seconds: number) => {
    const newTime = Math.max(0, currentTimeRef.current - seconds);
    seek(newTime);
  }, [seek]);

  const seekForward = React.useCallback((seconds: number) => {
    const newTime = Math.min(durationRef.current, currentTimeRef.current + seconds);
    seek(newTime);
  }, [seek]);

  return (
    <div className="bg-gray-100 min-h-screen">
      <div className="px-4 py-4 space-y-3 pb-6">
        {/* Compact Header */}
        <div className="flex items-center justify-between">
          <div className="flex items-center gap-2">
            <div className="w-8 h-8 bg-gradient-to-br from-blue-500 to-purple-600 rounded-lg flex items-center justify-center">
              <svg viewBox="0 0 24 24" fill="white" className="w-4 h-4">
                <polygon points="5 3 19 12 5 21 5 3" />
              </svg>
            </div>
            <div>
              <div className="text-base font-semibold text-gray-900">Native Video</div>
              {metadata && <div className="text-xs text-gray-500">{metadata}</div>}
            </div>
          </div>
          <div className="bg-gray-900 text-green-400 font-mono text-xs px-3 py-1.5 rounded-lg w-[180px] truncate">
            {eventLog}
          </div>
        </div>

        {/* Video Player */}
        <div className="bg-black rounded-xl overflow-hidden">
          <MemoizedVideo
            onPlay={onPlay}
            onPause={onPause}
            onEnded={onEnded}
            onWaiting={onWaiting}
            onTimeUpdate={onTimeUpdate}
            onFullscreenChange={onFullscreenChange}
            onLoadedMetadata={onLoadedMetadata}
          />
        </div>

        <div className="bg-white/80 backdrop-blur-xl rounded-2xl shadow-lg border border-white/20 p-5">
          <div className="text-xs text-gray-400 uppercase tracking-wider mb-4 font-semibold">Playback Controls</div>

          {/* Main Controls Row */}
          <div className="flex items-center justify-center gap-4 mb-5">
            {/* Seek Back 10s */}
            <button
              onClick={() => seekBackward(10)}
              className="w-12 h-12 rounded-full bg-gray-100 hover:bg-gray-200 active:scale-95 transition-all flex items-center justify-center"
            >
              <svg viewBox="0 0 24 24" fill="none" className="w-5 h-5 text-gray-600">
                <path d="M12 5V1L7 6l5 5V7c3.31 0 6 2.69 6 6s-2.69 6-6 6-6-2.69-6-6H4c0 4.42 3.58 8 8 8s8-3.58 8-8-3.58-8-8-8z" fill="currentColor" />
                <text x="12" y="14" textAnchor="middle" fontSize="6" fill="currentColor" fontWeight="bold">10</text>
              </svg>
            </button>

            {/* Play Button - Large */}
            <button
              onClick={() => play()}
              className="w-16 h-16 rounded-full bg-gradient-to-b from-green-400 to-green-600 hover:from-green-500 hover:to-green-700 active:scale-95 transition-all flex items-center justify-center shadow-lg shadow-green-500/30"
            >
              <svg viewBox="0 0 24 24" fill="white" className="w-7 h-7 ml-1">
                <polygon points="5 3 19 12 5 21 5 3" />
              </svg>
            </button>

            {/* Pause Button */}
            <button
              onClick={() => pause()}
              className="w-14 h-14 rounded-full bg-gradient-to-b from-gray-700 to-gray-900 hover:from-gray-600 hover:to-gray-800 active:scale-95 transition-all flex items-center justify-center shadow-lg shadow-gray-900/30"
            >
              <svg viewBox="0 0 24 24" fill="white" className="w-6 h-6">
                <rect x="6" y="4" width="4" height="16" rx="1" />
                <rect x="14" y="4" width="4" height="16" rx="1" />
              </svg>
            </button>

            {/* Seek Forward 10s */}
            <button
              onClick={() => seekForward(10)}
              className="w-12 h-12 rounded-full bg-gray-100 hover:bg-gray-200 active:scale-95 transition-all flex items-center justify-center"
            >
              <svg viewBox="0 0 24 24" fill="none" className="w-5 h-5 text-gray-600">
                <path d="M12 5V1l5 5-5 5V7c-3.31 0-6 2.69-6 6s2.69 6 6 6 6-2.69 6-6h2c0 4.42-3.58 8-8 8s-8-3.58-8-8 3.58-8 8-8z" fill="currentColor" />
                <text x="12" y="14" textAnchor="middle" fontSize="6" fill="currentColor" fontWeight="bold">10</text>
              </svg>
            </button>
          </div>

          {/* Secondary Controls */}
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
              <code className="bg-blue-100 px-1 py-0.5 rounded text-blue-800">&lt;LxVideo&gt;</code> uses native players (AVPlayer on iOS, ExoPlayer on Android, OH_AVPlayer on HarmonyOS) for optimal performance.
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}
