import { hilog } from '@kit.PerformanceAnalysisKit';
import { common } from '@kit.AbilityKit';
import { display, window } from '@kit.ArkUI';
import { LxNativeVideoPlayer } from './LxNativeVideoPlayer';
import { calculateSliderValue } from './VideoUtils';
import { VideoControlsView, VideoControlSize } from './VideoControls';

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
  @State private showPoster: boolean = false;
  @State private posterUrl: string = '';
  @State private isBuffering: boolean = false;
  @State private screenWidthVp: number = 0;
  @State private screenHeightVp: number = 0;

  private controlsTimer: number = -1;
  private progressTimer: number = -1;
  private seekTimer: number = -1;
  private pendingSeekTime: number = -1;
  private pendingSeekStartTime: number = -1;

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
    // Ensure system bar is restored when component is removed (e.g., parent removes overlay directly)
    this.restoreFromFullscreen();
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
    this.duration = LxNativeVideoPlayer.getDuration(this.componentId);
    this.volume = LxNativeVideoPlayer.getVolume(this.componentId);
    this.muted = LxNativeVideoPlayer.getMuted(this.componentId);
    this.sliderValue = calculateSliderValue(this.currentTime, this.duration);
  }

  private startProgressTimer(): void {
    if (this.progressTimer !== -1) return;
    this.progressTimer = setInterval(() => {
      if (!this.isSeeking) {
        const currentTime = LxNativeVideoPlayer.getCurrentTimeStatic(this.componentId);

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
    if (this.showPoster) return;
    this.isPlaying = false;
    this.showPoster = true;
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
        this.isPlaying = !this.isPlaying;
        if (this.isPlaying) {
          this.showPoster = false;
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
      }
    })
  }

  build() {
    // Landscape: swap dimensions so 90° rotation fills screen
    Stack() {
      Row().width('100%').height('100%').backgroundColor(Color.Black)

      XComponent({
        id: `fullscreen-${this.componentId}`,
        type: XComponentType.SURFACE,
        controller: this.xComponentController
      })
        .onLoad(() => {
          this.surfaceId = this.xComponentController.getXComponentSurfaceId();
          this.xComponentReady = true;

          this.posterUrl = LxNativeVideoPlayer.getPoster(this.componentId);
          const hasEnded = LxNativeVideoPlayer.hasEndedStatic(this.componentId);
          this.showPoster = hasEnded;

          LxNativeVideoPlayer.savePlaybackState(this.componentId);
          LxNativeVideoPlayer.switchToFullscreenSurface(this.componentId, this.surfaceId);

          this.isLandscape = LxNativeVideoPlayer.isLandscapeVideo(this.componentId);

          this.enterFullscreenMode();
          this.initFullscreenState();
          if (!hasEnded) {
            this.startProgressTimer();
          }
          this.resetControlsTimer();
        })
        .onDestroy(() => {
          this.xComponentReady = false;
          this.surfaceId = '';
          this.stopProgressTimer();
          this.clearControlsTimer();
        })
        .width('100%')
        .height('100%')

      if (this.posterUrl && this.showPoster) {
        Image(this.posterUrl)
          .width('100%').height('100%').objectFit(ImageFit.Cover)
          .hitTestBehavior(HitTestMode.Transparent)
      }

      // Buffering indicator
      if (this.isBuffering && this.isPlaying) {
        Row() {
          LoadingProgress().width(48).height(48).color(Color.White)
        }
        .width('100%').height('100%').justifyContent(FlexAlign.Center)
        .hitTestBehavior(HitTestMode.Transparent)
      }

      if (this.showPoster && !this.isPlaying) {
        Row() {
          Image($r('sys.media.ohos_ic_public_play'))
            .width(40).height(40).fillColor(Color.White)
        }
        .width(72).height(72).justifyContent(FlexAlign.Center).borderRadius(36)
        .backgroundColor('rgba(0,0,0,0.5)')
        .onClick(() => {
          this.showPoster = false;
          this.isPlaying = true;
          LxNativeVideoPlayer.playStatic(this.componentId);
          this.startProgressTimer();
          this.resetControlsTimer();
        })
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
