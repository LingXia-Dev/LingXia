// MARK: - Contract
// Shared player contract types

/**
 * Backend type for the player engine
 */
export enum BackendKind {
  URL = 'url',
  FEED = 'feed'
}

/**
 * Stable error codes for player errors
 */
export enum ErrorCode {
  ABORTED = 'aborted',           // Interrupted by stop/unmount/source_change
  NETWORK = 'network',           // Network/HTTP failure
  TIMEOUT = 'timeout',           // Timeout
  DECODE = 'decode',             // Decode failure
  UNSUPPORTED = 'unsupported',   // Format/codec not supported
  DRM = 'drm',                   // DRM/key failure
  SURFACE = 'surface',           // Surface lost/invalid
  INTERNAL = 'internal',         // Internal error
  UNKNOWN = 'unknown'            // Uncategorized
}

/**
 * Error detail from the engine
 */
export class EngineError {
  /** Stable error code */
  code: ErrorCode;

  /** Debug/report message */
  message: string;

  /** Platform native error code/exception type (optional) */
  nativeCode?: string;

  /** HTTP status for network errors (optional) */
  httpStatus?: number;

  /** Whether retry is recommended (optional) */
  retryable?: boolean;

  /** Which backend produced the error (optional) */
  backend?: BackendKind;

  constructor(
    code: ErrorCode,
    message: string,
    nativeCode?: string,
    httpStatus?: number,
    retryable?: boolean,
    backend?: BackendKind
  ) {
    this.code = code;
    this.message = message;
    this.nativeCode = nativeCode;
    this.httpStatus = httpStatus;
    this.retryable = retryable;
    this.backend = backend;
  }
}

/**
 * Player capabilities snapshot.
 * Fields may be unknown (undefined) until the engine is prepared.
 */
export class PlayerCapabilities {
  /** Whether the content is live (undefined = unknown) */
  isLive?: boolean;

  /** Whether seeking is supported (undefined = unknown) */
  canSeek?: boolean;

  /** Whether duration is available (undefined = unknown) */
  hasDuration?: boolean;

  /** Whether rate change is supported (engine fixed capability) */
  supportsRate: boolean;

  /** Whether quality switching is supported (engine fixed capability) */
  supportsQualities: boolean;

  constructor(
    supportsRate: boolean = false,
    supportsQualities: boolean = false,
    isLive?: boolean,
    canSeek?: boolean,
    hasDuration?: boolean
  ) {
    this.supportsRate = supportsRate;
    this.supportsQualities = supportsQualities;
    this.isLive = isLive;
    this.canSeek = canSeek;
    this.hasDuration = hasDuration;
  }

  copy(
    isLive?: boolean,
    canSeek?: boolean,
    hasDuration?: boolean
  ): PlayerCapabilities {
    return new PlayerCapabilities(
      this.supportsRate,
      this.supportsQualities,
      isLive ?? this.isLive,
      canSeek ?? this.canSeek,
      hasDuration ?? this.hasDuration
    );
  }
}

/**
 * Source type enum
 */
export enum PlayerSourceType {
  URL = 'url',
  FEED = 'feed'
}

/**
 * Source for the player
 */
export class PlayerSource {
  readonly type: PlayerSourceType;
  readonly url?: string;

  private constructor(type: PlayerSourceType, url?: string) {
    this.type = type;
    this.url = url;
  }

  static fromUrl(urlString: string): PlayerSource {
    return new PlayerSource(PlayerSourceType.URL, urlString);
  }

  static feed(): PlayerSource {
    return new PlayerSource(PlayerSourceType.FEED);
  }

  isUrl(): boolean {
    return this.type === PlayerSourceType.URL;
  }

  isFeed(): boolean {
    return this.type === PlayerSourceType.FEED;
  }
}

/**
 * Reason why the player stopped
 */
export enum StopReason {
  USER = 'user',                 // Explicit player.stop() call
  UNMOUNT = 'unmount',           // Component destroyed
  SOURCE_CHANGE = 'source_change' // Source switched
}

/**
 * Video dimensions
 */
export class VideoSize {
  readonly width: number;
  readonly height: number;

  constructor(width: number, height: number) {
    this.width = width;
    this.height = height;
  }
}

/**
 * Reason why the player is in waiting state
 */
export enum WaitingReason {
  INITIAL = 'initial',           // First load
  BUFFERING = 'buffering',       // Buffer underrun
  SEEKING = 'seeking',           // Seek in progress
  SURFACE_REBIND = 'surface_rebind', // Surface rebuild
  QUALITY_SWITCH = 'quality_switch', // Quality switching
  DECODER = 'decoder'            // Decoder not ready
}
