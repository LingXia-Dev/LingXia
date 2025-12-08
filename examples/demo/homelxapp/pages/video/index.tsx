import React from 'react';
import './tailwind.css';
import { LxVideo } from '@lingxia/view/react';

const VIDEO_SRC =
  'https://cdn.plyr.io/static/demo/View_From_A_Blue_Moon_Trailer-576p.mp4';
const VIDEO_POSTER =
  'https://cdn.plyr.io/static/demo/View_From_A_Blue_Moon_Trailer-HD.jpg';

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
  const [eventLog, setEventLog] = React.useState('Waiting for events...');
  const [metadata, setMetadata] = React.useState<string>('');

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
    // console.log('[VideoPage] TimeUpdate:', e.detail?.currentTime);
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
    const metadataText = `${width}×${height}, ${duration}s`;
    setMetadata(metadataText);
    setEventLog(`ℹ️ Metadata loaded: ${metadataText}`);
  }, []);

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

        {/* Placeholder for future H5 controls */}
        <div className="bg-white rounded-xl shadow-sm border border-gray-200 p-4">
          <div className="text-xs text-gray-400 text-center">
            H5 Playback Controls (coming soon)
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
