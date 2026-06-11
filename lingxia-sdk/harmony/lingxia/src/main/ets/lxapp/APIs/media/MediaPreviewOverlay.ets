import { hilog } from '@kit.PerformanceAnalysisKit';
import { common } from '@kit.AbilityKit';
import { display, window } from '@kit.ArkUI';
import { image } from '@kit.ImageKit';
import {
  LxNativeVideoPlayer,
  LxNativeVideoConfig,
  LxNativeVideoState,
  VideoErrorDetail
} from '../../media/LxNativeVideoPlayer';
import { VideoControlsView, VideoControlSize } from '../../media/VideoControls';
import { LxMediaFrameCache } from '../../media/LxMediaFrameCache';
import { resolveLxUriForFileAccess } from '../../media/LxUri';
import { onCallback, videoPlayerSetVolume } from 'liblingxia.so';

export interface MediaPreviewItem {
  id: string;
  uri: string;
  mediaType?: number; // 0 = image, 1 = video
  rotate?: number;
  objectFit?: string;
  durationMs?: number;
}

export interface MediaPreviewPageParams {
  items: MediaPreviewItem[];
  initialIndex?: number;
  callbackId?: string;
  /**
   * Callback id for the JS-side `presented` Promise. Fired once when the
   * first item becomes visually ready. Omit / pass empty for callers that
   * don't need the signal — overlay then no-ops on it.
   */
  presentedCallbackId?: string;
  /**
   * Callback id for the JS-side change stream. Fired with `{"index": N}`
   * whenever the displayed item changes, including the initial item.
   * Omit / pass "0" or empty to disable.
   */
  changeCallbackId?: string;
  advance?: MediaPreviewAdvance;
  showIndexIndicator: boolean;
}

export type MediaPreviewCloseReason = 'manual' | 'completed' | 'interrupted' | 'error';
export type MediaPreviewAdvance = 'manual' | 'next' | 'loop';

const DOMAIN = 0x0000;
const TAG = 'LingXia.MediaPreviewOverlay';

/**
 * Media preview overlay manager
 */
export class MediaPreviewOverlayManager {
  private static isShowing: boolean = false;
  private static overlayData: MediaPreviewPageParams | null = null;
  private static activePreviewPlayers: Set<string> = new Set();
  private static currentIndex: number = 0;
  private static lastNotifiedIndex: number = -1;
  private static callbackSent: boolean = false;
  /**
   * Pending JS-side `presented` Promise callback id. Set in show(); cleared
   * after firing or on overlay teardown to keep the signal idempotent.
   */
  private static pendingPresentedCallbackId: string = '';

  private static listeners: Set<() => void> = new Set();

  /**
   * Show media preview overlay
   */
  public static show(params: MediaPreviewPageParams): void {
    if (MediaPreviewOverlayManager.isShowing) {
      MediaPreviewOverlayManager.closeOverlay('interrupted');
    }
    // Defensive cleanup in case previous overlay exited abnormally.
    MediaPreviewOverlayManager.destroyActivePlayers();
    const clampedIndex = MediaPreviewOverlayManager.clampIndex(params.initialIndex ?? 0, params.items.length);
    MediaPreviewOverlayManager.lastNotifiedIndex = -1;
    MediaPreviewOverlayManager.overlayData = {
      items: params.items,
      initialIndex: clampedIndex,
      callbackId: params.callbackId,
      presentedCallbackId: params.presentedCallbackId,
      changeCallbackId: params.changeCallbackId,
      advance: params.advance ?? 'manual',
      showIndexIndicator: params.showIndexIndicator
    };
    MediaPreviewOverlayManager.isShowing = true;
    MediaPreviewOverlayManager.currentIndex = clampedIndex;
    MediaPreviewOverlayManager.callbackSent = false;
    MediaPreviewOverlayManager.pendingPresentedCallbackId = String(params.presentedCallbackId ?? '').trim();
    hilog.info(DOMAIN, TAG, `Showing media preview overlay with ${params.items.length} items`);
    MediaPreviewOverlayManager.notifyListeners();
  }

  /**
   * Hide media preview overlay
   */
  public static closeOverlay(reason: MediaPreviewCloseReason = 'manual'): void {
    MediaPreviewOverlayManager.emitSessionResult(reason);
    // Order matters: flip isShowing → notifyListeners FIRST so the overlay
    // Stack unmounts in one render pass. Each MediaViewer's aboutToDisappear
    // then tears down its player as the component tree shrinks. Doing it the
    // other way around (destroy players, then notify) creates a one-frame
    // window where players are gone (XComponent renders solid black) but the
    // overlay Stack is still on screen — visible as a close-time flicker.
    MediaPreviewOverlayManager.isShowing = false;
    MediaPreviewOverlayManager.overlayData = null;
    MediaPreviewOverlayManager.currentIndex = 0;
    hilog.info(DOMAIN, TAG, `Hiding media preview overlay, reason=${reason}`);
    MediaPreviewOverlayManager.notifyListeners();
    // Defensive: catch any player that escaped the natural aboutToDisappear
    // teardown (e.g. component leaked from a prior session). Delay this one
    // frame so ArkUI can remove the overlay tree before native XComponents are
    // force-destroyed; immediate destroy can still produce a close-time flash.
    setTimeout(() => {
      MediaPreviewOverlayManager.destroyActivePlayers();
    }, 80);
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

  public static cancelSession(callbackId: string): boolean {
    const activeCallbackId = String(MediaPreviewOverlayManager.overlayData?.callbackId ?? '').trim();
    if (!MediaPreviewOverlayManager.isShowing || !activeCallbackId || activeCallbackId !== callbackId) {
      return false;
    }
    MediaPreviewOverlayManager.closeOverlay('interrupted');
    return true;
  }

  public static updateCurrentIndex(index: number): void {
    const total = MediaPreviewOverlayManager.overlayData?.items.length ?? 0;
    MediaPreviewOverlayManager.currentIndex = MediaPreviewOverlayManager.clampIndex(index, total);
    MediaPreviewOverlayManager.notifyIndexChanged();
  }

  /**
   * Notify the JS-side change stream that {@link currentIndex} is on screen.
   * Dedupes consecutive fires for the same index.
   */
  private static notifyIndexChanged(): void {
    const id = String(MediaPreviewOverlayManager.overlayData?.changeCallbackId ?? '').trim();
    if (!id || id === '0') {
      return;
    }
    const index = MediaPreviewOverlayManager.currentIndex;
    if (index === MediaPreviewOverlayManager.lastNotifiedIndex) {
      return;
    }
    MediaPreviewOverlayManager.lastNotifiedIndex = index;
    onCallback(id, true, JSON.stringify({ index: index }));
  }

  public static registerListener(listener: () => void): void {
    MediaPreviewOverlayManager.listeners.add(listener);
  }

  public static unregisterListener(listener: () => void): void {
    MediaPreviewOverlayManager.listeners.delete(listener);
  }

  public static registerPreviewPlayer(playerId: string): void {
    if (!playerId) {
      return;
    }
    MediaPreviewOverlayManager.activePreviewPlayers.add(playerId);
  }

  public static unregisterPreviewPlayer(playerId: string): void {
    if (!playerId) {
      return;
    }
    MediaPreviewOverlayManager.activePreviewPlayers.delete(playerId);
  }

  private static destroyActivePlayers(): void {
    MediaPreviewOverlayManager.activePreviewPlayers.forEach((playerId) => {
      try {
        LxNativeVideoPlayer.destroyStatic(playerId);
      } catch {
        hilog.error(DOMAIN, TAG, `Failed to destroy preview player ${playerId}`);
      }
    });
    MediaPreviewOverlayManager.activePreviewPlayers.clear();
  }

  private static notifyListeners(): void {
    MediaPreviewOverlayManager.listeners.forEach(listener => listener());
  }

  /**
   * Fire the JS-side `presented` Promise exactly once. Safe to call from
   * multiple visual-ready paths (first image painted / first video frame /
   * teardown fallback) — second call is a no-op.
   *
   * Use this directly only from teardown paths where the visual is no
   * longer on screen anyway. Visual-ready paths should go through
   * {@link signalPresentedAfterCompositorCommit} so the consumer's `.then`
   * runs after the first pixel has actually been painted, matching the
   * Android/iOS timing semantics.
   */
  public static signalPresentedOnce(): void {
    const id = MediaPreviewOverlayManager.pendingPresentedCallbackId;
    if (!id) {
      return;
    }
    MediaPreviewOverlayManager.pendingPresentedCallbackId = '';
    onCallback(id, true, '{}');
  }

  /**
   * Defer {@link signalPresentedOnce} until after the next compositor tick
   * so the consumer's `presented.then(…)` callback runs against a frame
   * that has already painted the preview content. `setTimeout(0)` schedules
   * to the next macrotask, which Harmony's ArkTS runtime processes after
   * the pending paint commit.
   */
  public static signalPresentedAfterCompositorCommit(): void {
    if (!MediaPreviewOverlayManager.pendingPresentedCallbackId) {
      return;
    }
    setTimeout(() => {
      MediaPreviewOverlayManager.signalPresentedOnce();
    }, 0);
  }

  private static emitSessionResult(reason: MediaPreviewCloseReason): void {
    // Settle `presented` first so the JS Promise resolves even on a
    // session that ended before any item painted (degenerate path).
    MediaPreviewOverlayManager.signalPresentedOnce();

    const callbackId = String(MediaPreviewOverlayManager.overlayData?.callbackId ?? '').trim();
    if (!callbackId || MediaPreviewOverlayManager.callbackSent) {
      return;
    }
    MediaPreviewOverlayManager.callbackSent = true;
    const result = JSON.stringify({
      reason: reason,
      lastIndex: MediaPreviewOverlayManager.currentIndex
    });
    onCallback(callbackId, true, result);
  }

  private static clampIndex(index: number, length: number): number {
    if (length <= 0) {
      return 0;
    }
    if (!Number.isFinite(index)) {
      return 0;
    }
    return Math.max(0, Math.min(length - 1, Math.trunc(index)));
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
  private windowOpVersion: number = 0;
  
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
    if (this.isShowing && this.overlayData) {
      this.enterFullscreenMode(this.overlayData);
    }
  }

  aboutToDisappear() {
    MediaPreviewOverlayManager.unregisterListener(this.stateChangeListener);
    if (MediaPreviewOverlayManager.isOverlayShowing()) {
      MediaPreviewOverlayManager.closeOverlay('interrupted');
    }
    // Defensive restore: if overlay component is removed unexpectedly, put system bars back.
    this.exitFullscreenMode();
  }

  private enterFullscreenMode(_data: MediaPreviewPageParams): void {
    const opVersion = ++this.windowOpVersion;
    try {
      const context = getContext(this) as common.UIAbilityContext;
      context.windowStage.getMainWindow().then((mainWindow) => {
        if (opVersion !== this.windowOpVersion) {
          return;
        }
        mainWindow.setWindowSystemBarEnable([])
          .then(() => {
            if (opVersion !== this.windowOpVersion) {
              return;
            }
            hilog.info(DOMAIN, TAG, 'Entered fullscreen mode');
          })
          .catch(() => {
            if (opVersion !== this.windowOpVersion) {
              return;
            }
            hilog.error(DOMAIN, TAG, 'Failed to apply fullscreen bars');
          });
      }).catch(() => {
        if (opVersion !== this.windowOpVersion) {
          return;
        }
        hilog.error(DOMAIN, TAG, 'Failed to get main window for enter fullscreen');
      });
    } catch {
      hilog.error(DOMAIN, TAG, 'Failed to enter fullscreen mode');
    }
  }

  private exitFullscreenMode(): void {
    const opVersion = ++this.windowOpVersion;
    try {
      const context = getContext(this) as common.UIAbilityContext;
      context.windowStage.getMainWindow().then((mainWindow) => {
        if (opVersion !== this.windowOpVersion) {
          return;
        }
        mainWindow.setWindowSystemBarEnable(['status', 'navigation'])
          .then(() => {
            if (opVersion !== this.windowOpVersion) {
              return;
            }
            return mainWindow.setPreferredOrientation(window.Orientation.UNSPECIFIED);
          })
          .then(() => {
            if (opVersion !== this.windowOpVersion) {
              return;
            }
            hilog.info(DOMAIN, TAG, 'Exited fullscreen mode');
          })
          .catch(() => {
            if (opVersion !== this.windowOpVersion) {
              return;
            }
            hilog.error(DOMAIN, TAG, 'Failed to restore fullscreen bars/orientation');
          });
      }).catch(() => {
        if (opVersion !== this.windowOpVersion) {
          return;
        }
        hilog.error(DOMAIN, TAG, 'Failed to get main window for exit fullscreen');
      });
    } catch {
      hilog.error(DOMAIN, TAG, 'Failed to exit fullscreen mode');
    }
  }

  private handleClose(): void {
    MediaPreviewOverlayManager.closeOverlay('manual');
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
      .expandSafeArea([SafeAreaType.SYSTEM], [SafeAreaEdge.TOP, SafeAreaEdge.BOTTOM])
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
  private imageAutoTimer: number = -1;
  private transitionGeneration: number = 0;
  private readonly swiperController: SwiperController = new SwiperController();

  aboutToAppear() {
    this.currentIndex = this.clampIndex(this.params.initialIndex ?? 0);
    MediaPreviewOverlayManager.updateCurrentIndex(this.currentIndex);
    this.initStatusBarHeight();
    this.handleCurrentItemChanged();
    // Warm the first-frame cache for the initial item + its neighbour so the
    // transition overlay paints synchronously when the user swipes. Player
    // alone can't hide the black gap because AVImageGenerator decode is
    // typically slower than the engine reaching its first decoded frame.
    this.prefetchFramesAround(this.currentIndex);
  }

  aboutToDisappear() {
    this.clearImageAutoTimer();
  }

  private async initStatusBarHeight(): Promise<void> {
    try {
      const windowClass: window.Window = await window.getLastWindow(getContext());
      const safeArea: window.AvoidArea = await windowClass.getWindowAvoidArea(window.AvoidAreaType.TYPE_SYSTEM);
      this.statusBarHeight = safeArea.topRect.height || 44;
    } catch {
      hilog.error(DOMAIN, TAG, 'Failed to get status bar height');
    }
  }

  build() {
    Stack({ alignContent: Alignment.Top }) {
      if (this.shouldShowIndicator()) {
        Text(`${this.currentIndex + 1}/${this.params.items.length}`)
          .fontColor(Color.White)
          .fontSize(18)
          .fontWeight(FontWeight.Bold)
          .margin({ top: this.statusBarHeight + 16 })
          .width('100%')
          .textAlign(TextAlign.Center)
          .zIndex(10)
      }

      // Media display area (supports both images and videos)
      if (this.params.items.length > 0) {
        Swiper(this.swiperController) {
          ForEach(this.params.items, (item: MediaPreviewItem, index: number) => {
            MediaViewer({
              item: item,
              isActive: this.currentIndex === index,
              loopPlayback: this.shouldLoopCurrentItem(index),
              statusBarHeight: this.statusBarHeight,
              onZoomStateChanged: (zoomed: boolean) => {
                if (this.currentIndex === index) {
                  this.isZoomed = zoomed;
                }
              },
              onDismiss: this.onClose,
              onVideoTerminal: (event: 'ended' | 'error') => {
                if (this.currentIndex === index) {
                  this.handleVideoTerminal(event);
                }
              },
              onVisualReady: () => {
                if (this.currentIndex === index) {
                  MediaPreviewOverlayManager.signalPresentedAfterCompositorCommit();
                }
              }
            })
              .width('100%')
              .height('100%')
          })
        }
        .width('100%')
        .height('100%')
        .index(this.currentIndex)
        .indicator(false)
        .loop(this.shouldUseLoopPager())
        .disableSwipe(this.isZoomed)
        .cachedCount(this.params.items.length > 1 ? 1 : 0)
        .duration(360)
        .curve(Curve.EaseInOut)
        .onChange((index: number) => {
          this.handleSwiperChange(index);
        })
      }
    }
  }

  private clampIndex(value: number): number {
    if (this.params.items.length <= 0) {
      return 0;
    }
    if (!Number.isFinite(value)) {
      return 0;
    }
    return Math.max(0, Math.min(this.params.items.length - 1, Math.trunc(value)));
  }

  private shouldShowIndicator(): boolean {
    return !!this.params.showIndexIndicator && this.params.items.length > 0;
  }

  private clearImageAutoTimer(): void {
    if (this.imageAutoTimer !== -1) {
      clearTimeout(this.imageAutoTimer);
      this.imageAutoTimer = -1;
    }
  }

  private handleCurrentItemChanged(): void {
    this.clearImageAutoTimer();
    MediaPreviewOverlayManager.updateCurrentIndex(this.currentIndex);
    const currentItem = this.params.items[this.currentIndex];
    if (!currentItem) {
      return;
    }
    hilog.info(DOMAIN, TAG, `Preview switched to index=${this.currentIndex}, mediaType=${currentItem.mediaType ?? 0}`);
    if (currentItem.mediaType !== 1) {
      this.scheduleImageAutoAction();
    }
    this.prefetchFramesAround(this.currentIndex);
  }

  private prefetchFramesAround(centerIndex: number): void {
    const items = this.params.items;
    if (items.length === 0) {
      return;
    }
    const cache = LxMediaFrameCache.getInstance();
    // Prefetch the centre item plus immediate neighbours. The Swiper's
    // cachedCount keeps neighbour viewers ready; we want their first frames
    // ready in cache too so showOverlayForUrl resolves synchronously when the
    // player mounts on swipe.
    const candidates = [centerIndex, centerIndex - 1, centerIndex + 1];
    for (let i = 0; i < candidates.length; i++) {
      const idx = candidates[i];
      if (idx < 0 || idx >= items.length) {
        continue;
      }
      const item = items[idx];
      if (!item || item.mediaType !== 1) {
        continue;
      }
      const localPath = resolveLxUriForFileAccess(item.uri);
      if (!localPath) {
        continue;
      }
      cache.prefetch(localPath);
    }
  }

  private shouldUseLoopPager(): boolean {
    return (this.params.advance ?? 'manual') === 'loop' && this.params.items.length > 1;
  }

  private shouldLoopCurrentItem(index: number): boolean {
    return (this.params.advance ?? 'manual') === 'loop'
      && this.params.items.length === 1
      && this.currentIndex === index;
  }

  private handleSwiperChange(index: number): void {
    const normalizedIndex = this.clampIndex(index);
    if (normalizedIndex === this.currentIndex) {
      return;
    }
    this.currentIndex = normalizedIndex;
    this.isZoomed = false;
    this.handleCurrentItemChanged();
  }

  private async changeToIndex(nextIndex: number, animated: boolean): Promise<void> {
    const normalizedIndex = this.clampIndex(nextIndex);
    if (normalizedIndex === this.currentIndex) {
      this.handleCurrentItemChanged();
      return;
    }
    const generation = ++this.transitionGeneration;
    await this.ensureFrameReadyForIndex(normalizedIndex);
    if (generation !== this.transitionGeneration) {
      return;
    }
    this.isZoomed = false;
    this.swiperController.changeIndex(normalizedIndex, animated);
  }

  private async ensureFrameReadyForIndex(index: number): Promise<void> {
    const item = this.params.items[index];
    if (!item || item.mediaType !== 1) {
      return;
    }
    const localPath = resolveLxUriForFileAccess(item.uri);
    if (!localPath) {
      return;
    }
    await LxMediaFrameCache.getInstance().load(localPath);
  }

  private scheduleImageAutoAction(): void {
    const durationMs = this.params.items[this.currentIndex]?.durationMs;
    const advance = this.params.advance ?? 'manual';
    if (!durationMs || durationMs <= 0 || advance === 'manual') {
      return;
    }
    this.imageAutoTimer = setTimeout(() => {
      this.imageAutoTimer = -1;
      this.advanceFromCurrentItem();
    }, durationMs);
  }

  private handleVideoTerminal(event: 'ended' | 'error'): void {
    if (event === 'error') {
      MediaPreviewOverlayManager.closeOverlay('error');
      return;
    }
    this.advanceFromCurrentItem();
  }

  private advanceFromCurrentItem(): void {
    const advance = this.params.advance ?? 'manual';
    if (advance === 'manual') {
      return;
    }
    if (advance === 'next') {
      if (this.currentIndex < this.params.items.length - 1) {
        this.changeToIndex(this.currentIndex + 1, true);
        return;
      }
      MediaPreviewOverlayManager.closeOverlay('completed');
      return;
    }
    if (this.shouldUseLoopPager()) {
      this.isZoomed = false;
      this.swiperController.showNext();
      return;
    }
    if (this.params.items.length > 0) {
      this.handleCurrentItemChanged();
      return;
    }
    MediaPreviewOverlayManager.closeOverlay('completed');
  }
}

/**
 * Media viewer component (supports both images and videos)
 * Uses LxNativeVideoPlayer for video playback (same as video component)
 * Automatically detects video orientation and rotates content for landscape videos
 */
@Component
struct MediaViewer {
  @Prop @Watch('onItemChange') item: MediaPreviewItem;
  @Prop @Watch('onActiveStateChange') isActive: boolean = false;
  @Prop loopPlayback: boolean = false;
  // Safe-area top inset, propagated from MediaPreviewContent so the close
  // button can avoid the status bar / notch. Default matches the parent's
  // pre-measurement fallback (44 vp).
  @Prop statusBarHeight: number = 44;
  onZoomStateChanged: (zoomed: boolean) => void = () => {};
  onDismiss: () => void = () => {};
  onVideoTerminal: (event: 'ended' | 'error') => void = () => {};
  onVisualReady: () => void = () => {};

  @State private imageScale: number = 1;
  // Video player state
  @State private videoState: LxNativeVideoState = new LxNativeVideoState();
  @State private showControls: boolean = false;  // Hide controls initially, show on tap
  @State private isSeeking: boolean = false;
  @State private sliderValue: number = 0;
  @State private playerId: string = '';
  @State private videoConfig: LxNativeVideoConfig = new LxNativeVideoConfig();
  // Screen dimensions and orientation for landscape video rotation
  @State private screenWidthVp: number = 0;
  @State private screenHeightVp: number = 0;
  @State private isLandscapeVideo: boolean = false;
  @State private explicitRotation: number = -1;
  @State private effectiveRotation: number = 0;
  @State private orientationDetected: boolean = false;
  @State private framePixelMap: image.PixelMap | undefined = undefined;
  // Becomes true on the first 'playing' event for the current player and stays
  // true until the item is replaced. Gates the centre play button so that
  // autoplay never shows a fake "paused" overlay before the first frame, and
  // spurious early 'pause' signals from the decoder don't flash it either.
  @State private hasPlayedOnce: boolean = false;
  private readonly minScale: number = 1;
  private readonly maxScale: number = 3;
  private readonly tapDismissMoveThresholdPx: number = 16;
  private readonly tapDismissMaxDurationMs: number = 280;
  private readonly tapDismissDebounceMs: number = 220;
  private tapTrackingFingerId: number = -1;
  private tapStartX: number = 0;
  private tapStartY: number = 0;
  private tapStartAt: number = 0;
  private tapMoved: boolean = false;
  private lastTapDismissAt: number = 0;
  private controlsTimer: number = -1;
  private progressTimer: number = -1;
  private frameGeneration: number = 0;
  private framePath: string = '';

  aboutToAppear() {
    this.initScreenDimensions();
    this.applyCurrentItem();
  }

  private normalizeRotation(value: number | undefined): number | undefined {
    if (typeof value !== 'number' || !Number.isFinite(value)) {
      return undefined;
    }
    const parsed = Math.trunc(value);
    if (parsed === 0 || parsed === 90 || parsed === 180 || parsed === 270) {
      return parsed;
    }
    return undefined;
  }

  private normalizeObjectFit(value: string | undefined): string {
    if (typeof value !== 'string') {
      return 'contain';
    }
    const fit = value.trim().toLowerCase();
    if (fit === 'cover' || fit === 'contain' || fit === 'fill' || fit === 'fit') {
      return fit;
    }
    return 'contain';
  }

  private resolveImageFit(value: string | undefined): ImageFit {
    const fit = this.normalizeObjectFit(value);
    if (fit === 'cover') {
      return ImageFit.Cover;
    }
    if (fit === 'fill') {
      return ImageFit.Fill;
    }
    return ImageFit.Contain;
  }

  private resolveFrameFit(): ImageFit {
    return this.resolveImageFit(this.item.objectFit);
  }

  private resolveImageRotation(): number {
    const rotation = this.normalizeRotation(this.item.rotate);
    return rotation ?? 0;
  }

  private isQuarterTurn(rotation: number): boolean {
    return rotation === 90 || rotation === 270;
  }

  private updateEffectiveRotation(): void {
    if (this.explicitRotation >= 0) {
      this.effectiveRotation = this.explicitRotation;
      return;
    }
    this.effectiveRotation = this.isLandscapeVideo ? 90 : 0;
  }

  private initScreenDimensions(): void {
    try {
      const displayInfo = display.getDefaultDisplaySync();
      const density = displayInfo.densityPixels || 1;
      this.screenWidthVp = displayInfo.width / density;
      this.screenHeightVp = displayInfo.height / density;
      hilog.info(DOMAIN, TAG, `Screen dimensions: ${this.screenWidthVp}x${this.screenHeightVp} vp`);
    } catch {
      hilog.error(DOMAIN, TAG, 'Failed to get screen dimensions');
      // Fallback to reasonable defaults
      this.screenWidthVp = 360;
      this.screenHeightVp = 800;
    }
  }

  private detectVideoOrientation(): void {
    if (this.explicitRotation >= 0) {
      this.orientationDetected = true;
      this.updateEffectiveRotation();
      return;
    }
    if (this.orientationDetected || !this.playerId) {
      return;
    }
    const isLandscape = LxNativeVideoPlayer.isLandscapeVideo(this.playerId);
    hilog.info(DOMAIN, TAG, `Video orientation detected: isLandscape=${isLandscape}`);
    this.isLandscapeVideo = isLandscape;
    this.orientationDetected = true;
    this.updateEffectiveRotation();
  }

  aboutToDisappear() {
    // Cleanup timers
    this.clearControlsTimer();
    this.stopProgressTimer();
    this.teardownVideoPlayer(true);
    this.resetZoomState();
  }

  onItemChange(): void {
    this.applyCurrentItem();
  }

  onActiveStateChange(): void {
    this.applyCurrentItem();
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

  private resetZoomState(): void {
    this.imageScale = this.minScale;
    this.onZoomStateChanged(false);
  }

  private resetTapTracking(): void {
    this.tapTrackingFingerId = -1;
    this.tapStartX = 0;
    this.tapStartY = 0;
    this.tapStartAt = 0;
    this.tapMoved = false;
  }

  private getTrackingTouch(event: TouchEvent): TouchObject | null {
    if (!event) {
      return null;
    }
    const sources = [event.changedTouches, event.touches];
    for (let i = 0; i < sources.length; i++) {
      const list = sources[i];
      if (!list || list.length === 0) {
        continue;
      }
      for (let j = 0; j < list.length; j++) {
        const touch = list[j];
        if (touch.id === this.tapTrackingFingerId) {
          return touch;
        }
      }
    }
    return null;
  }

  private handleImageTouch(event: TouchEvent): void {
    if (!this.isActive || this.item.mediaType === 1) {
      this.resetTapTracking();
      return;
    }
    if (!event) {
      this.resetTapTracking();
      return;
    }

    if (event.type === TouchType.Cancel) {
      this.resetTapTracking();
      return;
    }

    if (event.type === TouchType.Down) {
      if (!event.changedTouches || event.changedTouches.length !== 1) {
        this.resetTapTracking();
        return;
      }
      const touch = event.changedTouches[0];
      this.tapTrackingFingerId = touch.id;
      this.tapStartX = touch.windowX;
      this.tapStartY = touch.windowY;
      this.tapStartAt = Date.now();
      this.tapMoved = false;
      return;
    }

    if (this.tapTrackingFingerId < 0) {
      return;
    }

    const trackingTouch = this.getTrackingTouch(event);
    if (event.type === TouchType.Move) {
      if (!trackingTouch) {
        this.tapMoved = true;
        return;
      }
      const deltaX = Math.abs(trackingTouch.windowX - this.tapStartX);
      const deltaY = Math.abs(trackingTouch.windowY - this.tapStartY);
      if (deltaX > this.tapDismissMoveThresholdPx || deltaY > this.tapDismissMoveThresholdPx) {
        this.tapMoved = true;
      }
      return;
    }

    if (event.type === TouchType.Up) {
      const now = Date.now();
      const durationMs = now - this.tapStartAt;
      if (!trackingTouch) {
        this.resetTapTracking();
        return;
      }
      const deltaX = Math.abs(trackingTouch.windowX - this.tapStartX);
      const deltaY = Math.abs(trackingTouch.windowY - this.tapStartY);
      const movedOnUp = deltaX > this.tapDismissMoveThresholdPx || deltaY > this.tapDismissMoveThresholdPx;
      const shouldDismiss = !this.tapMoved
        && !movedOnUp
        && durationMs <= this.tapDismissMaxDurationMs
        && (now - this.lastTapDismissAt) > this.tapDismissDebounceMs
        && this.imageScale <= this.minScale + 0.1;
      this.resetTapTracking();
      if (shouldDismiss) {
        this.lastTapDismissAt = now;
        this.onDismiss();
      }
      return;
    }
  }

  private applyCurrentItem(): void {
    this.resetZoomState();
    this.resetTapTracking();
    this.clearControlsTimer();
    this.stopProgressTimer();
    // Defer the native AVPlayer release so it can't stall the swipe animation
    // frame. Pause + mute happen synchronously inside teardownVideoPlayer so
    // there's no audible bleed during the 80ms grace window.
    this.teardownVideoPlayer(true);

    // hasPlayedOnce MUST be reset BEFORE videoState. The videoState assignment
    // is a @State object reassignment that triggers an ArkUI re-render; if
    // hasPlayedOnce were still stale-true from the previous item at that
    // render, the centre play button would flash for one frame.
    this.hasPlayedOnce = false;
    this.showControls = false;
    this.isSeeking = false;
    this.sliderValue = 0;
    this.videoState = new LxNativeVideoState();
    this.isLandscapeVideo = false;
    this.explicitRotation = -1;
    this.effectiveRotation = 0;
    this.orientationDetected = false;
    this.loadFramePreviewForCurrentItem();

    if (!this.isActive) {
      return;
    }

    if (this.item.mediaType !== 1) {
      return;
    }

    const config = new LxNativeVideoConfig();
    const nextPlayerId = `preview-video-${Date.now()}-${Math.random().toString(36).substring(2, 8)}`;
    config.src = this.item.uri;
    config.autoplay = true;
    config.loop = this.loopPlayback;
    config.muted = false;
    config.volume = 1.0;
    config.controls = false;
    config.cornerRadius = 0;
    config.objectFit = this.normalizeObjectFit(this.item.objectFit);

    this.playerId = nextPlayerId;
    this.videoConfig = config;
    MediaPreviewOverlayManager.registerPreviewPlayer(nextPlayerId);

    const rotation = this.normalizeRotation(this.item.rotate);
    this.explicitRotation = rotation !== undefined ? rotation : -1;
    this.updateEffectiveRotation();
    hilog.info(DOMAIN, TAG, `Video player initialized: playerId=${this.playerId}, src=${this.videoConfig.src}`);
  }

  private loadFramePreviewForCurrentItem(): void {
    if (this.item.mediaType !== 1) {
      this.frameGeneration += 1;
      this.framePath = '';
      this.framePixelMap = undefined;
      return;
    }
    const localPath = resolveLxUriForFileAccess(this.item.uri);
    if (!localPath) {
      this.frameGeneration += 1;
      this.framePath = '';
      this.framePixelMap = undefined;
      return;
    }

    if (localPath !== this.framePath) {
      this.framePath = localPath;
      this.framePixelMap = undefined;
    }
    const cache = LxMediaFrameCache.getInstance();
    const cached = cache.peek(localPath);
    if (cached) {
      this.framePixelMap = cached;
      return;
    }

    const generation = ++this.frameGeneration;
    cache.load(localPath).then((pixelMap) => {
      if (!pixelMap) {
        return;
      }
      if (generation !== this.frameGeneration || localPath !== this.framePath) {
        return;
      }
      this.framePixelMap = pixelMap;
    });
  }

  private teardownVideoPlayer(deferDestroy: boolean = false): void {
    if (!this.playerId) {
      return;
    }
    const previousPlayerId = this.playerId;
    this.playerId = '';
    MediaPreviewOverlayManager.unregisterPreviewPlayer(previousPlayerId);
    if (deferDestroy) {
      // Synchronously mute + pause so the deferred destroy can't bleed audio
      // during the swipe animation. videoPlayerDestroy itself (native AVPlayer
      // release) is the expensive part — pause/setVolume are cheap.
      try {
        videoPlayerSetVolume(previousPlayerId, 0);
        LxNativeVideoPlayer.pauseStatic(previousPlayerId);
      } catch {
        // Best-effort; proceed to destroy regardless.
      }
      setTimeout(() => {
        LxNativeVideoPlayer.destroyStatic(previousPlayerId);
      }, 80);
      return;
    }
    LxNativeVideoPlayer.destroyStatic(previousPlayerId);
  }

  private onVideoEvent(event: string, detail: object): void {
    if (!this.isActive || !this.playerId) {
      return;
    }
    // Skip logging frequent timeupdate events
    if (event !== 'timeupdate') {
      hilog.info(DOMAIN, TAG, `Video event: ${event}`);
    }

    switch (event) {
      case 'playing':
        // Set isPlaying BEFORE flipping hasPlayedOnce. videoState's nested
        // property changes don't trigger re-render in ArkUI v1, but
        // hasPlayedOnce (primitive @State) does — if it flipped first, the
        // render snapshot would see hasPlayedOnce=true && isPlaying=false and
        // flash the centre play button for a frame.
        this.videoState.isPlaying = true;
        this.videoState.isPaused = false;
        this.videoState.isBuffering = false;
        this.hasPlayedOnce = true;
        if (this.isActive) {
          this.startProgressTimer();
          this.detectVideoOrientation();
        }
        break;
      case 'firstframerendered':
        if (this.isActive) {
          this.onVisualReady();
        }
        break;
      case 'play':
      case 'playrequest':
        this.videoState.isPlaying = true;
        this.videoState.isPaused = false;
        this.videoState.isBuffering = false;
        break;
      case 'waiting':
        this.videoState.isPlaying = true;
        this.videoState.isPaused = false;
        this.videoState.isBuffering = true;
        break;
      case 'pause':
      case 'stop':
        this.videoState.isPlaying = false;
        this.videoState.isPaused = true;
        this.videoState.isBuffering = false;
        this.stopProgressTimer();
        break;
      case 'ended':
        this.videoState.isPlaying = false;
        this.videoState.isPaused = true;
        this.videoState.isBuffering = false;
        this.stopProgressTimer();
        this.sliderValue = 100;
        if (this.isActive) {
          this.onVideoTerminal('ended');
        }
        break;
      case 'error':
        hilog.error(DOMAIN, TAG, `Video error: ${(detail as VideoErrorDetail).message}`);
        this.videoState.isPlaying = false;
        this.videoState.isPaused = true;
        this.videoState.isBuffering = false;
        this.stopProgressTimer();
        if (this.isActive) {
          this.onVideoTerminal('error');
        }
        break;
      case 'seeking':
        this.isSeeking = true;
        break;
      case 'seeked': {
        this.isSeeking = false;
        const current = LxNativeVideoPlayer.getCurrentTimeStatic(this.playerId);
        if (current >= 0) {
          this.videoState.currentTime = current;
          if (this.videoState.duration > 0) {
            this.sliderValue = (current / this.videoState.duration) * 100;
          }
        }
        break;
      }
      case 'loadedmetadata':
      case 'prepared':
        this.detectVideoOrientation();
        break;
      default:
        break;
    }
  }

  private togglePlayPause(): void {
    if (this.playerId) {
      const shouldPlay = !this.videoState.isPlaying;
      LxNativeVideoPlayer.togglePlayPause(this.playerId);
      if (shouldPlay) {
        // Optimistically reflect "play intent"; native events will finalize state.
        this.videoState.isPlaying = true;
        this.videoState.isPaused = false;
        this.videoState.isBuffering = true;
        // Only reset timer (auto-hide) when playing, don't force show
        if (this.showControls) {
          this.resetControlsTimer();
        }
      } else {
        // Pause intent should be reflected immediately.
        this.videoState.isPlaying = false;
        this.videoState.isPaused = true;
        this.videoState.isBuffering = false;
        this.stopProgressTimer();
      }
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
    // Always-visible close affordance at the top-right. Anchored via
    // safe-area-aware `top: statusBarHeight + 8` so the button doesn't collide
    // with the status bar on devices with notches. `hitTestBehavior(Block)`
    // stops the tap from bubbling to the underlying image/video which would
    // otherwise trigger tap-to-dismiss / tap-to-toggle-controls behaviour.
    Row() {
      Image($r('sys.media.ohos_ic_public_cancel'))
        .width(20)
        .height(20)
        .fillColor('rgba(255,255,255,0.9)')
    }
    .width(40)
    .height(40)
    .justifyContent(FlexAlign.Center)
    .borderRadius(20)
    .backgroundColor('rgba(0,0,0,0.45)')
    .position({ right: 12, top: this.statusBarHeight + 8 })
    .zIndex(20)
    .hitTestBehavior(HitTestMode.Block)
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
          if (this.isActive && this.playerId) {
            LxNativeVideoPlayer({
              playerId: this.playerId,
              config: this.videoConfig,
              onEvent: (event: string, detail: object) => {
                this.onVideoEvent(event, detail);
              }
            })
              .width('100%')
              .height('100%')
              .zIndex(0)
          } else {
            if (this.framePixelMap !== undefined) {
              Image(this.framePixelMap)
                .width('100%')
                .height('100%')
                .objectFit(this.resolveFrameFit())
                .hitTestBehavior(HitTestMode.Transparent)
                .zIndex(0)
            } else {
              Column()
                .width('100%')
                .height('100%')
                .backgroundColor(Color.Black)
                .zIndex(0)
            }
          }

          if (this.isActive && !this.hasPlayedOnce && this.framePixelMap !== undefined) {
            Image(this.framePixelMap)
              .width('100%')
              .height('100%')
              .objectFit(this.resolveFrameFit())
              .hitTestBehavior(HitTestMode.Transparent)
              .zIndex(1)
          }

          // Video controls overlay
          if (this.isActive && this.showControls) {
            this.buildVideoControls()
          }

          // Centre play button only after the player has reached its first
          // 'playing' event for the current item. That gate kills both the
          // initial autoplay flash and any spurious early-pause flash.
          if (this.isActive && this.hasPlayedOnce && !this.videoState.isPlaying) {
            this.buildCenterPlayButton()
          }

          // Close button is rendered at the outer MediaViewer Stack level (see
          // below) so it stays screen-aligned and doesn't rotate together with
          // landscape videos.
        }
        // Swap dimensions for 90/270 rotation so content can fill screen.
        .width(this.effectiveRotation === 90 || this.effectiveRotation === 270 ? this.screenHeightVp : '100%')
        .height(this.effectiveRotation === 90 || this.effectiveRotation === 270 ? this.screenWidthVp : '100%')
        // Center the rotated content
        .position(this.effectiveRotation === 90 || this.effectiveRotation === 270 ?
          { x: (this.screenWidthVp - this.screenHeightVp) / 2, y: (this.screenHeightVp - this.screenWidthVp) / 2 } :
          { x: 0, y: 0 })
        // Rotate the entire video content (explicit value or auto-detected landscape fallback).
        .rotate({
          angle: this.effectiveRotation,
          centerX: '50%',
          centerY: '50%'
        })
        .backgroundColor(Color.Black)
        .onClick(() => {
          if (!this.isActive) {
            return;
          }
          this.showControls = !this.showControls;
          if (this.showControls) {
            this.resetControlsTimer();
          }
        })
      } else {
        Stack() {
          Image(this.item.uri)
            .width('100%')
            .height('100%')
            .objectFit(this.resolveImageFit(this.item.objectFit))
            .scale({ x: this.imageScale, y: this.imageScale })
            .onComplete(() => {
              if (this.isActive) {
                this.onVisualReady();
              }
            })
            .gesture(
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
        }
        .width(this.isQuarterTurn(this.resolveImageRotation()) ? this.screenHeightVp : '100%')
        .height(this.isQuarterTurn(this.resolveImageRotation()) ? this.screenWidthVp : '100%')
        .position(this.isQuarterTurn(this.resolveImageRotation()) ?
          { x: (this.screenWidthVp - this.screenHeightVp) / 2, y: (this.screenHeightVp - this.screenWidthVp) / 2 } :
          { x: 0, y: 0 })
        .rotate({
          angle: this.resolveImageRotation(),
          centerX: '50%',
          centerY: '50%'
        })
        .onTouch((event: TouchEvent) => {
          this.handleImageTouch(event);
        })
      }

      // Close button is hoisted out of the rotated content layer so it stays
      // screen-aligned even when the inner video/image is rotated 90/180/270
      // (landscape videos on portrait devices). Gated on `showControls` so it
      // tracks the same tap-to-reveal affordance as the video controls bar —
      // not a persistent overlay.
      if (this.isActive && this.showControls) {
        this.buildCloseButton()
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
