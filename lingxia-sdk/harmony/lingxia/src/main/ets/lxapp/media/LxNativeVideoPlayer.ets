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
import { calculateSliderValue } from './VideoUtils';
import { VideoControlsView, VideoControlSize } from './VideoControls';

const TAG = 'LingXia.NativeVideoPlayer';
function log(message: string): void { console.info(`${TAG}: ${message}`); }

// Event detail types
export class VideoErrorDetail { message: string = ''; }
export class VideoSeekingDetail { currentTime: number = 0; }
export class VideoFullscreenDetail {
  fullScreen: boolean = false;
  direction: string = 'horizontal';
}
export class VideoMetadataDetail {
  width: number = 0;
  height: number = 0;
  duration: number = 0;
}
export class VideoTimeUpdateDetail {
  currentTime: number = 0;
  duration: number = 0;
}
export class VideoEmptyDetail {}
export class LxMediaQualityChangeDetail {
  quality: string = '';
  url?: string = '';
}
export class LxMediaRateChangeDetail {
  rate: number = 1.0;
}

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
  currentTime: number = 0;
  duration: number = 0;
}

@Component
export struct LxNativeVideoPlayer {
  @Prop playerId: string = '';
  @Prop config: LxNativeVideoConfig = new LxNativeVideoConfig();
  @State state: LxNativeVideoState = new LxNativeVideoState();
  @State showControls: boolean = true;
  @State isFullscreen: boolean = false;
  @State private appliedCornerRadius: number = 0;
  @State private isSeeking: boolean = false;
  @State private sliderValue: number = 0;
  @State private showPoster: boolean = true;  // Show poster until first frame ready
  private cachedPoster: string = '';  // Cache poster URL since config.poster may be lost on rebuild
  private cachedAutoplay: boolean = false;  // Cache autoplay since config may be lost on rebuild
  private autoplaySuppressed: boolean = false;  // Block autoplay if paused before prepare
  private isPausedByUser: boolean = false;  // Track if user explicitly paused (vs buffering)
  private pendingSeekTime: number = -1;
  private pendingSeekStartTime: number = -1;
  private seekDebounceTimer: number = -1;  // Debounce rapid seeks
  private hasEnded: boolean = false;  // Track if video has ended (to prevent re-trigger)
  private pendingPlayEvent: boolean = false;  // Delay play event until video actually starts
  private lastTimeForPlayEvent: number = -1;  // Track time progression for play event
  
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
  private surfaceId: string = '';
  private playerCreated: boolean = false;
  private boundSurfaceId: string = '';
  private streamVideoSize?: StreamVideoSize;
  private static keepAliveIds: Set<string> = new Set();
  private static runtimeState: Map<string, LxNativeVideoRuntimeSnapshot> = new Map();
  private static nativePlayerExists: Set<string> = new Set();
  private static rebindInstance: Map<string, LxNativeVideoPlayer> = new Map();
  private static originalSurfaceIds: Map<string, string> = new Map();
  private static streamVideoSizes: Map<string, StreamVideoSize> = new Map();
  private static instanceCounter: number = 0;  // Global counter to generate unique XComponent IDs
  onEvent?: (event: string, detail: object) => void;

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

    const snapshot = LxNativeVideoPlayer.runtimeState.get(componentId);
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

  static onBufferingChange(componentId: string, isBuffering: boolean): void {
    LxNativeVideoPlayer.rebindInstance.get(componentId)?.handleBufferingChange(isBuffering);
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

  static getDuration(componentId: string): number {
    let duration = LxNativeVideoPlayer.rebindInstance.get(componentId)?.getDuration() ?? 0;
    if (duration <= 0) duration = LxNativeVideoPlayer.runtimeState.get(componentId)?.duration ?? 0;
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
        videoPlayerSetUrl(this.componentId, selectedQuality.url);
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
    this.emitEvent('playbackratechange', detail);
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
    snapshot.currentTime = this.getCurrentTime();
    snapshot.duration = this.state.duration;
    LxNativeVideoPlayer.runtimeState.set(this.componentId, snapshot);
  }

  private restoreRuntimeState(): void {
    if (!this.componentId) return;
    const snapshot = LxNativeVideoPlayer.runtimeState.get(this.componentId);
    if (snapshot) {
      this.state.isPlaying = snapshot.isPlaying;
      this.state.currentTime = snapshot.currentTime;
      this.state.duration = snapshot.duration;
      // Also restore slider value to prevent stale display
      this.sliderValue = calculateSliderValue(snapshot.currentTime, snapshot.duration);
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
    log(`[${this.componentId}] aboutToAppear: poster=${this.config.poster}, cachedPoster=${this.cachedPoster}, autoplay=${this.config.autoplay}, cachedAutoplay=${this.cachedAutoplay}, progressBar=${this.config.progressBar}, showPoster=${this.showPoster}, xComponentId=${this.xComponentId}, playerCreated=${this.playerCreated}, boundSurfaceId=${this.boundSurfaceId}, hasNativePlayer=${LxNativeVideoPlayer.hasNativePlayer(this.componentId)}, currentQuality=${this.state.currentQuality}, hasOnEvent=${!!this.onEvent}`);

    // Reset state for clean recreation (same-level remount may skip aboutToDisappear)
    this.playerCreated = false;
    this.boundSurfaceId = '';
    LxNativeVideoPlayer.setNativePlayerExists(this.componentId, false);
    LxNativeVideoPlayer.setKeepAlive(this.componentId, false);
    LxNativeVideoPlayer.runtimeState.delete(this.componentId);

    this.restoreRuntimeState();
    LxNativeVideoPlayer.rebindInstance.set(this.componentId, this);

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
    this.appliedCornerRadius = 0;
    LxNativeVideoPlayer.rebindInstance.delete(this.componentId);
    if (this.playerCreated) this.destroyPlayer();
  }

  private lastCurrentTimeForPoster: number = -1;

  private startProgressTimer(): void {
    if (this.progressTimer !== -1) return;
    this.lastCurrentTimeForPoster = -1;  // Reset on timer start
    this.lastTimeForPlayEvent = -1;  // Reset for play event detection
    this.progressTimer = setInterval(() => {
      const currentTime = this.getCurrentTime();
      const duration = this.getDuration();

      if (!this.config.loop && !this.hasEnded && duration > 0 && currentTime >= duration - 500) {
        log(`[${this.componentId}] progressTimer: detected playback end, currentTime=${currentTime}, duration=${duration}`);
        this.onPlaybackEnded();
        return;  // Don't process further after ended
      }

      // Emit play event when video actually starts playing (time progresses)
      if (this.pendingPlayEvent && duration > 0) {
        const timeAdvanced = this.lastTimeForPlayEvent >= 0 && currentTime > this.lastTimeForPlayEvent;
        if (currentTime > 0 && timeAdvanced) {
          log(`[${this.componentId}] progressTimer: emitting delayed play event, currentTime=${currentTime}, lastTime=${this.lastTimeForPlayEvent}`);
          this.pendingPlayEvent = false;
          this.emitEvent('play', new VideoEmptyDetail());
        }
        this.lastTimeForPlayEvent = currentTime;
      }

      // Hide poster when video is rendering frames (prevents black screen flash)
      if (this.showPoster && this.state.isPlaying && duration > 0) {
        const timeAdvanced = this.lastCurrentTimeForPoster >= 0 && currentTime > this.lastCurrentTimeForPoster;
        if (currentTime > 200 && timeAdvanced) {
          log(`[${this.componentId}] progressTimer: hiding poster, currentTime=${currentTime}, lastTime=${this.lastCurrentTimeForPoster}, duration=${duration}`);
          this.showPoster = false;
        }
        this.lastCurrentTimeForPoster = currentTime;
      }

      if (duration > 0 && this.state.duration !== duration) {
        this.state.duration = duration;
      }

      if (!this.isSeeking && currentTime >= 0) {
        // Wait for pending seek to complete (native callback or timeout fallback)
        if (this.pendingSeekTime >= 0 && duration > 0) {
          const elapsed = Date.now() - this.pendingSeekStartTime;
          if (this.pendingSeekStartTime > 0 && elapsed > 8000) {
            log(`[${this.componentId}] progress: seek timed out, syncing to native=${currentTime}`);
            this.pendingSeekTime = -1;
            this.pendingSeekStartTime = -1;
            this.state.currentTime = currentTime;
            this.sliderValue = calculateSliderValue(currentTime, duration);
          } else {
            return; // Keep slider at target position until seek completes
          }
        } else {
          this.state.currentTime = currentTime;
          if (duration > 0) this.sliderValue = calculateSliderValue(currentTime, duration);
          // Emit timeupdate event for JS tracking (enables relative seek)
          // Convert ms to seconds for consistency with Android/HTML5 video API
          if (duration > 0) {
            const timeUpdateDetail = new VideoTimeUpdateDetail();
            timeUpdateDetail.currentTime = currentTime / 1000;
            timeUpdateDetail.duration = duration / 1000;
            this.emitEvent('timeupdate', timeUpdateDetail);
          }
        }
        if (this.state.isPlaying) this.saveRuntimeState();
      }
    }, 250);
  }

  private onPlaybackEnded(): void {
    log(`[${this.componentId}] onPlaybackEnded() called, loop=${this.config.loop}, hasEnded=${this.hasEnded}`);
    if (this.config.loop || this.hasEnded) return;
    this.hasEnded = true;

    this.state.isPlaying = false;
    this.state.isPaused = true;
    // Cancel pending play event on end
    this.pendingPlayEvent = false;
    this.showPoster = true;  // Show poster again when video ends
    log(`[${this.componentId}] onPlaybackEnded() showPoster=true`);
    this.stopProgressTimer();
    this.saveRuntimeState();  // Save ended state
    this.emitEvent('ended', new VideoEmptyDetail());
  }

  static hasEndedStatic(componentId: string): boolean {
    const instance = LxNativeVideoPlayer.rebindInstance.get(componentId);
    if (!instance) return false;
    return instance.hasEnded || (instance.showPoster && !instance.state.isPlaying);
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

    this.boundSurfaceId = surfaceId;
    this.surfaceId = surfaceId;
    if (!this.hasSource()) {
      videoPlayerStoreSurface(this.componentId, surfaceId);
      videoPlayerRebindStreamSurface(this.componentId, surfaceId);
      return;
    }
    if (!LxNativeVideoPlayer.hasNativePlayer(this.componentId)) {
      this.ensurePlayer(surfaceId, false, undefined);
    }
  }

  private onSurfaceDestroyed(surfaceId: string): void {
    // XComponent surfaces can be recreated; ArkUI may report onDestroy for an old surface after a new
    // one is already created/bound. Only clear the stored surface when the destroyed surface is the
    // currently bound surface; otherwise we can accidentally clear the active surface and break
    // stream playback (Surface not set).
    const isCurrentSurface = this.surfaceId === surfaceId || this.boundSurfaceId === surfaceId;
    if (!isCurrentSurface) return;

    // Clear any existing pending clear operation
    this.cancelPendingSurfaceClear();

    // Delay clearing to allow temporary detach/reattach (similar to Android's 900ms pattern)
    // This improves robustness during navigation or layout changes
    this.clearSurfaceTimer = setTimeout(() => {
      // Double-check that the surface is still the one we expect to clear
      // (a new surface might have been created and bound during the delay)
      const stillCurrentSurface = this.surfaceId === surfaceId || this.boundSurfaceId === surfaceId;
      if (stillCurrentSurface) {
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
      this.emitEvent('error', { message: 'Failed to create native player' } as VideoErrorDetail);
      return;
    }
    this.playerCreated = true;
    LxNativeVideoPlayer.setNativePlayerExists(this.componentId, true);

    if (this.config.src) {
      if (!(videoPlayerSetUrl(this.componentId, this.config.src) as boolean)) {
        this.emitEvent('error', { message: 'Failed to set URL source' } as VideoErrorDetail);
        return;
      }
    }

    const targetSurface = surfaceId || this.boundSurfaceId || this.surfaceId;
    if (targetSurface) {
      if (!(videoPlayerSetSurface(this.componentId, targetSurface) as boolean)) {
        this.emitEvent('error', { message: 'Failed to set video surface' } as VideoErrorDetail);
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

    log(`[${this.componentId}] createPlayer done, autoplay=${this.config.autoplay}, showPoster=${this.showPoster}`);
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
  }

  play(): void {
    log(`[${this.componentId}] play() called, showPoster=${this.showPoster}`);
    // Explicit play request should override any pending autoplay suppression
    this.autoplaySuppressed = false;
    this.isPausedByUser = false;  // User wants to play
    if (videoPlayerPlay(this.componentId) as boolean) {
      this.state.isPlaying = true;
      this.state.isPaused = false;
      this.hasEnded = false;
      // Delay play event until video actually starts playing (time progresses)
      this.pendingPlayEvent = true;
      this.lastTimeForPlayEvent = -1;
      log(`[${this.componentId}] play() success, showPoster=${this.showPoster} (play event will emit when time progresses)`);
      this.startProgressTimer();
    }
  }

  pause(): void {
    this.isPausedByUser = true;  // User explicitly paused
    if (videoPlayerPause(this.componentId) as boolean) {
      this.state.isPlaying = false;
      this.state.isPaused = true;
      this.state.isBuffering = false;  // Hide buffering indicator immediately
      this.autoplaySuppressed = true;
      // Cancel pending play event if pause is called before video started
      this.pendingPlayEvent = false;
      this.stopProgressTimer();
      this.saveRuntimeState();
      this.emitEvent('pause', new VideoEmptyDetail());
    }
  }

  stop(): void {
    log(`[${this.componentId}] stop() called`);
    if (videoPlayerStop(this.componentId) as boolean) {
      // Reset playback state
      this.state.isPlaying = false;
      this.state.isPaused = true;
      this.autoplaySuppressed = true;
      this.state.currentTime = 0;
      this.sliderValue = 0;
      this.hasEnded = false;
      this.pendingPlayEvent = false;
      // Show poster and center play button (reset to initial state)
      this.showPoster = true;
      this.stopProgressTimer();
      this.saveRuntimeState();
      log(`[${this.componentId}] stop() success, reset to initial state`);
    }
  }

  seek(time: number): void {
    const timeInt = Math.round(time);
    // Track pending seek for UI sync
    this.pendingSeekTime = timeInt;
    this.pendingSeekStartTime = Date.now();
    if (videoPlayerSeek(this.componentId, timeInt) as boolean) {
      const seekDetail = new VideoSeekingDetail();
      seekDetail.currentTime = timeInt;
      this.emitEvent('seeking', seekDetail);
    }
  }

  handleSeekDone(positionMs: number): void {
    log(`[${this.componentId}] handleSeekDone: position=${positionMs}, pending=${this.pendingSeekTime}`);
    this.pendingSeekTime = -1;
    this.pendingSeekStartTime = -1;

    let actualPosition = positionMs > 0 ? positionMs : this.getCurrentTime();
    this.state.currentTime = actualPosition;
    const duration = this.getDuration();
    if (duration > 0) this.sliderValue = calculateSliderValue(actualPosition, duration);

    const seekDetail = new VideoSeekingDetail();
    seekDetail.currentTime = actualPosition;
    this.emitEvent('seeked', seekDetail);
    this.saveRuntimeState();
  }

  handleBufferingChange(isBuffering: boolean): void {
    log(`[${this.componentId}] handleBufferingChange: isBuffering=${isBuffering}, isPlaying=${this.state.isPlaying}, isPaused=${this.state.isPaused}`);
    this.state.isBuffering = isBuffering;

    if (isBuffering) {
      // Only emit waiting event if player is in playing state (not paused by user)
      if (this.state.isPlaying && !this.isPausedByUser) {
        log(`[${this.componentId}] handleBufferingChange: emitting waiting event`);
        this.emitEvent('waiting', new VideoEmptyDetail());
      }
    } else {
      // Buffering ended - emit appropriate event based on current state
      if (this.state.isPlaying) {
        // Emit play event when buffering ends and video resumes (HTML5 "playing" event equivalent)
        log(`[${this.componentId}] handleBufferingChange: emitting play event (buffering ended)`);
        this.emitEvent('play', new VideoEmptyDetail());
      } else if (this.state.isPaused) {
        // Buffering ended but paused - re-emit pause to clear buffering status
        log(`[${this.componentId}] handleBufferingChange: emitting pause event (buffering ended)`);
        this.emitEvent('pause', new VideoEmptyDetail());
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
    metadataDetail.duration = videoPlayerGetDuration(this.componentId);
    log(`[${this.componentId}] handlePrepared: metadata width=${metadataDetail.width}, height=${metadataDetail.height}, duration=${metadataDetail.duration}`);
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
    this.state.currentTime = 0;
    this.sliderValue = 0;
    this.hasEnded = false;
    this.pendingPlayEvent = false;
    this.showPoster = true;
    this.stopProgressTimer();
    this.saveRuntimeState();
  }

  handlePlaying(): void {
    log(`[${this.componentId}] handlePlaying: updating UI for playback`);
    this.state.isPlaying = true;
    this.state.isPaused = false;
    this.state.isBuffering = false;  // Clear buffering when playing resumes
    this.hasEnded = false;
    if (!this.config?.src || this.config.src.trim().length === 0) {
      this.showPoster = false;
    }
    this.startProgressTimer();
    this.saveRuntimeState();
    // Emit play event to JS
    this.emitEvent('play', new VideoEmptyDetail());
  }

  handlePaused(): void {
    log(`[${this.componentId}] handlePaused: updating UI for paused state`);
    this.state.isPlaying = false;
    this.state.isPaused = true;
    this.state.isBuffering = false;  // Clear buffering when paused
    this.stopProgressTimer();
    this.saveRuntimeState();
    // Emit pause event to JS
    this.emitEvent('pause', new VideoEmptyDetail());
  }

  getCurrentTime(): number { return videoPlayerGetCurrentTime(this.componentId); }
  getDuration(): number { return videoPlayerGetDuration(this.componentId); }

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

  private emitEvent(event: string, detail: VideoErrorDetail | VideoSeekingDetail | VideoFullscreenDetail | VideoMetadataDetail | VideoTimeUpdateDetail | VideoEmptyDetail | LxMediaQualityChangeDetail | LxMediaRateChangeDetail): void {
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

  private startSeekTimer(): void {
    this.clearSeekTimer();
    this.seekTimer = setTimeout(() => { this.isSeeking = false; this.seekTimer = -1; }, 300);
  }

  private getXComponentType(): XComponentType {
    if (!this.hasSource()) {
      return XComponentType.SURFACE;
    }
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
      showProgressBar: this.config.progressBar,
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
          this.clearSeekTimer(); this.isSeeking = true; this.pendingSeekTime = -1; this.sliderValue = value;
        } else if (mode === SliderChangeMode.Moving) {
          this.clearSeekTimer(); this.isSeeking = true; this.sliderValue = value;
          if (this.state.duration > 0) this.state.currentTime = (value / 100) * this.state.duration;
        } else if (mode === SliderChangeMode.End || mode === SliderChangeMode.Click) {
          this.clearSeekTimer(); this.isSeeking = true;
          if (this.state.duration > 0) {
            const seekTime = Math.round((value / 100) * this.state.duration);
            this.pendingSeekTime = seekTime; this.pendingSeekStartTime = Date.now();
            this.state.currentTime = seekTime; this.sliderValue = value;
            if (this.seekDebounceTimer !== -1) clearTimeout(this.seekDebounceTimer);
            this.seekDebounceTimer = setTimeout(() => { this.seekDebounceTimer = -1; this.seek(seekTime); }, 100) as number;
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
    return this.cachedPoster || this.config.poster || '';
  }

  private lastPosterLogState: string = '';

  private shouldShowPoster(): boolean {
    const posterUrl = this.getPosterUrl();
    const show = !!(posterUrl && this.showPoster);

    // 只在状态变化时打印日志，避免日志刷屏
    const currentState = `${posterUrl ? 'set' : 'empty'}:${this.showPoster}:${show}`;
    if (currentState !== this.lastPosterLogState) {
      log(`[${this.componentId}] shouldShowPoster: posterUrl=${posterUrl ? 'set' : 'empty'}, showPoster=${this.showPoster}, result=${show}`);
      this.lastPosterLogState = currentState;
    }

    return show;
  }

  private isStreamMode(): boolean {
    return !this.hasSource();
  }

  @Builder
  buildPoster(): void {
    if (this.shouldShowPoster()) {
      Image(this.getPosterUrl())
        .width('100%').height('100%').objectFit(ImageFit.Cover)
        .borderRadius(this.isFullscreen ? 0 : this.appliedCornerRadius)
        .zIndex(1).hitTestBehavior(HitTestMode.Transparent)
    }
  }

  @Builder
  buildBufferingIndicator(): void {
    if (this.state.isBuffering && this.state.isPlaying) {
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

      if (this.showControls && this.config.controls) { this.buildControls() }
    }
    .width('100%').height('100%')
    .borderRadius((this.isFullscreen || this.isStreamMode()) ? 0 : this.appliedCornerRadius)
    .clip(this.isFullscreen ? false : !this.isStreamMode()) // Don't clip in fullscreen to allow settings overlay
    .backgroundColor(Color.Black).hitTestBehavior(HitTestMode.Transparent)
  }
}
