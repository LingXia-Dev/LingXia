// MARK: - PlayerEvent
// Player events to JS - aligned with PLAYER_EVENTS.md

import { BackendKind, ErrorCode, StopReason, WaitingReason } from './Contract';

/**
 * Player event types
 */
export enum PlayerEventType {
  PLAY_REQUEST = 'playRequest',
  PLAY = 'play',
  PLAYING = 'playing',
  PAUSE = 'pause',
  WAITING = 'waiting',
  SEEKING = 'seeking',
  SEEKED = 'seeked',
  TIME_UPDATE = 'timeUpdate',
  LOADED_METADATA = 'loadedMetadata',
  ENDED = 'ended',
  ERROR = 'error',
  RATE_CHANGE = 'rateChange',
  VOLUME_CHANGE = 'volumeChange',
  STOP = 'stop',
  FULLSCREEN_CHANGE = 'fullscreenChange'
}

/**
 * Base class for player events
 */
export class PlayerEvent {
  type: PlayerEventType;
  constructor(type: PlayerEventType) {
    this.type = type;
  }
}

export class PlayRequestEvent extends PlayerEvent {
  constructor() {
    super(PlayerEventType.PLAY_REQUEST);
  }
}

export class PlayEvent extends PlayerEvent {
  constructor() {
    super(PlayerEventType.PLAY);
  }
}

export class PlayingEvent extends PlayerEvent {
  currentTimeMs?: number;
  constructor(currentTimeMs?: number) {
    super(PlayerEventType.PLAYING);
    this.currentTimeMs = currentTimeMs;
  }
}

export class PauseEvent extends PlayerEvent {
  currentTimeMs?: number;
  constructor(currentTimeMs?: number) {
    super(PlayerEventType.PAUSE);
    this.currentTimeMs = currentTimeMs;
  }
}

export class WaitingEvent extends PlayerEvent {
  reason: WaitingReason;
  currentTimeMs?: number;
  constructor(reason: WaitingReason, currentTimeMs?: number) {
    super(PlayerEventType.WAITING);
    this.reason = reason;
    this.currentTimeMs = currentTimeMs;
  }
}

export class SeekingEvent extends PlayerEvent {
  targetTimeMs: number;
  currentTimeMs?: number;
  constructor(targetTimeMs: number, currentTimeMs?: number) {
    super(PlayerEventType.SEEKING);
    this.targetTimeMs = targetTimeMs;
    this.currentTimeMs = currentTimeMs;
  }
}

export class SeekedEvent extends PlayerEvent {
  currentTimeMs: number;
  constructor(currentTimeMs: number) {
    super(PlayerEventType.SEEKED);
    this.currentTimeMs = currentTimeMs;
  }
}

export class TimeUpdateEvent extends PlayerEvent {
  currentTimeMs: number;
  durationMs?: number;
  constructor(currentTimeMs: number, durationMs?: number) {
    super(PlayerEventType.TIME_UPDATE);
    this.currentTimeMs = currentTimeMs;
    this.durationMs = durationMs;
  }
}

export class LoadedMetadataEvent extends PlayerEvent {
  durationMs?: number;
  width: number;
  height: number;
  constructor(width: number, height: number, durationMs?: number) {
    super(PlayerEventType.LOADED_METADATA);
    this.durationMs = durationMs;
    this.width = width;
    this.height = height;
  }
}

export class EndedEvent extends PlayerEvent {
  currentTimeMs?: number;
  constructor(currentTimeMs?: number) {
    super(PlayerEventType.ENDED);
    this.currentTimeMs = currentTimeMs;
  }
}

export class ErrorEvent extends PlayerEvent {
  code: ErrorCode;
  message: string;
  nativeCode?: string;
  httpStatus?: number;
  retryable?: boolean;
  backend?: BackendKind;
  constructor(code: ErrorCode, message: string, nativeCode?: string, httpStatus?: number, retryable?: boolean, backend?: BackendKind) {
    super(PlayerEventType.ERROR);
    this.code = code;
    this.message = message;
    this.nativeCode = nativeCode;
    this.httpStatus = httpStatus;
    this.retryable = retryable;
    this.backend = backend;
  }
}

export class RateChangeEvent extends PlayerEvent {
  rate: number;
  constructor(rate: number) {
    super(PlayerEventType.RATE_CHANGE);
    this.rate = rate;
  }
}

export class VolumeChangeEvent extends PlayerEvent {
  volume: number;
  muted: boolean;
  constructor(volume: number, muted: boolean) {
    super(PlayerEventType.VOLUME_CHANGE);
    this.volume = volume;
    this.muted = muted;
  }
}

export class StopEvent extends PlayerEvent {
  reason: StopReason;
  constructor(reason: StopReason) {
    super(PlayerEventType.STOP);
    this.reason = reason;
  }
}

export class FullscreenChangeEvent extends PlayerEvent {
  fullscreen: boolean;
  constructor(fullscreen: boolean) {
    super(PlayerEventType.FULLSCREEN_CHANGE);
    this.fullscreen = fullscreen;
  }
}

/**
 * Get event name for JS
 */
export function getEventName(event: PlayerEvent): string {
  switch (event.type) {
    case PlayerEventType.PLAY_REQUEST: return 'playrequest';
    case PlayerEventType.PLAY: return 'play';
    case PlayerEventType.PLAYING: return 'playing';
    case PlayerEventType.PAUSE: return 'pause';
    case PlayerEventType.WAITING: return 'waiting';
    case PlayerEventType.SEEKING: return 'seeking';
    case PlayerEventType.SEEKED: return 'seeked';
    case PlayerEventType.TIME_UPDATE: return 'timeupdate';
    case PlayerEventType.LOADED_METADATA: return 'loadedmetadata';
    case PlayerEventType.ENDED: return 'ended';
    case PlayerEventType.ERROR: return 'error';
    case PlayerEventType.RATE_CHANGE: return 'ratechange';
    case PlayerEventType.VOLUME_CHANGE: return 'volumechange';
    case PlayerEventType.STOP: return 'stop';
    case PlayerEventType.FULLSCREEN_CHANGE: return 'fullscreenchange';
    default: return 'unknown';
  }
}
