// MARK: - UrlEngine
// AVPlayer-based engine for URL playback via Rust FFI

import {
  videoPlayerCreate,
  videoPlayerSetUrl,
  videoPlayerSetSurface,
  videoPlayerPrepare,
  videoPlayerPlay,
  videoPlayerPause,
  videoPlayerStop,
  videoPlayerSeek,
  videoPlayerSetVolume,
  videoPlayerSetLoop,
  videoPlayerSetSpeed,
  videoPlayerGetCurrentTime,
  videoPlayerGetDuration,
  videoPlayerGetVideoWidth,
  videoPlayerGetVideoHeight,
  videoPlayerDestroy
} from 'liblingxia.so';

import { BackendKind, EngineError, ErrorCode, PlayerCapabilities, PlayerSource, VideoSize, WaitingReason } from './Contract';
import {
  EngineListener,
  PreparedEvent,
  CapabilitiesChangedEvent,
  BufferingChangedEvent,
  PlayingChangedEvent,
  TimeUpdateEvent,
  SeekCompletedEvent,
  FirstFrameRenderedEvent,
  EndedEvent,
  ErrorEvent
} from './Engine';
import { PlayerEngine } from './Engine';
import { SurfaceToken } from './SurfaceHost';

const TAG = 'LingXia.UrlEngine';
function log(message: string): void { console.info(`${TAG}: ${message}`); }

/**
 * Engine for URL-based playback using AVPlayer via Rust FFI
 */
export class UrlEngine implements PlayerEngine {
  private readonly componentId: string;
  private listener: EngineListener | null = null;
  private _capabilities: PlayerCapabilities = new PlayerCapabilities(true, true);

  private playerCreated: boolean = false;
  private boundSurfaceId: string = '';
  private volume: number = 1.0;
  private muted: boolean = false;
  private playing: boolean = false;
  private hasRenderedFirstFrame: boolean = false;
  private progressTimer: number = -1;
  private loop: boolean = false;
  private rate: number = 1.0;

  get capabilities(): PlayerCapabilities {
    return this._capabilities;
  }

  constructor(componentId: string) {
    this.componentId = componentId;
  }

  setListener(listener: EngineListener | null): void {
    this.listener = listener;
  }

  setSource(source: PlayerSource): void {
    if (!source.isUrl() || !source.url) return;

    this.cleanup();
    this.destroyPlayer();
    this.playing = false;
    this.hasRenderedFirstFrame = false;

    const playerPtr = videoPlayerCreate(this.componentId, 0) as number;
    if (playerPtr === 0) {
      this.listener?.onEngineEvent(new ErrorEvent(
        new EngineError(ErrorCode.INTERNAL, 'Failed to create native player', undefined, undefined, undefined, BackendKind.URL)
      ));
      return;
    }
    this.playerCreated = true;

    if (!(videoPlayerSetUrl(this.componentId, source.url) as boolean)) {
      this.listener?.onEngineEvent(new ErrorEvent(
        new EngineError(ErrorCode.INTERNAL, 'Failed to set URL source', undefined, undefined, undefined, BackendKind.URL)
      ));
      this.destroyPlayer();
      return;
    }

    if (this.boundSurfaceId) {
      videoPlayerSetSurface(this.componentId, this.boundSurfaceId);
    }

    this.applyAudioState();
    videoPlayerSetLoop(this.componentId, this.loop);

    videoPlayerPrepare(this.componentId);
  }

  attachSurface(token: SurfaceToken): void {
    this.boundSurfaceId = token.id;
    if (this.playerCreated) {
      videoPlayerSetSurface(this.componentId, token.id);
    }
  }

  detachSurface(token: SurfaceToken): void {
    // Keep surface connected for URL playback
  }

  play(): void {
    this.playing = true;
    if (this.playerCreated) {
      videoPlayerPlay(this.componentId);
      videoPlayerSetSpeed(this.componentId, this.rate);
    }
  }

  pause(): void {
    this.playing = false;
    if (this.playerCreated) {
      videoPlayerPause(this.componentId);
    }
    this.stopProgressTimer();
  }

  stop(): void {
    this.playing = false;
    if (this.playerCreated) {
      videoPlayerStop(this.componentId);
    }
    this.stopProgressTimer();
  }

  seek(positionMs: number): void {
    if (this.playerCreated) {
      videoPlayerSeek(this.componentId, Math.round(positionMs));
    }
  }

  setDurationMs(durationMs: number | undefined): void {
    // URL engine duration is driven by the media pipeline
  }

  setVolume(volume: number): void {
    this.volume = Math.max(0, Math.min(1, volume));
    this.applyAudioState();
  }

  setMuted(muted: boolean): void {
    this.muted = muted;
    this.applyAudioState();
  }

  setRate(rate: number): void {
    this.rate = rate;
    if (this.playerCreated && this.playing) {
      videoPlayerSetSpeed(this.componentId, rate);
    }
  }

  getCurrentTimeMs(): number {
    if (!this.playerCreated) return 0;
    return videoPlayerGetCurrentTime(this.componentId) as number;
  }

  getDurationMs(): number | undefined {
    if (!this.playerCreated) return undefined;
    const duration = videoPlayerGetDuration(this.componentId) as number;
    return duration > 0 ? duration : undefined;
  }

  isPlaying(): boolean {
    return this.playing;
  }

  release(): void {
    this.cleanup();
    this.destroyPlayer();
  }

  // --- Native event handlers (called from NativeBridge) ---

  handlePrepared(): void {
    const durationMs = this.getDurationMs();
    const width = videoPlayerGetVideoWidth(this.componentId) as number;
    const height = videoPlayerGetVideoHeight(this.componentId) as number;
    const videoSize = (width > 0 && height > 0) ? new VideoSize(width, height) : undefined;

    this.updateCapabilities(durationMs);
    this.listener?.onEngineEvent(new PreparedEvent(durationMs, videoSize));
    this.listener?.onEngineEvent(new BufferingChangedEvent(false, WaitingReason.INITIAL));
  }

  handlePlaying(): void {
    this.playing = true;
    this.listener?.onEngineEvent(new PlayingChangedEvent(true));
    this.startProgressTimer();
  }

  handlePaused(): void {
    this.playing = false;
    this.listener?.onEngineEvent(new PlayingChangedEvent(false));
    this.stopProgressTimer();
  }

  handleBufferingChange(isBuffering: boolean): void {
    this.listener?.onEngineEvent(new BufferingChangedEvent(isBuffering, WaitingReason.BUFFERING));
  }

  handleSeekDone(positionMs: number): void {
    this.listener?.onEngineEvent(new SeekCompletedEvent(positionMs));
  }

  handleEnded(): void {
    this.playing = false;
    this.stopProgressTimer();
    this.listener?.onEngineEvent(new EndedEvent());
  }

  handleError(code: string, message: string): void {
    this.listener?.onEngineEvent(new ErrorEvent(
      new EngineError(
        this.mapErrorCode(code),
        message,
        code,
        undefined,
        undefined,
        BackendKind.URL
      )
    ));
  }

  // --- Private helpers ---

  private applyAudioState(): void {
    if (this.playerCreated) {
      videoPlayerSetVolume(this.componentId, this.muted ? 0 : this.volume);
    }
  }

  private updateCapabilities(durationMs?: number): void {
    const hasDuration = durationMs !== undefined && durationMs > 0;
    this._capabilities = this._capabilities.copy(
      !hasDuration, // isLive
      hasDuration,  // canSeek
      hasDuration   // hasDuration
    );
    this.listener?.onEngineEvent(new CapabilitiesChangedEvent(this._capabilities));
  }

  private startProgressTimer(): void {
    if (this.progressTimer !== -1) return;

    this.progressTimer = setInterval(() => {
      if (!this.playing) {
        this.stopProgressTimer();
        return;
      }

      const currentTimeMs = this.getCurrentTimeMs();
      const durationMs = this.getDurationMs();

      this.listener?.onEngineEvent(new TimeUpdateEvent(currentTimeMs, durationMs));

      // Check for first frame
      if (!this.hasRenderedFirstFrame && currentTimeMs > 0) {
        this.hasRenderedFirstFrame = true;
        this.listener?.onEngineEvent(new FirstFrameRenderedEvent());
      }
    }, 100) as number;
  }

  private stopProgressTimer(): void {
    if (this.progressTimer !== -1) {
      clearInterval(this.progressTimer);
      this.progressTimer = -1;
    }
  }

  private cleanup(): void {
    this.stopProgressTimer();
    this.hasRenderedFirstFrame = false;
  }

  private destroyPlayer(): void {
    if (!this.playerCreated) return;
    videoPlayerDestroy(this.componentId);
    this.playerCreated = false;
  }

  private mapErrorCode(raw: string): ErrorCode {
    switch (raw) {
      case 'network': return ErrorCode.NETWORK;
      case 'timeout': return ErrorCode.TIMEOUT;
      case 'decode': return ErrorCode.DECODE;
      case 'unsupported': return ErrorCode.UNSUPPORTED;
      case 'drm': return ErrorCode.DRM;
      case 'surface': return ErrorCode.SURFACE;
      case 'aborted': return ErrorCode.ABORTED;
      default: return ErrorCode.UNKNOWN;
    }
  }
}

// Engine registry for NativeBridge routing
const engineRegistry = new Map<string, UrlEngine>();

export function registerUrlEngine(componentId: string, engine: UrlEngine): void {
  engineRegistry.set(componentId, engine);
}

export function unregisterUrlEngine(componentId: string): void {
  engineRegistry.delete(componentId);
}

export function getUrlEngine(componentId: string): UrlEngine | undefined {
  return engineRegistry.get(componentId);
}
