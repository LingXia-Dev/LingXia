import { hilog } from '@kit.PerformanceAnalysisKit';
import { onCallback, cameraInit, cameraRelease, cameraSwitchFacing, cameraTakePhoto, cameraStartPhotoWithSurface, cameraStartVideoWithSurface, cameraVideoOutputStart, cameraVideoOutputStopAndRelease, getCurrentLxapp, getLxAppInfo, type LxAppInfo, type CurrentLxApp } from 'liblingxia.so';
import { image } from '@kit.ImageKit';
import { media } from '@kit.MediaKit';
import { fileIo } from '@kit.CoreFileKit';
import common from '@ohos.app.ability.common';
import { promptAction } from '@kit.ArkUI';

const DOMAIN = 0x0000;
const TAG = 'LingXia.CameraCapture';

// Helper function to convert degrees to radians
function toRadians(degrees: number): number {
  return degrees * Math.PI / 180;
}

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

// V-shaped cutout chevron button - white circle with cutout V
@Component
struct CutoutChevronButton {
  private settings: RenderingContextSettings = new RenderingContextSettings(true);
  private context: CanvasRenderingContext2D = new CanvasRenderingContext2D(this.settings);

  build() {
    Canvas(this.context)
      .width(28)
      .height(28)
      .onReady(() => {
        const ctx = this.context;
        const size = 28;
        const cx = size / 2;
        const cy = size / 2;

        // Draw white circle background
        ctx.fillStyle = '#FFFFFF';
        ctx.beginPath();
        ctx.arc(cx, cy, cx, 0, Math.PI * 2);
        ctx.fill();

        // Use destination-out to cut out V shape
        ctx.globalCompositeOperation = 'destination-out';
        ctx.strokeStyle = '#000000';  // Color doesn't matter
        ctx.lineWidth = size * 0.055;  // Thinner stroke
        ctx.lineCap = 'round';
        ctx.lineJoin = 'round';

        // Draw V shape (chevron pointing down) - smaller opening, centered
        const vWidth = size * 0.32;  // Narrower V
        const vHeight = size * 0.18;  // Shorter height
        ctx.beginPath();
        ctx.moveTo(cx - vWidth / 2, cy - vHeight / 2);  // Left top
        ctx.lineTo(cx, cy + vHeight / 2);               // Bottom center
        ctx.lineTo(cx + vWidth / 2, cy - vHeight / 2);  // Right top
        ctx.stroke();

        // Reset composite operation
        ctx.globalCompositeOperation = 'source-over';
      })
  }
}

// Camera switch icon (Canvas)
@Component
struct CameraSwitchIcon {
  private settings: RenderingContextSettings = new RenderingContextSettings(true);
  private context: CanvasRenderingContext2D = new CanvasRenderingContext2D(this.settings);

  build() {
    Canvas(this.context)
      .width(44)
      .height(44)
      .onReady(() => {
        const ctx = this.context;
        const size = 44;
        const cx = size / 2;
        const cy = size / 2;

        // Draw white camera body (rounded rect)
        ctx.fillStyle = '#FFFFFF';
        const bodyWidth = size * 0.65;
        const bodyHeight = size * 0.40;
        const bodyLeft = cx - bodyWidth * 0.5;
        const bodyRight = cx + bodyWidth * 0.5;
        const bodyTop = cy - bodyHeight * 0.2;
        const bodyBottom = cy + bodyHeight * 0.8;
        const bodyRadius = size * 0.05;
        ctx.beginPath();
        ctx.moveTo(bodyLeft + bodyRadius, bodyTop);
        ctx.lineTo(bodyRight - bodyRadius, bodyTop);
        ctx.arcTo(bodyRight, bodyTop, bodyRight, bodyTop + bodyRadius, bodyRadius);
        ctx.lineTo(bodyRight, bodyBottom - bodyRadius);
        ctx.arcTo(bodyRight, bodyBottom, bodyRight - bodyRadius, bodyBottom, bodyRadius);
        ctx.lineTo(bodyLeft + bodyRadius, bodyBottom);
        ctx.arcTo(bodyLeft, bodyBottom, bodyLeft, bodyBottom - bodyRadius, bodyRadius);
        ctx.lineTo(bodyLeft, bodyTop + bodyRadius);
        ctx.arcTo(bodyLeft, bodyTop, bodyLeft + bodyRadius, bodyTop, bodyRadius);
        ctx.closePath();
        ctx.fill();

        // Draw trapezoid viewfinder hump
        const topWidth = bodyWidth * 0.35;
        const bottomWidth = bodyWidth * 0.45;
        const trapHeight = bodyHeight * 0.35;
        const topLeft = cx - topWidth * 0.5;
        const topRight = cx + topWidth * 0.5;
        const bottomLeft = cx - bottomWidth * 0.5;
        const bottomRight = cx + bottomWidth * 0.5;
        const trapTop = bodyTop - trapHeight * 0.8;
        const trapBottom = bodyTop + trapHeight * 0.2;
        ctx.beginPath();
        ctx.moveTo(topLeft, trapTop);
        ctx.lineTo(topRight, trapTop);
        ctx.lineTo(bottomRight, trapBottom);
        ctx.lineTo(bottomLeft, trapBottom);
        ctx.closePath();
        ctx.fill();

        // Cut-out curved arrows (destination-out)
        ctx.globalCompositeOperation = 'destination-out';
        ctx.strokeStyle = '#000000';
        ctx.lineWidth = size * 0.04;
        ctx.lineCap = 'round';
        ctx.lineJoin = 'round';

        const radius = size * 0.15;
        const bodyCenter = cy + bodyHeight * 0.30;

        // Top curved arrow
        ctx.beginPath();
        ctx.arc(cx, bodyCenter, radius * 0.7, toRadians(-30), toRadians(90), false);
        ctx.stroke();
        const topEndAngle = 90;
        const topEndX = cx + radius * 0.7 * Math.cos(toRadians(topEndAngle));
        const topEndY = bodyCenter + radius * 0.7 * Math.sin(toRadians(topEndAngle));
        this.drawArrowhead(ctx, topEndX, topEndY, topEndAngle + 30, size);

        // Bottom curved arrow
        ctx.beginPath();
        ctx.arc(cx, bodyCenter, radius * 0.7, toRadians(150), toRadians(270), false);
        ctx.stroke();
        const bottomEndAngle = -90;
        const bottomEndX = cx + radius * 0.7 * Math.cos(toRadians(bottomEndAngle));
        const bottomEndY = bodyCenter + radius * 0.7 * Math.sin(toRadians(bottomEndAngle));
        this.drawArrowhead(ctx, bottomEndX, bottomEndY, bottomEndAngle - 30, size);

        ctx.globalCompositeOperation = 'source-over';
      })
  }

  private drawArrowhead(ctx: CanvasRenderingContext2D, x: number, y: number, angle: number, size: number): void {
    const arrowSize = size * 0.03;
    const rad1 = toRadians(angle + 25);
    const rad2 = toRadians(angle - 25);
    const x1 = x + arrowSize * Math.cos(rad1);
    const y1 = y + arrowSize * Math.sin(rad1);
    const x2 = x + arrowSize * Math.cos(rad2);
    const y2 = y + arrowSize * Math.sin(rad2);
    ctx.beginPath();
    ctx.moveTo(x, y);
    ctx.lineTo(x1, y1);
    ctx.stroke();
    ctx.beginPath();
    ctx.moveTo(x, y);
    ctx.lineTo(x2, y2);
    ctx.stroke();
  }
}
// Round back arrow button for preview  - white circle with cutout curved arrow
@Component
struct RoundBackArrowButton {
  private settings: RenderingContextSettings = new RenderingContextSettings(true);
  private context: CanvasRenderingContext2D = new CanvasRenderingContext2D(this.settings);

  build() {
    Canvas(this.context)
      .width(28)
      .height(28)
      .onReady(() => {
        const ctx = this.context;
        const size = 28;
        const cx = size / 2;
        const cy = size / 2;
        const r = size / 2;

        // Step 1: Draw white circle background
        ctx.fillStyle = '#FFFFFF';
        ctx.beginPath();
        ctx.arc(cx, cy, r, 0, Math.PI * 2);
        ctx.fill();

        // Step 2: Use destination-out to cut out curved arrow
        ctx.globalCompositeOperation = 'destination-out';
        ctx.strokeStyle = '#000000';
        ctx.lineWidth = Math.max(size * 0.06, 1.5);
        ctx.lineCap = 'round';
        ctx.lineJoin = 'round';

        // Draw curved return arrow path
        const chevronTipX = cx - r * 0.4;
        const chevronY = cy - r * 0.1;
        const chevronSize = r * 0.2;

        // Left chevron (<)
        ctx.beginPath();
        ctx.moveTo(chevronTipX + chevronSize, chevronY - chevronSize * 0.6);
        ctx.lineTo(chevronTipX, chevronY);
        ctx.lineTo(chevronTipX + chevronSize, chevronY + chevronSize * 0.6);
        ctx.stroke();

        // Horizontal line from chevron to the right
        ctx.beginPath();
        ctx.moveTo(chevronTipX + chevronSize, chevronY);
        ctx.lineTo(cx + r * 0.4, chevronY);
        ctx.stroke();

        // Smooth arc using quadratic curve
        const arcStartX = cx + r * 0.4;
        const arcStartY = chevronY;
        const arcEndX = cx + r * 0.25;
        const arcEndY = cy + r * 0.25;

        ctx.beginPath();
        ctx.moveTo(arcStartX, arcStartY);
        ctx.quadraticCurveTo(
          arcStartX + r * 0.1, arcEndY,  // Control point
          arcEndX, arcEndY
        );
        ctx.stroke();

        // Short line after the turn
        ctx.beginPath();
        ctx.moveTo(arcEndX, arcEndY);
        ctx.lineTo(cx - r * 0.05, arcEndY);
        ctx.stroke();

        // Reset composite operation
        ctx.globalCompositeOperation = 'source-over';
      })
  }
}

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
  @State private hintText: string = '轻触拍照';
  @State private isCapturePressed: boolean = false; // For visual feedback
  @State private captureProgress: number = 0; // For video recording progress
  @State private showTimer: boolean = false;
  @State private timerDisplay: string = '00:00';
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

  private async onSurfaceReady(surfaceId: string) {
    try {
      // Initialize camera with preview surface FIRST
      await NativeCameraBridge.init(surfaceId, this.facing);

      // New session always requires PhotoOutput reconfiguration
      this.photoOutputConfigured = false;
      this.photoSurfaceId = '';

      this.createImageReceiver();
    } catch (err) {
      hilog.error(DOMAIN, TAG, `init camera failed: ${err}`);
    }
  }

  private createImageReceiver() {
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
      hilog.error(DOMAIN, TAG, `createImageReceiver failed: ${JSON.stringify(err)}`);
    }
  }

  private async savePhoto(buffer: ArrayBuffer, img: image.Image) {
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
      hilog.error(DOMAIN, TAG, `savePhoto failed: ${JSON.stringify(err)}`);
      const cb = this.options?.callbackId || '';
      if (cb) {
        onCallback(cb, false, JSON.stringify({ error: String(err) }));
      }
    }
  }

  private async takePhoto() {
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
      hilog.error(DOMAIN, TAG, `takePhoto FAILED: ${err instanceof Error ? err.message : String(err)}`);
      promptAction.showToast({ message: `拍照失败: ${err instanceof Error ? err.message : String(err)}`, duration: 3000 });
      // Notify logic layer of failure
      onCallback(cb, false, String(err));
      CameraCaptureOverlayManager.hide();
      await NativeCameraBridge.release().catch(() => {});
    }
  }

  private async startVideo() {
    try {
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
    } catch (err) {
      hilog.error(DOMAIN, TAG, `startRecording failed: ${err}`);
    }
  }

  private startRecordingTimer() {
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

  private stopRecordingTimer() {
    this.clearRecordingInterval();
    this.clearMaxDurationTimer();
    this.resetRecordingIndicators();
  }

  private clearRecordingInterval() {
    if (this.recordingTimer !== -1) {
      clearInterval(this.recordingTimer);
      this.recordingTimer = -1;
    }
  }

  private resetRecordingIndicators() {
    this.recordingDuration = 0;
    this.captureProgress = 0;
    this.showTimer = false;
    this.timerDisplay = '00:00';
    this.recordingStartTs = 0;
  }

  private clearMaxDurationTimer() {
    if (this.maxDurationTimer !== -1) {
      clearTimeout(this.maxDurationTimer);
      this.maxDurationTimer = -1;
    }
  }

  private clearLongPressTimer() {
    if (this.longPressTimer !== -1) {
      clearTimeout(this.longPressTimer);
      this.longPressTimer = -1;
    }
  }

  private async stopVideo() {
    const cb = this.options?.callbackId || '';
    if (!cb) return;
    try {
      this.clearLongPressTimer();
      this.stopRecordingTimer();
      try { await this.videoRecorder?.stop(); } catch {}
      try { await this.videoRecorder?.release(); } catch {}
      this.videoRecorder = undefined;
      await NativeCameraBridge.stopVideoOutput();
      try {
        if (this.videoFd >= 0) {
          await fileIo.close(this.videoFd);
          this.videoFd = -1;
        }
      } catch {}
      this.isRecording = false;
      this.updateHint();
      const resPath = this.lastVideoPath || '';
      // Ensure entering preview mode
      if (resPath) {
        this.enterPreviewMode(resPath, 'video');
      } else {
        onCallback(cb, true, '{"cancel":true}');
        CameraCaptureOverlayManager.hide();
        await NativeCameraBridge.release().catch(() => {});
      }
    } catch (err) {
      onCallback(cb, false, String(err));
      CameraCaptureOverlayManager.hide();
      await NativeCameraBridge.release().catch(() => {});
    }
  }

  private cancel() {
    const cb = this.options?.callbackId || '';
    if (cb) { onCallback(cb, true, '{"cancel":true}'); }
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
    this.previewUri = uri;
    this.previewType = type;
    this.isPreview = true;
    this.isRecording = false;
    this.stopRecordingTimer();
    this.isCapturePressed = false;
    this.captureProgress = 0;
    this.showTimer = false;
    this.timerDisplay = '00:00';
    this.clearLongPressTimer();
  }

  private exitPreview() {
    // Delete the captured file and return to camera
    try {
      if (this.previewUri) {
        fileIo.unlinkSync(this.previewUri);
      }
    } catch (e) {
      hilog.error(DOMAIN, TAG, `Failed to delete preview file: ${e}`);
    }

    this.isPreview = false;
    this.previewUri = '';
    this.previewType = 'image';
    this.photoOutputConfigured = false;
    this.photoSurfaceId = '';
    this.resetToCameraMode();
    this.updateHint();
  }

  private completeCapture() {
    const cb = this.options?.callbackId || '';
    if (cb && this.previewUri) {
      const res: CameraCaptureResult = { uri: this.previewUri, fileType: this.previewType, isOriginal: true };
      onCallback(cb, true, JSON.stringify([res]));
    }
    CameraCaptureOverlayManager.hide();
    NativeCameraBridge.release().catch(() => {});
  }

  private resetToCameraMode() {
    this.isRecording = false;
    this.stopRecordingTimer();
    this.captureProgress = 0;
    this.isCapturePressed = false;
  }

  private onPreviewTouch(event: TouchEvent) {
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

  private async switchCamera() {
    if (this.isRecording || this.isPreview) return;
    this.facing = this.facing === 'back' ? 'front' : 'back';
    await NativeCameraBridge.switchFacing(this.facing).catch(() => {});
  }

  private updateHint(): void {
    const activeMode = this.isVideoOnly ? 'video' : (this.isImagesOnly ? 'photo' : this.currentMode);
    if (this.isRecording) {
      this.hintText = '松开停止';
      return;
    }
    if (activeMode === 'video') {
      this.hintText = '长按摄像';
      return;
    }
    this.hintText = '轻触拍照';
  }

  // Handle capture button touch events for both photo and video modes
  private handleCaptureTouch(event: TouchEvent) {
    if (this.isPreview) return;
    const supportsVideo = this.isVideoOnly || this.isMix;

    switch (event.type) {
      case TouchType.Down:
        if (supportsVideo) {
          this.isCapturePressed = true;
          this.clearLongPressTimer();
          this.longPressTimer = setTimeout(() => {
            this.longPressTimer = -1;
            if (!this.isRecording) {
              this.startVideo();
            }
          }, this.LONG_PRESS_DURATION) as number;
        } else {
          // images only
          this.takePhoto();
        }
        break;

      case TouchType.Up:
      case TouchType.Cancel:
        if (supportsVideo) {
          if (this.longPressTimer !== -1) {
            // short tap
            this.clearLongPressTimer();
            if (this.isMix) {
              this.takePhoto();
            }
          } else if (this.isRecording) {
            // long press finished
            this.stopVideo();
          }
        }
        this.isCapturePressed = false;
        break;
    }
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
            .objectFit(ImageFit.Cover)
            .controls(false)
            .autoPlay(true)
            .loop(true)
        } else {
          XComponent({ type: XComponentType.SURFACE, controller: this.ctrl })
            .onLoad(() => {
              try {
                const sid = this.ctrl.getXComponentSurfaceId();
                this.onSurfaceReady(String(sid));
              } catch (e) {
                hilog.error(DOMAIN, TAG, `get surfaceId failed: ${e}`);
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
          // Back button using Canvas-drawn component
          Button({}) {
            RoundBackArrowButton()
          }
          .width(28)
          .height(28)
          .padding(0)
          .backgroundColor(Color.Transparent)
          .position({ left: 16, top: 44 })
          .onClick(() => { this.exitPreview(); })

          Button({}) {
            Text('完成')
              .fontSize(14)
              .fontColor(Color.White)
              .fontWeight(FontWeight.Normal)
          }
          .padding({ left: 16, right: 16, top: 6, bottom: 6 })
          .backgroundColor('#FF07C160')
          .borderRadius(18)
          .position({ right: 20, bottom: 20 })
          .onClick(() => {
            this.completeCapture();
          })
        } else {
          if (!this.isRecording) {
            // Camera switch button
            Button({}) {
              CameraSwitchIcon()
            }
            .width(44)
            .height(44)
            .padding(0)
            .backgroundColor(Color.Transparent)
            .position({ right: 16, top: 60 })
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
                .padding({ bottom: 16 })
            } else {
              Blank()
                .height(16)
            }

            Row({}) {
              if (!this.isRecording) {
                // V-shaped cutout chevron button
                Button({}) {
                  CutoutChevronButton()
                }
                .width(28)
                .height(28)
                .padding(0)
                .backgroundColor(Color.Transparent)
                .onClick(() => {
                  this.cancel();
                })
              } else {
                Blank()
                  .width(28)
                  .height(28)
              }

              ShutterButton({
                // Show ring when recording or in mix while pressed
                isRecording: this.isRecording,
                progress: this.captureProgress,
                isPressed: this.isCapturePressed,
                buttonSize: 88
              })
              .margin({ left: 36, right: 36 })
              .onTouch((event: TouchEvent) => {
                this.handleCaptureTouch(event);
              })

              Blank()
                .width(32)
                .height(32)
            }
            .justifyContent(FlexAlign.Center)
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
