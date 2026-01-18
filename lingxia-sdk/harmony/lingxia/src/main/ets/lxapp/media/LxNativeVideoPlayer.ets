import {
  videoPlayerCreate,
  videoPlayerSetUrl,
  videoPlayerSetSurface,
  videoPlayerStoreSurface,
  videoPlayerClearSurface,
  videoPlayerRebindStreamSurface,
  videoPlayerPrepare,
  videoPlayerPlay,
  videoPlayerPause,
  videoPlayerStop,
  videoPlayerSeek,
  videoPlayerRebindSurface,
  videoPlayerSetVolume,
  videoPlayerSetLoop,
  videoPlayerSetSpeed,
  videoPlayerGetCurrentTime,
  videoPlayerGetDuration,
  videoPlayerGetVideoWidth,
  videoPlayerGetVideoHeight,
  videoPlayerDestroy
} from 'liblingxia.so';
import { calculateSliderValue, calculateTimeFromSlider } from './VideoUtils';
import { VideoControlsView, VideoControlSize } from './VideoControls';
import { resolveLxUriForFileAccess } from './LxUri';

const TAG = 'LingXia.NativeVideoPlayer';
function log(message: string): void { console.info(`${TAG}: ${message}`); }
type StreamPlaybackState = 'idle' | 'waiting' | 'playing' | 'ended';
type NativePlayerEvent =
  | 'seekDone'
  | 'error'
  | 'buffering'
  | 'prepared'
  | 'stop'
  | 'playing'
  | 'pause'
  | 'ended'
  | 'waiting'
  | 'streamConfig'
  | 'duration'
  | 'enterFullscreen'
  | 'exitFullscreen';

interface StreamVideoConfig {
  width?: number;
  height?: number;
}

// Event detail types
// NOTE: All time values in event details use SECONDS (not milliseconds) for cross-platform Rust callback contract.
// Internal UI state (state.currentTime, sliderValue) uses milliseconds for precision.
export class VideoErrorDetail { code: string = ''; message: string = ''; }
export class VideoSeekingDetail { currentTime: number = 0; } // seconds
export class VideoFullscreenDetail {
  fullScreen: boolean = false;
  direction: string = 'horizontal';
}
export class VideoMetadataDetail {
  width: number = 0;
  height: number = 0;
  duration: number = 0; // seconds
}
export class VideoTimeUpdateDetail {
  currentTime: number = 0; // seconds
  duration: number = 0;    // seconds
}
export class VideoEmptyDetail {}
export class VideoWaitingDetail { reason: string = ''; }
export class VideoControlDetail { reason: string = ''; currentTime: number = 0; } // seconds
export class LxMediaQualityChangeDetail {
  quality: string = '';
  url?: string = '';
}
export class LxMediaRateChangeDetail {
  rate: number = 1.0;
}

interface NativeErrorPayload {
  code?: string;
  message?: string;
}

export type VideoEventDetail =
  | VideoErrorDetail
  | VideoSeekingDetail
  | VideoFullscreenDetail
  | VideoMetadataDetail
  | VideoTimeUpdateDetail
  | VideoEmptyDetail
  | VideoWaitingDetail
  | VideoControlDetail
  | LxMediaQualityChangeDetail
  | LxMediaRateChangeDetail;

// Quality structure
export class LxMediaQuality {
  label: string = '';
  url?: string = '';
}

// Configuration
export class LxNativeVideoConfig {
  src: string = '';
  poster?: string;
  autoplay: boolean = false;
  loop: boolean = false;
  muted: boolean = false;
  volume: number = 1.0;
  controls: boolean = true;
  progressBar: boolean = true;
  cornerRadius: number = 0;
  objectFit: string = 'contain';
  qualities: LxMediaQuality[] = []; // Available quality options
  playbackRates: number[] = [];     // Available playback rates
}

// State
export class LxNativeVideoState {
  isPlaying: boolean = false;
  isPaused: boolean = false;
  isBuffering: boolean = false;
  isFullscreen: boolean = false;
  currentTime: number = 0;
  duration: number = 0;
  bufferedTime: number = 0;
  videoWidth: number = 0;
  videoHeight: number = 0;
  error?: string;
  currentQuality: string = '';       // Currently selected quality
  currentPlaybackRate: number = 1.0; // Current playback rate
}

class StreamVideoSize {
  width: number = 0;
  height: number = 0;

  constructor(width: number, height: number) {
    this.width = width;
    this.height = height;
  }
}

class LxNativeVideoRuntimeSnapshot {
  isPlaying: boolean = false;
  isBuffering: boolean = false;
  currentTime: number = 0;
  duration: number = 0;
  hasRenderedFirstFrame: boolean = false;
  hasEverPlayed: boolean = false;
}

@Component
export struct LxNativeVideoPlayer {
  @Prop playerId: string = '';
  @Prop config: LxNativeVideoConfig = new LxNativeVideoConfig();
  @State state: LxNativeVideoState = new LxNativeVideoState();
  @State showControls: boolean = false;
  @State isFullscreen: boolean = false;
  @State private appliedCornerRadius: number = 0;
  @State private isSeeking: boolean = false;
  @State private sliderValue: number = 0;
  @State private hasRenderedFirstFrame: boolean = false;  // Poster shows only when this is false
  private hasEverPlayed: boolean = false; // Persistent lock for poster
  private cachedPoster: string = '';  // Cache poster URL since config.poster may be lost on rebuild
  private cachedAutoplay: boolean = false;  // Cache autoplay since config may be lost on rebuild
  private autoplaySuppressed: boolean = false;  // Block autoplay if paused before prepare
  private isPausedByUser: boolean = false;
  private lastLoadingIndicatorShown: boolean = false;
  private lastSeekTimestamp: number = 0;
  private wasPlayingOnDragStart: boolean = false;
  private pendingSeekTime: number = -1;
  private pendingSeekStartTime: number = -1;
  private seekDebounceTimer: number = -1;  // Debounce rapid seeks
  @State private hasEnded: boolean = false;  // Track if video has ended - @State for poster UI binding
  private pendingPlayEvent: boolean = false;  // Delay playing event until video actually starts
  private lastTimeForPlayEvent: number = -1;  // Track time progression for playing event
  private streamState: StreamPlaybackState = 'idle';
  private streamTimeAnchorMs: number = -1;
  private streamTimeAnchorPositionMs: number = 0;

  // State restoration for quality switching
  private pendingQualitySwitchTime: number = -1;
  private pendingQualitySwitchPlaying: boolean = false;

  private componentId: string = '';
  @State private xComponentId: string = '';  // Unique ID for XComponent to force recreation
  private xComponentController: XComponentController = new XComponentController();
  private controlsTimer: number = -1;
  private progressTimer: number = -1;
  private seekTimer: number = -1;
  private clearSurfaceTimer: number = -1;  // Delay surface clear to allow reattach
  private streamSurfaceRefreshTimer1: number = -1;
  private streamSurfaceRefreshTimer2: number = -1;
  private streamSurfaceWarmupTimer: number = -1;
  private streamSurfaceWarmupDone: boolean = false;
  private surfaceId: string = '';
  private lastSurfaceCreatedAt: number = 0;
  private playerCreated: boolean = false;
  private boundSurfaceId: string = '';
  private streamVideoSize?: StreamVideoSize;
  private static keepAliveIds: Set<string> = new Set();
  private static runtimeState: Map<string, LxNativeVideoRuntimeSnapshot> = new Map();
  private static nativePlayerExists: Set<string> = new Set();
  private static rebindInstance: Map<string, LxNativeVideoPlayer> = new Map();
  private static originalSurfaceIds: Map<string, string> = new Map();
  private static streamVideoSizes: Map<string, StreamVideoSize> = new Map();
  private static nativeEventLastPayload: Map<string, string> = new Map();
  private static instanceCounter: number = 0;  // Global counter to generate unique XComponent IDs
  // Global registry of componentIds that have ever rendered a frame.
  // Once a frame is rendered, poster should NEVER show again (cold start only).
  private static hasEverRenderedFrame: Set<string> = new Set();
  onEvent?: (event: string, detail: VideoEventDetail) => void;

  static setKeepAlive(componentId: string, keepAlive: boolean): void {
    if (!componentId) return;
    keepAlive ? LxNativeVideoPlayer.keepAliveIds.add(componentId) : LxNativeVideoPlayer.keepAliveIds.delete(componentId);
  }

  static isKeepAlive(componentId: string): boolean {
    return componentId ? LxNativeVideoPlayer.keepAliveIds.has(componentId) : false;
  }

  static hasNativePlayer(componentId: string): boolean {
    return componentId ? LxNativeVideoPlayer.nativePlayerExists.has(componentId) : false;
  }

  static setNativePlayerExists(componentId: string, exists: boolean): void {
    if (!componentId) return;
    exists ? LxNativeVideoPlayer.nativePlayerExists.add(componentId) : LxNativeVideoPlayer.nativePlayerExists.delete(componentId);
  }

  static setStreamVideoSize(componentId: string, width: number, height: number): void {
    if (!componentId) return;
    if (width <= 0 || height <= 0) {
      LxNativeVideoPlayer.clearStreamVideoSize(componentId);
      return;
    }
    const size = new StreamVideoSize(width, height);
    LxNativeVideoPlayer.streamVideoSizes.set(componentId, size);
    const instance = LxNativeVideoPlayer.rebindInstance.get(componentId);
    if (instance) {
      instance.streamVideoSize = size;
    }
  }

  static clearStreamVideoSize(componentId: string): void {
    if (!componentId) return;
    LxNativeVideoPlayer.streamVideoSizes.delete(componentId);
    const instance = LxNativeVideoPlayer.rebindInstance.get(componentId);
    if (instance) {
      instance.streamVideoSize = undefined;
    }
  }

  static getStreamVideoSize(componentId: string): StreamVideoSize | undefined {
    const instance = LxNativeVideoPlayer.rebindInstance.get(componentId);
    return instance?.streamVideoSize || LxNativeVideoPlayer.streamVideoSizes.get(componentId);
  }

  // Switch player to fullscreen surface
  static switchToFullscreenSurface(componentId: string, fullscreenSurfaceId: string): boolean {
    if (!componentId || !fullscreenSurfaceId) return false;
    const hasNative = LxNativeVideoPlayer.hasNativePlayer(componentId);

    const instance = LxNativeVideoPlayer.rebindInstance.get(componentId);
    if (instance?.surfaceId) {
      LxNativeVideoPlayer.originalSurfaceIds.set(componentId, instance.surfaceId);
    }

    // Mark as remounting to prevent poster flash during transition
    const snapshot = LxNativeVideoPlayer.runtimeState.get(componentId);
    if (snapshot) {
      snapshot.hasEverPlayed = true; // Ensure this is locked before transition
    }

    const success = hasNative
      ? (videoPlayerRebindSurface(componentId, fullscreenSurfaceId, snapshot?.currentTime ?? 0, snapshot?.isPlaying ?? true) as boolean)
      : (videoPlayerRebindStreamSurface(componentId, fullscreenSurfaceId) as boolean);

    // Update instance's boundSurfaceId so quality switching works in fullscreen
    if (success && instance) {
      instance.boundSurfaceId = fullscreenSurfaceId;
      log(`[${componentId}] switched to fullscreen surface: ${fullscreenSurfaceId}`);
    }

    return success;
  }

  static switchToOriginalSurface(componentId: string): boolean {
    const originalSurfaceId = LxNativeVideoPlayer.originalSurfaceIds.get(componentId);
    if (!componentId || !originalSurfaceId) return false;
    const hasNative = LxNativeVideoPlayer.hasNativePlayer(componentId);

    const snapshot = LxNativeVideoPlayer.runtimeState.get(componentId);
    if (snapshot) {
      snapshot.hasEverPlayed = true; // Ensure this is locked before transition
    }

    const success = hasNative
      ? (videoPlayerRebindSurface(componentId, originalSurfaceId, snapshot?.currentTime ?? 0, snapshot?.isPlaying ?? true) as boolean)
      : (videoPlayerRebindStreamSurface(componentId, originalSurfaceId) as boolean);
    if (success) {
      LxNativeVideoPlayer.originalSurfaceIds.delete(componentId);
      // Force sync slider state on the instance after exiting fullscreen
      const instance = LxNativeVideoPlayer.rebindInstance.get(componentId);
      if (instance) {
        instance.boundSurfaceId = originalSurfaceId;
        log(`[${componentId}] switched back to original surface: ${originalSurfaceId}`);
        instance.syncSliderFromNative();
      }
    }
    return success;
  }

  static savePlaybackState(componentId: string): void {
    LxNativeVideoPlayer.rebindInstance.get(componentId)?.saveRuntimeState();
  }

  static getPlaybackState(componentId: string): LxNativeVideoRuntimeSnapshot | undefined {
    return LxNativeVideoPlayer.runtimeState.get(componentId);
  }

  static togglePlayPause(componentId: string): void {
    LxNativeVideoPlayer.rebindInstance.get(componentId)?.togglePlayPause();
  }

  static pauseStatic(componentId: string): void {
    const instance = LxNativeVideoPlayer.rebindInstance.get(componentId);
    if (instance) {
      instance.pause();
    } else {
      videoPlayerPause(componentId);
      const state = LxNativeVideoPlayer.runtimeState.get(componentId);
      if (state) state.isPlaying = false;
    }
  }

  static playStatic(componentId: string): void {
    const instance = LxNativeVideoPlayer.rebindInstance.get(componentId);
    instance ? instance.play() : videoPlayerPlay(componentId);
  }

  static stopStatic(componentId: string): void {
    const instance = LxNativeVideoPlayer.rebindInstance.get(componentId);
    if (instance) {
      instance.stop();
    } else {
      videoPlayerStop(componentId);
      const state = LxNativeVideoPlayer.runtimeState.get(componentId);
      if (state) {
        state.isPlaying = false;
        state.currentTime = 0;
      }
    }
  }

  static resumeStatic(componentId: string): void {
    const state = LxNativeVideoPlayer.runtimeState.get(componentId);
    if (state?.isPlaying) LxNativeVideoPlayer.playStatic(componentId);
  }

  private static clearNativeEventCache(componentId: string): void {
    LxNativeVideoPlayer.nativeEventLastPayload.delete(`${componentId}:buffering`);
  }

  static destroyStatic(componentId: string): void {
    const instance = LxNativeVideoPlayer.rebindInstance.get(componentId);
    if (instance) {
      instance.destroyPlayer();
    } else if (LxNativeVideoPlayer.nativePlayerExists.has(componentId)) {
      videoPlayerDestroy(componentId);
      LxNativeVideoPlayer.nativePlayerExists.delete(componentId);
      LxNativeVideoPlayer.runtimeState.delete(componentId);
      LxNativeVideoPlayer.keepAliveIds.delete(componentId);
      LxNativeVideoPlayer.originalSurfaceIds.delete(componentId);
      LxNativeVideoPlayer.clearNativeEventCache(componentId);
    }
  }

  static seekTo(componentId: string, timeMs: number): void {
    const instance = LxNativeVideoPlayer.rebindInstance.get(componentId);
    instance ? instance.seek(timeMs) : videoPlayerSeek(componentId, Math.round(timeMs));
  }

  static onSeekDone(componentId: string, positionMs: number): void {
    const instance = LxNativeVideoPlayer.rebindInstance.get(componentId);
    if (instance) {
      instance.handleSeekDone(positionMs);
    } else {
      const state = LxNativeVideoPlayer.runtimeState.get(componentId);
      if (state) state.currentTime = positionMs;
    }
  }

  static handleNativeEvent(componentId: string, event: string, payload?: string): boolean {
    const eventName = event as NativePlayerEvent;
    if (eventName === 'buffering') {
      const key = `${componentId}:buffering`;
      const value = payload ?? '';
      const last = LxNativeVideoPlayer.nativeEventLastPayload.get(key);
      if (value !== last) {
        LxNativeVideoPlayer.nativeEventLastPayload.set(key, value);
        log(`nativeEvent: componentId=${componentId} event=${eventName} payload=${value}`);
      }
    } else if (
      eventName === 'playing' ||
      eventName === 'pause' ||
      eventName === 'waiting' ||
      eventName === 'prepared' ||
      eventName === 'stop' ||
      eventName === 'ended'
    ) {
      log(`nativeEvent: componentId=${componentId} event=${eventName}`);
    }

    switch (eventName) {
      case 'seekDone': {
        const position = parseInt(payload ?? '', 10);
        if (!Number.isFinite(position)) return false;
        LxNativeVideoPlayer.onSeekDone(componentId, position);
        return true;
      }
      case 'error': {
        LxNativeVideoPlayer.onError(componentId, payload ?? '');
        return true;
      }
      case 'buffering':
        LxNativeVideoPlayer.onBufferingChange(componentId, (payload ?? '') === '1');
        return true;
      case 'prepared':
        LxNativeVideoPlayer.onPrepared(componentId);
        return true;
      case 'stop':
        LxNativeVideoPlayer.onStopped(componentId);
        return true;
      case 'playing':
        LxNativeVideoPlayer.onPlaying(componentId);
        return true;
      case 'pause':
        LxNativeVideoPlayer.onPaused(componentId);
        return true;
      case 'ended':
        LxNativeVideoPlayer.onEnded(componentId);
        return true;
      case 'waiting':
        LxNativeVideoPlayer.onBufferingChange(componentId, true);
        return true;
      case 'streamConfig': {
        if (!payload) return false;
        try {
          const config = JSON.parse(payload) as StreamVideoConfig;
          const width = Number(config?.width ?? 0);
          const height = Number(config?.height ?? 0);
          if (!Number.isFinite(width) || !Number.isFinite(height)) return false;
          LxNativeVideoPlayer.setStreamVideoSize(componentId, width, height);
          return true;
        } catch (error) {
          log(`nativeEvent: streamConfig parse error=${String(error)}`);
          return false;
        }
      }
      case 'duration': {
        const duration = parseInt(payload ?? '', 10);
        if (!Number.isFinite(duration)) return false;
        LxNativeVideoPlayer.onDuration(componentId, duration);
        return true;
      }
      case 'enterFullscreen':
        LxNativeVideoPlayer.enterFullscreenStatic(componentId);
        return true;
      case 'exitFullscreen':
        LxNativeVideoPlayer.exitFullscreen(componentId);
        return true;
      default:
        log(`nativeEvent: unknown event=${String(event)}`);
        return false;
    }
  }

  static onBufferingChange(componentId: string, isBuffering: boolean): void {
    const instance = LxNativeVideoPlayer.rebindInstance.get(componentId);
    if (instance) {
      if (isBuffering && instance.isStreamMode()) {
        log(`[${componentId}] static onBufferingChange: buffering start`);
        instance.hasEnded = false;
      }
      instance.handleBufferingChange(isBuffering);
    } else {
      log(`[${componentId}] static onBufferingChange: no instance found`);
    }
  }

  static onPrepared(componentId: string): void {
    LxNativeVideoPlayer.rebindInstance.get(componentId)?.handlePrepared();
  }

  static onStopped(componentId: string): void {
    const instance = LxNativeVideoPlayer.rebindInstance.get(componentId);
    LxNativeVideoPlayer.clearStreamVideoSize(componentId);
    if (instance) {
      instance.handleStopped();
    } else {
      const state = LxNativeVideoPlayer.runtimeState.get(componentId);
      if (state) {
        state.isPlaying = false;
        state.currentTime = 0;
      }
    }
  }

  static onPlaying(componentId: string): void {
    const instance = LxNativeVideoPlayer.rebindInstance.get(componentId);
    if (instance) {
      instance.handlePlaying();
    } else {
      const state = LxNativeVideoPlayer.runtimeState.get(componentId);
      if (state) {
        state.isPlaying = true;
      }
    }
  }

  static onPaused(componentId: string): void {
    const instance = LxNativeVideoPlayer.rebindInstance.get(componentId);
    if (instance) {
      instance.handlePaused();
    } else {
      const state = LxNativeVideoPlayer.runtimeState.get(componentId);
      if (state) {
        state.isPlaying = false;
      }
    }
  }

  static onEnded(componentId: string): void {
    const instance = LxNativeVideoPlayer.rebindInstance.get(componentId);
    if (instance) {
      instance.onPlaybackEnded();
    } else {
      const state = LxNativeVideoPlayer.runtimeState.get(componentId);
      if (state) {
        state.isPlaying = false;
      }
    }
  }

  static onDuration(componentId: string, durationMs: number): void {
    const duration = Math.max(0, Math.round(durationMs));
    const instance = LxNativeVideoPlayer.rebindInstance.get(componentId);
    if (instance) {
      instance.state.duration = duration;
      if (duration > 0) {
        if (instance.isStreamMode()) {
          // If we are seeking, don't reset state to beginning just because duration arrived
          const isRecentSeek = Date.now() - instance.lastSeekTimestamp < 2000;
          // Also don't reset if video has already started playing (late duration event)
          // This prevents UI from flashing back to 0 when duration arrives after playback began
          const hasAlreadyStarted = instance.hasRenderedFirstFrame || instance.hasEverPlayed || instance.state.currentTime > 0;
          if (!isRecentSeek && !hasAlreadyStarted) {
            instance.state.currentTime = 0;
            instance.sliderValue = 0;
            instance.hasEnded = false;
            instance.pendingPlayEvent = false;
            instance.lastTimeForPlayEvent = -1;
            instance.lastCurrentTimeForBuffering = -1;
            instance.lastCurrentTimeForPoster = -1;
            instance.streamTimeAnchorMs = Date.now();
            instance.streamTimeAnchorPositionMs = 0;
            if (instance.isPausedByUser) {
              instance.setStreamState('idle', 'duration');
            } else {
              instance.setStreamState('waiting', 'duration');
              instance.traceLoadingIndicator('duration:segment_switch');
            }
            instance.startProgressTimer();
          } else {
             log(`[${componentId}] onDuration: skipping state reset (recentSeek=${isRecentSeek}, alreadyStarted=${hasAlreadyStarted})`);
          }
        } else {
          instance.state.currentTime = Math.min(instance.state.currentTime, duration);
          instance.sliderValue = calculateSliderValue(instance.state.currentTime, duration);
        }
      }
      instance.saveRuntimeState();
    } else {
      const state = LxNativeVideoPlayer.runtimeState.get(componentId);
      if (state) {
        state.duration = duration;
        state.currentTime = Math.min(state.currentTime, duration);
      }
    }
  }

  static onError(componentId: string, message?: string): void {
    const instance = LxNativeVideoPlayer.rebindInstance.get(componentId);
    let code = 'native_error';
    let msg = message ?? '';
    if (msg) {
      try {
        const parsed = JSON.parse(msg) as NativeErrorPayload;
        if (parsed && typeof parsed === 'object') {
          if (typeof parsed.code === 'string' && parsed.code.trim().length > 0) {
            code = parsed.code.trim();
          }
          if (typeof parsed.message === 'string') {
            msg = parsed.message;
          }
        }
      } catch (_) {
        // Plain string payload; treat as message.
      }
    }
    if (instance) {
      instance.handleError(code, msg);
      return;
    }
    const detail = new VideoErrorDetail();
    detail.code = code;
    detail.message = msg;
    const state = LxNativeVideoPlayer.runtimeState.get(componentId);
    if (state) {
      state.isPlaying = false;
      state.isBuffering = false;
    }
    // No instance to emit to; the JS side will see the error once component remounts.
    log(`[${componentId}] static onError: no instance, code=${detail.code} message=${detail.message}`);
  }

  static getDuration(componentId: string): number {
    const instance = LxNativeVideoPlayer.rebindInstance.get(componentId);
    if (instance?.isStreamMode()) {
      return instance.state.duration;
    }
    let duration = instance?.state.duration ?? 0;
    if (duration <= 0) duration = LxNativeVideoPlayer.runtimeState.get(componentId)?.duration ?? 0;
    if (duration <= 0) duration = videoPlayerGetDuration(componentId);
    return duration;
  }

  static getCurrentTimeStatic(componentId: string): number {
    return LxNativeVideoPlayer.rebindInstance.get(componentId)?.getCurrentTime() ?? 0;
  }

  static enterFullscreenStatic(componentId: string): void {
    const instance = LxNativeVideoPlayer.rebindInstance.get(componentId);
    if (instance && !instance.isFullscreen) {
      instance.toggleFullscreen();
    }
  }

  static exitFullscreen(componentId: string): void {
    const instance = LxNativeVideoPlayer.rebindInstance.get(componentId);
    if (instance && instance.isFullscreen) {
      instance.toggleFullscreen();
    }
  }

  static setVolume(componentId: string, volume: number): void {
    videoPlayerSetVolume(componentId, volume);
  }

  static getVolume(componentId: string): number {
    return LxNativeVideoPlayer.rebindInstance.get(componentId)?.config.volume ?? 0.8;
  }

  static getMuted(componentId: string): boolean {
    return LxNativeVideoPlayer.rebindInstance.get(componentId)?.config.muted ?? false;
  }

  static getPoster(componentId: string): string {
    const instance = LxNativeVideoPlayer.rebindInstance.get(componentId);
    return instance?.cachedPoster || instance?.config.poster || '';
  }

  static getConfig(componentId: string): LxNativeVideoConfig | undefined {
    return LxNativeVideoPlayer.rebindInstance.get(componentId)?.config;
  }

  static selectQuality(componentId: string, qualityLabel: string): void {
    const instance = LxNativeVideoPlayer.rebindInstance.get(componentId);
    if (instance) {
      instance.handleQualitySelect(qualityLabel);
    }
  }

  static selectPlaybackRate(componentId: string, rate: number): void {
    const instance = LxNativeVideoPlayer.rebindInstance.get(componentId);
    if (instance) {
      instance.handlePlaybackRateSelect(rate);
    }
  }

  static getCurrentQuality(componentId: string): string {
    const instance = LxNativeVideoPlayer.rebindInstance.get(componentId);
    return instance?.state.currentQuality || '';
  }

  static getCurrentPlaybackRate(componentId: string): number {
    const instance = LxNativeVideoPlayer.rebindInstance.get(componentId);
    return instance?.state.currentPlaybackRate || 1.0;
  }

  private handleQualitySelect(qualityLabel: string): void {
    this.state.currentQuality = qualityLabel;

    const selectedQuality = this.config.qualities.find((q: LxMediaQuality) => q.label === qualityLabel);
    const surface = this.boundSurfaceId || this.surfaceId;

    if (selectedQuality?.url && surface) {
      const currentTime = this.getCurrentTime();
      const wasPlaying = this.state.isPlaying;

      // Destroy existing player to reset state (SetURLSource requires IDLE state)
      videoPlayerDestroy(this.componentId);

      // Re-create player
      const createResult = videoPlayerCreate(this.componentId, 0) as number;
      if (createResult !== 0) {
        LxNativeVideoPlayer.setNativePlayerExists(this.componentId, true);
        videoPlayerSetUrl(this.componentId, resolveLxUriForFileAccess(selectedQuality.url));
        videoPlayerSetSurface(this.componentId, surface);

        if (this.config.volume !== undefined) videoPlayerSetVolume(this.componentId, this.config.volume);
        videoPlayerSetLoop(this.componentId, this.config.loop);
        if (this.state.currentPlaybackRate !== 1.0) videoPlayerSetSpeed(this.componentId, this.state.currentPlaybackRate);

        this.pendingQualitySwitchTime = currentTime;
        this.pendingQualitySwitchPlaying = wasPlaying;
        videoPlayerPrepare(this.componentId);
      } else {
        log(`[${this.componentId}] ERROR: failed to recreate player for quality switch, result=${createResult}`);
      }
    }

    const detail = new LxMediaQualityChangeDetail();
    detail.quality = qualityLabel;
    detail.url = selectedQuality?.url;
    this.emitEvent('qualitychange', detail);
  }

  private handlePlaybackRateSelect(rate: number): void {
    this.state.currentPlaybackRate = rate;
    videoPlayerSetSpeed(this.componentId, rate);

    const detail = new LxMediaRateChangeDetail();
    detail.rate = rate;
    this.emitEvent('ratechange', detail);
  }

  static getVideoWidth(componentId: string): number {
    return videoPlayerGetVideoWidth(componentId) as number;
  }

  static getVideoHeight(componentId: string): number {
    return videoPlayerGetVideoHeight(componentId) as number;
  }

  static isLandscapeVideo(componentId: string): boolean {
    let width = LxNativeVideoPlayer.getVideoWidth(componentId);
    let height = LxNativeVideoPlayer.getVideoHeight(componentId);
    if (width <= 0 || height <= 0) {
      const streamSize = LxNativeVideoPlayer.getStreamVideoSize(componentId);
      if (streamSize) {
        width = streamSize.width;
        height = streamSize.height;
      }
    }
    return width > 0 && height > 0 && width > height;
  }

  static isLoopEnabled(componentId: string): boolean {
    return LxNativeVideoPlayer.rebindInstance.get(componentId)?.config.loop ?? false;
  }

  static isBuffering(componentId: string): boolean {
    return LxNativeVideoPlayer.rebindInstance.get(componentId)?.state.isBuffering ?? false;
  }

  private saveRuntimeState(): void {
    if (!this.componentId) return;
    const snapshot = new LxNativeVideoRuntimeSnapshot();
    snapshot.isPlaying = this.state.isPlaying;
    // Use state.currentTime which is the calculated effectiveTime in progress timer,
    // not native time (which doesn't account for stream mode time anchor)
    snapshot.currentTime = this.state.currentTime;
    snapshot.duration = this.state.duration;
    snapshot.hasRenderedFirstFrame = this.hasRenderedFirstFrame;
    snapshot.hasEverPlayed = this.hasEverPlayed || this.hasRenderedFirstFrame || (this.state.currentTime > 0);
    LxNativeVideoPlayer.runtimeState.set(this.componentId, snapshot);
  }

  private restoreRuntimeState(): void {
    if (!this.componentId) return;
    const snapshot = LxNativeVideoPlayer.runtimeState.get(this.componentId);
    if (snapshot) {
      this.state.isPlaying = snapshot.isPlaying;
      let restoredTime = snapshot.currentTime;
      // Sanitize restored time to prevent "poisoned" state loop
      if (!isFinite(restoredTime) || restoredTime < 0 || restoredTime > 3000000000000) {
          log(`[${this.componentId}] restoreRuntimeState: sanitizing invalid currentTime=${restoredTime}`);
          restoredTime = 0;
      }
      this.state.currentTime = restoredTime;
      this.state.duration = snapshot.duration;
      // Restore hasRenderedFirstFrame from snapshot OR global Set (whichever is true)
      this.hasRenderedFirstFrame = snapshot.hasRenderedFirstFrame || LxNativeVideoPlayer.hasEverRenderedFrame.has(this.componentId);
      this.hasEverPlayed = snapshot.hasEverPlayed || this.hasRenderedFirstFrame;
      log(`[${this.componentId}] restoreRuntimeState: hasRenderedFirstFrame=${this.hasRenderedFirstFrame}, hasEverPlayed=${this.hasEverPlayed}, currentTime=${restoredTime}`);
      // Also restore slider value to prevent stale display
      this.sliderValue = calculateSliderValue(restoredTime, snapshot.duration);
      if (snapshot.isPlaying) this.startProgressTimer();
    }
  }

  private cornerRadiusTimer: number = -1;

  aboutToAppear(): void {
    this.componentId = this.playerId?.length > 0 ? this.playerId : `native-video-${Date.now()}`;
    // Generate unique XComponent ID to force recreation when component remounts
    LxNativeVideoPlayer.instanceCounter++;
    this.xComponentId = `${this.componentId}-xc-${LxNativeVideoPlayer.instanceCounter}`;
    // Cache config values since @Prop config may be reset during rebuild
    if (this.config.poster && !this.cachedPoster) {
      this.cachedPoster = this.config.poster;
    }
    if (this.config.autoplay) {
      this.cachedAutoplay = true;
    }
    this.autoplaySuppressed = false;
    // Initialize currentQuality from config if not already set
    if (!this.state.currentQuality && this.config.qualities.length > 0) {
      this.state.currentQuality = this.config.qualities[0].label;
    }
    log(`[${this.componentId}] aboutToAppear: poster=${this.config.poster}, cachedPoster=${this.cachedPoster}, autoplay=${this.config.autoplay}, cachedAutoplay=${this.cachedAutoplay}, progressBar=${this.config.progressBar}, xComponentId=${this.xComponentId}, playerCreated=${this.playerCreated}, boundSurfaceId=${this.boundSurfaceId}, hasNativePlayer=${LxNativeVideoPlayer.hasNativePlayer(this.componentId)}, currentQuality=${this.state.currentQuality}, hasOnEvent=${!!this.onEvent}`);

    // Reset state for clean recreation (same-level remount may skip aboutToDisappear)
    this.playerCreated = false;
    this.boundSurfaceId = '';
    LxNativeVideoPlayer.setNativePlayerExists(this.componentId, false);
    LxNativeVideoPlayer.setKeepAlive(this.componentId, false);
    // LxNativeVideoPlayer.runtimeState.delete(this.componentId); // Do not delete state, we need to restore it!

    this.restoreRuntimeState();
    LxNativeVideoPlayer.rebindInstance.set(this.componentId, this);
    
    // CRITICAL: Final gate to prevent poster flash on component remount (e.g. fullscreen toggle).
    // If the video was already playing, has ended, or is at any position other than start,
    // we MUST consider the first frame as already rendered.
    if (this.state.currentTime > 0 || this.hasEnded || this.state.isPlaying || this.hasEverPlayed || LxNativeVideoPlayer.hasEverRenderedFrame.has(this.componentId)) {
      log(`[${this.componentId}] aboutToAppear: forcing hasRenderedFirstFrame=true to match previous state`);
      this.markFirstFrameRendered();
    }

    this.appliedCornerRadius = 0;
    if (this.config.cornerRadius > 0) {
      this.cornerRadiusTimer = setTimeout(() => {
        this.appliedCornerRadius = this.config.cornerRadius;
      }, 50);
    }
  }

  aboutToDisappear(): void {
    this.clearControlsTimer();
    this.stopProgressTimer();
    this.clearCornerRadiusTimer();
    this.clearSeekTimer();
    this.clearSeekDebounceTimer();
    this.cancelPendingSurfaceClear();
    this.cancelStreamSurfaceRefreshTimers();
    this.cancelStreamSurfaceWarmupTimer();
    this.appliedCornerRadius = 0;
    if (LxNativeVideoPlayer.rebindInstance.get(this.componentId) === this) {
      LxNativeVideoPlayer.rebindInstance.delete(this.componentId);
      LxNativeVideoPlayer.clearNativeEventCache(this.componentId);
      log(`[${this.componentId}] aboutToDisappear: removed instance from registry`);
    } else {
      log(`[${this.componentId}] aboutToDisappear: instance registry mismatch (already replaced?), skipping remove`);
    }
    if (this.playerCreated) this.destroyPlayer();
  }

  private lastCurrentTimeForPoster: number = -1;
  private lastCurrentTimeForBuffering: number = -1;

  private startProgressTimer(): void {
    if (this.progressTimer !== -1) return;
    this.lastCurrentTimeForPoster = -1;  // Reset on timer start
    this.lastTimeForPlayEvent = -1;  // Reset for playing event detection
    this.lastCurrentTimeForBuffering = -1;  // Reset for buffering auto-clear
    this.progressTimer = setInterval(() => {
      const currentTime = this.getCurrentTime();
      const nativeTime = currentTime;
      const duration = this.getDuration();
      let effectiveTime = currentTime;
      const endEpsilonMs = 30;

      if (this.isStreamMode() && duration > 0) {
        let streamTime = currentTime;

        // IMPORTANT: In stream mode, native time (from stream decoder) is relative to
        // stream start (from 0), NOT absolute playback position including seek offset.
        // We NEVER use native time to overwrite anchor directly. Instead, we use
        // wall-clock extrapolation from the anchor point set during seek/play.
        // Native time is only used to verify playback is progressing (not frozen).

        // Extrapolate from anchor if playing
        if (this.streamState === 'playing' && this.streamTimeAnchorMs > 0) {
           const elapsed = Date.now() - this.streamTimeAnchorMs;
           streamTime = Math.min(duration, this.streamTimeAnchorPositionMs + elapsed);
        }

        effectiveTime = this.streamState === 'playing' ? streamTime : this.state.currentTime;
      }

      if (duration > 0 && effectiveTime >= duration - 500) {
        // Just before end, make sure we consider it rendered
        this.markFirstFrameRendered();
      }
      if (!this.config.loop && !this.hasEnded && duration > 0 && effectiveTime >= duration - endEpsilonMs) {
        if (this.isStreamMode() && this.streamState !== 'playing') {
          return;
        }
        log(`[${this.componentId}] progressTimer: detected playback end, currentTime=${effectiveTime}, duration=${duration}`);
        this.onPlaybackEnded();
        return;  // Don't process further after ended
      }

      // Emit playing event when video actually starts playing (time progresses)
      if (this.pendingPlayEvent && duration > 0) {
        const timeAdvanced = this.lastTimeForPlayEvent >= 0 && effectiveTime > this.lastTimeForPlayEvent;
        if (effectiveTime > 0 && timeAdvanced) {
          log(`[${this.componentId}] progressTimer: emitting delayed playing event, currentTime=${effectiveTime}, lastTime=${this.lastTimeForPlayEvent}`);
          this.pendingPlayEvent = false;
          this.emitEvent('playing', new VideoEmptyDetail());
        }
        this.lastTimeForPlayEvent = effectiveTime;
      }

      if (this.isStreamMode() && this.state.isBuffering && !this.isPausedByUser) {
        const timeAdvanced = this.lastCurrentTimeForBuffering >= 0 && effectiveTime > this.lastCurrentTimeForBuffering;
        if (effectiveTime > 0 && timeAdvanced) {
          log(`[${this.componentId}] progressTimer: clearing buffering on time advance, currentTime=${effectiveTime}, lastTime=${this.lastCurrentTimeForBuffering}`);
          this.state.isBuffering = false;
          this.traceLoadingIndicator('progressTimer:time_advanced');
        }
        this.lastCurrentTimeForBuffering = effectiveTime;
      }

      // Record when first frame is rendered
      // For stream mode, duration might be 0 (live stream), so we just check if time is advancing
      const canDetectFrame = this.isStreamMode() || (this.state.isPlaying && duration > 0);
      if (!this.hasRenderedFirstFrame && canDetectFrame) {
        const timeAdvanced = this.lastCurrentTimeForPoster >= 0 && effectiveTime > this.lastCurrentTimeForPoster;
        if (effectiveTime > 200 && timeAdvanced) {
          log(`[${this.componentId}] progressTimer: first frame rendered, currentTime=${effectiveTime}`);
          this.markFirstFrameRendered();
        }
        this.lastCurrentTimeForPoster = effectiveTime;
      }

      if (duration > 0 && this.state.duration !== duration) {
        this.state.duration = duration;
      }

      // Pending seek protection: keep slider at target position until seek completes.
      // This MUST run even while isSeeking=true to prevent slider snap-back on release.
      if (this.pendingSeekTime >= 0) {
        const elapsed = Date.now() - this.pendingSeekStartTime;
        const target = this.pendingSeekTime;
        const closeEnough = duration > 0 && nativeTime >= 0 && Math.abs(nativeTime - target) <= 800;
        if (closeEnough) {
          // Seek completed, clear pending state and sync to native time
          this.pendingSeekTime = -1;
          this.pendingSeekStartTime = -1;
          effectiveTime = nativeTime;
          // Fall through to normal update logic
        } else if (this.pendingSeekStartTime > 0 && elapsed > 8000) {
          // Seek timed out, sync to native time
          log(`[${this.componentId}] progress: seek timed out, syncing to native=${effectiveTime}`);
          this.pendingSeekTime = -1;
          this.pendingSeekStartTime = -1;
          this.state.currentTime = effectiveTime;
          if (duration > 0) this.sliderValue = calculateSliderValue(effectiveTime, duration);
          return;
        } else {
          // Keep slider at target position until seek completes
          this.state.currentTime = target;
          if (duration > 0) this.sliderValue = calculateSliderValue(target, duration);
          return;
        }
      }

      // Normal progress update (only when not actively seeking via slider)
      if (!this.isSeeking && effectiveTime >= 0) {
        this.state.currentTime = effectiveTime;
        if (duration > 0) this.sliderValue = calculateSliderValue(effectiveTime, duration);
        // Emit timeupdate event for JS tracking (enables relative seek)
        if (duration > 0) {
          const timeUpdateDetail = new VideoTimeUpdateDetail();
          timeUpdateDetail.currentTime = effectiveTime / 1000;
          timeUpdateDetail.duration = duration / 1000;
          this.emitEvent('timeupdate', timeUpdateDetail);
        }
        if (this.state.isPlaying) this.saveRuntimeState();
      }
    }, 250);
  }

  private onPlaybackEnded(): void {
    log(`[${this.componentId}] onPlaybackEnded() called, loop=${this.config.loop}, hasEnded=${this.hasEnded}`);
    if (this.config.loop || this.hasEnded) return;
    if (this.isStreamMode()) {
      // Enforce real stop in stream mode: without this the progress can reach 100% while audio/video continues.
      const ok = videoPlayerPause(this.componentId) as boolean;
      if (!ok) {
        log(`[${this.componentId}] onPlaybackEnded: videoPlayerPause returned false in stream mode; continuing UI end state`);
      }
    }
    this.hasEnded = true;
    // Ended should keep the last rendered frame visible (never show poster on ended).
    this.markFirstFrameRendered();
    if (this.state.duration > 0) {
      this.state.currentTime = this.state.duration;
      this.sliderValue = 100;
    }
    if (this.isStreamMode()) {
      this.setStreamState('ended', 'ended');
    } else {
      this.state.isPlaying = false;
      this.state.isPaused = true;
      this.state.isBuffering = false;
    }
    // Cancel pending playing event on end
    this.pendingPlayEvent = false;
    log(`[${this.componentId}] onPlaybackEnded()`);
    this.stopProgressTimer();
    this.saveRuntimeState();  // Save ended state
    this.emitEvent('ended', new VideoEmptyDetail());
  }

  static hasEndedStatic(componentId: string): boolean {
    const instance = LxNativeVideoPlayer.rebindInstance.get(componentId);
    if (!instance) return false;
    return instance.hasEnded || (!instance.hasRenderedFirstFrame && !instance.state.isPlaying);
  }

  private stopProgressTimer(): void {
    if (this.progressTimer !== -1) {
      clearInterval(this.progressTimer);
      this.progressTimer = -1;
    }
  }

  private onSurfaceCreated(surfaceId: string): void {
    // Cancel any pending surface clear operation since a new surface was created
    this.cancelPendingSurfaceClear();
    this.cancelStreamSurfaceRefreshTimers();

    this.lastSurfaceCreatedAt = Date.now();
    this.boundSurfaceId = surfaceId;
    this.surfaceId = surfaceId;
    if (!this.hasSource()) {
      log(`[${this.componentId}] onSurfaceCreated(stream): surfaceId=${surfaceId}, xType=${this.getXComponentType()}, xId=${this.xComponentId}`);
      videoPlayerStoreSurface(this.componentId, surfaceId);
      const ok = videoPlayerRebindStreamSurface(this.componentId, surfaceId) as boolean;
      log(`[${this.componentId}] videoPlayerRebindStreamSurface: ok=${ok}, surfaceId=${surfaceId}`);
      this.scheduleStreamSurfaceRefresh();
      return;
    }
    if (!LxNativeVideoPlayer.hasNativePlayer(this.componentId)) {
      this.ensurePlayer(surfaceId, false, undefined);
    }
  }

  private onSurfaceDestroyed(surfaceId: string): void {
    // Stream-only mode: surfaces are frequently detached/reattached by ArkUI. Clearing the stored
    // surface ID here is unsafe because onDestroy may fire for an old XComponent after a new one is
    // already created and bound, which would cause a black screen. Rebind on next onLoad instead.
    if (!this.hasSource()) {
      log(`[${this.componentId}] stream mode surface destroyed, skipping clear: ${surfaceId}`);
      this.cancelStreamSurfaceRefreshTimers();
      this.cancelStreamSurfaceWarmupTimer();
      return;
    }

    // XComponent surfaces can be recreated; ArkUI may report onDestroy for an old surface after a new
    // one is already created/bound. Only clear the stored surface when the destroyed surface is the
    // currently bound surface; otherwise we can accidentally clear the active surface and break
    // stream playback (Surface not set).
    const isCurrentSurface = this.surfaceId === surfaceId || this.boundSurfaceId === surfaceId;
    if (!isCurrentSurface) return;

    // Extra guard: if we very recently created/bound a surface, treat this destroy as a transient
    // detach (likely for the previous instance) and skip clearing to avoid black screens.
    if (Date.now() - this.lastSurfaceCreatedAt < 1200) {
      log(`[${this.componentId}] surface destroyed soon after create, skipping clear: ${surfaceId}`);
      return;
    }

    // Clear any existing pending clear operation
    this.cancelPendingSurfaceClear();

    // Delay clearing to allow temporary detach/reattach (similar to Android's 900ms pattern)
    // This improves robustness during navigation or layout changes
    this.clearSurfaceTimer = setTimeout(() => {
      // Double-check that the surface is still the one we expect to clear
      // (a new surface might have been created and bound during the delay)
      const stillCurrentSurface = this.surfaceId === surfaceId || this.boundSurfaceId === surfaceId;
      if (stillCurrentSurface) {
        if (this.isFullscreen || LxNativeVideoPlayer.isKeepAlive(this.componentId)) {
          log(`[${this.componentId}] surface detach during fullscreen/keepAlive, skipping clear: ${surfaceId}`);
          return;
        }
        log(`[${this.componentId}] surface detached for 800ms, clearing: ${surfaceId}`);
        if (this.boundSurfaceId === surfaceId) this.boundSurfaceId = '';
        if (this.surfaceId === surfaceId) this.surfaceId = '';
        videoPlayerClearSurface(this.componentId);
      } else {
        log(`[${this.componentId}] surface reattached during delay, skipping clear: ${surfaceId}`);
      }
      this.clearSurfaceTimer = -1;
    }, 800) as number;  // 800ms delay (slightly less than Android's 900ms for Harmony's faster reattach)
  }

  private ensurePlayer(surfaceId: string, wasKeepAlive: boolean = false, snapshot?: LxNativeVideoRuntimeSnapshot): void {
    if (!this.hasSource()) {
      if (surfaceId) videoPlayerStoreSurface(this.componentId, surfaceId);
      return;
    }
    if (wasKeepAlive || LxNativeVideoPlayer.isKeepAlive(this.componentId)) {
      this.playerCreated = true;
      this.rebindSurface(surfaceId, snapshot);
      if ((snapshot?.isPlaying) || LxNativeVideoPlayer.runtimeState.get(this.componentId)?.isPlaying) {
        this.state.isPlaying = true;
        this.startProgressTimer();
      }
      return;
    }
    if (!this.playerCreated) {
      this.createPlayer(surfaceId);
      return;
    }
    this.rebindSurface(surfaceId, snapshot);
  }

  private createPlayer(surfaceId?: string): void {
    if (!this.hasSource()) {
      if (surfaceId) videoPlayerStoreSurface(this.componentId, surfaceId);
      return;
    }
    const playerPtr = videoPlayerCreate(this.componentId, 0) as number;
    if (playerPtr === 0) {
      const detail = new VideoErrorDetail();
      detail.code = 'create_failed';
      detail.message = 'Failed to create native player';
      this.emitEvent('error', detail);
      return;
    }
    this.playerCreated = true;
    LxNativeVideoPlayer.setNativePlayerExists(this.componentId, true);

    if (this.config.src) {
      const resolvedSrc = resolveLxUriForFileAccess(this.config.src);
      if (!(videoPlayerSetUrl(this.componentId, resolvedSrc) as boolean)) {
        const detail = new VideoErrorDetail();
        detail.code = 'set_url_failed';
        detail.message = 'Failed to set URL source';
        this.emitEvent('error', detail);
        return;
      }
    }

    const targetSurface = surfaceId || this.boundSurfaceId || this.surfaceId;
    if (targetSurface) {
      if (!(videoPlayerSetSurface(this.componentId, targetSurface) as boolean)) {
        const detail = new VideoErrorDetail();
        detail.code = 'set_surface_failed';
        detail.message = 'Failed to set video surface';
        this.emitEvent('error', detail);
        return;
      }
      this.boundSurfaceId = targetSurface;
    }

    if (this.config.volume !== undefined) videoPlayerSetVolume(this.componentId, this.config.volume);
    videoPlayerSetLoop(this.componentId, this.config.loop ?? false);
    if (this.config.src) {
      videoPlayerPrepare(this.componentId);
    }
    // Note: loadedmetadata and autoplay will be triggered in handlePrepared() callback when player is ready

    log(`[${this.componentId}] createPlayer done, autoplay=${this.config.autoplay}`);
  }

  private rebindSurface(surfaceId: string, snapshot?: LxNativeVideoRuntimeSnapshot): void {
    if (!surfaceId || !this.playerCreated) return;
    const positionMs = snapshot?.currentTime ?? this.state.currentTime;
    const shouldPlay = snapshot?.isPlaying ?? this.state.isPlaying;

    if (videoPlayerRebindSurface(this.componentId, surfaceId, positionMs, shouldPlay) as boolean) {
      this.boundSurfaceId = surfaceId;
      this.state.isPlaying = shouldPlay;
      this.state.isPaused = !shouldPlay;
      shouldPlay ? this.startProgressTimer() : this.stopProgressTimer();
    }
  }

  private destroyPlayer(): void {
    videoPlayerDestroy(this.componentId);
    this.playerCreated = false;
    this.boundSurfaceId = '';
    LxNativeVideoPlayer.setKeepAlive(this.componentId, false);
    LxNativeVideoPlayer.setNativePlayerExists(this.componentId, false);
    // Clear runtime state to avoid stale data when video is recreated with different source
    LxNativeVideoPlayer.runtimeState.delete(this.componentId);
    LxNativeVideoPlayer.clearNativeEventCache(this.componentId);
  }

  play(): void {
    log(`[${this.componentId}] play() called`);
    // Explicit play request should override any pending autoplay suppression
    this.autoplaySuppressed = false;
    this.isPausedByUser = false;

    if (this.isStreamMode()) {
      // If playback has ended (or currentTime is at/past duration), seek to beginning first.
      // This prevents "endTime > beginTime" errors when resume() tries to reconnect near the
      // end of the playback segment.
      const duration = this.getDuration();
      const shouldSeekToStart = this.hasEnded ||
        (duration > 0 && this.state.currentTime >= duration - 500);
      if (shouldSeekToStart) {
        log(`[${this.componentId}] play(): playback ended or at end, seeking to start first`);
        this.state.currentTime = 0;
        this.sliderValue = 0;
        this.streamTimeAnchorMs = Date.now();
        this.streamTimeAnchorPositionMs = 0;
        // Seek to 0 before play - this ensures the stream session starts from the beginning
        videoPlayerSeek(this.componentId, 0);
      }

      // Stream decoder mode: update UI state *before* calling native play to avoid a race where
      // the native `play` event arrives before we set `isBuffering=true` (which would leave
      // the loading indicator stuck).
      this.setStreamState('waiting', 'play');
      this.hasEnded = false;
      // DON'T set streamTimeAnchorMs here - only set the position.
      // The time anchor will be set in handlePlaying() when video actually starts.
      // This prevents progress bar from advancing before video is actually playing.
      // Exception: if we just seeked, preserve the anchor so progress continues from seek point.
      if (Date.now() - this.lastSeekTimestamp > 2000) {
        this.streamTimeAnchorMs = 0;
      }
      this.streamTimeAnchorPositionMs = Math.max(0, this.state.currentTime);
      this.saveRuntimeState();
      this.traceLoadingIndicator('play(stream):ui_state_set');
      this.startProgressTimer();
      const ok = videoPlayerPlay(this.componentId) as boolean;
      if (!ok) {
        log(`[${this.componentId}] videoPlayerPlay returned false in stream mode; updating UI state anyway`);
      }

      // IMPORTANT: Emit the play request after unpausing the native decoder. Otherwise the stream
      // session might start pushing frames while the decoder is still paused, dropping the first
      // keyframe and leaving the loading indicator stuck until the next keyframe.
      this.emitEvent('play', new VideoEmptyDetail());
      this.emitEvent('playrequest', new VideoEmptyDetail());

      const waitingDetail = new VideoWaitingDetail();
      waitingDetail.reason = 'buffering';
      this.emitEvent('waiting', waitingDetail);
      return;
    }

    const ok = videoPlayerPlay(this.componentId) as boolean;
    if (ok) {
      // Don't set isPlaying=true yet. Wait for native 'playing' event.
      // This prevents the progress bar from moving while the video is still loading/buffering.
      this.state.isPaused = false;
      this.hasEnded = false;
      this.pendingPlayEvent = true;
      this.lastTimeForPlayEvent = -1;
      this.emitEvent('play', new VideoEmptyDetail());
      log(`[${this.componentId}] play() success (waiting for native 'playing' event to start progress)`);
      // this.startProgressTimer(); // Removed: started in handlePlaying
    }
  }

  pause(): void {
    log(`[${this.componentId}] pause() called`);
    this.isPausedByUser = true;
    const ok = videoPlayerPause(this.componentId) as boolean;
    if (this.isStreamMode() || ok) {
      if (this.isStreamMode() && !ok) {
        log(`[${this.componentId}] videoPlayerPause returned false in stream mode; updating UI state anyway`);
      }
      if (this.isStreamMode()) {
        this.setStreamState('idle', 'pause');
      } else {
        this.state.isPlaying = false;
        this.state.isPaused = true;
        this.state.isBuffering = false;  // Hide buffering indicator immediately
        this.pendingPlayEvent = false;
      }
      this.autoplaySuppressed = true;
      this.stopProgressTimer();
      this.saveRuntimeState();
      this.traceLoadingIndicator('pause()');
      if (this.isStreamMode()) {
        const detail = new VideoControlDetail();
        detail.reason = 'user';
        detail.currentTime = Math.max(0, this.state.currentTime) / 1000.0;
        this.emitEvent('pause', detail);
      } else {
        this.emitEvent('pause', new VideoEmptyDetail());
      }
    }
  }

  stop(): void {
    log(`[${this.componentId}] stop() called`);
    this.isPausedByUser = true;
    // Reset seek timestamp so that next source change will properly reset progress bar
    this.lastSeekTimestamp = 0;
    const ok = videoPlayerStop(this.componentId) as boolean;
    if (this.isStreamMode() || ok) {
      // Reset playback state
      this.state.isPlaying = false;
      this.state.isPaused = true;
      this.state.isBuffering = false;
      this.autoplaySuppressed = true;
      this.state.currentTime = 0;
      // Keep duration for stream VOD so the progress bar stays consistent across stop/replay.
      if (!this.isStreamMode()) {
        this.state.duration = 0;
      }
      this.sliderValue = 0;
      this.hasEnded = false;
      this.pendingPlayEvent = false;
      // Show poster and center play button (reset to initial state / cold start)
      this.hasRenderedFirstFrame = false;
      this.hasEverPlayed = false;
      LxNativeVideoPlayer.hasEverRenderedFrame.delete(this.componentId);
      this.stopProgressTimer();
      this.saveRuntimeState();
      this.traceLoadingIndicator('stop()');
      log(`[${this.componentId}] stop() success, reset to initial state`);
      if (this.isStreamMode()) {
        const detail = new VideoControlDetail();
        detail.reason = 'user';
        detail.currentTime = 0;
        this.emitEvent('stop', detail);
      } else {
        this.emitEvent('stop', new VideoEmptyDetail());
      }
    }
  }

  seek(time: number): void {
    if (isNaN(time) || time < 0) {
      log(`[${this.componentId}] seek ignored: invalid time=${time}`);
      return;
    }
    const timeInt = Math.round(time);
    log(`[${this.componentId}] seek: time=${time} timeInt=${timeInt} streamMode=${this.isStreamMode()} duration=${this.state.duration}`);
    // Track pending seek for UI sync (non-stream mode only)
    // In stream mode, we clear it immediately below.
    this.pendingSeekTime = timeInt;
    this.pendingSeekStartTime = Date.now();

    // Stream mode: update time anchor immediately because native seekDone callback
    // won't be received (stream seek reopens the connection instead of using AVPlayer seek)
    if (this.isStreamMode()) {
      log(`[${this.componentId}] seek: stream mode, updating time anchor to ${timeInt}ms`);
      // Use Date.now() to ensure timer moves immediately using wall-clock fallback if native time is 0/stale
      this.streamTimeAnchorMs = Date.now();
      this.streamTimeAnchorPositionMs = timeInt;
      this.lastSeekTimestamp = Date.now();
      this.state.currentTime = timeInt;
      const duration = this.getDuration();
      if (duration > 0) this.sliderValue = calculateSliderValue(timeInt, duration);

      // Clear pending seek immediately for stream mode because native seekDone/playing events
      // are not reliable triggers to unblock the progress timer.
      this.pendingSeekTime = -1;
      this.pendingSeekStartTime = -1;
    }

    log(`[${this.componentId}] seek: calling videoPlayerSeek with timeInt=${timeInt}`);
    if (videoPlayerSeek(this.componentId, timeInt) as boolean) {
      // In stream mode, FFI layer handles seek via callback - don't emit events that trigger duplicate seeks
      if (!this.isStreamMode()) {
        const seekDetail = new VideoSeekingDetail();
        // Convert ms to seconds for the event (Rust handler expects seconds)
        seekDetail.currentTime = timeInt / 1000;
        this.emitEvent('seeking', seekDetail);
      }
    }
  }

  handleSeekDone(positionMs: number): void {
    log(`[${this.componentId}] handleSeekDone: position=${positionMs}, pending=${this.pendingSeekTime}`);

    let actualPosition = positionMs > 0 ? positionMs : this.getCurrentTime();

    if (this.isStreamMode()) {
      // In stream mode, prefer the pending seek target over native position to avoid snap-back.
      // The native callback often arrives with positionMs=0 before the stream has seeked.
      if (this.pendingSeekTime >= 0) {
        actualPosition = this.pendingSeekTime;
      }

      const timeSinceSeek = Date.now() - this.lastSeekTimestamp;
      if (timeSinceSeek < 2000) {
         log(`[${this.componentId}] handleSeekDone: ignoring callback in stream mode (recent manual seek), actualPosition=${actualPosition}`);
         // We trust seek() to have set the anchor correctly.
         // Just clear the pending state and return.
         this.pendingSeekTime = -1;
         this.pendingSeekStartTime = -1;
         this.finishSeeking();
         return;
      }

      // Anchor the current wall-clock time (now) to the playback position from the seek.
      this.streamTimeAnchorMs = Date.now();
      this.streamTimeAnchorPositionMs = Math.max(0, actualPosition);

      // NOW clear pending seek - we've used it to set actualPosition
      this.pendingSeekTime = -1;
      this.pendingSeekStartTime = -1;
    } else {
      this.pendingSeekTime = -1;
      this.pendingSeekStartTime = -1;
    }

    this.state.currentTime = actualPosition;
    const duration = this.getDuration();
    if (duration > 0) this.sliderValue = calculateSliderValue(actualPosition, duration);

    // Clear isSeeking state now that seek is complete
    this.finishSeeking();

    const seekDetail = new VideoSeekingDetail();
    // Convert ms to seconds for the event
    seekDetail.currentTime = actualPosition / 1000;
    this.emitEvent('seeked', seekDetail);
    this.saveRuntimeState();
  }

  private setStreamState(next: StreamPlaybackState, _reason: string): void {
    if (!this.isStreamMode()) return;
    if (this.streamState === next) return;
    this.streamState = next;
    switch (next) {
      case 'idle':
        this.state.isPlaying = false;
        this.state.isPaused = true;
        this.state.isBuffering = false;
        // Note: don't touch hasRenderedFirstFrame - poster only shows before first frame
        this.hasEnded = false;
        this.pendingPlayEvent = false;
        break;
      case 'waiting':
        this.state.isPlaying = true;
        this.state.isPaused = false;
        this.state.isBuffering = true;
        this.hasEnded = false;
        this.pendingPlayEvent = false;
        this.isPausedByUser = false;
        break;
      case 'playing':
        this.state.isPlaying = true;
        this.state.isPaused = false;
        this.state.isBuffering = false;
        this.hasEnded = false;
        this.pendingPlayEvent = false;
        this.isPausedByUser = false;
        // Always reset wall-clock anchor to now when playback actually starts.
        // This ensures we don't count buffering/loading time as playback time.
        // Preserve streamTimeAnchorPositionMs if it was set by a recent seek,
        // otherwise use current state time.
        this.streamTimeAnchorMs = Date.now();
        if (this.streamTimeAnchorPositionMs <= 0) {
          this.streamTimeAnchorPositionMs = this.state.currentTime;
        }
        break;
      case 'ended':
        this.state.isPlaying = false;
        this.state.isPaused = true;
        this.state.isBuffering = false;
        // Note: don't touch hasRenderedFirstFrame - poster only shows before first frame
        this.hasEnded = true;
        this.pendingPlayEvent = false;
        break;
    }
  }

  handleBufferingChange(isBuffering: boolean): void {
    const isStream = this.isStreamMode();
    // After `ended`, some native pipelines may still emit buffering/decoder-wait signals while
    // tearing down. Don't re-enter waiting (which would show a stuck loading indicator) unless
    // a new user intent transitions us away from 'ended' (e.g. play(), seek+play()).
    if (isStream && this.streamState === 'ended' && isBuffering) {
      return;
    }
    if (this.state.isBuffering === isBuffering) {
      return;
    }
    log(`[${this.componentId}] handleBufferingChange: isBuffering=${isBuffering}, isPlaying=${this.state.isPlaying}, isPaused=${this.state.isPaused}`);

    this.state.isBuffering = isBuffering;
    this.traceLoadingIndicator(`handleBufferingChange(${isBuffering})`);

    // Stream decoder mode uses `waiting` + `playing` to manage the loading indicator.
    // Avoid emitting extra play/pause events here, which can cause incorrect UI transitions.
    if (isStream) {
      if (isBuffering) {
        if (this.state.duration > 0 && this.state.currentTime >= this.state.duration - 500) {
          // New segment is starting; reset local timeline so we don't auto-end immediately.
          this.state.currentTime = 0;
          this.sliderValue = 0;
          this.hasEnded = false;
          this.streamTimeAnchorMs = Date.now();
          this.streamTimeAnchorPositionMs = 0;
        }
        if (!this.isPausedByUser) {
          this.setStreamState('waiting', 'buffering');
          // Force poster hidden logic is now handled by !hasRenderedFirstFrame (which should be true by now)
          this.hasEnded = false;
          log(`[${this.componentId}] handleBufferingChange: emitting waiting event`);
          const waitingDetail = new VideoWaitingDetail();
          waitingDetail.reason = 'buffering';
          this.emitEvent('waiting', waitingDetail);
        }
      } else {
        if (this.streamState === 'waiting' && !this.isPausedByUser) {
          log(`[${this.componentId}] handleBufferingChange: buffering ended, transitioning to playing`);
          this.setStreamState('playing', 'buffering_end');
        }
      }
      return;
    }

    if (isBuffering) {
      // Only emit waiting event if player is in playing state (not paused by user)
      if (this.state.isPlaying && !this.isPausedByUser) {
        log(`[${this.componentId}] handleBufferingChange: emitting waiting event`);
        const waitingDetail = new VideoWaitingDetail();
        waitingDetail.reason = 'buffering';
        this.emitEvent('waiting', waitingDetail);
      }
    } else {
      // Buffering ended - emit appropriate event based on current state
      if (this.state.isPlaying) {
        log(`[${this.componentId}] handleBufferingChange: emitting playing event (buffering ended)`);
        this.emitEvent('playing', new VideoEmptyDetail());
      }
    }
  }

  handlePrepared(): void {
    // Use cached autoplay since config.autoplay may be reset during rebuild
    const shouldAutoplay = this.cachedAutoplay || this.config.autoplay;
    log(`[${this.componentId}] handlePrepared: player is ready, autoplay=${this.config.autoplay}, cachedAutoplay=${this.cachedAutoplay}, shouldAutoplay=${shouldAutoplay}`);
    // Emit loadedmetadata with actual video dimensions and duration
    const metadataDetail = new VideoMetadataDetail();
    metadataDetail.width = videoPlayerGetVideoWidth(this.componentId);
    metadataDetail.height = videoPlayerGetVideoHeight(this.componentId);
    // For stream mode, only report duration if it has been set by onDuration callback.
    // Avoid reporting 0 or uninitialized values which may cause incorrect UI display.
    const durationMs = this.isStreamMode() ? (this.state.duration > 0 ? this.state.duration : 0) : this.getDuration();
    // Convert ms to seconds for the event
    metadataDetail.duration = durationMs / 1000;
    log(`[${this.componentId}] handlePrepared: metadata width=${metadataDetail.width}, height=${metadataDetail.height}, duration=${metadataDetail.duration}`);
    if (durationMs > 0 && !this.isStreamMode()) {
      this.state.duration = durationMs;
    }
    // Only emit loadedmetadata if we have valid dimensions or duration, or if not in stream mode
    this.emitEvent('loadedmetadata', metadataDetail);

    // Handle quality switch restoration (Highest Priority)
    if (this.pendingQualitySwitchTime >= 0) {
        log(`[${this.componentId}] handlePrepared: restoring quality switch state, time=${this.pendingQualitySwitchTime}, playing=${this.pendingQualitySwitchPlaying}`);
        this.seek(this.pendingQualitySwitchTime);
        if (this.pendingQualitySwitchPlaying) {
            this.play();
        }
        this.pendingQualitySwitchTime = -1;
        this.pendingQualitySwitchPlaying = false;
        return;
    }

    // Trigger autoplay after player is prepared (not before)
    if (shouldAutoplay && !this.autoplaySuppressed && !this.state.isPlaying) {
      log(`[${this.componentId}] handlePrepared: triggering autoplay`);
      this.play();
    }
  }

  handleStopped(): void {
    log(`[${this.componentId}] handleStopped: resetting UI to initial state`);
    this.state.isPlaying = false;
    this.state.isPaused = true;
    this.state.isBuffering = false;
    this.state.currentTime = 0;
    this.sliderValue = 0;
    // Reset poster state on stop so it shows again on cold start/fullscreen.
    this.hasRenderedFirstFrame = false;
    this.hasEverPlayed = false;
    LxNativeVideoPlayer.hasEverRenderedFrame.delete(this.componentId);
    this.hasEnded = false;
    this.pendingPlayEvent = false;
    this.stopProgressTimer();
    this.cancelStreamSurfaceRefreshTimers();
    this.cancelStreamSurfaceWarmupTimer();
    this.streamSurfaceWarmupDone = false;
    this.saveRuntimeState();
    this.traceLoadingIndicator('handleStopped');
  }

  handlePlaying(): void {
    log(`[${this.componentId}] handlePlaying: updating UI for playback`);
    if (this.isStreamMode()) {
      this.setStreamState('playing', 'playing');
      this.markFirstFrameRendered();
      // Force critical state flags even if setStreamState guard returned early
      // (e.g. state was already 'playing' due to concurrent calls or XComponent recreation).
      // This ensures replay after 'ended' always hides the poster and resets hasEnded.
      this.hasEnded = false;
      this.state.isBuffering = false;
      this.pendingSeekTime = -1;
      this.pendingSeekStartTime = -1;
    } else {
      this.state.isPlaying = true;
      this.state.isPaused = false;
      this.isPausedByUser = false;
      this.hasEnded = false;
      this.state.isBuffering = false;  // Clear buffering when playing resumes
    }
    if (!this.config?.src || this.config.src.trim().length === 0) {
      // No src, do nothing specific for poster as it depends on shouldShowPoster
    }
    this.startProgressTimer();
    this.saveRuntimeState();
    this.traceLoadingIndicator('handlePlaying');
    this.scheduleStreamSurfaceRefresh();
    this.scheduleStreamSurfaceWarmupRecreate();
    this.emitEvent('playing', new VideoEmptyDetail());
  }

  handlePaused(): void {
    log(`[${this.componentId}] handlePaused: updating UI for paused state`);
    this.state.isPlaying = false;
    this.state.isPaused = true;
    this.state.isBuffering = false;  // Clear buffering when paused
    this.stopProgressTimer();
    this.saveRuntimeState();
    this.traceLoadingIndicator('handlePaused');
    // Emit pause event to JS
    this.emitEvent('pause', new VideoEmptyDetail());
  }

  private handleError(code: string, message: string): void {
    log(`[${this.componentId}] handleError: code=${code} message=${message}`);
    this.stopProgressTimer();
    this.pendingPlayEvent = false;
    this.hasEnded = false;
    this.state.isBuffering = false;
    if (this.isStreamMode()) {
      this.setStreamState('idle', 'error');
    } else {
      this.state.isPlaying = false;
      this.state.isPaused = true;
    }
    this.saveRuntimeState();
    this.traceLoadingIndicator('handleError');
    const detail = new VideoErrorDetail();
    detail.code = code || 'unknown';
    detail.message = message || 'unknown error';
    this.emitEvent('error', detail);
  }

  getCurrentTime(): number { return videoPlayerGetCurrentTime(this.componentId); }
  getDuration(): number { return LxNativeVideoPlayer.getDuration(this.componentId); }

  syncSliderFromNative(): void {
    const currentTime = this.getCurrentTime();
    const duration = this.getDuration();
    if (duration > 0) {
      this.state.currentTime = currentTime;
      this.state.duration = duration;
      this.sliderValue = calculateSliderValue(currentTime, duration);
      log(`[${this.componentId}] syncSliderFromNative: currentTime=${currentTime}, duration=${duration}, sliderValue=${this.sliderValue}`);
    }
  }

  private emitEvent(event: string, detail: VideoEventDetail): void {
    if (event === 'waiting' || event === 'play' || event === 'playing' || event === 'pause' || event === 'ended' || event === 'stop') {
      let extra = '';
      if (event === 'waiting') {
        extra = ` reason=${(detail as VideoWaitingDetail)?.reason ?? ''}`;
      }
      log(`[${this.componentId}] emitEvent: ${event} hasOnEvent=${typeof this.onEvent === 'function'}${extra}`);
    }
    this.onEvent?.(event, detail);
  }

  private toggleFullscreen(): void {
    this.isFullscreen = !this.isFullscreen;
    this.state.isFullscreen = this.isFullscreen;
    const fsDetail = new VideoFullscreenDetail();
    fsDetail.fullScreen = this.isFullscreen;

    // Determine orientation based on video aspect ratio
    let videoWidth = videoPlayerGetVideoWidth(this.componentId) as number;
    let videoHeight = videoPlayerGetVideoHeight(this.componentId) as number;

    // If in stream mode (no native player), get dimensions from stream config cache
    if ((videoWidth === 0 || videoHeight === 0) && !this.hasSource()) {
      const streamSize = LxNativeVideoPlayer.getStreamVideoSize(this.componentId);
      if (streamSize) {
        videoWidth = streamSize.width;
        videoHeight = streamSize.height;
      }
    }

    const isLandscape = videoWidth > 0 && videoHeight > 0 && videoWidth > videoHeight;
    fsDetail.direction = isLandscape ? 'horizontal' : 'vertical';

    log(`[${this.componentId}] toggleFullscreen: fullscreen=${this.isFullscreen}, videoWidth=${videoWidth}, videoHeight=${videoHeight}, direction=${fsDetail.direction}`);
    this.emitEvent('fullscreenchange', fsDetail);
  }

  private clearControlsTimer(): void {
    if (this.controlsTimer !== -1) { clearTimeout(this.controlsTimer); this.controlsTimer = -1; }
  }

  private clearCornerRadiusTimer(): void {
    if (this.cornerRadiusTimer !== -1) { clearTimeout(this.cornerRadiusTimer); this.cornerRadiusTimer = -1; }
  }

  private resetControlsTimer(): void {
    this.clearControlsTimer();
    this.showControls = true;
    this.controlsTimer = setTimeout(() => { if (this.state.isPlaying) this.showControls = false; }, 3000);
  }

  private clearSeekTimer(): void {
    if (this.seekTimer !== -1) { clearTimeout(this.seekTimer); this.seekTimer = -1; }
  }

  private clearSeekDebounceTimer(): void {
    if (this.seekDebounceTimer !== -1) { clearTimeout(this.seekDebounceTimer); this.seekDebounceTimer = -1; }
  }

  private cancelPendingSurfaceClear(): void {
    if (this.clearSurfaceTimer !== -1) { clearTimeout(this.clearSurfaceTimer); this.clearSurfaceTimer = -1; }
  }

  private cancelStreamSurfaceRefreshTimers(): void {
    if (this.streamSurfaceRefreshTimer1 !== -1) { clearTimeout(this.streamSurfaceRefreshTimer1); this.streamSurfaceRefreshTimer1 = -1; }
    if (this.streamSurfaceRefreshTimer2 !== -1) { clearTimeout(this.streamSurfaceRefreshTimer2); this.streamSurfaceRefreshTimer2 = -1; }
  }

  private cancelStreamSurfaceWarmupTimer(): void {
    if (this.streamSurfaceWarmupTimer !== -1) { clearTimeout(this.streamSurfaceWarmupTimer); this.streamSurfaceWarmupTimer = -1; }
  }

  private scheduleStreamSurfaceRefresh(): void {
    if (this.hasSource()) return;
    if (!this.componentId) return;
    const surfaceId = this.boundSurfaceId || this.surfaceId;
    if (!surfaceId) return;
    if (this.streamSurfaceRefreshTimer1 !== -1 || this.streamSurfaceRefreshTimer2 !== -1) return;

    const expectedSurfaceId = surfaceId;
    this.streamSurfaceRefreshTimer1 = setTimeout(() => {
      this.streamSurfaceRefreshTimer1 = -1;
      if (this.hasSource()) return;
      if (!this.componentId) return;
      if (this.boundSurfaceId !== expectedSurfaceId && this.surfaceId !== expectedSurfaceId) return;
      const ok = videoPlayerRebindStreamSurface(this.componentId, expectedSurfaceId) as boolean;
      log(`[${this.componentId}] videoPlayerRebindStreamSurface(delayed): ok=${ok}, surfaceId=${expectedSurfaceId}, delayMs=180`);
    }, 180);

    this.streamSurfaceRefreshTimer2 = setTimeout(() => {
      this.streamSurfaceRefreshTimer2 = -1;
      if (this.hasSource()) return;
      if (!this.componentId) return;
      if (this.boundSurfaceId !== expectedSurfaceId && this.surfaceId !== expectedSurfaceId) return;
      const ok = videoPlayerRebindStreamSurface(this.componentId, expectedSurfaceId) as boolean;
      log(`[${this.componentId}] videoPlayerRebindStreamSurface(delayed): ok=${ok}, surfaceId=${expectedSurfaceId}, delayMs=800`);
    }, 800);
  }

  private scheduleStreamSurfaceWarmupRecreate(): void {
    if (this.hasSource()) return;
    if (!this.componentId) return;
    if (this.streamSurfaceWarmupDone) return;
    const expectedSurfaceId = this.boundSurfaceId || this.surfaceId;
    if (!expectedSurfaceId) return;

    this.streamSurfaceWarmupDone = true;
    this.cancelStreamSurfaceWarmupTimer();
    this.streamSurfaceWarmupTimer = setTimeout(() => {
      this.streamSurfaceWarmupTimer = -1;
      if (this.hasSource()) return;
      if (!this.componentId) return;
      if (this.isFullscreen) return;
      if (this.boundSurfaceId !== expectedSurfaceId && this.surfaceId !== expectedSurfaceId) return;

      // Force XComponent recreation to obtain a fresh SurfaceId; this mirrors the effect of the
      // fullscreen surface swap without changing UI state.
      LxNativeVideoPlayer.instanceCounter++;
      const nextId = `${this.componentId}-xc-${LxNativeVideoPlayer.instanceCounter}`;
      log(`[${this.componentId}] stream surface warmup: forcing XComponent recreation: ${this.xComponentId} -> ${nextId}`);
      this.xComponentId = nextId;
    }, 900);
  }

  private startSeekTimer(): void {
    this.clearSeekTimer();
    // Clear isSeeking after a short delay to allow seek to complete
    // This prevents progressTimer from overwriting slider value during seek
    this.seekTimer = setTimeout(() => {
      this.isSeeking = false;
      this.seekTimer = -1;
    }, 300);
  }

  private finishSeeking(): void {
    this.clearSeekTimer();
    this.isSeeking = false;
  }

  /**
   * Mark first frame as rendered - centralized poster state management.
   * Call this when video first renders. Poster will never show again until stop() resets.
   */
  private markFirstFrameRendered(): void {
    if (this.hasRenderedFirstFrame) return;  // Already marked
    this.hasRenderedFirstFrame = true;
    this.hasEverPlayed = true;
    LxNativeVideoPlayer.hasEverRenderedFrame.add(this.componentId);
  }

  private getXComponentType(): XComponentType {
    return XComponentType.TEXTURE;
  }

  private hasSource(): boolean {
    return typeof this.config?.src === 'string' && this.config.src.trim().length > 0;
  }

  private togglePlayPause(): void {
    this.state.isPlaying ? this.pause() : this.play();
    this.resetControlsTimer();
  }

  @Builder
  buildControls(): void {
    VideoControlsView({
      isPlaying: this.state.isPlaying,
      currentTime: this.state.currentTime,
      duration: this.state.duration,
      sliderValue: this.sliderValue,
      volume: this.config.volume,
      muted: this.config.muted,
      controlSize: VideoControlSize.COMPACT,
      // Only show progress bar if explicitly enabled OR in stream mode with valid seekable duration
      // Don't show for live streams (duration == 0 or not set)
      showProgressBar: (this.config.progressBar && !(this.isStreamMode() && this.state.duration <= 0)) || (this.isStreamMode() && this.state.duration > 0),
      showVolumeSlider: true,
      showFullscreenButton: !this.isFullscreen,
      showExitButton: this.isFullscreen,
      showTimeRemaining: true,
      qualities: this.config.qualities.map((q: LxMediaQuality) => q.label),
      currentQuality: this.state.currentQuality || (this.config.qualities.length > 0 ? this.config.qualities[0].label : ''),
      playbackRates: this.config.playbackRates,
      currentPlaybackRate: this.state.currentPlaybackRate,
      onSliderChange: (value: number, mode: SliderChangeMode) => {
        if (mode === SliderChangeMode.Begin) {
          this.clearSeekTimer();
          this.isSeeking = true;
          this.pendingSeekTime = -1;
          this.sliderValue = value;

          // Pause on drag start.
          // This ensures visual feedback and reliable seek state (Stop -> Seek -> Play).
          // If playback has ended, treat it as "was playing" so drag-release resumes playback.
          this.wasPlayingOnDragStart = this.state.isPlaying || this.hasEnded;
          // Force pause if playing, to stop stream buffer consumption and prepare for seek
          if (this.state.isPlaying) {
             this.pause();
          }
        } else if (mode === SliderChangeMode.Moving) {
          this.clearSeekTimer(); this.isSeeking = true; this.sliderValue = value;
          if (this.state.duration > 0) this.state.currentTime = calculateTimeFromSlider(value, this.state.duration);
        } else if (mode === SliderChangeMode.End || mode === SliderChangeMode.Click) {
          this.clearSeekTimer(); this.isSeeking = true;
          // Click mode doesn't trigger Begin, so capture playing state here
          if (mode === SliderChangeMode.Click) {
            this.wasPlayingOnDragStart = this.state.isPlaying || this.hasEnded;
            if (this.state.isPlaying) {
              this.pause();
            }
          }
          if (this.state.duration > 0) {
            const seekTime = calculateTimeFromSlider(value, this.state.duration);
            this.pendingSeekTime = seekTime; this.pendingSeekStartTime = Date.now();
            this.state.currentTime = seekTime; this.sliderValue = value;
            if (this.seekDebounceTimer !== -1) clearTimeout(this.seekDebounceTimer);

            // Resume if it was playing before drag started, OR if playback had ended (user wants to replay from new position)
            const shouldAutoPlay = this.wasPlayingOnDragStart || this.hasEnded;

            this.seekDebounceTimer = setTimeout(() => {
              this.seekDebounceTimer = -1;
              log(`[${this.componentId}] onSliderDragEnd position=${seekTime} wasPlaying=${this.wasPlayingOnDragStart} videoSource=${this.config.src}`);
              this.seek(seekTime);
              if (shouldAutoPlay) {
                this.play();
              }
            }, 100) as number;
          }
          this.startSeekTimer();
        }
      },
      onPlayPauseClick: () => this.togglePlayPause(),
      onMuteClick: () => {
        this.config.muted = !this.config.muted;
        videoPlayerSetVolume(this.componentId, this.config.muted ? 0 : this.config.volume);
      },
      onVolumeChange: (value: number) => {
        this.config.volume = value / 100; this.config.muted = value === 0;
        videoPlayerSetVolume(this.componentId, this.config.volume);
      },
      onFullscreenClick: () => this.toggleFullscreen(),
      onExitClick: () => this.toggleFullscreen(),
      onQualitySelect: (qualityLabel: string) => this.handleQualitySelect(qualityLabel),
      onPlaybackRateSelect: (rate: number) => this.handlePlaybackRateSelect(rate)
    })
  }

  private getPosterUrl(): string {
    // Use cachedPoster first, fallback to config.poster
    return resolveLxUriForFileAccess(this.cachedPoster || this.config.poster || '');
  }

  private isStreamMode(): boolean {
    return !this.hasSource();
  }

  private traceLoadingIndicator(reason: string): void {
    const shouldShow =
      this.state.isBuffering && !this.isPausedByUser && (this.isStreamMode() || this.state.isPlaying);
    if (shouldShow !== this.lastLoadingIndicatorShown) {
      this.lastLoadingIndicatorShown = shouldShow;
      log(
        `[${this.componentId}] loadingIndicator: shown=${shouldShow} reason=${reason} buffering=${this.state.isBuffering} playing=${this.state.isPlaying} paused=${this.state.isPaused} pausedByUser=${this.isPausedByUser} streamMode=${this.isStreamMode()}`
      );
    }
  }

  @Builder
  buildPoster(): void {
    // Poster shows in two scenarios:
    // 1. Cold start (before any frame has ever been rendered)
    // 2. After playback ended AND in fullscreen mode (hasEnded && isFullscreen)
    //
    // PLATFORM DIFFERENCE - Why we show poster on ended+fullscreen (HarmonyOS only):
    // - Android TextureView and iOS CALayer retain the last rendered frame in GPU memory
    // - HarmonyOS XComponent does NOT retain frames - when fullscreen toggles, the component
    //   is destroyed and recreated, and the stream decoder has no frame to re-render
    // - In NON-fullscreen mode, ended keeps the last rendered frame visible (no XComponent rebuild)
    // - In FULLSCREEN mode after ended, XComponent is rebuilt with no frame → BLACK SCREEN
    // - The poster provides a fallback image when the last frame cannot be preserved
    // - If no poster is configured, black screen is unavoidable (XComponent limitation)
    if (this.getPosterUrl() && (!this.hasRenderedFirstFrame || (this.hasEnded && this.isFullscreen))) {
      Image(this.getPosterUrl())
        .width('100%').height('100%').objectFit(ImageFit.Cover)
        .borderRadius(this.isFullscreen ? 0 : this.appliedCornerRadius)
        .zIndex(1).hitTestBehavior(HitTestMode.Transparent)
    }
  }

  @Builder
  buildBufferingIndicator(): void {
    if (this.state.isBuffering && !this.isPausedByUser && (this.isStreamMode() || this.state.isPlaying)) {
      Row() {
        LoadingProgress().width(40).height(40).color(Color.White)
      }
      .width('100%').height('100%').justifyContent(FlexAlign.Center)
      .zIndex(3).hitTestBehavior(HitTestMode.Transparent)
    }
  }

  build() {
    Stack() {
      XComponent({ id: this.xComponentId, type: this.getXComponentType(), controller: this.xComponentController })
        .onLoad(() => this.onSurfaceCreated(this.xComponentController.getXComponentSurfaceId()))
        .onDestroy(() => this.onSurfaceDestroyed(this.surfaceId))
        .width('100%').height('100%')
        .borderRadius((this.isFullscreen || this.isStreamMode()) ? 0 : this.appliedCornerRadius)
        .clip(!this.isFullscreen && !this.isStreamMode())
        .hitTestBehavior(HitTestMode.Transparent).zIndex(0)

      this.buildPoster()
      this.buildBufferingIndicator()

      if (!this.showControls && this.config.controls) {
        Row().width('100%').height('100%').zIndex(5).hitTestBehavior(HitTestMode.Block)
          .onClick(() => { this.showControls = true; this.resetControlsTimer(); })
      }

      if (this.showControls && this.config.controls) {
        Stack() {
          this.buildControls()
        }
        .width('100%').height('100%')
        .zIndex(6)
      }
    }
    .width('100%').height('100%')
    .borderRadius((this.isFullscreen || this.isStreamMode()) ? 0 : this.appliedCornerRadius)
    .clip(this.isFullscreen ? false : !this.isStreamMode()) // Don't clip in fullscreen to allow settings overlay
    .backgroundColor(Color.Black).hitTestBehavior(HitTestMode.Transparent)
  }
}
