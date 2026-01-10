// MARK: - Engine
// Engine interfaces and events

import { EngineError, PlayerCapabilities, PlayerSource, VideoSize, WaitingReason } from './Contract';
import { SurfaceToken } from './SurfaceHost';

/**
 * Engine event types
 */
export enum EngineEventType {
  PREPARED = 'prepared',
  CAPABILITIES_CHANGED = 'capabilitiesChanged',
  BUFFERING_CHANGED = 'bufferingChanged',
  PLAYING_CHANGED = 'playingChanged',
  TIME_UPDATE = 'timeUpdate',
  SEEK_COMPLETED = 'seekCompleted',
  FIRST_FRAME_RENDERED = 'firstFrameRendered',
  ENDED = 'ended',
  ERROR = 'error'
}

/**
 * Base class for engine events
 */
export class EngineEvent {
  type: EngineEventType;
  constructor(type: EngineEventType) {
    this.type = type;
  }
}

export class PreparedEvent extends EngineEvent {
  durationMs?: number;
  videoSize?: VideoSize;
  constructor(durationMs?: number, videoSize?: VideoSize) {
    super(EngineEventType.PREPARED);
    this.durationMs = durationMs;
    this.videoSize = videoSize;
  }
}

export class CapabilitiesChangedEvent extends EngineEvent {
  capabilities: PlayerCapabilities;
  constructor(capabilities: PlayerCapabilities) {
    super(EngineEventType.CAPABILITIES_CHANGED);
    this.capabilities = capabilities;
  }
}

export class BufferingChangedEvent extends EngineEvent {
  isBuffering: boolean;
  reason: WaitingReason;
  constructor(isBuffering: boolean, reason: WaitingReason) {
    super(EngineEventType.BUFFERING_CHANGED);
    this.isBuffering = isBuffering;
    this.reason = reason;
  }
}

export class PlayingChangedEvent extends EngineEvent {
  isPlaying: boolean;
  constructor(isPlaying: boolean) {
    super(EngineEventType.PLAYING_CHANGED);
    this.isPlaying = isPlaying;
  }
}

export class TimeUpdateEvent extends EngineEvent {
  currentTimeMs: number;
  durationMs?: number;
  constructor(currentTimeMs: number, durationMs?: number) {
    super(EngineEventType.TIME_UPDATE);
    this.currentTimeMs = currentTimeMs;
    this.durationMs = durationMs;
  }
}

export class SeekCompletedEvent extends EngineEvent {
  currentTimeMs: number;
  constructor(currentTimeMs: number) {
    super(EngineEventType.SEEK_COMPLETED);
    this.currentTimeMs = currentTimeMs;
  }
}

export class FirstFrameRenderedEvent extends EngineEvent {
  constructor() {
    super(EngineEventType.FIRST_FRAME_RENDERED);
  }
}

export class EndedEvent extends EngineEvent {
  constructor() {
    super(EngineEventType.ENDED);
  }
}

export class ErrorEvent extends EngineEvent {
  error: EngineError;
  constructor(error: EngineError) {
    super(EngineEventType.ERROR);
    this.error = error;
  }
}

/**
 * Listener interface for engine events
 */
export interface EngineListener {
  onEngineEvent(event: EngineEvent): void;
}

/**
 * Protocol for player engines (URL/Feed).
 * All backend-specific logic (time base, seek, buffering, ended judgment) must stay inside the engine.
 */
export interface PlayerEngine {
  /**
   * Last known capabilities snapshot. Any field may be unknown (undefined) until prepared.
   * Must be safe to read at any time.
   */
  readonly capabilities: PlayerCapabilities;

  /** Sets the engine event listener. */
  setListener(listener: EngineListener | null): void;

  /** Sets the source for playback. */
  setSource(source: PlayerSource): void;

  /** Attaches a surface for rendering. */
  attachSurface(token: SurfaceToken): void;

  /** Detaches a surface from rendering. */
  detachSurface(token: SurfaceToken): void;

  /** Starts playback. */
  play(): void;

  /** Pauses playback. */
  pause(): void;

  /** Stops playback and resets state. */
  stop(): void;

  /** Seeks to the specified position. */
  seek(positionMs: number): void;

  /** Sets the duration (for feed engines where duration is provided externally). */
  setDurationMs(durationMs: number | undefined): void;

  /** Sets the volume (0.0 to 1.0). */
  setVolume(volume: number): void;

  /** Sets the muted state. */
  setMuted(muted: boolean): void;

  /** Sets the playback rate. */
  setRate(rate: number): void;

  /** Gets the current playback position in milliseconds. */
  getCurrentTimeMs(): number;

  /** Gets the duration in milliseconds, if known. */
  getDurationMs(): number | undefined;

  /** Returns whether the engine is currently playing. */
  isPlaying(): boolean;

  /** Releases all resources. */
  release(): void;
}
