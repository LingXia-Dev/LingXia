import { useLxPage } from '@lingxia/react';
import '../../tailwind.css';

type PageData = {
  supported?: boolean;
  earpiece?: boolean;
  status?: string;
  trackTitle?: string;
  index?: number;
  hasNext?: boolean;
  tracks?: Array<{ id: string; title: string }>;
  positionText?: string;
  durationText?: string;
  progress?: number;
  rate?: number;
  repeat?: string;
  muted?: boolean;
  volume?: number;
  output?: string;
  routeText?: string;
  soundReady?: boolean;
  eventLog?: string[];
};

type PageActions = {
  togglePlay(): void;
  stop(): void;
  previous(): void;
  next(): void;
  skipTo(params: { index: number }): void;
  seekBy(params: { deltaMs: number }): void;
  cycleRate(): void;
  cycleRepeat(): void;
  toggleMute(): void;
  setVolume(params: { volume: number }): void;
  toggleOutput(): void;
  playSound(params: { pan: number }): void;
  stopSounds(): void;
};

const secondary = 'py-2.5 rounded-lg bg-surface-200 text-gray-900 text-sm font-medium disabled:opacity-40';

export default function AudioPage() {
  const { data, actions } = useLxPage<PageData, PageActions>();
  const {
    togglePlay, stop, previous, next, skipTo, seekBy, cycleRate, cycleRepeat,
    toggleMute, setVolume, toggleOutput, playSound, stopSounds,
  } = actions;

  if (data?.supported === false) {
    return (
      <div className="min-h-screen bg-surface-100 px-3 py-3">
        <div className="bg-surface rounded-lg shadow-sm px-4 py-4 text-sm text-gray-700" data-testid="audio-unsupported">
          lx.audio is not available on this platform.
        </div>
      </div>
    );
  }

  const playing = data?.status === 'playing';
  const volume = data?.volume ?? 1;

  return (
    <div className="min-h-screen bg-surface-100 overflow-y-auto">
      <div className="px-3 py-3 pb-12 space-y-3">
        <div className="bg-surface rounded-lg shadow-sm">
          <div className="px-4 py-4 border-b border-line-100">
            <div className="text-base text-gray-900 font-medium">Player</div>
            <div className="text-xs text-gray-500 mt-1">
              lx.audio.createPlayer plays a native queue: two generated tones, then a remote MP3.
            </div>
          </div>
          <div className="px-4 py-4 space-y-3">
            <div className="text-sm text-gray-900 font-medium" data-testid="audio-track">{data?.trackTitle || 'Nothing loaded'}</div>
            <div className="h-1.5 rounded bg-surface-200 overflow-hidden">
              <div className="h-full bg-blue-500" style={{ width: `${Math.round((data?.progress ?? 0) * 100)}%` }} />
            </div>
            <div className="flex justify-between text-xs text-gray-500">
              <span data-testid="audio-position">{data?.positionText || '--:--'}</span>
              <span data-testid="audio-status">{data?.status || 'idle'}</span>
              <span>{data?.durationText || '--:--'}</span>
            </div>
            <div className="grid grid-cols-4 gap-2">
              <button className={secondary} onClick={previous}>Prev</button>
              <button
                data-testid="audio-toggle"
                className="py-2.5 rounded-lg bg-blue-500 text-white text-sm font-medium"
                onClick={togglePlay}
              >
                {playing ? 'Pause' : 'Play'}
              </button>
              <button className={secondary} disabled={!data?.hasNext} onClick={next}>Next</button>
              <button className={secondary} onClick={stop}>Stop</button>
            </div>
            <div className="grid grid-cols-2 gap-2">
              <button className={secondary} onClick={() => seekBy({ deltaMs: -1000 })}>−1 s</button>
              <button className={secondary} onClick={() => seekBy({ deltaMs: 1000 })}>+1 s</button>
            </div>
          </div>
        </div>

        <div className="bg-surface rounded-lg shadow-sm">
          <div className="px-4 py-4 border-b border-line-100">
            <div className="text-sm text-gray-900 font-medium">Queue</div>
          </div>
          <div>
            {(data?.tracks ?? []).map((track, index) => (
              <div
                key={track.id}
                className={`px-4 py-3 text-sm cursor-pointer border-t border-line-100 first:border-t-0 ${index === data?.index ? 'text-blue-600 font-medium' : 'text-gray-700'}`}
                onClick={() => skipTo({ index })}
              >
                {track.title}
              </div>
            ))}
          </div>
        </div>

        <div className="bg-surface rounded-lg shadow-sm">
          <div className="px-4 py-4 border-b border-line-100">
            <div className="text-sm text-gray-900 font-medium">Settings</div>
            <div className="text-xs text-gray-500 mt-1" data-testid="audio-route">
              Route: {data?.routeText || 'unknown'}
            </div>
          </div>
          <div className="px-4 py-4 space-y-3">
            <div className="grid grid-cols-3 gap-2">
              <button className={secondary} onClick={cycleRate}>Rate {data?.rate ?? 1}×</button>
              <button className={secondary} onClick={cycleRepeat}>Repeat {data?.repeat || 'off'}</button>
              <button className={secondary} onClick={toggleMute}>{data?.muted ? 'Unmute' : 'Mute'}</button>
            </div>
            <div className="grid grid-cols-3 gap-2">
              {[0.25, 0.5, 1].map((value) => (
                <button
                  key={value}
                  className={`py-2.5 rounded-lg text-sm font-medium ${volume === value ? 'bg-blue-500 text-white' : 'bg-surface-200 text-gray-900'}`}
                  onClick={() => setVolume({ volume: value })}
                >
                  Volume {Math.round(value * 100)}%
                </button>
              ))}
            </div>
            {data?.earpiece && (
              <button className={`w-full ${secondary}`} onClick={toggleOutput}>
                {data?.output === 'earpiece' ? 'Use speaker' : 'Use earpiece'}
              </button>
            )}
          </div>
        </div>

        <div className="bg-surface rounded-lg shadow-sm">
          <div className="px-4 py-4 border-b border-line-100">
            <div className="text-sm text-gray-900 font-medium">Sound effects</div>
            <div className="text-xs text-gray-500 mt-1">
              lx.audio.loadSound preloads a short beep; each tap is an independent voice that mixes with the player.
            </div>
          </div>
          <div className="px-4 py-4 grid grid-cols-4 gap-2">
            <button disabled={!data?.soundReady} className={secondary} onClick={() => playSound({ pan: -1 })}>Left</button>
            <button
              data-testid="audio-sound"
              disabled={!data?.soundReady}
              className="py-2.5 rounded-lg bg-blue-500 text-white text-sm font-medium disabled:opacity-50"
              onClick={() => playSound({ pan: 0 })}
            >
              Beep
            </button>
            <button disabled={!data?.soundReady} className={secondary} onClick={() => playSound({ pan: 1 })}>Right</button>
            <button className={secondary} onClick={stopSounds}>Stop all</button>
          </div>
        </div>

        <div className="bg-surface rounded-lg shadow-sm">
          <div className="px-4 py-4 border-b border-line-100">
            <div className="text-sm text-gray-900 font-medium">Events</div>
          </div>
          <div className="px-4 py-3 space-y-1 text-xs text-gray-500 font-mono" data-testid="audio-events">
            {(data?.eventLog ?? []).length === 0 ? <div>No events yet</div> : data!.eventLog!.map((line, i) => <div key={i}>{line}</div>)}
          </div>
        </div>
      </div>
    </div>
  );
}
