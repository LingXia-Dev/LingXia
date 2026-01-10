import { hilog } from '@kit.PerformanceAnalysisKit';
import { display, window } from '@kit.ArkUI';
import {
  LxNativeVideoPlayer,
  LxNativeVideoConfig,
  LxNativeVideoState,
  VideoErrorDetail
} from '../../media/LxNativeVideoPlayer';
import { VideoControlsView, VideoControlSize } from '../../media/VideoControls';
import { videoPlayerSetVolume } from 'liblingxia.so';

export interface MediaPreviewItem {
  id: string;
  uri: string;
  mediaType?: number; // 0 = image, 1 = video
  width?: number;  // Video width (optional, will be detected)
  height?: number; // Video height (optional, will be detected)
}

export interface MediaPreviewPageParams {
  items: MediaPreviewItem[];
  initialIndex?: number;
}

const DOMAIN = 0x0000;
const TAG = 'LingXia.MediaPreviewOverlay';

/**
 * Media preview overlay manager
 */
export class MediaPreviewOverlayManager {
  private static isShowing: boolean = false;
  private static overlayData: MediaPreviewPageParams | null = null;

  private static listeners: Set<() => void> = new Set();

  /**
   * Show media preview overlay
   */
  public static show(params: MediaPreviewPageParams): void {
    MediaPreviewOverlayManager.overlayData = params;
    MediaPreviewOverlayManager.isShowing = true;
    hilog.info(DOMAIN, TAG, `Showing media preview overlay with ${params.items.length} items`);
    MediaPreviewOverlayManager.notifyListeners();
  }

  /**
   * Hide media preview overlay
   */
  public static hide(): void {
    MediaPreviewOverlayManager.isShowing = false;
    MediaPreviewOverlayManager.overlayData = null;
    hilog.info(DOMAIN, TAG, 'Hiding media preview overlay');
    MediaPreviewOverlayManager.notifyListeners();
  }

  /**
   * Get current showing status
   */
  public static isOverlayShowing(): boolean {
    return MediaPreviewOverlayManager.isShowing;
  }

  /**
   * Get current overlay data
   */
  public static getOverlayData(): MediaPreviewPageParams | null {
    return MediaPreviewOverlayManager.overlayData;
  }

  public static registerListener(listener: () => void): void {
    MediaPreviewOverlayManager.listeners.add(listener);
  }

  public static unregisterListener(listener: () => void): void {
    MediaPreviewOverlayManager.listeners.delete(listener);
  }

  private static notifyListeners(): void {
    MediaPreviewOverlayManager.listeners.forEach(listener => listener());
  }
}

/**
 * Media preview overlay component
 * For integration by host application
 */
@Component
export struct MediaPreviewOverlay {
  @State private isShowing: boolean = MediaPreviewOverlayManager.isOverlayShowing();
  @State private overlayData: MediaPreviewPageParams | null = MediaPreviewOverlayManager.getOverlayData();
  
  private stateChangeListener = () => {
    const newShowing = MediaPreviewOverlayManager.isOverlayShowing();
    const newData = MediaPreviewOverlayManager.getOverlayData();

    // Detect show/hide transitions
    if (newShowing && !this.isShowing && newData) {
      this.enterFullscreenMode(newData);
    } else if (!newShowing && this.isShowing) {
      this.exitFullscreenMode();
    }

    this.isShowing = newShowing;
    this.overlayData = newData;
  }

  aboutToAppear() {
    MediaPreviewOverlayManager.registerListener(this.stateChangeListener);
  }

  aboutToDisappear() {
    MediaPreviewOverlayManager.unregisterListener(this.stateChangeListener);
  }

  private async enterFullscreenMode(data: MediaPreviewPageParams): Promise<void> {
    try {
      const windowClass = await window.getLastWindow(getContext(this));
      // Hide system bars for immersive experience
      await windowClass.setWindowSystemBarEnable([]);
      // Video orientation will be set by MediaViewer based on video dimensions
      hilog.info(DOMAIN, TAG, 'Entered fullscreen mode, waiting for video orientation detection');
    } catch (error) {
      hilog.error(DOMAIN, TAG, `Failed to enter fullscreen mode: ${error}`);
    }
  }

  private async exitFullscreenMode(): Promise<void> {
    try {
      const windowClass = await window.getLastWindow(getContext(this));
      // Restore system bars and orientation
      await windowClass.setWindowSystemBarEnable(['status', 'navigation']);
      await windowClass.setPreferredOrientation(window.Orientation.UNSPECIFIED);
      hilog.info(DOMAIN, TAG, 'Exited fullscreen mode, restored orientation');
    } catch (error) {
      hilog.error(DOMAIN, TAG, `Failed to exit fullscreen mode: ${error}`);
    }
  }

  private handleClose(): void {
    this.exitFullscreenMode();
    MediaPreviewOverlayManager.hide();
  }

  build() {
    if (this.isShowing && this.overlayData) {
      Stack() {
        // Semi-transparent background
        Column()
          .width('100%')
          .height('100%')
          .backgroundColor(Color.Black)

        // Media preview content
        MediaPreviewContent({
          params: this.overlayData,
          onClose: () => this.handleClose()
        })
          .width('100%')
          .height('100%')
      }
      .width('100%')
      .height('100%')
      .position({ x: 0, y: 0 })
      .zIndex(9999)
    }
  }
}

/**
 * Media preview content component
 */
@Component
struct MediaPreviewContent {
  @Prop params: MediaPreviewPageParams;
  onClose: () => void = () => {};

  @State private currentIndex: number = 0;
  @State private statusBarHeight: number = 44;
  @State private isZoomed: boolean = false;

  aboutToAppear() {
    this.currentIndex = this.params.initialIndex || 0;
    this.initStatusBarHeight();
  }

  private async initStatusBarHeight(): Promise<void> {
    try {
      const windowClass: window.Window = await window.getLastWindow(getContext());
      const safeArea: window.AvoidArea = await windowClass.getWindowAvoidArea(window.AvoidAreaType.TYPE_SYSTEM);
      this.statusBarHeight = safeArea.topRect.height || 44;
    } catch (error) {
      hilog.error(DOMAIN, TAG, `Failed to get status bar height: ${error}`);
    }
  }

  build() {
    Stack({ alignContent: Alignment.Top }) {
      // Indicator - always show when there are multiple items
      if (this.params.items.length > 1) {
        Text(`${this.currentIndex + 1}/${this.params.items.length}`)
          .fontColor(Color.White)
          .fontSize(18)
          .fontWeight(FontWeight.Bold)
          .margin({ top: 60 })
          .width('100%')
          .textAlign(TextAlign.Center)
          .zIndex(10)
      }

      // Media display area (supports both images and videos)
      if (this.params.items.length > 0) {
        Stack() {
          MediaViewer({
            item: this.params.items[this.currentIndex],
            onZoomStateChanged: (zoomed: boolean) => this.isZoomed = zoomed,
            onDismiss: this.onClose
          })
        }
        .width('100%')
        .height('100%')
        .gesture(
          PanGesture({ fingers: 1, direction: PanDirection.Horizontal })
            .onActionEnd((event: GestureEvent) => {
              // Disable swipe navigation when zoomed
              if (!this.isZoomed) {
                const offsetX = event.offsetX;
                if (offsetX > 50 && this.currentIndex > 0) {
                  this.currentIndex--;
                  this.isZoomed = false; // Reset zoom state when changing items
                } else if (offsetX < -50 && this.currentIndex < this.params.items.length - 1) {
                  this.currentIndex++;
                  this.isZoomed = false; // Reset zoom state when changing items
                }
              }
            })
        )
      }
    }
  }
}

/**
 * Media viewer component (supports both images and videos)
 * Uses LxNativeVideoPlayer for video playback (same as video component)
 * Automatically detects video orientation and rotates content for landscape videos
 */
@Component
struct MediaViewer {
  @Prop item: MediaPreviewItem;
  onZoomStateChanged: (zoomed: boolean) => void = () => {};
  onDismiss: () => void = () => {};

  @State private imageScale: number = 1;
  // Video player state
  @State private videoState: LxNativeVideoState = new LxNativeVideoState();
  @State private showControls: boolean = true;
  @State private isSeeking: boolean = false;
  @State private sliderValue: number = 0;
  @State private playerId: string = '';
  @State private videoConfig: LxNativeVideoConfig = new LxNativeVideoConfig();
  // Screen dimensions and orientation for landscape video rotation
  @State private screenWidthVp: number = 0;
  @State private screenHeightVp: number = 0;
  @State private isLandscapeVideo: boolean = false;
  @State private orientationDetected: boolean = false;
  private readonly minScale: number = 1;
  private readonly maxScale: number = 3;
  private lastTapTime: number = 0;
  private controlsTimer: number = -1;
  private progressTimer: number = -1;

  aboutToAppear() {
    // Reset zoom when a new item is displayed
    this.resetZoom();
    // Initialize screen dimensions for landscape rotation
    this.initScreenDimensions();
    // Generate unique player ID and config for videos
    if (this.item.mediaType === 1) {
      this.playerId = `preview-video-${Date.now()}-${Math.random().toString(36).substring(2, 8)}`;
      // Initialize video config once
      this.videoConfig.src = this.item.uri;
      this.videoConfig.autoplay = true;
      this.videoConfig.loop = false;
      this.videoConfig.muted = false;
      this.videoConfig.volume = 1.0;
      this.videoConfig.controls = false; // We use custom controls
      this.videoConfig.cornerRadius = 0;
      this.videoConfig.objectFit = 'contain';
      hilog.info(DOMAIN, TAG, `Video player initialized: playerId=${this.playerId}, src=${this.videoConfig.src}`);
    }
  }

  private initScreenDimensions(): void {
    try {
      const displayInfo = display.getDefaultDisplaySync();
      const density = displayInfo.densityPixels || 1;
      this.screenWidthVp = displayInfo.width / density;
      this.screenHeightVp = displayInfo.height / density;
      hilog.info(DOMAIN, TAG, `Screen dimensions: ${this.screenWidthVp}x${this.screenHeightVp} vp`);
    } catch (error) {
      hilog.error(DOMAIN, TAG, `Failed to get screen dimensions: ${error}`);
      // Fallback to reasonable defaults
      this.screenWidthVp = 360;
      this.screenHeightVp = 800;
    }
  }

  private detectVideoOrientation(): void {
    if (this.orientationDetected || !this.playerId) {
      return;
    }
    const isLandscape = LxNativeVideoPlayer.isLandscapeVideo(this.playerId);
    hilog.info(DOMAIN, TAG, `Video orientation detected: isLandscape=${isLandscape}`);
    this.isLandscapeVideo = isLandscape;
    this.orientationDetected = true;
  }

  aboutToDisappear() {
    // Cleanup timers
    this.clearControlsTimer();
    this.stopProgressTimer();
    // Destroy the video player if it exists
    if (this.item.mediaType === 1 && this.playerId) {
      LxNativeVideoPlayer.destroyStatic(this.playerId);
    }
  }

  private clearControlsTimer(): void {
    if (this.controlsTimer !== -1) {
      clearTimeout(this.controlsTimer);
      this.controlsTimer = -1;
    }
  }

  private resetControlsTimer(): void {
    this.clearControlsTimer();
    this.showControls = true;
    this.controlsTimer = setTimeout(() => {
      if (this.videoState.isPlaying) {
        this.showControls = false;
      }
    }, 3000);
  }

  private startProgressTimer(): void {
    if (this.progressTimer !== -1) {
      return;
    }
    this.progressTimer = setInterval(() => {
      if (this.videoState.isPlaying && !this.isSeeking && this.playerId) {
        const currentTime = LxNativeVideoPlayer.getCurrentTimeStatic(this.playerId);
        const duration = LxNativeVideoPlayer.getDuration(this.playerId);
        if (currentTime >= 0) {
          this.videoState.currentTime = currentTime;
          if (duration > 0) {
            this.sliderValue = (currentTime / duration) * 100;
          }
        }
        if (duration > 0 && this.videoState.duration !== duration) {
          this.videoState.duration = duration;
        }
      }
    }, 250);
  }

  private stopProgressTimer(): void {
    if (this.progressTimer !== -1) {
      clearInterval(this.progressTimer);
      this.progressTimer = -1;
    }
  }

  private onVideoEvent(event: string, detail: object): void {
    hilog.info(DOMAIN, TAG, `Video event: ${event}`);
    if (event === 'playing') {
      this.videoState.isPlaying = true;
      this.videoState.isPaused = false;
      this.startProgressTimer();
      this.resetControlsTimer();
      // Detect video orientation when video starts playing (dimensions available)
      this.detectVideoOrientation();
    } else if (event === 'pause') {
      this.videoState.isPlaying = false;
      this.videoState.isPaused = true;
      this.stopProgressTimer();
    } else if (event === 'prepared') {
      // Also try to detect orientation when prepared
      this.detectVideoOrientation();
    } else if (event === 'error') {
      const errorDetail = detail as VideoErrorDetail;
      hilog.error(DOMAIN, TAG, `Video error: ${errorDetail.message}`);
    }
  }

  private togglePlayPause(): void {
    if (this.playerId) {
      LxNativeVideoPlayer.togglePlayPause(this.playerId);
      this.videoState.isPlaying = !this.videoState.isPlaying;
      this.videoState.isPaused = !this.videoState.isPlaying;
      if (this.videoState.isPlaying) {
        this.startProgressTimer();
      } else {
        this.stopProgressTimer();
      }
      this.resetControlsTimer();
    }
  }

  @State private volume: number = 0.8;
  @State private muted: boolean = false;

  @Builder
  buildVideoControls(): void {
    VideoControlsView({
      isPlaying: this.videoState.isPlaying,
      currentTime: this.videoState.currentTime,
      duration: this.videoState.duration,
      sliderValue: this.sliderValue,
      volume: this.volume,
      muted: this.muted,
      controlSize: VideoControlSize.COMPACT,
      showVolumeSlider: true,
      showFullscreenButton: false,
      showExitButton: false,
      showTimeRemaining: false,
      onSliderChange: (value: number, mode: SliderChangeMode) => {
        if (mode === SliderChangeMode.Begin) {
          this.isSeeking = true;
          this.sliderValue = value;
        } else if (mode === SliderChangeMode.Moving) {
          this.sliderValue = value;
          if (this.videoState.duration > 0) {
            this.videoState.currentTime = (value / 100) * this.videoState.duration;
          }
        } else if (mode === SliderChangeMode.End) {
          if (this.videoState.duration > 0 && this.playerId) {
            const seekTime = (value / 100) * this.videoState.duration;
            LxNativeVideoPlayer.seekTo(this.playerId, seekTime);
            this.videoState.currentTime = seekTime;
            this.sliderValue = value;
          }
          setTimeout(() => { this.isSeeking = false; }, 500);
        }
      },
      onPlayPauseClick: () => this.togglePlayPause(),
      onMuteClick: () => {
        this.muted = !this.muted;
        if (this.playerId) {
          videoPlayerSetVolume(this.playerId, this.muted ? 0 : this.volume);
        }
      },
      onVolumeChange: (value: number) => {
        this.volume = value / 100;
        this.muted = value === 0;
        if (this.playerId) {
          videoPlayerSetVolume(this.playerId, this.volume);
        }
      }
    })
  }

  @Builder
  buildCloseButton(): void {
    // Close button at top-left - subtle style
    Row() {
      Image($r('sys.media.ohos_ic_public_cancel'))
        .width(20)
        .height(20)
        .fillColor('rgba(255,255,255,0.7)')
    }
    .width(36)
    .height(36)
    .justifyContent(FlexAlign.Center)
    .borderRadius(18)
    .backgroundColor('rgba(0,0,0,0.3)')
    .position({ x: 12, y: 48 })
    .onClick(() => {
      this.onDismiss();
    })
  }

  @Builder
  buildCenterPlayButton(): void {
    Row() {
      Image($r('sys.media.ohos_ic_public_play'))
        .width(36)
        .height(36)
        .fillColor(Color.White)
    }
    .width(72)
    .height(72)
    .justifyContent(FlexAlign.Center)
    .borderRadius(36)
    .backgroundColor('rgba(0,0,0,0.5)')
    .align(Alignment.Center)
    .zIndex(2)
    .hitTestBehavior(HitTestMode.Block)
    .onClick(() => {
      this.togglePlayPause();
    })
  }

  build() {
    Stack({ alignContent: Alignment.Center }) {
      // Check if this is a video (mediaType = 1)
      if (this.item.mediaType === 1) {
        // Video player using LxNativeVideoPlayer
        // For landscape videos: rotate content 90 degrees and swap width/height
        // This avoids changing window orientation while properly displaying landscape videos
        Stack() {
          LxNativeVideoPlayer({
            playerId: this.playerId,
            config: this.videoConfig,
            onEvent: (event: string, detail: object) => {
              this.onVideoEvent(event, detail);
            }
          })
            .width('100%')
            .height('100%')

          // Video controls overlay
          if (this.showControls) {
            this.buildVideoControls()
          }

          // Center play button when paused (rendered above controls for easy access)
          if (!this.videoState.isPlaying) {
            this.buildCenterPlayButton()
          }

          // Close button (always visible)
          this.buildCloseButton()
        }
        // For landscape videos: swap width/height so after 90° rotation, content fills screen
        .width(this.isLandscapeVideo ? this.screenHeightVp : '100%')
        .height(this.isLandscapeVideo ? this.screenWidthVp : '100%')
        // Center the rotated content
        .position(this.isLandscapeVideo ?
          { x: (this.screenWidthVp - this.screenHeightVp) / 2, y: (this.screenHeightVp - this.screenWidthVp) / 2 } :
          { x: 0, y: 0 })
        // Rotate the entire video content for landscape videos
        .rotate({
          angle: this.isLandscapeVideo ? 90 : 0,
          centerX: '50%',
          centerY: '50%'
        })
        .backgroundColor(Color.Black)
        .onClick(() => {
          this.showControls = !this.showControls;
          if (this.showControls) {
            this.resetControlsTimer();
          }
        })
      } else {
        // Image viewer (default) - use contain to maintain aspect ratio and fit screen
        Image(this.item.uri)
          .width('100%')
          .height('100%')
          .objectFit(ImageFit.Contain)
          .scale({ x: this.imageScale, y: this.imageScale })
          .gesture(
            GestureGroup(GestureMode.Exclusive,
              TapGesture({ count: 1 })
                .onAction(() => {
                  if (this.imageScale <= this.minScale + 0.1) {
                    const now = Date.now();
                    const timeSinceLastTap = now - this.lastTapTime;

                    if (timeSinceLastTap > 300) {
                      this.lastTapTime = now;
                      this.onDismiss();
                    }
                  }
                }),

              PinchGesture({ fingers: 2, distance: 1 })
                .onActionUpdate((event: GestureEvent) => {
                  if (event.scale) {
                    this.imageScale = Math.min(this.maxScale, Math.max(this.minScale, event.scale));
                    this.onZoomStateChanged(this.imageScale > this.minScale + 0.1);
                  }
                })
                .onActionEnd(() => {
                  if (this.imageScale <= this.minScale + 0.1) {
                    this.resetZoom();
                  }
                })
            )
          )
      }
    }
    .width('100%')
    .height('100%')
    .backgroundColor(Color.Transparent)
  }

  private resetZoom(): void {
    if (this.imageScale !== this.minScale) {
      animateTo({ duration: 200, curve: Curve.EaseOut }, () => {
        this.imageScale = this.minScale;
      });
      this.onZoomStateChanged(false);
    }
  }
}
