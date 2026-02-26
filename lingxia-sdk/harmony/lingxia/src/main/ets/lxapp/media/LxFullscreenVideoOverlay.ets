import { hilog } from '@kit.PerformanceAnalysisKit';
import { common } from '@kit.AbilityKit';
import { display, window } from '@kit.ArkUI';
import { LxNativeVideoPlayer } from './LxNativeVideoPlayer';
import { calculateSliderValue } from './VideoUtils';
import { VideoControlsView, VideoControlSize } from './VideoControls';
import { resolveLxUriForFileAccess } from './LxUri';

const DOMAIN = 0x0000;
const TAG = 'LingXia.FullscreenOverlay';

@Component
export struct LxFullscreenVideoOverlay {
  @Prop componentId: string;
  @Prop webTag: string;
  onClose: () => void = () => {};

  @State private xComponentReady: boolean = false;
  private xComponentController: XComponentController = new XComponentController();
  private surfaceId: string = '';

  @State private isPlaying: boolean = false;
  @State private currentTime: number = 0;
  @State private duration: number = 0;
  @State private volume: number = 0.8;
  @State private muted: boolean = false;
  @State private showControls: boolean = true;
  @State private isSeeking: boolean = false;
  @State private sliderValue: number = 0;
  @State private isLandscape: boolean = false;
  @State private isBuffering: boolean = false;
  @State private hasEnded: boolean = false;  // Track if playback ended (show replay button)
  @State private hasRenderedFirstFrame: boolean = false;
  @State private screenWidthVp: number = 0;
  @State private screenHeightVp: number = 0;
  @State private qualities: string[] = [];
  @State private currentQuality: string = '';
  @State private playbackRates: number[] = [];
  @State private currentPlaybackRate: number = 1.0;

  private controlsTimer: number = -1;
  private progressTimer: number = -1;
  private seekTimer: number = -1;
  private pendingSeekTime: number = -1;
  private pendingSeekStartTime: number = -1;
  private surfaceRefreshTimer1: number = -1;
  private surfaceRefreshTimer2: number = -1;
  private wasPlayingOnEnter: boolean = false;

  aboutToAppear() {
    this.initScreenDimensions();
  }

  aboutToDisappear() {
    this.cleanup();
  }

  private initScreenDimensions(): void {
    try {
      // Fetch fresh display info, do not rely on AppStorage.get which might be outdated or not set
      const defaultDisplay = display.getDefaultDisplaySync();
      const density = defaultDisplay.densityPixels || 1;
      this.screenWidthVp = defaultDisplay.width / density;
      this.screenHeightVp = defaultDisplay.height / density;
    } catch (error) {
      hilog.error(DOMAIN, TAG, `Failed to get screen dimensions: ${error}`);
      this.screenWidthVp = 360; // Fallback value
      this.screenHeightVp = 800; // Fallback value
    }
  }

  private cleanup() {
    this.stopProgressTimer();
    this.clearControlsTimer();
    this.clearSeekTimer();
    this.cancelSurfaceRefreshTimers();
    // Ensure system bar is restored when component is removed (e.g., parent removes overlay directly)
    this.restoreFromFullscreen();
  }

  private cancelSurfaceRefreshTimers(): void {
    if (this.surfaceRefreshTimer1 !== -1) {
      clearTimeout(this.surfaceRefreshTimer1);
      this.surfaceRefreshTimer1 = -1;
    }
    if (this.surfaceRefreshTimer2 !== -1) {
      clearTimeout(this.surfaceRefreshTimer2);
      this.surfaceRefreshTimer2 = -1;
    }
  }

  private scheduleSurfaceRefresh(): void {
    if (!this.componentId || !this.surfaceId) return;
    this.cancelSurfaceRefreshTimers();

    const expectedSurfaceId = this.surfaceId;
    this.surfaceRefreshTimer1 = setTimeout(() => {
      this.surfaceRefreshTimer1 = -1;
      if (this.surfaceId !== expectedSurfaceId) return;
      LxNativeVideoPlayer.switchToFullscreenSurface(this.componentId, expectedSurfaceId);
      hilog.info(DOMAIN, TAG, `Surface refresh 1: componentId=${this.componentId}, surfaceId=${expectedSurfaceId}`);
    }, 180) as number;

    this.surfaceRefreshTimer2 = setTimeout(() => {
      this.surfaceRefreshTimer2 = -1;
      if (this.surfaceId !== expectedSurfaceId) return;
      LxNativeVideoPlayer.switchToFullscreenSurface(this.componentId, expectedSurfaceId);
      hilog.info(DOMAIN, TAG, `Surface refresh 2: componentId=${this.componentId}, surfaceId=${expectedSurfaceId}`);
    }, 800) as number;
  }

  private enterFullscreenMode(): void {
    try {
      const context = getContext(this) as common.UIAbilityContext;
      context.windowStage.getMainWindow().then((mainWindow) => {
        mainWindow.setWindowSystemBarEnable([]);
      });
    } catch (e) {
      hilog.error(DOMAIN, TAG, `Error entering fullscreen mode: ${e}`);
    }
  }

  private restoreFromFullscreen(): void {
    try {
      const context = getContext(this) as common.UIAbilityContext;
      context.windowStage.getMainWindow().then((mainWindow) => {
        mainWindow.setWindowSystemBarEnable(['status', 'navigation']);
      });
    } catch (e) {
      hilog.error(DOMAIN, TAG, `Error restoring from fullscreen: ${e}`);
    }
  }

  private initFullscreenState(): void {
    const state = LxNativeVideoPlayer.getPlaybackState(this.componentId);
    this.isPlaying = state?.isPlaying ?? false;
    this.currentTime = state?.currentTime ?? 0;
    this.hasRenderedFirstFrame = state?.hasRenderedFirstFrame ?? false;
    this.duration = LxNativeVideoPlayer.getDuration(this.componentId);
    this.volume = LxNativeVideoPlayer.getVolume(this.componentId);
    this.muted = LxNativeVideoPlayer.getMuted(this.componentId);
    this.sliderValue = calculateSliderValue(this.currentTime, this.duration);
    hilog.info(DOMAIN, TAG, `initFullscreenState: isPlaying=${this.isPlaying}, hasEnded=${this.hasEnded}, currentTime=${this.currentTime}, duration=${this.duration}`);

    // Get quality and playback rate settings from the player config
    const config = LxNativeVideoPlayer.getConfig(this.componentId);
    if (config) {
      this.qualities = config.qualities?.map((q) => q.label) ?? [];
      this.playbackRates = config.playbackRates ?? [];
    }
    // Get actual current quality and playback rate from player state
    this.currentQuality = LxNativeVideoPlayer.getCurrentQuality(this.componentId) ||
                          (this.qualities.length > 0 ? this.qualities[0] : '');
    this.currentPlaybackRate = LxNativeVideoPlayer.getCurrentPlaybackRate(this.componentId);
  }

  private startProgressTimer(): void {
    if (this.progressTimer !== -1) return;
    this.progressTimer = setInterval(() => {
      if (!this.isSeeking) {
        // Get currentTime from main player's state (handles stream mode time anchor correctly)
        const playbackState = LxNativeVideoPlayer.getPlaybackState(this.componentId);
        const currentTime = playbackState?.currentTime ?? 0;

        if (this.pendingSeekTime >= 0) {
          const tolerance = 500;
          const timedOut = this.pendingSeekStartTime > 0 && (Date.now() - this.pendingSeekStartTime) > 5000;

          if (Math.abs(currentTime - this.pendingSeekTime) < tolerance) {
            this.pendingSeekTime = -1;
            this.pendingSeekStartTime = -1;
          } else if (timedOut) {
            this.pendingSeekTime = -1;
            this.pendingSeekStartTime = -1;
            const duration = LxNativeVideoPlayer.getDuration(this.componentId);
            if (duration > 0) this.sliderValue = calculateSliderValue(currentTime, duration);
          } else {
            return;
          }
        }

        // Sync play/pause state from native player (but don't override if ended)
        if (playbackState) {
          if (!this.hasEnded) {
            this.isPlaying = playbackState.isPlaying;
          }
          this.hasRenderedFirstFrame = playbackState.hasRenderedFirstFrame;
        }

        this.currentTime = currentTime;
        this.isBuffering = LxNativeVideoPlayer.isBuffering(this.componentId);
        const duration = LxNativeVideoPlayer.getDuration(this.componentId);
        if (duration > 0) {
          this.duration = duration;
          this.sliderValue = calculateSliderValue(this.currentTime, duration);
          const isLoop = LxNativeVideoPlayer.isLoopEnabled(this.componentId);
          if (!isLoop && currentTime >= duration - 500) this.onPlaybackEnded();
        }
      }
    }, 250);
  }

  private onPlaybackEnded(): void {
    if (this.hasEnded) return;
    this.isPlaying = false;
    this.hasEnded = true;
    if (this.duration > 0) {
      this.currentTime = this.duration;
      this.sliderValue = 100;
    }
    this.showControls = true;
    this.stopProgressTimer();
    LxNativeVideoPlayer.pauseStatic(this.componentId);
  }

  private stopProgressTimer(): void {
    if (this.progressTimer !== -1) {
      clearInterval(this.progressTimer);
      this.progressTimer = -1;
    }
    this.clearSeekTimer();
  }

  private clearSeekTimer(): void {
    if (this.seekTimer !== -1) {
      clearTimeout(this.seekTimer);
      this.seekTimer = -1;
    }
  }

  private startSeekTimer(): void {
    this.clearSeekTimer();
    this.seekTimer = setTimeout(() => { this.isSeeking = false; this.seekTimer = -1; }, 2000);
  }

  private resetControlsTimer(): void {
    this.clearControlsTimer();
    this.showControls = true;
    this.controlsTimer = setTimeout(() => { this.showControls = false; }, 3000);
  }

  private clearControlsTimer(): void {
    if (this.controlsTimer !== -1) {
      clearTimeout(this.controlsTimer);
      this.controlsTimer = -1;
    }
  }

  @Builder
  private buildControls(): void {
    VideoControlsView({
      isPlaying: this.isPlaying,
      currentTime: this.currentTime,
      duration: this.duration,
      sliderValue: this.sliderValue,
      volume: this.volume,
      muted: this.muted,
      controlSize: VideoControlSize.STANDARD,
      showVolumeSlider: true,
      showFullscreenButton: false,
      showExitButton: true,
      showTimeRemaining: false,
      qualities: this.qualities,
      currentQuality: this.currentQuality,
      playbackRates: this.playbackRates,
      currentPlaybackRate: this.currentPlaybackRate,
      onSliderChange: (value: number, mode: SliderChangeMode) => {
        if (mode === SliderChangeMode.Begin) {
          this.clearSeekTimer();
          this.isSeeking = true;
          this.pendingSeekTime = -1;
          this.sliderValue = value;
        } else if (mode === SliderChangeMode.Moving) {
          this.clearSeekTimer();
          this.isSeeking = true;
          this.sliderValue = value;
          if (this.duration > 0) this.currentTime = (value / 100) * this.duration;
          this.resetControlsTimer();
        } else if (mode === SliderChangeMode.End || mode === SliderChangeMode.Click) {
          this.clearSeekTimer();
          this.isSeeking = true;
          if (this.duration > 0) {
            const seekTime = Math.round((value / 100) * this.duration);
            this.pendingSeekTime = seekTime;
            this.pendingSeekStartTime = Date.now();
            LxNativeVideoPlayer.seekTo(this.componentId, seekTime);
            this.currentTime = seekTime;
            this.sliderValue = value;
          }
          this.startSeekTimer();
          this.resetControlsTimer();
        }
      },
      onPlayPauseClick: () => {
        LxNativeVideoPlayer.togglePlayPause(this.componentId);
        // State will be synced from native player in progress timer
        // Optimistically update for immediate UI feedback
        const willBePlaying = !this.isPlaying;
        this.isPlaying = willBePlaying;
        if (willBePlaying) {
          this.hasEnded = false;
          this.startProgressTimer();
        }
        this.resetControlsTimer();
      },
      onMuteClick: () => {
        this.muted = !this.muted;
        LxNativeVideoPlayer.setVolume(this.componentId, this.muted ? 0 : this.volume);
        this.resetControlsTimer();
      },
      onVolumeChange: (value: number) => {
        this.volume = value / 100;
        this.muted = value === 0;
        LxNativeVideoPlayer.setVolume(this.componentId, this.volume);
        this.resetControlsTimer();
      },
      onExitClick: () => {
        this.stopProgressTimer();
        this.clearControlsTimer();
        LxNativeVideoPlayer.exitFullscreen(this.componentId);
        this.restoreFromFullscreen();
        this.onClose();
      },
      onQualitySelect: (quality: string) => {
        this.currentQuality = quality;
        // Call the player's quality switch logic
        LxNativeVideoPlayer.selectQuality(this.componentId, quality);
        this.resetControlsTimer();
      },
      onPlaybackRateSelect: (rate: number) => {
        this.currentPlaybackRate = rate;
        LxNativeVideoPlayer.selectPlaybackRate(this.componentId, rate);
        this.resetControlsTimer();
      }
    })
  }

  private getPosterUrl(): string {
    return resolveLxUriForFileAccess(LxNativeVideoPlayer.getPoster(this.componentId));
  }

  private getPosterFit(): ImageFit {
    const fit = (LxNativeVideoPlayer.getConfig(this.componentId)?.objectFit || 'cover').toLowerCase();
    // Harmony poster supports contain/cover directly; fill currently falls back to cover.
    return (fit === 'contain' || fit === 'fit') ? ImageFit.Contain : ImageFit.Cover;
  }

  @Builder
  private buildPoster(): void {
    if (this.getPosterUrl() && (!this.hasRenderedFirstFrame || this.hasEnded)) {
      Image(this.getPosterUrl())
        .width('100%').height('100%').objectFit(this.getPosterFit())
        .zIndex(1).hitTestBehavior(HitTestMode.Transparent)
    }
  }

  build() {
    // Landscape: swap dimensions so 90° rotation fills screen
    Stack() {
      Row().width('100%').height('100%').backgroundColor(Color.Black)

      XComponent({
        id: `fullscreen-${this.componentId}`,
        type: XComponentType.TEXTURE,
        controller: this.xComponentController
      })
        .onLoad(() => {
          this.surfaceId = this.xComponentController.getXComponentSurfaceId();
          this.xComponentReady = true;

          // Save state BEFORE switching surface to capture current playback state
          LxNativeVideoPlayer.savePlaybackState(this.componentId);
          const snapshot = LxNativeVideoPlayer.getPlaybackState(this.componentId);
          this.wasPlayingOnEnter = snapshot?.isPlaying ?? false;
          this.hasEnded = LxNativeVideoPlayer.hasEndedStatic(this.componentId);
          hilog.info(DOMAIN, TAG, `onLoad: hasEnded=${this.hasEnded}, wasPlaying=${this.wasPlayingOnEnter}, isPlaying=${this.isPlaying}`);

          LxNativeVideoPlayer.switchToFullscreenSurface(this.componentId, this.surfaceId);
          // Schedule surface refresh with delays to ensure decoder binds correctly
          this.scheduleSurfaceRefresh();

          this.isLandscape = LxNativeVideoPlayer.isLandscapeVideo(this.componentId);

          this.enterFullscreenMode();
          this.initFullscreenState();
          // Always start progress timer in fullscreen (even if ended, user might replay)
          this.startProgressTimer();
          this.resetControlsTimer();

          // For stream mode, explicitly resume playback if it was playing when entering fullscreen.
          // The surface switch may have interrupted the decoder, so we need to ensure it resumes.
          if (this.wasPlayingOnEnter) {
            hilog.info(DOMAIN, TAG, `Resuming playback after fullscreen enter: componentId=${this.componentId}`);
            LxNativeVideoPlayer.playStatic(this.componentId);
            this.isPlaying = true;
          }
        })
        .onDestroy(() => {
          this.xComponentReady = false;
          this.surfaceId = '';
          this.stopProgressTimer();
          this.clearControlsTimer();
          this.cancelSurfaceRefreshTimers();
        })
        .width('100%')
        .height('100%')

      this.buildPoster()

      // Buffering indicator
      if (this.isBuffering && this.isPlaying) {
        Row() {
          LoadingProgress().width(48).height(48).color(Color.White)
        }
        .width('100%').height('100%').justifyContent(FlexAlign.Center)
        .hitTestBehavior(HitTestMode.Transparent)
      }



      if (this.showControls) {
        this.buildControls()
      }
    }
    .width(this.isLandscape ? this.screenHeightVp : this.screenWidthVp)
    .height(this.isLandscape ? this.screenWidthVp : this.screenHeightVp)
    .position(this.isLandscape ?
      { x: (this.screenWidthVp - this.screenHeightVp) / 2, y: (this.screenHeightVp - this.screenWidthVp) / 2 } :
      { x: 0, y: 0 })
    .zIndex(50000)
    .expandSafeArea([SafeAreaType.SYSTEM], [SafeAreaEdge.TOP, SafeAreaEdge.BOTTOM])
    .rotate({
      angle: this.isLandscape ? 90 : 0,
      centerX: '50%',
      centerY: '50%'
    })
    .onClick(() => {
      this.showControls = !this.showControls;
      if (this.showControls) {
        this.resetControlsTimer();
      }
    })
  }
}
