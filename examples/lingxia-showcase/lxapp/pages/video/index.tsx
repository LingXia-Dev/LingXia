import React from 'react';
import '../../tailwind.css';

// MDN's compact CC0 demo supports byte-range playback.
const DEMO_VIDEO = 'https://interactive-examples.mdn.mozilla.net/media/cc0-videos/flower.mp4';
const SEEK_STEP_SECONDS = 10;

export default function App() {
  const videoRef = React.useRef<HTMLVideoElement | null>(null);
  const [eventLog, setEventLog] = React.useState('Ready');
  const [currentTime, setCurrentTime] = React.useState(0);
  const [duration, setDuration] = React.useState(0);

  const play = () => {
    void videoRef.current?.play().catch((error: unknown) => {
      setEventLog(`Play failed: ${error instanceof Error ? error.message : String(error)}`);
    });
  };
  const pause = () => videoRef.current?.pause();
  const stop = () => {
    const video = videoRef.current;
    if (!video) return;
    video.pause();
    video.currentTime = 0;
    setEventLog('Stopped');
  };
  const seekBy = (seconds: number) => {
    const video = videoRef.current;
    if (!video) return;
    const max = Number.isFinite(video.duration) ? video.duration : Number.POSITIVE_INFINITY;
    video.currentTime = Math.min(max, Math.max(0, video.currentTime + seconds));
  };
  const fullscreen = () => {
    void videoRef.current?.requestFullscreen?.().catch(() => setEventLog('Fullscreen unavailable'));
  };

  return (
    <div className="bg-surface-100 min-h-screen" data-testid="video-page">
      <div className="px-4 py-4 space-y-3 pb-6">
        <div className="flex items-center justify-between">
          <div className="text-base font-semibold text-gray-900">HTML Video</div>
          <div data-testid="video-event" className="bg-surface-900 text-green-400 font-mono text-xs px-3 py-1.5 rounded-lg w-[180px] truncate">
            {eventLog}
          </div>
        </div>

        <div className="bg-black rounded-xl overflow-hidden">
          <video
            ref={videoRef}
            id="video-player"
            data-testid="video-element"
            src={DEMO_VIDEO}
            controls
            playsInline
            preload="metadata"
            className="block w-full bg-black"
            style={{ aspectRatio: '16 / 9' }}
            onPlaying={() => setEventLog('Playing')}
            onPause={() => setEventLog('Paused')}
            onEnded={() => setEventLog('Ended')}
            onWaiting={() => setEventLog('Buffering...')}
            onError={() => setEventLog('Playback error')}
            onLoadedMetadata={(event) => setDuration(event.currentTarget.duration)}
            onTimeUpdate={(event) => setCurrentTime(event.currentTarget.currentTime)}
          />
        </div>

        <div className="bg-surface/80 rounded-2xl shadow-lg border border-white/20 p-5">
          <div className="text-xs text-gray-400 uppercase tracking-wider mb-4 font-semibold">Playback Controls</div>
          <div data-testid="video-progress" className="mb-4 text-center font-mono text-xs text-gray-500">
            {currentTime.toFixed(1)} / {duration.toFixed(1)} s
          </div>
          <div className="flex items-center justify-center gap-3 flex-wrap">
            <button
              onClick={() => seekBy(-SEEK_STEP_SECONDS)}
              className="px-4 py-2.5 rounded-xl bg-surface-100 text-sm font-medium text-gray-700 active:scale-95"
            >
              -{SEEK_STEP_SECONDS}s
            </button>
            <button
              data-testid="video-play"
              onClick={play}
              className="px-4 py-2.5 rounded-xl bg-green-500 text-sm font-medium text-white active:scale-95"
            >
              Play
            </button>
            <button
              data-testid="video-pause"
              onClick={pause}
              className="px-4 py-2.5 rounded-xl bg-surface-900 text-sm font-medium text-white active:scale-95"
            >
              Pause
            </button>
            <button
              data-testid="video-stop"
              onClick={stop}
              className="px-4 py-2.5 rounded-xl bg-red-50 text-sm font-medium text-red-600 active:scale-95"
            >
              Stop
            </button>
            <button
              onClick={() => seekBy(SEEK_STEP_SECONDS)}
              className="px-4 py-2.5 rounded-xl bg-surface-100 text-sm font-medium text-gray-700 active:scale-95"
            >
              +{SEEK_STEP_SECONDS}s
            </button>
            <button
              onClick={fullscreen}
              className="px-4 py-2.5 rounded-xl bg-indigo-50 text-sm font-medium text-indigo-600 active:scale-95"
            >
              Fullscreen
            </button>
          </div>
        </div>
      </div>
    </div>
  );
}
