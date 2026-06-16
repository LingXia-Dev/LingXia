/**
 * Media picker, preview, scan, and file processing APIs.
 */

export interface GetImageInfoOptions {
  path: string;
}

export interface ImageInfo {
  width: number;
  height: number;
  type: string;
  path: string;
}

export interface CompressImageOptions {
  path: string;
  quality?: number;
  compressedWidth?: number;
  compressedHeight?: number;
}

export interface CompressImageResult {
  tempFilePath: string;
}

export type VideoCompressQuality = 'low' | 'medium' | 'high';

export interface CompressVideoOptions {
  /**
   * Source video path or `lx://` URI.
   */
  path: string;
  /**
   * Cross-platform note: video compression parameters are best-effort and may map to
   * native presets instead of exact encoder settings.
   *
   * Compression quality preset.
   * When provided, `bitrate`, `fps`, and `resolution` are ignored.
   */
  quality?: VideoCompressQuality;
  /**
   * Preferred target video bitrate in kbps.
   * May be adjusted or ignored by platform codec/runtime limitations.
   */
  bitrate?: number;
  /**
   * Preferred target frame rate in fps.
   * Some platforms may ignore this option.
   */
  fps?: number;
  /**
   * Target resolution scale ratio relative to source size, in range `(0, 1]`.
   * May be approximated or ignored by platform transcoder capabilities.
   */
  resolution?: number;
  /**
   * Optional output path for compressed file.
   */
  outputPath?: string;
}

export interface CompressVideoResult {
  tempFilePath: string;
  width: number;
  height: number;
  durationMs: number;
  /**
   * Output file size in bytes.
   * Could be close to source size when platform falls back to source content.
   */
  size: number;
  type: string;
}

export interface CompressVideoProgressEvent {
  /** Transcode progress in percent, `0`-`100`. */
  progress: number;
}

export interface CompressVideoIteratorResult {
  done: boolean;
  value?: CompressVideoProgressEvent;
}

/**
 * Handle returned by `lx.compressVideo`.
 *
 * Awaiting the task resolves with the final {@link CompressVideoResult}.
 * Iterating it with `for await` yields {@link CompressVideoProgressEvent}s
 * while the transcode runs.
 */
export interface CompressVideoTask
  extends PromiseLike<CompressVideoResult>,
    AsyncIterable<CompressVideoProgressEvent> {
  next(): Promise<CompressVideoIteratorResult>;
  /** Stops iteration only. Does not cancel the compression. */
  return(): Promise<CompressVideoIteratorResult>;
  catch<TResult = never>(
    onrejected?: ((reason: unknown) => TResult | PromiseLike<TResult>) | null,
  ): Promise<CompressVideoResult | TResult>;
  finally(onfinally?: (() => void) | null): Promise<CompressVideoResult>;
  /**
   * Cancels the transcode and deletes any partial output.
   * The task promise rejects with an `AbortError` (`code: 'E_ABORT'`).
   */
  cancel(): void;
  wait(): Promise<CompressVideoResult>;
}

export interface GetVideoInfoOptions {
  /**
   * Video file path or `lx://` URI.
   */
  path: string;
}

export interface VideoInfo {
  /**
   * Encoded display width in pixels.
   */
  width: number;
  /**
   * Encoded display height in pixels.
   */
  height: number;
  /**
   * Video duration in milliseconds.
   */
  durationMs: number;
  /**
   * Clockwise rotation in degrees (usually `0 | 90 | 180 | 270`).
   */
  rotation?: number;
  /**
   * Average bitrate in bits per second (bps).
   */
  bitrate?: number;
  /**
   * Frame rate in frames per second (fps).
   */
  fps?: number;
  /**
   * MIME type, e.g. `video/mp4`.
   */
  type?: string;
  /**
   * Resolved path used by runtime (typically `lx://...`).
   */
  path: string;
}

export interface ExtractVideoThumbnailOptions {
  /**
   * Source video path or `lx://` URI.
   */
  path: string;
  /**
   * Optional output image path. If omitted, runtime chooses a temporary path.
   */
  outputPath?: string;
  /**
   * Max output width in pixels.
   * Optional; when set with/without `maxHeight`, output keeps aspect ratio (no cropping).
   */
  maxWidth?: number;
  /**
   * Max output height in pixels.
   * Optional; when set with/without `maxWidth`, output keeps aspect ratio (no cropping).
   */
  maxHeight?: number;
  /**
   * Target frame time in milliseconds from video start.
   * `0` means first frame.
   */
  timeMs?: number;
  /**
   * JPEG quality in range `0-100`.
   */
  quality?: number;
}

export interface ExtractVideoThumbnailResult {
  /**
   * Generated thumbnail file path.
   */
  tempFilePath: string;
  /**
   * Output image width in pixels.
   */
  width: number;
  /**
   * Output image height in pixels.
   */
  height: number;
  /**
   * Output MIME type, usually `image/jpeg`.
   */
  type: string;
}

export interface ChooseMediaOptions {
  count?: number;
  mediaType?: ('image' | 'video')[];
  sourceType?: ('album' | 'camera')[];
  camera?: 'back' | 'front';
  maxDuration?: number;
}

export interface ChosenMediaEntry {
  tempFilePath: string;
  fileType: 'image' | 'video';
  isOriginal: boolean;
}

export type MediaRotation = 0 | 90 | 180 | 270;

export type MediaObjectFit = 'cover' | 'contain' | 'fill' | 'fit';

export interface PreviewMediaSource {
  /**
   * Media source path.
   * Recommended: `lx://` path (for example `lx://usercache/...`) or a sandbox-local path
   * that can be resolved by runtime access rules.
  */
  path: string;
  type?: 'image' | 'video';
  /**
   * Optional clockwise rotation in degrees (`0 | 90 | 180 | 270`).
   * Default: when omitted, runtime resolves orientation from media metadata.
   */
  rotate?: MediaRotation;
  /**
   * Optional display fit mode for video preview.
   * Default: `contain`.
   */
  objectFit?: MediaObjectFit;
  /**
   * Display duration in milliseconds.
   * Effective when preview `advance` is not `manual`.
   */
  durationMs?: number;
}

export type PreviewMediaAdvance = 'manual' | 'next' | 'loop';

export interface PreviewMediaSingleOptions extends PreviewMediaSource {
  /**
   * Auto behavior for the preview session.
   *
   * - `manual`: never auto-advance
   * - `next`: advance to the next item; if already on the last item, close the session
   * - `loop`: advance to the next item; if already on the last item, wrap to the first item
   *
   * Default: `manual`
   */
  advance?: PreviewMediaAdvance;
  /**
   * Optional cancellation signal for the preview request.
   *
   * Aborting rejects the returned promise with a cancellation error and requests the active
   * native preview session to close immediately.
   */
  signal?: AbortSignal;
  /**
   * Whether to show the top `current/total` indicator.
   *
   * Default: `true` when previewing multiple items, otherwise `false`.
   */
  showIndexIndicator?: boolean;
}

export interface PreviewMediaSequenceOptions {
  /**
   * Preview list. Supports images, videos, or a mixed queue.
   */
  sources: PreviewMediaSource[];
  /**
   * Initial item index in `sources`.
   * Must be an integer.
   * Out-of-range values are clamped by runtime.
   * Default: `0`.
   */
  startIndex?: number;
  /**
   * Auto behavior for the preview session.
   *
   * - `manual`: never auto-advance
   * - `next`: advance to the next item; if already on the last item, close the session
   * - `loop`: advance to the next item; if already on the last item, wrap to the first item
   *
   * Default: `manual`
   */
  advance?: PreviewMediaAdvance;
  /**
   * Optional cancellation signal for the preview request.
   *
   * Aborting rejects the returned promise with a cancellation error and requests the active
   * native preview session to close immediately.
   */
  signal?: AbortSignal;
  /**
   * Whether to show the top `current/total` indicator.
   *
   * Default: `true` when previewing multiple items, otherwise `false`.
   */
  showIndexIndicator?: boolean;
}

export type PreviewMediaOptions =
  | string
  | PreviewMediaSingleOptions
  | PreviewMediaSequenceOptions;

export type PreviewMediaCloseReason = 'manual' | 'completed' | 'interrupted' | 'error';

/**
 * The item the user is (or was) looking at, handed back as the caller
 * described it — `path` is returned verbatim, so it can be matched against
 * the caller's own data without re-indexing an array.
 */
export interface PreviewMediaShownSource {
  /** The path exactly as passed in the request. */
  path: string;
  /** Resolved media kind (after extension inference when `type` was omitted). */
  type: 'image' | 'video';
}

/** One change-stream event / the `current` snapshot. */
export interface PreviewMediaChange {
  index: number;
  source: PreviewMediaShownSource;
}

export interface PreviewMediaResult {
  /**
   * Why the preview session finished.
   */
  reason: PreviewMediaCloseReason;
  /**
   * Index of the item on screen when the session closed.
   */
  index: number;
  /**
   * The item on screen when the session closed — "what the user just
   * viewed/played", without mapping `index` back yourself.
   */
  source: PreviewMediaShownSource;
}

/**
 * Handle returned synchronously from `lx.previewMedia(...)` — synchronous so
 * listeners can be attached before the first event fires:
 *
 * - `presented` resolves once the first pixel of the underlying media has
 *   been composited to screen. Use this to time the hide of an overlay
 *   surface above the preview so the swap is seamless. Never rejects;
 *   resolves with no value when the first frame is up. Safe to ignore.
 * - `current` is a live `{ index, source }` snapshot of the item on screen,
 *   updated as the user swipes and as the session auto-advances.
 * - `onChange(listener)` fires for every item change. Returns an
 *   unsubscribe function.
 * - `completed` resolves `{ reason, index, source }` when the preview
 *   session ends (manual / auto / interrupted / error), or rejects on abort.
 *
 * If the call was aborted before any frame was presented, `presented` still
 * resolves (with no value) once the abort takes effect — it never rejects,
 * to keep fire-and-forget usage safe.
 *
 * @example
 * const preview = lx.previewMedia({ sources, startIndex: 2 });
 * preview.onChange(({ source }) => markAsViewed(source.path));
 * const { reason, source } = await preview.completed;
 */
export interface PreviewMediaHandle {
  readonly presented: Promise<void>;
  readonly current: PreviewMediaChange;
  onChange(listener: (change: PreviewMediaChange) => void): () => void;
  readonly completed: Promise<PreviewMediaResult>;
}

export interface SaveMediaOptions {
  filePath: string;
}

export interface ScanCodeOptions {
  onlyFromCamera?: boolean;
  scanType?: ('barCode' | 'qrCode' | 'datamatrix' | 'pdf417')[];
}

export interface ScanCodeResult {
  scanResult: string;
  scanType: string;
}

export interface StreamSourceOptions {
  provider: string;
  isLive: boolean;
  duration?: number;
  params?: Record<string, unknown>;
}

export interface VideoContext {
  play(): void;
  pause(): void;
  stop(): void;
  seek(position: number): void;
  requestFullScreen(): void;
  exitFullScreen(): void;
  setStreamSource(options: StreamSourceOptions): void;
}
