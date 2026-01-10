// MARK: - PlayerCore
// Core state machine for player - owns all event semantics, de-dup, and throttling

import { BackendKind, PlayerCapabilities, PlayerSource, PlayerSourceType, StopReason, WaitingReason } from './Contract';
import {
  EngineEvent,
  EngineEventType,
  EngineListener,
  PreparedEvent,
  BufferingChangedEvent,
  PlayingChangedEvent,
  TimeUpdateEvent as EngineTimeUpdateEvent,
  SeekCompletedEvent as EngineSeekCompletedEvent,
  ErrorEvent as EngineErrorEvent
} from './Engine';
import { PlayerEngine } from './Engine';
import { SurfaceToken } from './SurfaceHost';
import {
  PlayerEvent,
  PlayRequestEvent,
  PlayEvent,
  PlayingEvent,
  PauseEvent,
  WaitingEvent,
  SeekingEvent,
  SeekedEvent,
  TimeUpdateEvent,
  LoadedMetadataEvent,
  EndedEvent,
  ErrorEvent,
  RateChangeEvent,
  VolumeChangeEvent,
  StopEvent
} from './PlayerEvent';

// MARK: - PlaybackState
// Playback state enumeration

/**
 * Current playback state
 */
export enum PlaybackState {
  IDLE = 'idle',
  PLAYING = 'playing',
  PAUSED = 'paused',
  BUFFERING = 'buffering',
  ENDED = 'ended'
}

const TAG = 'LingXia.PlayerCore';
function log(message: string): void { console.info(`${TAG}: ${message}`); }

export type EngineFactory = () => PlayerEngine;
export type EventHandler = (event: PlayerEvent) => void;

/**
 * Core state machine for the player.
 * Responsible for:
 * - Owning the engine and switching between URL/Feed backends
 * - Event semantics, ordering, de-duplication
 * - timeupdate throttling
 * - Enforcing "play only after time advances" rule
 */
export class PlayerCore implements EngineListener {
  private readonly createUrlEngine: EngineFactory;
  private readonly createFeedEngine: EngineFactory;
  private readonly emit: EventHandler;

  private engine: PlayerEngine | null = null;
  private source: PlayerSource | null = null;
  private backend: BackendKind | null = null;
  private surfaceToken: SurfaceToken | null = null;

  private playIntent: boolean = false;
  private hasEmittedPlay: boolean = false;
  private enginePlaying: boolean = false;
  private hasEmittedPlayingSinceLastWaiting: boolean = false;
  private hasFirstFrame: boolean = false;
  private lastKnownTimeMs: number | undefined = undefined;
  private lastKnownDurationMs: number | undefined = undefined;
  private volume: number = 1.0;
  private muted: boolean = false;
  private rate: number = 1.0;

  // Throttling
  private lastTimeUpdateEmitAt: number = 0;
  private lastTimeUpdateValueMs: number | undefined = undefined;
  private pendingSeekTargetMs: number | undefined = undefined;

  private capabilities: PlayerCapabilities = new PlayerCapabilities();

  private lastWaitingReason: WaitingReason | undefined = undefined;
  private isWaiting: boolean = false;
  private hasEnded: boolean = false;

  constructor(
    createUrlEngine: EngineFactory,
    createFeedEngine: EngineFactory,
    emit: EventHandler
  ) {
    this.createUrlEngine = createUrlEngine;
    this.createFeedEngine = createFeedEngine;
    this.emit = emit;
  }

  // --- Public Accessors ---

  getBackend(): BackendKind | null { return this.backend; }
  getCapabilitiesSnapshot(): PlayerCapabilities { return this.capabilities; }
  getLastKnownTimeMs(): number | undefined { return this.lastKnownTimeMs; }
  getLastKnownDurationMs(): number | undefined { return this.lastKnownDurationMs; }

  // --- Surface Management ---

  setSurfaceToken(token: SurfaceToken | null): void {
    const old = this.surfaceToken;
    this.surfaceToken = token;
    const currentEngine = this.engine;
    if (!currentEngine) return;
    if (old) {
      currentEngine.detachSurface(old);
    }
    if (token) {
      currentEngine.attachSurface(token);
    }
  }

  // --- Source Management ---

  setSource(next: PlayerSource | null): void {
    const nextBackend = next ? this.backendOf(next) : null;
    const prevBackend = this.backend;
    const hadSource = this.source !== null;
    const isSourceChanging = this.source !== next;

    if (hadSource && isSourceChanging) {
      this.playIntent = false;
      this.hasEnded = false;
      this.isWaiting = false;
      this.lastWaitingReason = undefined;
      this.emit(new StopEvent(StopReason.SOURCE_CHANGE));
    }

    if (nextBackend !== prevBackend) {
      this.teardownEngine();
      if (nextBackend === BackendKind.URL) {
        this.engine = this.createUrlEngine();
      } else if (nextBackend === BackendKind.FEED) {
        this.engine = this.createFeedEngine();
      } else {
        this.engine = null;
      }
      this.engine?.setListener(this);
      this.backend = nextBackend;
    } else if (hadSource && isSourceChanging) {
      this.engine?.stop();
    }

    this.source = next;
    this.playIntent = false;
    this.hasEnded = false;
    this.enginePlaying = false;
    this.hasEmittedPlay = false;
    this.hasEmittedPlayingSinceLastWaiting = false;
    this.hasFirstFrame = false;
    this.lastKnownTimeMs = undefined;
    this.pendingSeekTargetMs = undefined;
    this.lastTimeUpdateEmitAt = 0;
    this.lastTimeUpdateValueMs = undefined;
    this.isWaiting = false;
    this.lastWaitingReason = undefined;

    const currentEngine = this.engine;
    if (!currentEngine) return;

    currentEngine.setVolume(this.volume);
    currentEngine.setMuted(this.muted);
    currentEngine.setRate(this.rate);

    const surface = this.surfaceToken;
    if (surface) {
      currentEngine.attachSurface(surface);
    }

    if (next) {
      currentEngine.setSource(next);
      this.capabilities = currentEngine.capabilities;
    }
  }

  // --- Playback Control ---

  play(): void {
    this.playIntent = true;
    const currentEngine = this.engine;
    const currentSource = this.source;

    if (!currentEngine || !currentSource) {
      if (this.backend === BackendKind.FEED || this.backend === null) {
        this.emit(new PlayRequestEvent());
        this.emit(new WaitingEvent(WaitingReason.INITIAL, undefined));
      }
      return;
    }

    if (this.hasEnded) {
      this.hasEnded = false;
      // Replay semantics: restart from the beginning.
      this.seekInternal(0, /* resume */ false);
    }

    this.maybeEmitPlay();

    if (this.backend === BackendKind.FEED) {
      this.emit(new PlayRequestEvent());
      this.emit(new WaitingEvent(WaitingReason.INITIAL, undefined));
      this.isWaiting = true;
      this.lastWaitingReason = WaitingReason.INITIAL;
      this.hasEmittedPlayingSinceLastWaiting = false;
    }

    currentEngine.play();
  }

  pause(): void {
    this.playIntent = false;
    this.hasEmittedPlay = false;
    this.enginePlaying = false;
    this.hasEmittedPlayingSinceLastWaiting = false;
    this.hasEnded = false;
    this.engine?.pause();
    this.emit(new PauseEvent(this.lastKnownTimeMs));
  }

  stop(reason: StopReason): void {
    this.playIntent = false;
    this.pendingSeekTargetMs = undefined;
    this.hasEmittedPlay = false;
    this.enginePlaying = false;
    this.hasEmittedPlayingSinceLastWaiting = false;
    this.hasFirstFrame = false;
    this.hasEnded = false;
    this.isWaiting = false;
    this.lastWaitingReason = undefined;
    this.engine?.stop();
    this.emit(new StopEvent(reason));
  }

  seek(targetTimeMs: number): void {
    const shouldResumeAfterSeek = this.playIntent || this.hasEnded;
    if (this.hasEnded) {
      // Seeking after ended is treated as replay intent (dragging the scrubber then releasing).
      this.playIntent = true;
    }
    this.hasEnded = false;
    this.hasEmittedPlayingSinceLastWaiting = false;
    this.pendingSeekTargetMs = targetTimeMs;
    this.emit(new SeekingEvent(targetTimeMs, this.lastKnownTimeMs));
    const currentEngine = this.engine;
    currentEngine?.seek(targetTimeMs);
    if (shouldResumeAfterSeek) {
      currentEngine?.play();
    }
  }

  setDurationMs(durationMs: number | undefined): void {
    this.lastKnownDurationMs = durationMs;
    this.engine?.setDurationMs(durationMs);
  }

  setVolume(volume: number): void {
    this.volume = Math.max(0, Math.min(1, volume));
    this.engine?.setVolume(this.volume);
    this.emit(new VolumeChangeEvent(this.volume, this.muted));
  }

  setMuted(muted: boolean): void {
    this.muted = muted;
    this.engine?.setMuted(muted);
    this.emit(new VolumeChangeEvent(this.volume, this.muted));
  }

  setRate(rate: number): void {
    if (Math.abs(this.rate - rate) < 0.0001) return;
    this.rate = rate;
    this.engine?.setRate(rate);
    this.emit(new RateChangeEvent(rate));
  }

  release(): void {
    this.teardownEngine();
  }

  // --- EngineListener ---

  onEngineEvent(event: EngineEvent): void {
    switch (event.type) {
      case EngineEventType.CAPABILITIES_CHANGED: {
        const e = event as import('./Engine').CapabilitiesChangedEvent;
        this.capabilities = e.capabilities;
        break;
      }

      case EngineEventType.PREPARED: {
        const e = event as PreparedEvent;
        this.lastKnownDurationMs = e.durationMs ?? this.lastKnownDurationMs;
        const size = e.videoSize;
        this.emit(new LoadedMetadataEvent(
          size?.width ?? 0,
          size?.height ?? 0,
          this.lastKnownDurationMs
        ));
        break;
      }

      case EngineEventType.BUFFERING_CHANGED: {
        const e = event as BufferingChangedEvent;
        if (e.isBuffering) {
          if (this.hasEnded && !this.playIntent) return;
          if (!this.isWaiting || this.lastWaitingReason !== e.reason) {
            this.isWaiting = true;
            this.lastWaitingReason = e.reason;
            this.hasEmittedPlayingSinceLastWaiting = false;
            this.emit(new WaitingEvent(e.reason, this.lastKnownTimeMs));
          }
        } else {
          const wasWaiting = this.isWaiting;
          this.isWaiting = false;
          this.lastWaitingReason = undefined;
          if (wasWaiting) {
            this.maybeEmitPlaying();
          }
        }
        break;
      }

      case EngineEventType.FIRST_FRAME_RENDERED:
        this.hasFirstFrame = true;
        this.maybeEmitPlaying();
        break;

      case EngineEventType.PLAYING_CHANGED: {
        const e = event as PlayingChangedEvent;
        this.enginePlaying = e.isPlaying;
        if (e.isPlaying) {
          this.playIntent = true;
          this.isWaiting = false;
          this.lastWaitingReason = undefined;
          this.maybeEmitPlay();
          this.maybeEmitPlaying();
        } else {
          this.hasEmittedPlayingSinceLastWaiting = false;
        }
        break;
      }

      case EngineEventType.TIME_UPDATE: {
        const e = event as EngineTimeUpdateEvent;
        this.lastKnownTimeMs = e.currentTimeMs;
        this.lastKnownDurationMs = e.durationMs ?? this.lastKnownDurationMs;
        this.maybeEmitTimeUpdate(e.currentTimeMs, this.lastKnownDurationMs);
        this.maybeEmitSeekedIfNeeded(e.currentTimeMs);
        this.maybeEmitPlaying();
        break;
      }

      case EngineEventType.SEEK_COMPLETED: {
        const e = event as EngineSeekCompletedEvent;
        this.lastKnownTimeMs = e.currentTimeMs;
        this.maybeEmitSeekedIfNeeded(e.currentTimeMs);
        break;
      }

      case EngineEventType.ENDED:
        this.playIntent = false;
        this.enginePlaying = false;
        this.hasEmittedPlay = false;
        this.hasEmittedPlayingSinceLastWaiting = false;
        this.hasEnded = true;
        this.isWaiting = false;
        this.lastWaitingReason = undefined;
        this.emit(new EndedEvent(this.lastKnownTimeMs));
        break;

      case EngineEventType.ERROR: {
        const e = event as EngineErrorEvent;
        const err = e.error;
        this.emit(new ErrorEvent(
          err.code,
          err.message,
          err.nativeCode,
          err.httpStatus,
          err.retryable,
          err.backend
        ));
        break;
      }
    }
  }

  // --- Private Helpers ---

  private maybeEmitPlaying(): void {
    if (!this.playIntent) return;
    if (!this.enginePlaying) return;
    if (this.hasEmittedPlayingSinceLastWaiting) return;

    const time = this.lastKnownTimeMs;
    if (this.hasFirstFrame || (time !== undefined && time > 0)) {
      this.hasEmittedPlayingSinceLastWaiting = true;
      this.isWaiting = false;
      this.lastWaitingReason = undefined;
      this.emit(new PlayingEvent(time));
    }
  }

  private maybeEmitPlay(): void {
    if (this.hasEmittedPlay) return;
    this.hasEmittedPlay = true;
    this.emit(new PlayEvent());
  }

  private seekInternal(targetTimeMs: number, resume: boolean): void {
    this.hasEnded = false;
    this.hasEmittedPlayingSinceLastWaiting = false;
    this.pendingSeekTargetMs = targetTimeMs;
    this.emit(new SeekingEvent(targetTimeMs, this.lastKnownTimeMs));
    const currentEngine = this.engine;
    currentEngine?.seek(targetTimeMs);
    if (resume) {
      currentEngine?.play();
    }
  }

  private maybeEmitSeekedIfNeeded(currentTimeMs: number): void {
    const target = this.pendingSeekTargetMs;
    if (target === undefined) return;
    const closeEnough = Math.abs(currentTimeMs - target) <= 50;
    if (closeEnough) {
      this.pendingSeekTargetMs = undefined;
      this.emit(new SeekedEvent(currentTimeMs));
    }
  }

  private maybeEmitTimeUpdate(currentTimeMs: number, durationMs: number | undefined): void {
    const now = Date.now();
    const lastSentAt = this.lastTimeUpdateEmitAt;
    const lastSentValue = this.lastTimeUpdateValueMs;

    const valueChanged = lastSentValue === undefined || Math.abs(currentTimeMs - lastSentValue) >= 30;
    const timeOk = now - lastSentAt >= 200; // 200ms = 5Hz

    if (valueChanged && timeOk) {
      this.lastTimeUpdateEmitAt = now;
      this.lastTimeUpdateValueMs = currentTimeMs;
      this.emit(new TimeUpdateEvent(currentTimeMs, durationMs));
    }
  }

  private backendOf(source: PlayerSource): BackendKind {
    return source.isUrl() ? BackendKind.URL : BackendKind.FEED;
  }

  private teardownEngine(): void {
    this.engine?.setListener(null);
    this.engine?.release();
    this.engine = null;
  }
}
