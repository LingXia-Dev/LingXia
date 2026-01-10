// MARK: - FeedEngine
// StreamDecoder-based engine for feed/stream playback

import {
  videoPlayerPlay,
  videoPlayerPause,
  videoPlayerSeek,
  videoPlayerStoreSurface,
  videoPlayerRebindStreamSurface,
  streamPlaybackPositionSeconds
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

const TAG = 'LingXia.FeedEngine';
function log(message: string): void { console.info(`${TAG}: ${message}`); }

/**
 * Engine for feed-based playback using StreamDecoder
 */
export class FeedEngine implements PlayerEngine {
  private readonly componentId: string;
  private listener: EngineListener | null = null;
  private _capabilities: PlayerCapabilities = new PlayerCapabilities(false, false);

  private baseOffsetMs: number = 0;
  private durationMs: number | undefined = undefined;
  private playing: boolean = false;
  private lastAttachedToken: SurfaceToken | null = null;
  private lastTimeMs: number = 0;
  private lastVideoSize: VideoSize | undefined = undefined;
  private pollingTimer: number = -1;
  private endStallCount: number = 0;
  private readonly endToleranceMs: number = 250;

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
    // Feed is keyed by componentId (frames/configs are pushed from Rust via ComponentRouter)
    this.baseOffsetMs = 0;
    this.lastTimeMs = 0;
    this.playing = false;
    this.endStallCount = 0;
    this.updateCapabilities();
    this.listener?.onEngineEvent(new PreparedEvent(this.durationMs, this.lastVideoSize));
  }

  attachSurface(token: SurfaceToken): void {
    this.lastAttachedToken = token;
    videoPlayerStoreSurface(this.componentId, token.id);
    videoPlayerRebindStreamSurface(this.componentId, token.id);
  }

  detachSurface(token: SurfaceToken): void {
    const current = this.lastAttachedToken;
    if (!current) return;
    if (current.ownerKey !== token.ownerKey || current.generation !== token.generation) return;
    this.lastAttachedToken = null;
  }

  play(): void {
    this.playing = true;
    this.endStallCount = 0;
    this.startPolling();
    videoPlayerPlay(this.componentId);
  }

  pause(): void {
    this.playing = false;
    this.stopPolling();
    this.endStallCount = 0;
    videoPlayerPause(this.componentId);
  }

  stop(): void {
    this.playing = false;
    this.baseOffsetMs = 0;
    this.lastTimeMs = 0;
    this.stopPolling();
    this.endStallCount = 0;
  }

  seek(positionMs: number): void {
    this.baseOffsetMs = Math.max(0, positionMs);
    this.lastTimeMs = this.baseOffsetMs;
    this.endStallCount = 0;
    this.listener?.onEngineEvent(new BufferingChangedEvent(true, WaitingReason.SEEKING));
    videoPlayerSeek(this.componentId, Math.round(positionMs));
    this.startPolling();
  }

  setDurationMs(durationMs: number | undefined): void {
    this.durationMs = durationMs;
    this.endStallCount = 0;
    this.updateCapabilities();
    this.listener?.onEngineEvent(new PreparedEvent(durationMs, this.lastVideoSize));
  }

  setVolume(volume: number): void {
    // Volume control via stream decoder command
  }

  setMuted(muted: boolean): void {
    // Mute control via stream decoder command
  }

  setRate(rate: number): void {
    // Not supported by decoder pipeline today
  }

  getCurrentTimeMs(): number {
    return this.lastTimeMs;
  }

  getDurationMs(): number | undefined {
    return this.durationMs;
  }

  isPlaying(): boolean {
    return this.playing;
  }

  release(): void {
    this.listener = null;
    this.stopPolling();
    this.endStallCount = 0;
  }

  // --- Stream decoder event handlers (called from NativeBridge) ---

  handleStreamDecoderEvent(event: string, detail: Record<string, Object>): void {
    switch (event) {
      case 'loadedmetadata': {
        const w = (detail['width'] as number) ?? 0;
        const h = (detail['height'] as number) ?? 0;
        this.lastVideoSize = (w > 0 && h > 0) ? new VideoSize(w, h) : undefined;
        this.listener?.onEngineEvent(new PreparedEvent(this.durationMs, this.lastVideoSize));
        break;
      }
      case 'waiting': {
        const reason = this.mapWaitingReason(detail['reason'] as string);
        const d = this.durationMs;
        const atEnd = d !== undefined && d > 0 && this.lastTimeMs >= Math.max(0, d - this.endToleranceMs);
        if (this.playing && reason === WaitingReason.DECODER && atEnd) {
          this.playing = false;
          this.stopPolling();
          this.endStallCount = 0;
          this.listener?.onEngineEvent(new EndedEvent());
          videoPlayerPause(this.componentId);
          return;
        }
        this.listener?.onEngineEvent(new BufferingChangedEvent(true, reason));
        break;
      }
      case 'playing': {
        this.playing = true;
        this.listener?.onEngineEvent(new PlayingChangedEvent(true));
        this.listener?.onEngineEvent(new FirstFrameRenderedEvent());
        this.listener?.onEngineEvent(new BufferingChangedEvent(false, WaitingReason.BUFFERING));
        this.startPolling();
        break;
      }
      case 'pause': {
        this.playing = false;
        this.listener?.onEngineEvent(new PlayingChangedEvent(false));
        this.stopPolling();
        this.endStallCount = 0;
        break;
      }
      case 'stop': {
        this.playing = false;
        this.listener?.onEngineEvent(new PlayingChangedEvent(false));
        this.stopPolling();
        this.endStallCount = 0;
        break;
      }
      case 'ended': {
        this.playing = false;
        this.stopPolling();
        this.endStallCount = 0;
        this.listener?.onEngineEvent(new EndedEvent());
        break;
      }
      case 'error': {
        const code = (detail['code'] as string) ?? '';
        const message = (detail['message'] as string) ?? 'Decoder error';
        this.listener?.onEngineEvent(new ErrorEvent(
          new EngineError(
            this.mapErrorCode(code),
            message,
            code,
            undefined,
            undefined,
            BackendKind.FEED
          )
        ));
        break;
      }
    }
  }

  // --- Private helpers ---

  private startPolling(): void {
    if (this.pollingTimer !== -1) return;

    this.pollingTimer = setInterval(() => {
      if (!this.playing) {
        this.stopPolling();
        return;
      }

      const relSeconds = streamPlaybackPositionSeconds(this.componentId) as number | null;
      if (relSeconds === null || relSeconds === undefined) {
        this.listener?.onEngineEvent(new BufferingChangedEvent(true, WaitingReason.DECODER));
        return;
      }

      const relMs = Math.max(0, Math.floor(relSeconds * 1000));
      const rawMs = this.baseOffsetMs + relMs;
      const duration = this.durationMs;
      const currentMs = (duration !== undefined && duration > 0) ? Math.min(rawMs, duration) : rawMs;
      const prevMs = this.lastTimeMs;
      this.lastTimeMs = currentMs;

      this.listener?.onEngineEvent(new TimeUpdateEvent(currentMs, this.durationMs));

      // Check for ended
      const atEnd = duration !== undefined && duration > 0 && currentMs >= Math.max(0, duration - this.endToleranceMs);
      if (atEnd) {
        const stalled = Math.abs(currentMs - prevMs) <= 1;
        this.endStallCount = stalled ? this.endStallCount + 1 : 0;
        if (this.endStallCount >= 4) {
          this.playing = false;
          this.stopPolling();
          this.listener?.onEngineEvent(new EndedEvent());
          videoPlayerPause(this.componentId);
          return;
        }
      } else {
        this.endStallCount = 0;
      }
    }, 50) as number;
  }

  private stopPolling(): void {
    if (this.pollingTimer !== -1) {
      clearInterval(this.pollingTimer);
      this.pollingTimer = -1;
    }
  }

  private updateCapabilities(): void {
    const hasDuration = this.durationMs !== undefined && this.durationMs > 0;
    this._capabilities = this._capabilities.copy(
      false,       // isLive
      hasDuration, // canSeek
      hasDuration  // hasDuration
    );
    this.listener?.onEngineEvent(new CapabilitiesChangedEvent(this._capabilities));
  }

  private mapWaitingReason(raw: string | undefined): WaitingReason {
    switch (raw) {
      case 'initial': return WaitingReason.INITIAL;
      case 'buffering': return WaitingReason.BUFFERING;
      case 'seeking': return WaitingReason.SEEKING;
      case 'surface_rebind': return WaitingReason.SURFACE_REBIND;
      case 'quality_switch': return WaitingReason.QUALITY_SWITCH;
      case 'decoder': return WaitingReason.DECODER;
      default: return WaitingReason.DECODER;
    }
  }

  private mapErrorCode(raw: string): ErrorCode {
    switch (raw) {
      case 'network': return ErrorCode.NETWORK;
      case 'timeout': return ErrorCode.TIMEOUT;
      case 'decode':
      case 'stream_decoder': return ErrorCode.DECODE;
      case 'unsupported': return ErrorCode.UNSUPPORTED;
      case 'drm': return ErrorCode.DRM;
      case 'surface': return ErrorCode.SURFACE;
      case 'aborted': return ErrorCode.ABORTED;
      default: return ErrorCode.UNKNOWN;
    }
  }
}

// Engine registry for NativeBridge routing
const feedEngineRegistry = new Map<string, FeedEngine>();

export function registerFeedEngine(componentId: string, engine: FeedEngine): void {
  feedEngineRegistry.set(componentId, engine);
}

export function unregisterFeedEngine(componentId: string): void {
  feedEngineRegistry.delete(componentId);
}

export function getFeedEngine(componentId: string): FeedEngine | undefined {
  return feedEngineRegistry.get(componentId);
}
