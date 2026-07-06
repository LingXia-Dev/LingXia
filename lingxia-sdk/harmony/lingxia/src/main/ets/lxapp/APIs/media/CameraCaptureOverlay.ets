import { hilog } from '@kit.PerformanceAnalysisKit';
import { onCallback, cameraInit, cameraRelease, cameraSwitchFacing, cameraSetFlashMode, cameraTakePhoto, cameraStartPhotoWithSurface, cameraStartVideoWithSurface, cameraVideoOutputStart, cameraVideoOutputStopAndRelease, getCurrentLxapp, getLxAppInfo, type LxAppInfo, type CurrentLxApp } from 'liblingxia.so';
import { image } from '@kit.ImageKit';
import { media } from '@kit.MediaKit';
import { fileIo } from '@kit.CoreFileKit';
import common from '@ohos.app.ability.common';
import { promptAction } from '@kit.ArkUI';
import { LxLog } from '../../../app/LxLog';

const DOMAIN = 0x0000;
const TAG = 'LingXia.CameraCapture';

// camera UI dimensions and positioning
const CAMERA_BUTTON_SIZE = 72;
const VIDEO_BUTTON_SIZE = 64;
const CAMERA_ICON_SIZE = 32;
const CONTROLS_PADDING = 24;
const CONTROLS_BOTTOM_MARGIN = 40;
const CAMERA_SWITCH_SIZE = 40;
const MODE_SWITCH_WIDTH = 120;
const MODE_SWITCH_HEIGHT = 36;

class FocusPositionType {
  x: number;
  y: number;
  constructor(x: number, y: number) {
    this.x = x;
    this.y = y;
  }
}

export type CameraFacing = 'front' | 'back';
export type CaptureMode = 'photo' | 'video';

// Native camera bridge (to be implemented via NAPI + NDK using camera-data-collection)
export class NativeCameraBridge {
  static async init(surfaceId: string, facing: CameraFacing): Promise<boolean> {
    try {
      return !!cameraInit(String(surfaceId), String(facing));
    } catch {
      return false;
    }
  }
  static async release(): Promise<void> {
    try { cameraRelease(); } catch {}
  }
  static async switchFacing(_facing: CameraFacing): Promise<boolean> {
    try {
      const isBack = _facing === 'back';
      return !!cameraSwitchFacing(isBack);
    } catch { return false; }
  }
  static async startPhotoWithSurface(photoSurfaceId: string, callbackId: string, cacheDir: string): Promise<boolean> {
    try {
      return !!cameraStartPhotoWithSurface(photoSurfaceId, callbackId, cacheDir);
    } catch {
      return false;
    }
  }
  static async capturePhoto(): Promise<boolean> {
    try {
      return cameraTakePhoto();
    } catch {
      return false;
    }
  }
  static async startVideoWithSurface(videoSurfaceId: string): Promise<boolean> { try { return !!cameraStartVideoWithSurface(videoSurfaceId); } catch { return false; }
  }
  static async startVideoOutput(): Promise<boolean> { try { return !!cameraVideoOutputStart(); } catch { return false; }
  }
  static async stopVideoOutput(): Promise<boolean> { try { return !!cameraVideoOutputStopAndRelease(); } catch { return false; }
  }
}

export interface CameraCaptureOptions {
  callbackId: string;
  mode: string; // 'images' | 'videos' | 'mix'
  maxDurationSeconds?: number;
  cameraFacing?: CameraFacing;
}

export interface CameraCaptureResult {
  uri: string;
  fileType: string; // 'image' | 'video'
  isOriginal: boolean; // always true for capture
}

export class CameraCaptureOverlayManager {
  private static showing = false;
  private static options: CameraCaptureOptions | null = null;

  static show(options: CameraCaptureOptions): void {
    CameraCaptureOverlayManager.options = options;
    CameraCaptureOverlayManager.showing = true;
  }

  static hide(): void {
    CameraCaptureOverlayManager.showing = false;
    CameraCaptureOverlayManager.options = null;
  }

  static isShowing(): boolean { return CameraCaptureOverlayManager.showing; }
  static getOptions(): CameraCaptureOptions | null { return CameraCaptureOverlayManager.options; }
}

// Icon size constants
const ICON_BUTTON_SIZE = 32;
const CAMERA_SWITCH_ICON_SIZE = 44;

// Progress arc canvas that repaints on progress change
@Component
struct ShutterProgressCanvas {
  @Prop progress: number = 0;
  // Avoid clashing with CustomComponent.size(...) by renaming
  @Prop diameter: number = 88;
  @Prop stroke: number = 3;

  private settings: RenderingContextSettings = new RenderingContextSettings(true);
  private ctx: CanvasRenderingContext2D = new CanvasRenderingContext2D(this.settings);
  private ready: boolean = false;
  private lastDrawnProgress: number = -1;
  private repaintTimer: number = -1;

  private draw() {
    const size = this.diameter;
    const cx = size / 2;
    const cy = size / 2;
    const p = Math.min(1, Math.max(0, this.progress));
    const start = -Math.PI / 2;
    const end = start + Math.PI * 2 * p;
    // Align to ring outer edge centerline for perfect overlay
    const r = (size / 2) - (this.stroke / 2);
    const ctx = this.ctx;
    ctx.clearRect(0, 0, size, size);
    if (p <= 0) { return; }
    ctx.beginPath();
    ctx.strokeStyle = '#07C160';
    ctx.lineCap = 'round';
    ctx.lineJoin = 'round';
    ctx.lineWidth = this.stroke;
    ctx.arc(cx, cy, r, start, end, false);
    ctx.stroke();
  }

  aboutToRender() {
    if (this.ready && this.lastDrawnProgress !== this.progress) {
      this.draw();
      this.lastDrawnProgress = this.progress;
    }
  }

  aboutToAppear() {
    if (this.repaintTimer === -1) {
      this.repaintTimer = setInterval(() => {
        if (this.ready) {
          this.draw();
        }
      }, 120) as number;
    }
  }

  aboutToDisappear() {
    if (this.repaintTimer !== -1) {
      clearInterval(this.repaintTimer);
      this.repaintTimer = -1;
    }
  }

  build() {
    Canvas(this.ctx)
      .width(this.diameter)
      .height(this.diameter)
      .backgroundColor(Color.Transparent)
      .onReady(() => { this.ready = true; this.draw(); this.lastDrawnProgress = this.progress; })
  }
}

// Custom shutter button component
@Component
struct ShutterButton {
  @Prop isRecording: boolean = false;
  @Prop progress: number = 0;
  @Prop isPressed: boolean = false;
  @Prop buttonSize: number = 88;

  private getProgressStrokeWidth(): number {
    // Slim progress line: ~3% of diameter, min 3px for visibility
    return Math.max(3, Math.round(this.buttonSize * 0.03));
  }

  private getInnerCircleSize(): number {
    // Reduce shrink amount further for a balanced look
    // Idle ~16% of diameter, pressed/recording ~28%
    const d = this.buttonSize;
    const ringThickness = (this.isPressed || this.isRecording) ? d * 0.28 : d * 0.16;
    const innerDiameter = Math.max(0, d - 2 * ringThickness);
    return innerDiameter;
  }

  // Progress arc drawing moved to ShutterProgressCanvas above

  build() {
    Stack({}) {
      // Base ring (outer disk fill; inner circle above creates ring)
      Circle()
        .width(this.buttonSize)
        .height(this.buttonSize)
        .fill('#553A3A3C')
        .animation({ duration: 120, curve: Curve.EaseInOut })

      // Inner circle with smooth animation (render on top of outer disk)
      Circle()
        .width(this.getInnerCircleSize())
        .height(this.getInnerCircleSize())
        .fill((this.isRecording || this.isPressed) ? '#E0E0E0' : '#FFFFFF')
        .animation({ duration: 160, curve: Curve.EaseInOut })

      // Progress indicator (green arc) aligned with ring outer edge (topmost)
      ShutterProgressCanvas({
        progress: this.progress,
        diameter: this.buttonSize,
        stroke: this.getProgressStrokeWidth(),
      })
    }
    .width(this.buttonSize)
    .height(this.buttonSize)
  }
}

@Component
export struct CameraCaptureOverlay {
  @State private visible: boolean = CameraCaptureOverlayManager.isShowing();
  @State private options: CameraCaptureOptions | null = CameraCaptureOverlayManager.getOptions();
  @State private facing: CameraFacing = 'back';
  @State private isRecording: boolean = false;
  @State private currentMode: 'photo' | 'video' = 'photo';
  @State private recordingDuration: number = 0;
  @State private showFocusIndicator: boolean = false;
  @State private focusPosition: FocusPositionType = new FocusPositionType(0.5, 0.5);
  @State private isMix: boolean = false;
  @State private isImagesOnly: boolean = false;
  @State private isVideoOnly: boolean = false;
  @State private isPreview: boolean = false;
  @State private previewUri: string = '';
  @State private previewType: 'image' | 'video' = 'image';
  @State private hintText: ResourceStr = $r('app.string.lx_camera_tap_to_capture');
  @State private isCapturePressed: boolean = false; // For visual feedback
  @State private captureProgress: number = 0; // For video recording progress
  @State private showTimer: boolean = false;
  @State private timerDisplay: string = '00:00';
  @State private showingErrorHint: boolean = false;
  @State private flashOn: boolean = false;
  private recordingTimer: number = -1;
  private ctrl: XComponentController = new XComponentController();
  private photoSurfaceId: string = ''; // ArkTS ImageReceiver surface ID
  private photoOutputConfigured: boolean = false; // Track if PhotoOutput has been configured
  private imageReceiver?: image.ImageReceiver; // ArkTS ImageReceiver
  private videoRecorder?: media.AVRecorder;
  private videoFd: number = -1;
  private lastPhotoPath: string = '';
  private lastVideoPath: string = '';
  private cacheRoot: string = '';
  private longPressTimer: number = -1; // For long press detection
  private maxDurationTimer: number = -1; // For max duration timeout
  private recordingStartTs: number = 0;
  private readonly LONG_PRESS_DURATION: number = 280;
  private maxDurationSeconds: number = -1; // Max recording duration
  private readonly DEFAULT_PROGRESS_PERIOD: number = 15; // seconds, ring cycles when no max
  private stoppingVideo: boolean = false; // prevent duplicate stop
  private readonly MIN_RECORD_SECONDS: number = 1; // too-short threshold
  private shortHintTimer: number = -1;
  private startingVideo: boolean = false; // startVideo in progress
  private stopRequestedDuringStart: boolean = false; // request stop before start completes
  private stopDeferTimer: number = -1; // polls for start completion to stop

  // Small async helpers
  private async sleep(ms: number): Promise<void> {
    await new Promise<void>((resolve) => setTimeout(resolve, ms));
  }

  // Wait until file size stops growing to avoid playing half-finalized MP4
  private async waitFileStable(path: string, attempts: number = 6, intervalMs: number = 100): Promise<void> {
    try {
      let lastSize = -1;
      for (let i = 0; i < attempts; i++) {
        let size = 0;
        try {
          const st = fileIo.statSync(path);
          size = (st && typeof st.size === 'number') ? st.size : 0;
        } catch {
          size = 0;
        }
        if (size > 0 && size === lastSize) {
          break; // stable
        }
        lastSize = size;
        await this.sleep(intervalMs);
      }
    } catch {
      // ignore
    }
  }

  // Resolve and cache base cache directory
  private getCacheRoot(): string {
    if (this.cacheRoot && this.cacheRoot.length > 0) {
      return this.cacheRoot;
    }
    try {
      const current: CurrentLxApp = getCurrentLxapp();
      const lxinfo: LxAppInfo | null = getLxAppInfo(current.appid);
      if (lxinfo && lxinfo.cacheDir) {
        this.cacheRoot = String(lxinfo.cacheDir);
        return this.cacheRoot;
      }
    } catch { /* ignore and fallback */ }
    // Fallback to UI context cacheDir
    try {
      const ui = this.getUIContext();
      const ctx = ui.getHostContext() as common.UIAbilityContext;
      this.cacheRoot = String(ctx.cacheDir);
      return this.cacheRoot;
    } catch { /* ignore */ }
    return this.cacheRoot; // may be empty; callers should handle
  }

  aboutToAppear() {
    setInterval(() => {
      const now = CameraCaptureOverlayManager.isShowing();
      if (now !== this.visible) {
        this.visible = now;
        this.options = CameraCaptureOverlayManager.getOptions();
        if (now) {
          const m: string = (this.options?.mode || 'images').toLowerCase();
          this.isMix = (m === 'mix');
          this.isImagesOnly = (m === 'images');
          this.isVideoOnly = (m === 'videos');
          this.currentMode = this.isVideoOnly ? 'video' : 'photo';
          const facingPref = this.options?.cameraFacing;
          if (facingPref === 'front' || facingPref === 'back') {
            this.facing = facingPref;
          } else {
            this.facing = 'back';
          }
          const maxInput = this.options?.maxDurationSeconds;
          if (typeof maxInput === 'number' && Number.isFinite(maxInput) && maxInput > 0) {
            this.maxDurationSeconds = Math.floor(maxInput);
          } else {
            this.maxDurationSeconds = -1;
          }
          this.isRecording = false;
          this.showTimer = false;
          this.timerDisplay = '00:00';
          this.captureProgress = 0;
          this.recordingDuration = 0;
          this.isPreview = false;
          this.previewUri = '';
          this.previewType = 'image';
          this.isCapturePressed = false;
          this.lastPhotoPath = '';
          this.lastVideoPath = '';
          // Ensure we re-bind PhotoOutput for each new session
          this.photoOutputConfigured = false;
          this.photoSurfaceId = '';
          this.stopRecordingTimer();
          this.clearMaxDurationTimer();
          this.clearLongPressTimer();
          this.updateHint();
        }
      }
    }, 120);
  }

  private async onSurfaceReady(surfaceId: string): Promise<void> {
    try {
      // Initialize camera with preview surface FIRST
      await NativeCameraBridge.init(surfaceId, this.facing);

      // New session always requires PhotoOutput reconfiguration
      this.photoOutputConfigured = false;
      this.photoSurfaceId = '';

      this.createImageReceiver();
    } catch (err) {
      LxLog.e(TAG, `init camera failed: ${err}`);
    }
  }

  private createImageReceiver(): void {
    try {
      // Create ImageReceiver for 1280x720 JPEG images, capacity 8
      const imageSize: image.Size = { width: 1280, height: 720 };
      this.imageReceiver = image.createImageReceiver(imageSize, image.ImageFormat.JPEG, 8);

      // Register imageArrival callback (minimal logs)
      this.imageReceiver.on('imageArrival', () => {
        if (!this.imageReceiver) { return; }
        this.imageReceiver.readNextImage((err, img: image.Image) => {
          if (err || !img) { return; }
          img.getComponent(image.ComponentType.JPEG, (errMsg, component) => {
            if (errMsg || !component || !component.byteBuffer) {
              img.release();
              return;
            }
            this.savePhoto(component.byteBuffer, img);
          });
        });
      });

      // Get surface ID
      this.imageReceiver.getReceivingSurfaceId().then((surfaceId: string) => {
        this.photoSurfaceId = surfaceId;
      });

    } catch (err) {
      LxLog.e(TAG, `createImageReceiver failed: ${JSON.stringify(err)}`);
    }
  }

  private async savePhoto(buffer: ArrayBuffer, img: image.Image): Promise<void> {
    try {
      // Save directly under lxapp cache dir
      const base = this.getCacheRoot();
      const photoPath = `${base}/photo_${Date.now()}.jpg`;

      const photoFile = fileIo.openSync(photoPath, fileIo.OpenMode.READ_WRITE | fileIo.OpenMode.CREATE);
      fileIo.writeSync(photoFile.fd, buffer);
      fileIo.closeSync(photoFile);
      this.lastPhotoPath = photoPath;

      await img.release();
      this.enterPreviewMode(photoPath, 'image');

    } catch (err) {
      LxLog.e(TAG, `savePhoto failed: ${JSON.stringify(err)}`);
      const cb = this.options?.callbackId || '';
      if (cb) {
        onCallback(cb, false, JSON.stringify({ error: String(err) }));
      }
    }
  }

  private async takePhoto(): Promise<void> {
    const cb = this.options?.callbackId || '';
    if (!cb) return;

    try {
      // Ensure PhotoOutput configured before capture
      if (!this.photoOutputConfigured) {
        // Ensure cacheRoot prepared
        this.cacheRoot = this.getCacheRoot();
        if (!this.photoSurfaceId) {
          throw new Error('Photo surface not ready');
        }
        const rcBind = await NativeCameraBridge.startPhotoWithSurface(this.photoSurfaceId, cb, this.cacheRoot);
        if (!rcBind) {
          throw new Error('Configure PhotoOutput failed');
        }
        this.photoOutputConfigured = true;
        // Small delay to ensure PhotoOutput is ready
        await new Promise<void>((resolve) => setTimeout(resolve, 100));
      }

      // Trigger capture - ImageReceiver callback will handle the photo
      const success = await NativeCameraBridge.capturePhoto();
      if (!success) {
        throw new Error('Failed to start photo capture');
      }

      // Don't close UI yet - imageArrival callback will save photo and close UI
    } catch (err) {
      LxLog.e(TAG, `takePhoto FAILED: ${err instanceof Error ? err.message : String(err)}`);
      promptAction.showToast({ message: `拍照失败: ${err instanceof Error ? err.message : String(err)}`, duration: 3000 });
      // Notify logic layer of failure
      onCallback(cb, false, String(err));
      CameraCaptureOverlayManager.hide();
      await NativeCameraBridge.release().catch(() => {});
    }
  }

  private async startVideo(): Promise<void> {
    try {
      this.startingVideo = true;
      this.stopRequestedDuringStart = false;
      this.longPressTimer = -1;
      this.cacheRoot = this.getCacheRoot();

      this.videoRecorder = await media.createAVRecorder();
      const base = this.getCacheRoot();
      const videoPath = `${base}/video_${Date.now()}.mp4`;
      const fv = fileIo.openSync(videoPath, fileIo.OpenMode.READ_WRITE | fileIo.OpenMode.CREATE);
      this.videoFd = fv.fd;
      this.lastVideoPath = videoPath;
      const cfg: media.AVRecorderConfig = {
        audioSourceType: media.AudioSourceType.AUDIO_SOURCE_TYPE_MIC,
        videoSourceType: media.VideoSourceType.VIDEO_SOURCE_TYPE_SURFACE_YUV,
        profile: {
          audioBitrate: 48000,
          audioChannels: 1,
          audioCodec: media.CodecMimeType.AUDIO_AAC,
          audioSampleRate: 48000,
          fileFormat: media.ContainerFormatType.CFT_MPEG_4,
          videoBitrate: 3_000_000,
          videoCodec: media.CodecMimeType.VIDEO_AVC,
          videoFrameWidth: 1280,
          videoFrameHeight: 720,
          videoFrameRate: 30
        },
        url: `fd://${this.videoFd}`,
        metadata: { videoOrientation: (this.facing === 'front' ? '270' : '90') }
      };
      await this.videoRecorder!.prepare(cfg);
      const videoSurfaceId = await this.videoRecorder!.getInputSurface();
      await NativeCameraBridge.startVideoWithSurface(String(videoSurfaceId));
      await NativeCameraBridge.startVideoOutput();
      await this.videoRecorder!.start();
      this.isRecording = true;
      this.startRecordingTimer();
      this.updateHint();
      this.startingVideo = false;
      if (this.stopRequestedDuringStart) {
        // Defer stop to next tick to avoid race with start
        this.stopRequestedDuringStart = false;
        setTimeout(() => { this.stopVideo(); }, 0);
        return;
      }
    } catch (err) {
      LxLog.e(TAG, `startRecording failed: ${err}`);
      this.startingVideo = false;
      this.stopRequestedDuringStart = false;
    }
  }

  private startRecordingTimer(): void {
    this.clearRecordingInterval();
    this.clearMaxDurationTimer();
    this.recordingDuration = 0;
    this.captureProgress = 0; // start from zero to avoid initial back-jump
    this.recordingStartTs = Date.now();
    const initial = this.maxDurationSeconds > 0 ? this.maxDurationSeconds : this.DEFAULT_PROGRESS_PERIOD;
    this.showTimer = true;
    this.timerDisplay = this.formatRecordingDuration(initial);
    this.recordingTimer = setInterval(() => {
      const now = Date.now();
      const elapsed = Math.floor((now - this.recordingStartTs) / 1000);
      this.recordingDuration = Math.max(0, elapsed);
      if (this.maxDurationSeconds > 0) {
        const remaining = Math.max(0, this.maxDurationSeconds - this.recordingDuration);
        this.captureProgress = Math.min(1.0, this.recordingDuration / this.maxDurationSeconds);
        this.timerDisplay = this.formatRecordingDuration(remaining);
        if (remaining === 0) {
          this.clearRecordingInterval();
          // Backup stop in case maxDuration timeout missed
          this.stopVideo();
        }
      } else {
        // cycle countdown + ring when no max is provided
        const period = this.DEFAULT_PROGRESS_PERIOD;
        const t = this.recordingDuration % period;
        const remaining = period - t;
        this.captureProgress = t / period;
        this.timerDisplay = this.formatRecordingDuration(remaining);
      }
    }, 1000);
    if (this.maxDurationSeconds > 0) {
      this.maxDurationTimer = setTimeout(() => {
        this.stopVideo();
      }, this.maxDurationSeconds * 1000) as number;
    }
  }

  private stopRecordingTimer(): void {
    this.clearRecordingInterval();
    this.clearMaxDurationTimer();
    this.resetRecordingIndicators();
  }

  private clearRecordingInterval(): void {
    if (this.recordingTimer !== -1) {
      clearInterval(this.recordingTimer);
      this.recordingTimer = -1;
    }
  }

  private resetRecordingIndicators(): void {
    this.recordingDuration = 0;
    this.captureProgress = 0;
    this.showTimer = false;
    this.timerDisplay = '00:00';
    this.recordingStartTs = 0;
  }

  private clearMaxDurationTimer(): void {
    if (this.maxDurationTimer !== -1) {
      clearTimeout(this.maxDurationTimer);
      this.maxDurationTimer = -1;
    }
  }

  private clearLongPressTimer(): void {
    if (this.longPressTimer !== -1) {
      clearTimeout(this.longPressTimer);
      this.longPressTimer = -1;
    }
  }

  private clearStopDeferTimer(): void {
    if (this.stopDeferTimer !== -1) {
      clearInterval(this.stopDeferTimer);
      this.stopDeferTimer = -1;
    }
  }

  private ensureStopAfterStart(): void {
    this.clearStopDeferTimer();
    this.stopDeferTimer = setInterval(() => {
      if (this.isRecording) {
        this.clearStopDeferTimer();
        this.stopVideo();
      } else if (!this.startingVideo) {
        // Start aborted or failed
        this.clearStopDeferTimer();
      }
    }, 60) as number;
  }

  private async stopVideo(): Promise<void> {
    const cb = this.options?.callbackId || '';
    if (this.stoppingVideo) return;
    this.stoppingVideo = true;
    try {
      this.stopRequestedDuringStart = false;
      this.startingVideo = false;
      const elapsedMs = this.recordingStartTs > 0 ? (Date.now() - this.recordingStartTs) : 0;
      this.clearLongPressTimer();
      this.stopRecordingTimer();
      this.isCapturePressed = false;
      // Finalize recorder first, then stop camera output (aligns with reference impl)
      try { await this.videoRecorder?.stop(); } catch {}
      try { await this.videoRecorder?.release(); } catch {}
      this.videoRecorder = undefined;
      try { await NativeCameraBridge.stopVideoOutput(); } catch {}
      try {
        if (this.videoFd >= 0) {
          await fileIo.close(this.videoFd);
          this.videoFd = -1;
        }
      } catch {}
      this.isRecording = false;
      this.clearStopDeferTimer();
      const resPath = this.lastVideoPath || '';

      // Determine actual elapsed seconds precisely (from snapshot)
      const elapsedSec = Math.max(0, Math.floor(elapsedMs / 1000));
      const tooShort = elapsedSec < this.MIN_RECORD_SECONDS;

      if (tooShort) {
        // Delete too-short file and show in-place hint instead of exiting to preview
        try { if (resPath) { fileIo.unlinkSync(resPath); } } catch {}
        this.lastVideoPath = '';
        // Show short-duration hint at the hint text location
        this.hintText = $r('app.string.lx_error_video_too_short');
        this.showingErrorHint = true;
        if (this.shortHintTimer !== -1) { clearTimeout(this.shortHintTimer); this.shortHintTimer = -1; }
        this.shortHintTimer = setTimeout(() => {
          this.shortHintTimer = -1;
          this.showingErrorHint = false;
          this.updateHint();
        }, 1500) as number;
        // Stay in camera mode; do not enter preview, do not close overlay
        // Reset camera pipeline to clear any latent short-stop state
        try {
          const sid = String(this.ctrl.getXComponentSurfaceId());
          await NativeCameraBridge.release();
          await NativeCameraBridge.init(sid, this.facing);
          this.photoOutputConfigured = false;
          this.photoSurfaceId = '';
          this.createImageReceiver();
        } catch {}
      } else {
        // Normal case: enter preview when valid path exists; otherwise cancel
      if (resPath) {
          // Ensure file has stabilized before preview
          await this.waitFileStable(resPath, 6, 100);
          setTimeout(() => { this.enterPreviewMode(resPath, 'video'); }, 200);
      } else {
          onCallback(cb, false, '2000');
          CameraCaptureOverlayManager.hide();
          await NativeCameraBridge.release().catch(() => {});
        }
      }
    } catch (err) {
      onCallback(cb, false, String(err));
      CameraCaptureOverlayManager.hide();
      await NativeCameraBridge.release().catch(() => {});
    }
    this.stoppingVideo = false;
  }

  private cancel(): void {
    const cb = this.options?.callbackId || '';
    if (cb) { onCallback(cb, false, '2000'); }
    this.clearLongPressTimer();
    this.stopRecordingTimer();
    this.captureProgress = 0;
    this.isCapturePressed = false;
    this.photoOutputConfigured = false;
    this.photoSurfaceId = '';
    CameraCaptureOverlayManager.hide();
    NativeCameraBridge.release().catch(() => {});
  }

  private enterPreviewMode(uri: string, type: 'image' | 'video') {
    // Force re-mount of preview to avoid stale internal state causing black frame
    this.isPreview = false;
    this.previewUri = '';
    this.previewType = type;
    const normalized = this.normalizeMediaUri(uri);
    setTimeout(() => {
      this.previewUri = normalized;
      this.isPreview = true;
    }, 0);
    this.isRecording = false;
    this.stopRecordingTimer();
    this.isCapturePressed = false;
    this.captureProgress = 0;
    this.showTimer = false;
    this.timerDisplay = '00:00';
    this.clearLongPressTimer();
  }

  private exitPreview(): void {
    // Delete the captured file and return to camera
    try {
      if (this.previewUri) {
        const uri = this.previewUri;
        const path = uri.startsWith('file://') ? uri.replace('file://', '') : uri;
        fileIo.unlinkSync(path);
      }
    } catch (e) {
      LxLog.e(TAG, `Failed to delete preview file: ${e}`);
    }

    this.isPreview = false;
    this.previewUri = '';
    this.previewType = 'image';
    this.photoOutputConfigured = false;
    this.photoSurfaceId = '';
    this.resetToCameraMode();
    this.updateHint();
  }

  private completeCapture(): void {
    const cb = this.options?.callbackId || '';
    if (cb) {
      // Prefer raw filesystem path for logic layer to avoid extra copy via URI
      let outPath = '';
      if (this.previewType === 'video') {
        outPath = this.lastVideoPath || this.previewUri;
      } else {
        outPath = this.lastPhotoPath || this.previewUri;
      }
      if (outPath.startsWith('file://')) {
        outPath = outPath.replace('file://', '');
      }
      const res: CameraCaptureResult = { uri: outPath, fileType: this.previewType, isOriginal: true };
      onCallback(cb, true, JSON.stringify([res]));
    }
    CameraCaptureOverlayManager.hide();
    NativeCameraBridge.release().catch(() => {});
  }

  private resetToCameraMode(): void {
    this.isRecording = false;
    this.stopRecordingTimer();
    this.captureProgress = 0;
    this.isCapturePressed = false;
  }

  private onPreviewTouch(event: TouchEvent): void {
    if (event.type === TouchType.Down && event.touches.length === 1) {
      const touch = event.touches[0];
      const newPos = new FocusPositionType(touch.x, touch.y);
      this.focusPosition = newPos;
      this.showFocusIndicator = true;

      // Hide focus indicator after 2 seconds
      setTimeout(() => {
        this.showFocusIndicator = false;
      }, 2000);
    }
  }

  private formatRecordingDuration(seconds: number): string {
    const mins = Math.floor(seconds / 60);
    const secs = seconds % 60;
    return `${mins.toString().padStart(2, '0')}:${secs.toString().padStart(2, '0')}`;
  }

  private normalizeMediaUri(uri: string): string {
    if (!uri) { return ''; }
    const lower = uri.toLowerCase();
    if (lower.startsWith('file://') || lower.startsWith('content://') || lower.startsWith('datashare://')) {
      return uri;
    }
    if (uri.startsWith('/')) {
      return `file://${uri}`;
    }
    return uri;
  }

  private previewSource(): string {
    return this.normalizeMediaUri(this.previewUri);
  }

  private async switchCamera(): Promise<void> {
    if (this.isRecording || this.isPreview) return;
    this.facing = this.facing === 'back' ? 'front' : 'back';
    await NativeCameraBridge.switchFacing(this.facing).catch(() => {});
  }

  private toggleFlash(): void {
    if (this.isRecording || this.isPreview || this.activeMode() === 'video') return;
    const newState = !this.flashOn;
    const success: boolean = cameraSetFlashMode(newState);
    if (success) {
      this.flashOn = newState;
      hilog.info(DOMAIN, TAG, `Flash ${this.flashOn ? 'ON' : 'OFF'}`);
    } else {
      hilog.warn(DOMAIN, TAG, `Failed to set flash mode to ${newState ? 'ON' : 'OFF'}`);
    }
  }

  private updateHint(): void {
    const activeMode = this.isVideoOnly ? 'video' : (this.isImagesOnly ? 'photo' : this.currentMode);
    if (this.showingErrorHint) {
      return;
    }
    if (this.isRecording) {
      this.hintText = $r('app.string.lx_camera_release_to_stop');
      return;
    }
    if (activeMode === 'video') {
      this.hintText = $r('app.string.lx_camera_long_press_to_record');
      return;
    }
    this.hintText = $r('app.string.lx_camera_tap_to_capture');
  }

  private activeMode(): 'photo' | 'video' {
    if (this.isVideoOnly) { return 'video'; }
    if (this.isImagesOnly) { return 'photo'; }
    return this.currentMode;
  }

  build() {
    if (!this.visible) {
      Row({}) {
      }.width(0).height(0);
    } else {
      Stack({ alignContent: Alignment.TopStart }) {
        if (this.isPreview && this.previewType === 'image') {
          Image(this.previewSource())
            .width('100%')
            .height('100%')
            .objectFit(ImageFit.Cover)
        } else if (this.isPreview) {
          Video({ src: this.previewSource() })
            .width('100%')
            .height('100%')
            .objectFit(ImageFit.Contain)
            .controls(false)
            .muted(true)
            .autoPlay(true)
            .loop(true)
        } else {
          XComponent({ type: XComponentType.SURFACE, controller: this.ctrl })
            .onLoad(() => {
              try {
                const sid = this.ctrl.getXComponentSurfaceId();
                this.onSurfaceReady(String(sid));
              } catch (e) {
                LxLog.e(TAG, `get surfaceId failed: ${e}`);
              }
            })
            .width('100%')
            .height('100%')
            .backgroundColor('#000000')
            .onTouch((event: TouchEvent) => {
              this.onPreviewTouch(event);
            })
        }

        // Gradient overlay for better UI contrast
        Column({})
          .width('100%')
          .height('100%')
          .linearGradient({
            angle: 180,
            colors: [
              ['rgba(0,0,0,0.25)', 0.0],
              ['rgba(0,0,0,0)', 0.15],
              ['rgba(0,0,0,0)', 0.75],
              ['rgba(0,0,0,0.38)', 1.0]
            ]
          })
          .hitTestBehavior(HitTestMode.Transparent)

        if (this.isPreview) {
          // Back button using SVG icon
          Image($rawfile('icons/icon_back_curved.svg'))
            .width(ICON_BUTTON_SIZE)
            .height(ICON_BUTTON_SIZE)
            .fillColor(Color.White)
            .position({ left: 16, top: 44 })
            .onClick(() => { this.exitPreview(); })

          Button({}) {
            Text($r('app.string.lx_common_done'))
              .fontSize(14)
              .fontColor(Color.White)
              .fontWeight(FontWeight.Normal)
          }
          .padding({ left: 16, right: 16, top: 6, bottom: 6 })
          .backgroundColor('#FF007AFF')
          .borderRadius(18)
          .position({ right: 20, bottom: 20 })
          .onClick(() => {
            this.completeCapture();
          })
        } else {
          if (!this.isRecording) {
            // Camera switch button using SVG icon
            Image($rawfile('icons/icon_camera_switch.svg'))
              .width(CAMERA_SWITCH_ICON_SIZE)
              .height(CAMERA_SWITCH_ICON_SIZE)
              .fillColor(Color.White)
              .position({ right: 24, top: 60 })
              .onClick((): void => { this.switchCamera(); })
          }

        if (this.showTimer) {
          // Centered countdown bubble
          Row({}) {
            Row({}) {
              Text('●')
                .fontSize(16)
                .fontColor(Color.Red)
                .margin({ right: 4 })
              Text(this.timerDisplay)
                .fontSize(16)
                .fontColor(Color.White)
                .fontFamily('monospace')
            }
            .padding({ left: 10, right: 10, top: 6, bottom: 6 })
            .backgroundColor('rgba(0,0,0,0.6)')
            .borderRadius(14)
          }
          .width('100%')
          .justifyContent(FlexAlign.Center)
          .position({ top: 38, left: 0, right: 0 })
        }

          Column({}) {
            if (this.isMix) {
              Row({}) {
                Button({}) {
                  Text('Photo')
                    .fontSize(14)
                    .fontColor(Color.White)
                    .opacity(this.activeMode() === 'photo' ? 1 : 0.7)
                }
                .width(72)
                .height(36)
                .backgroundColor(this.activeMode() === 'photo' ? '#007AFF' : 'rgba(255,255,255,0.15)')
                .borderRadius(18)
                .onClick(() => {
                  if (this.isRecording) { return; }
                  this.currentMode = 'photo';
                  this.updateHint();
                  this.captureProgress = 0;
                  this.isCapturePressed = false;
                })

                Button({}) {
                  Text('Video')
                    .fontSize(14)
                    .fontColor(Color.White)
                    .opacity(this.activeMode() === 'video' ? 1 : 0.7)
                }
                .width(72)
                .height(36)
                .backgroundColor(this.activeMode() === 'video' ? '#FF3B30' : 'rgba(255,255,255,0.15)')
                .borderRadius(18)
                .margin({ left: 8 })
                .onClick(() => {
                  if (this.isRecording) { return; }
                  this.currentMode = 'video';
                  this.updateHint();
                  this.captureProgress = 0;
                  this.isCapturePressed = false;
                })
              }
              .justifyContent(FlexAlign.Center)
              .padding({ bottom: 20 })
            } else {
              Blank()
                .height(20)
            }

            if (!this.isRecording) {
              Text(this.hintText)
                .fontSize(14)
                .fontColor('rgba(204,204,204,1.0)')
                .textAlign(TextAlign.Center)
                .padding({ bottom: 28 })
            } else {
              Blank().height(16)
            }

            Row({}) {
              if (!this.isRecording) {
                // Cancel button using chevron down SVG icon
                Image($rawfile('icons/icon_chevron_down.svg'))
                  .width(ICON_BUTTON_SIZE)
                  .height(ICON_BUTTON_SIZE)
                  .fillColor(Color.White)
                  .onClick(() => {
                    this.cancel();
                  })
              } else {
                Blank()
                  .width(ICON_BUTTON_SIZE)
                  .height(ICON_BUTTON_SIZE)
              }

              ShutterButton({
                // Show ring when recording or in mix while pressed
                isRecording: this.isRecording,
                progress: this.captureProgress,
                isPressed: this.isCapturePressed,
                buttonSize: 88
              })
              .margin({ left: 36, right: 36 })

              if (!this.isRecording && this.activeMode() !== 'video') {
                // Flash toggle button
                Image(this.flashOn ? $rawfile('icons/icon_camera_flash_on.svg') : $rawfile('icons/icon_camera_flash_off.svg'))
                  .width(ICON_BUTTON_SIZE)
                  .height(ICON_BUTTON_SIZE)
                  .fillColor(Color.White)
                  .onClick(() => {
                    this.toggleFlash();
                  })
              } else {
                Blank()
                  .width(ICON_BUTTON_SIZE)
                  .height(ICON_BUTTON_SIZE)
              }
            }
            .justifyContent(FlexAlign.Center)
            .gesture(
              GestureGroup(GestureMode.Exclusive,
                LongPressGesture({ repeat: false, duration: this.LONG_PRESS_DURATION })
                  .onAction(() => {
                    if (!this.isPreview && this.isVideoOnly && !this.isRecording && !this.startingVideo) {
                      this.isCapturePressed = true;
                      this.startVideo();
                    }
                  })
                  .onActionEnd(() => {
                    if (this.isRecording) {
                      this.stopVideo();
                    } else if (this.startingVideo) {
                      this.stopRequestedDuringStart = true;
                      this.ensureStopAfterStart();
                    } else {
                      this.isCapturePressed = false;
                    }
                  }),
                TapGesture({ count: 1 })
                  .onAction(() => {
                    if (!this.isPreview && this.isImagesOnly) {
                      this.takePhoto();
                    }
                  })
              )
            )
          }
          .width('100%')
          .position({ left: 0, right: 0, bottom: 44 })
        }

        // Focus Indicator (only in camera mode)
        if (this.showFocusIndicator && !this.isPreview) {
          Circle({})
            .width(60)
            .height(60)
            .stroke(Color.White)
            .strokeWidth(2)
            .fill(Color.Transparent)
            .position({ x: this.focusPosition.x - 30, y: this.focusPosition.y - 30 })
            .markAnchor({ x: 0, y: 0 })
            .animation({
              duration: 200,
              curve: Curve.EaseInOut
            })
        }
      }
      .width('100%')
      .height('100%')
      .backgroundColor('#000000')
    }
  }
}
