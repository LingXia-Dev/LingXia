import { hilog } from '@kit.PerformanceAnalysisKit';
import { LxLog } from '../../app/LxLog';
import { MediaPreviewOverlayManager, MediaPreviewItem, MediaPreviewPageParams } from './media/MediaPreviewOverlay';
import { photoAccessHelper } from '@kit.MediaLibraryKit';
import { CameraCaptureOverlayManager, CameraCaptureOptions } from './media/CameraCaptureOverlay';
import { media } from '@kit.MediaKit';
import { fileIo } from '@kit.CoreFileKit';
import { Lingxia } from '../../app/Lingxia';
import common from '@ohos.app.ability.common';
import { onCallback } from 'liblingxia.so';
import { PermissionManager } from '../../app/PermissionManager';

const DOMAIN = 0xD001100;
const TAG = 'LingXiaMedia';
const LOG_PREFIX = '[LingXia.Media]';
const TRANSCODER_AUDIO_BITRATE_DEFAULT = 48_000;
const TRANSCODER_AUDIO_BITRATE_MAX = 500_000;
const TRANSCODER_VIDEO_WIDTH_MIN = 240;
const TRANSCODER_VIDEO_WIDTH_MAX = 3840;
const TRANSCODER_VIDEO_HEIGHT_MIN = 240;
const TRANSCODER_VIDEO_HEIGHT_MAX = 2160;

interface PreviewMediaPayload {
  path: string;
  media_type: number;
  rotate?: number;
  object_fit?: string;
  durationMs?: number;
}

interface PreviewMediaRequestPayload {
  sources: PreviewMediaPayload[];
  startIndex: number;
  advance: string;
  showIndexIndicator: boolean;
  callbackId: string;
  // JS-side `presented` Promise callback id; fired once when the first
  // item becomes visually ready. May be "0" or empty for callers that
  // don't care; native side then no-ops.
  presentedCallbackId: string;
  // JS-side change-stream callback id; fired with {"index": N} whenever the
  // displayed item changes (including the initial item). "0"/empty disables.
  changeCallbackId: string;
}

interface SaveMediaPayload {
    path: string;
    mediaType: number;
}

interface PhotoAssetHandle {
  uri?: string;
  path?: string;
}

interface PickerUriHandle extends PhotoAssetHandle {
  fileUri?: string;
  photoUri?: string;
}

interface FileDescriptorHandle {
  fd: number;
}

interface PhotoAssetFdHandle extends FileDescriptorHandle {
  close?: () => Promise<void>;
}

interface ChooseMediaResult { uri: string; fileType: string; isOriginal: boolean }

interface ChooseMediaOptions {
  callbackId: string;
  maxCount: number;
  mode: string; // images/videos/mix
  allowAlbum: boolean;
  allowCamera: boolean;
  maxDurationSeconds?: number;
  // alias accepted from callers
  maxDuration?: number;
  duration?: number;
  cameraFacing?: 'front' | 'back';
}

interface CompressVideoPayload {
  callbackId: string;
  sourceUri: string;
  outputPath: string;
  quality?: 'low' | 'medium' | 'high';
  bitrateKbps?: number;
  fps?: number;
  resolutionRatio?: number;
}

interface CompressVideoResultPayload {
  success: boolean;
  path: string;
  width: number;
  height: number;
  durationMs: number;
  size: number;
  mimeType: string;
}

interface VideoMetadataSnapshot {
  width: number;
  height: number;
  durationMs: number;
  mimeType: string;
  hasAudio: boolean;
}

interface VideoScaleTarget {
  width: number;
  height: number;
}

export class LxAppMedia {
  private static logInfo(message: string): void {
    hilog.info(DOMAIN, TAG, `${LOG_PREFIX} ${message}`);
  }

  private static logError(message: string): void {
    LxLog.e(TAG, `${LOG_PREFIX} ${message}`);
  }

  private static lastSaveKey: string = '';
  private static lastSaveAt: number = 0;
  private static getCreateExtension(path: string, photoType: number): string {
    const lower = (path || '').toLowerCase();
    const idx = lower.lastIndexOf('.');
    let ext = '';
    if (idx !== -1) {
      ext = lower.substring(idx + 1); // without dot
    }
    if (photoType === (photoAccessHelper.PhotoType.VIDEO as number)) {
      const validVideo = ['mp4', 'mov', 'm4v', '3gp', '3gpp', 'avi', 'mkv', 'webm'];
      if (validVideo.indexOf(ext) !== -1) { return ext; }
      return 'mp4';
    } else {
      // Prefer original image ext; normalize jpg->jpeg; fallback jpeg
      const validImage = ['jpeg', 'jpg', 'png', 'gif', 'bmp', 'webp', 'heic', 'heif'];
      if (ext === 'jpg') { return 'jpeg'; }
      if (validImage.indexOf(ext) !== -1) { return ext; }
      return 'jpeg';
    }
  }
  //
  private static async requestCameraPermissions(mode?: string): Promise<string | null> {
    const normalizedMode = (mode || '').toLowerCase();
    const includeMicrophone =
      normalizedMode === 'videos' || normalizedMode === 'mix';
    try {
      const cameraGranted: boolean = await PermissionManager.ensureCameraAccess(false);
      if (!cameraGranted) {
        return '3001';
      }
      if (!includeMicrophone) {
        return null;
      }
      const micGranted: boolean = await PermissionManager.ensureMicrophoneAccess();
      return micGranted ? null : '3003';
    } catch (error) {
      LxLog.e(TAG,
        `requestCameraPermissions error: ${String(error)}`
      );
      return '1000';
    }
  }

  public static previewMedia(payload: string): boolean {
    try {
      const decoded = typeof payload === 'string' ? payload.replace(/%7C/g, '|') : '';
      const parsed: PreviewMediaRequestPayload = JSON.parse(decoded || '{}');
      if (!parsed || typeof parsed.callbackId !== 'string' || parsed.callbackId.trim().length === 0) {
        return false;
      }
      if (!Array.isArray(parsed.sources) || parsed.sources.length === 0) {
        return false;
      }
      if (!Number.isFinite(parsed.startIndex)) {
        return false;
      }
      if (Math.trunc(parsed.startIndex) !== parsed.startIndex) {
        return false;
      }
      if (typeof parsed.advance !== 'string' || !LxAppMedia.isValidAdvance(parsed.advance)) {
        return false;
      }
      if (typeof parsed.showIndexIndicator !== 'boolean') {
        return false;
      }

      const items: MediaPreviewItem[] = [];
      for (let index = 0; index < parsed.sources.length; index++) {
        const normalized = LxAppMedia.normalizeEntry(parsed.sources[index], index);
        if (!normalized) {
          return false;
        }
        items.push(normalized);
      }

      return LxAppMedia.navigateToMediaPreview(parsed, items);
    } catch (_) {
      return false;
    }
  }

  public static closePreview(callbackId: string): boolean {
    const normalized = String(callbackId ?? '').trim();
    if (!normalized) {
      return false;
    }
    return MediaPreviewOverlayManager.cancelSession(normalized);
  }

  public static chooseMedia(payload: string): boolean {
    try {
      const options: ChooseMediaOptions = JSON.parse(payload || '{}');
      if (!options || typeof options.callbackId !== 'string' || options.callbackId.length === 0) {
        LxLog.e(TAG, 'chooseMedia: invalid payload');
        return false;
      }

      const normalized: ChooseMediaOptions = {
        callbackId: options.callbackId,
        maxCount: Number.isFinite(options.maxCount) && options.maxCount > 0 ? Math.floor(options.maxCount) : 1,
        mode: options.mode ?? 'images',
        allowAlbum: options.allowAlbum !== false,
        allowCamera: !!options.allowCamera
      };
      const rawDuration = ((): number | undefined => {
        if (typeof options.maxDurationSeconds === 'number' && Number.isFinite(options.maxDurationSeconds)) {
          return options.maxDurationSeconds;
        }
        if (typeof options.maxDuration === 'number' && Number.isFinite(options.maxDuration)) {
          return options.maxDuration;
        }
        if (typeof options.duration === 'number' && Number.isFinite(options.duration)) {
          return options.duration;
        }
        return undefined;
      })();
      if (typeof rawDuration === 'number' && Number.isFinite(rawDuration) && rawDuration > 0) {
        normalized.maxDurationSeconds = Math.floor(rawDuration);
      }
      if (options.cameraFacing === 'front' || options.cameraFacing === 'back') {
        normalized.cameraFacing = options.cameraFacing;
      }

      // Route to camera capture when only camera is requested
      if (normalized.allowCamera && !normalized.allowAlbum) {
        const cap: CameraCaptureOptions = {
          callbackId: normalized.callbackId,
          mode: normalized.mode,
          maxDurationSeconds: normalized.maxDurationSeconds,
          cameraFacing: normalized.cameraFacing
        };
        LxAppMedia.requestCameraPermissions(normalized.mode).then((denyCode: string | null) => {
          if (denyCode) {
            hilog.warn(
              DOMAIN,
              TAG,
              `Camera permissions not granted; cancelling chooseMedia(camera), code=${denyCode}`
            );
            onCallback(normalized.callbackId, false, denyCode);
            return;
          }
          CameraCaptureOverlayManager.show(cap);
        });
        return true;
      }

      // Use official system photo picker to avoid restricted READ_IMAGEVIDEO access.
      const ctx = Lingxia.context as common.UIAbilityContext | null;
      if (!ctx) {
        onCallback(normalized.callbackId, false, "1000");
        return false;
      }

      (async () => {
        try {
          const count = Math.max(1, normalized.maxCount);
          // Use photoAccessHelper PhotoViewPicker with options
          const pv = new photoAccessHelper.PhotoViewPicker();
          // Select MIME types per requested mode
          let mime: photoAccessHelper.PhotoViewMIMETypes;
          const mode = (normalized.mode || 'images').toLowerCase();
          if (mode === 'videos') {
            mime = photoAccessHelper.PhotoViewMIMETypes.VIDEO_TYPE;
          } else if (mode === 'mix') {
            mime = photoAccessHelper.PhotoViewMIMETypes.IMAGE_VIDEO_TYPE;
          } else {
            mime = photoAccessHelper.PhotoViewMIMETypes.IMAGE_TYPE;
          }

          const opts: photoAccessHelper.PhotoSelectOptions = {
            MIMEType: mime,
            maxSelectNumber: count,
            isPhotoTakingSupported: false,
            isSearchSupported: false,
            isOriginalSupported: true // Always enable original option, let user choose
          };
          const res: photoAccessHelper.PhotoSelectResult = await pv.select(opts);
          const pickerItems: Array<string | PickerUriHandle> = Array.isArray(res && res.photoUris)
            ? (res.photoUris as Array<string | PickerUriHandle>)
            : [];
          const isOriginalPick: boolean = (res && (res as photoAccessHelper.PhotoSelectResult).isOriginalPhoto === true);

          if (!pickerItems || pickerItems.length === 0) {
            onCallback(normalized.callbackId, false, '2000');
            return;
          }

          const results: ChooseMediaResult[] = [];
          for (let i = 0; i < Math.min(pickerItems.length, count); i++) {
            const rawUri = LxAppMedia.extractPickerUri(pickerItems[i]);
            const uri = LxAppMedia.normalizeUri(rawUri);
            if (!uri) {
              LxAppMedia.logError(`chooseMedia: skipped invalid picker entry at index=${i}, sample=${LxAppMedia.describePickerEntry(pickerItems[i])}`);
              continue;
            }
            const fileType = LxAppMedia.detectMediaTypeFromUri(uri);
            // Enforce mode-specific filtering as safeguard
            if (normalized.mode === 'images' && fileType !== 'image') { continue; }
            if (normalized.mode === 'videos' && fileType !== 'video') { continue; }
            results.push({ uri, fileType, isOriginal: isOriginalPick });
          }
          if (results.length === 0) {
            onCallback(normalized.callbackId, false, '2000');
            return;
          }
          onCallback(normalized.callbackId, true, JSON.stringify(results));
        } catch (err) {
          const message = err instanceof Error ? err.message : `${err}`;
          LxLog.e(TAG, `chooseMedia (PhotoViewPicker) failed: ${message}`);
          onCallback(normalized.callbackId, false, '1000');
        }
      })();

      return true;
    } catch (err) {
      const message = err instanceof Error ? err.message : `${err}`;
      LxLog.e(TAG, `chooseMedia: failed to parse payload: ${message}`);
      return false;
    }
  }

  public static async saveMediaToAlbum(path: string, mediaType: number): Promise<boolean> {
    if (!path || mediaType === undefined || mediaType === null || !Lingxia.context) { return false; }

    // Deduplicate rapid duplicate requests for the same file/type (2s window)
    const now = Date.now();
    const mapType = (mediaType === 2) ? (photoAccessHelper.PhotoType.VIDEO as number) : (photoAccessHelper.PhotoType.IMAGE as number);
    const key = `${mapType}:${path}`;
    if (LxAppMedia.lastSaveKey === key && (now - LxAppMedia.lastSaveAt) < 2000) { return true; }
    LxAppMedia.lastSaveKey = key;
    LxAppMedia.lastSaveAt = now;

    const helper = photoAccessHelper.getPhotoAccessHelper(Lingxia.context as common.UIAbilityContext);
    const type = mapType;
    const ext = LxAppMedia.getCreateExtension(path, type);

    let uri: string | undefined;
    try {
      uri = LxAppMedia.resolveAssetUri(await helper.createAsset(type, ext));
      if (!uri) { LxLog.e(TAG, 'saveMediaToAlbum: empty asset uri'); return false; }
      const src = await fileIo.open(path, fileIo.OpenMode.READ_ONLY);
      try {
        const dst = await fileIo.open(uri, fileIo.OpenMode.READ_WRITE | fileIo.OpenMode.CREATE);
        try {
          await fileIo.copyFile(src.fd, dst.fd);
          hilog.info(DOMAIN, TAG, `saveMediaToAlbum: saved to ${uri}`);
          return true;
        } finally {
          try { await fileIo.close(dst.fd); } catch {}
        }
      } finally {
        try { await fileIo.close(src.fd); } catch {}
      }
    } catch (err) {
      LxLog.e(TAG, `saveMediaToAlbum failed: ${err}`);
      return false;
    }
  }

  public static saveMedia(path: string, mediaType: number, callbackId: string): boolean {
    const cb = String(callbackId ?? '').trim();
    if (!cb) { return false; }

    const normalizedPath = String(path ?? '').replace(/%7C/g, '|');
    if (!normalizedPath || !Number.isFinite(mediaType)) {
      onCallback(cb, false, '1000');
      return false;
    }

    (async () => {
      try {
        const granted = await PermissionManager.ensureMediaWriteAccess();
        if (!granted) {
          onCallback(cb, false, '3004');
          return;
        }
        const ok = await LxAppMedia.saveMediaToAlbum(normalizedPath, mediaType);
        onCallback(cb, ok, ok ? '{}' : '1001');
      } catch (error) {
        LxLog.e(TAG, `saveMedia error: ${String(error)}`);
        onCallback(cb, false, '1000');
      }
    })();

    return true;
  }

  public static compressVideo(payload: string): boolean {
    let options: CompressVideoPayload;
    try {
      options = JSON.parse(payload || '{}') as CompressVideoPayload;
    } catch {
      return false;
    }

    const callbackId = String(options?.callbackId ?? '').trim();
    if (!callbackId) {
      return false;
    }

    const sourcePath = LxAppMedia.toFsPath(String(options?.sourceUri ?? ''));
    const outputPath = LxAppMedia.toFsPath(String(options?.outputPath ?? ''));
    if (!sourcePath || !outputPath) {
      onCallback(callbackId, false, '1001');
      return true;
    }

    (async () => {
      let srcFile: fileIo.File | null = null;
      let dstFile: fileIo.File | null = null;
      try {
        const inputQuality = String(options.quality ?? '').toLowerCase();
        const inputBitrateKbps = Number(options.bitrateKbps ?? 0);
        const inputResolutionRatio = Number(options.resolutionRatio ?? 0);
        const inputFps = Number(options.fps ?? 0);
        LxAppMedia.logInfo(
          `compressVideo begin source=${sourcePath} output=${outputPath} quality=${inputQuality} bitrateKbps=${inputBitrateKbps} resolutionRatio=${inputResolutionRatio} fps=${inputFps}`,
        );

        const sourceMeta = await LxAppMedia.readVideoMetadataFromPath(sourcePath);
        srcFile = await fileIo.open(sourcePath, fileIo.OpenMode.READ_ONLY);
        dstFile = await fileIo.open(
          outputPath,
          fileIo.OpenMode.CREATE | fileIo.OpenMode.READ_WRITE | fileIo.OpenMode.TRUNC
        );

        const srcStat = await fileIo.stat(sourcePath);
        const fdSrc: media.AVFileDescriptor = {
          fd: srcFile.fd,
          offset: 0,
          length: Number(srcStat.size ?? 0),
        };
        LxAppMedia.logInfo(
          `compressVideo sourceMeta width=${sourceMeta.width} height=${sourceMeta.height} durationMs=${sourceMeta.durationMs} mime=${sourceMeta.mimeType} hasAudio=${sourceMeta.hasAudio} fileSize=${fdSrc.length}`,
        );

        const quality = inputQuality;
        let bitrateBps: number | undefined = undefined;
        if (quality) {
          if (quality === 'low') {
            bitrateBps = 800_000;
          } else if (quality === 'medium') {
            bitrateBps = 1_500_000;
          } else {
            bitrateBps = 3_000_000;
          }
        } else if (typeof options.bitrateKbps === 'number' && options.bitrateKbps > 0) {
          bitrateBps = Math.floor(options.bitrateKbps * 1000);
        }

        let scaleTarget: VideoScaleTarget | undefined = undefined;
        const ratio = Number(options.resolutionRatio ?? 0);
        if (Number.isFinite(ratio) && ratio > 0 && ratio < 1 && sourceMeta.width > 0 && sourceMeta.height > 0) {
          const scaledWidth = Math.floor(sourceMeta.width * ratio);
          const scaledHeight = Math.floor(sourceMeta.height * ratio);
          const boundedWidth = Math.min(sourceMeta.width, Math.max(1, scaledWidth));
          const boundedHeight = Math.min(sourceMeta.height, Math.max(1, scaledHeight));
          const target = LxAppMedia.buildScaleTargetForTranscoder(boundedWidth, boundedHeight, sourceMeta.width, sourceMeta.height);
          if (target) {
            scaleTarget = target;
            LxAppMedia.logInfo(
              `compressVideo: scaled resolution ${sourceMeta.width}x${sourceMeta.height} -> ${target.width}x${target.height} (ratio=${ratio})`,
            );
          } else {
            LxAppMedia.logInfo(
              `compressVideo: scaled resolution is below transcoder limits, keep default source resolution (scaled=${boundedWidth}x${boundedHeight}, source=${sourceMeta.width}x${sourceMeta.height})`,
            );
          }
        }

        if (typeof options.fps === 'number' && options.fps > 0) {
          LxAppMedia.logInfo(`compressVideo: fps=${options.fps} is not supported by AVTranscoderConfig, ignored`);
        }

        if (!scaleTarget && sourceMeta.width > 0 && sourceMeta.height > 0) {
          const target = LxAppMedia.buildScaleTargetForTranscoder(
            sourceMeta.width,
            sourceMeta.height,
            sourceMeta.width,
            sourceMeta.height,
          );
          if (target) {
            scaleTarget = target;
            LxAppMedia.logInfo(
              `compressVideo: use source resolution ${sourceMeta.width}x${sourceMeta.height} -> ${target.width}x${target.height}`,
            );
          } else {
            LxAppMedia.logInfo(
              `compressVideo: source resolution ${sourceMeta.width}x${sourceMeta.height} is outside explicit config range, keep transcoder default`,
            );
          }
        }

        await LxAppMedia.runTranscodeWithFallbacks(fdSrc, dstFile.fd, sourceMeta.hasAudio, bitrateBps, scaleTarget);

        const outStat = await fileIo.stat(outputPath);
        const outputMeta = await LxAppMedia.readVideoMetadataFromPath(outputPath);
        LxAppMedia.logInfo(
          `compressVideo success output=${outputPath} size=${Number(outStat.size ?? 0)} width=${outputMeta.width} height=${outputMeta.height} durationMs=${outputMeta.durationMs} mime=${outputMeta.mimeType}`,
        );
        const result: CompressVideoResultPayload = {
          success: true,
          path: outputPath,
          width: outputMeta.width > 0 ? outputMeta.width : sourceMeta.width,
          height: outputMeta.height > 0 ? outputMeta.height : sourceMeta.height,
          durationMs: outputMeta.durationMs > 0 ? outputMeta.durationMs : sourceMeta.durationMs,
          size: Number(outStat.size ?? 0),
          mimeType: outputMeta.mimeType || sourceMeta.mimeType || 'video/mp4',
        };
        onCallback(callbackId, true, JSON.stringify(result));
      } catch (error) {
        const message = LxAppMedia.errorToMessage(error);
        LxAppMedia.logError(`compressVideo failed: ${message}`);
        onCallback(callbackId, false, '1000');
      } finally {
        if (srcFile !== null) {
          try { await fileIo.close(srcFile.fd); } catch {}
        }
        if (dstFile !== null) {
          try { await fileIo.close(dstFile.fd); } catch {}
        }
      }
    })();

    return true;
  }

  private static async waitForTranscodeComplete(transcoder: media.AVTranscoder, timeoutMs: number): Promise<void> {
    await new Promise<void>((resolve, reject) => {
      let finished = false;
      let timer: number | undefined;

      const cleanup = (): void => {
        try { transcoder.off('complete', onComplete); } catch {}
        try { transcoder.off('error', onError); } catch {}
        if (typeof timer === 'number') {
          clearTimeout(timer);
        }
      };

      const done = (error?: Error): void => {
        if (finished) {
          return;
        }
        finished = true;
        cleanup();
        if (error) {
          reject(error);
        } else {
          resolve();
        }
      };

      const onComplete = (): void => {
        done();
      };

      const onError = (error: Error): void => {
        const message = LxAppMedia.errorToMessage(error);
        LxAppMedia.logInfo(`compressVideo event error: ${message}`);
        done(new Error(`AVTranscoder error: ${message}`));
      };

      transcoder.on('complete', onComplete);
      transcoder.on('error', onError);

      timer = setTimeout(() => {
        done(new Error('AVTranscoder timeout'));
      }, timeoutMs);
    });
  }

  private static buildTranscoderConfig(
    includeAudio: boolean,
    bitrateBps?: number,
    scaleTarget?: VideoScaleTarget,
  ): media.AVTranscoderConfig {
    const config: media.AVTranscoderConfig = {
      fileFormat: media.ContainerFormatType.CFT_MPEG_4,
      videoCodec: media.CodecMimeType.VIDEO_AVC,
    };
    if (includeAudio) {
      config.audioCodec = media.CodecMimeType.AUDIO_AAC;
      config.audioBitrate = LxAppMedia.clampAudioBitrate(TRANSCODER_AUDIO_BITRATE_DEFAULT);
    }
    if (typeof bitrateBps === 'number' && bitrateBps > 0) {
      config.videoBitrate = bitrateBps;
    }
    if (scaleTarget && scaleTarget.width > 0 && scaleTarget.height > 0) {
      config.videoFrameWidth = scaleTarget.width;
      config.videoFrameHeight = scaleTarget.height;
    }
    return config;
  }

  private static configSignature(config: media.AVTranscoderConfig): string {
    return [
      `a:${String(config.audioCodec ?? '')}`,
      `ab:${String(config.audioBitrate ?? '')}`,
      `b:${String(config.videoBitrate ?? '')}`,
      `w:${String(config.videoFrameWidth ?? '')}`,
      `h:${String(config.videoFrameHeight ?? '')}`,
    ].join('|');
  }

  private static configToLog(config: media.AVTranscoderConfig): string {
    return [
      `audio=${String(config.audioCodec ?? '')}`,
      `audioBitrate=${String(config.audioBitrate ?? '')}`,
      `bitrate=${String(config.videoBitrate ?? '')}`,
      `width=${String(config.videoFrameWidth ?? '')}`,
      `height=${String(config.videoFrameHeight ?? '')}`,
    ].join(', ');
  }

  private static buildTranscodeAttempts(
    hasAudio: boolean,
    bitrateBps?: number,
    scaleTarget?: VideoScaleTarget,
  ): media.AVTranscoderConfig[] {
    const attempts: media.AVTranscoderConfig[] = [];
    const seen = new Set<string>();

    const pushAttempt = (config: media.AVTranscoderConfig): void => {
      const key = LxAppMedia.configSignature(config);
      if (seen.has(key)) {
        return;
      }
      seen.add(key);
      attempts.push(config);
    };

    const includeAudioFirst = hasAudio;

    // First round: use metadata-derived audio decision.
    pushAttempt(LxAppMedia.buildTranscoderConfig(includeAudioFirst, bitrateBps, scaleTarget));
    pushAttempt(LxAppMedia.buildTranscoderConfig(includeAudioFirst, bitrateBps, undefined));
    pushAttempt(LxAppMedia.buildTranscoderConfig(includeAudioFirst, undefined, scaleTarget));
    pushAttempt(LxAppMedia.buildTranscoderConfig(includeAudioFirst, undefined, undefined));

    // Second round: invert audio decision to handle metadata mis-detection
    // and avoid invalid internal defaults on some devices.
    pushAttempt(LxAppMedia.buildTranscoderConfig(!includeAudioFirst, bitrateBps, scaleTarget));
    pushAttempt(LxAppMedia.buildTranscoderConfig(!includeAudioFirst, bitrateBps, undefined));
    pushAttempt(LxAppMedia.buildTranscoderConfig(!includeAudioFirst, undefined, scaleTarget));
    pushAttempt(LxAppMedia.buildTranscoderConfig(!includeAudioFirst, undefined, undefined));

    return attempts;
  }

  private static async runTranscodeWithFallbacks(
    fdSrc: media.AVFileDescriptor,
    dstFd: number,
    hasAudio: boolean,
    bitrateBps?: number,
    scaleTarget?: VideoScaleTarget,
  ): Promise<void> {
    const attempts = LxAppMedia.buildTranscodeAttempts(hasAudio, bitrateBps, scaleTarget);
    LxAppMedia.logInfo(
      `compressVideo attempts=${attempts.length} srcLength=${fdSrc.length} hasAudio=${hasAudio} requestedBitrate=${String(bitrateBps ?? '')} requestedScale=${scaleTarget ? `${scaleTarget.width}x${scaleTarget.height}` : ''}`,
    );
    let lastError: Error = new Error('compressVideo failed');

    for (let i = 0; i < attempts.length; i++) {
      const config = attempts[i];
      let transcoder: media.AVTranscoder | null = null;
      let phase = 'create';
      const attemptStartedAt = Date.now();
      try {
        phase = 'create';
        transcoder = await media.createAVTranscoder();
        phase = 'setFd';
        transcoder.fdSrc = fdSrc;
        transcoder.fdDst = dstFd;

        LxAppMedia.logInfo(
          `compressVideo: attempt=${i + 1}/${attempts.length} config(${LxAppMedia.configToLog(config)})`,
        );

        phase = 'prepare';
        await transcoder.prepare(config);
        phase = 'start';
        const waitDone = LxAppMedia.waitForTranscodeComplete(transcoder, 180_000);
        await transcoder.start();
        phase = 'wait';
        await waitDone;
        phase = 'release';
        await transcoder.release();
        const costMs = Date.now() - attemptStartedAt;
        LxAppMedia.logInfo(
          `compressVideo: attempt=${i + 1} success costMs=${costMs}`,
        );
        return;
      } catch (error) {
        const message = LxAppMedia.errorToMessage(error);
        const costMs = Date.now() - attemptStartedAt;
        lastError = new Error(message);
        LxAppMedia.logInfo(
          `compressVideo: attempt=${i + 1} failed phase=${phase} costMs=${costMs} config(${LxAppMedia.configToLog(config)}) error=${message}`,
        );
        try { await transcoder?.cancel(); } catch {}
        try { await transcoder?.release(); } catch {}
      }
    }

    throw lastError;
  }

  private static async readVideoMetadataFromPath(path: string): Promise<VideoMetadataSnapshot> {
    const fallback: VideoMetadataSnapshot = {
      width: 0,
      height: 0,
      durationMs: 0,
      mimeType: 'video/mp4',
      hasAudio: false,
    };
    let srcFile: fileIo.File | null = null;
    let extractor: media.AVMetadataExtractor | null = null;
    try {
      srcFile = await fileIo.open(path, fileIo.OpenMode.READ_ONLY);
      const stat = await fileIo.stat(path);
      extractor = await media.createAVMetadataExtractor();
      const fdSrc: media.AVFileDescriptor = {
        fd: srcFile.fd,
        offset: 0,
        length: Number(stat.size ?? 0),
      };
      extractor.fdSrc = fdSrc;
      const metadata = await extractor.fetchMetadata();
      const snapshot: VideoMetadataSnapshot = {
        width: LxAppMedia.toNonNegativeInt(metadata?.videoWidth),
        height: LxAppMedia.toNonNegativeInt(metadata?.videoHeight),
        durationMs: LxAppMedia.toNonNegativeInt(metadata?.duration),
        mimeType: String(metadata?.mimeType ?? '').trim() || 'video/mp4',
        hasAudio: LxAppMedia.detectHasAudioTrack(metadata),
      };
      return snapshot;
    } catch (error) {
      LxAppMedia.logInfo(`readVideoMetadataFromPath failed: ${String(error)}`);
      return fallback;
    } finally {
      if (extractor !== null) {
        try { await extractor.release(); } catch {}
      }
      if (srcFile !== null) {
        try { await fileIo.close(srcFile.fd); } catch {}
      }
    }
  }

  private static toNonNegativeInt(value: string | number | undefined): number {
    const raw = typeof value === 'number' ? value : Number(String(value ?? '').trim());
    if (!Number.isFinite(raw) || raw <= 0) {
      return 0;
    }
    return Math.floor(raw);
  }

  private static toEvenInt(value: number): number {
    if (!Number.isFinite(value) || value <= 0) {
      return 0;
    }
    const rounded = Math.floor(value);
    return rounded % 2 === 0 ? rounded : rounded - 1;
  }

  private static clampAudioBitrate(value: number): number {
    if (!Number.isFinite(value) || value <= 0) {
      return TRANSCODER_AUDIO_BITRATE_DEFAULT;
    }
    const rounded = Math.floor(value);
    if (rounded < 1) {
      return 1;
    }
    return Math.min(TRANSCODER_AUDIO_BITRATE_MAX, rounded);
  }

  private static buildScaleTargetForTranscoder(
    rawWidth: number,
    rawHeight: number,
    sourceWidth: number,
    sourceHeight: number,
  ): VideoScaleTarget | undefined {
    if (rawWidth <= 0 || rawHeight <= 0 || sourceWidth <= 0 || sourceHeight <= 0) {
      return undefined;
    }

    if (sourceWidth < TRANSCODER_VIDEO_WIDTH_MIN || sourceHeight < TRANSCODER_VIDEO_HEIGHT_MIN) {
      return undefined;
    }

    const safeSourceWidth = Math.min(sourceWidth, TRANSCODER_VIDEO_WIDTH_MAX);
    const safeSourceHeight = Math.min(sourceHeight, TRANSCODER_VIDEO_HEIGHT_MAX);
    const boundedWidth = Math.max(
      TRANSCODER_VIDEO_WIDTH_MIN,
      Math.min(safeSourceWidth, Math.floor(rawWidth)),
    );
    const boundedHeight = Math.max(
      TRANSCODER_VIDEO_HEIGHT_MIN,
      Math.min(safeSourceHeight, Math.floor(rawHeight)),
    );
    const evenWidth = LxAppMedia.toEvenInt(boundedWidth);
    const evenHeight = LxAppMedia.toEvenInt(boundedHeight);
    if (
      evenWidth < TRANSCODER_VIDEO_WIDTH_MIN ||
      evenHeight < TRANSCODER_VIDEO_HEIGHT_MIN ||
      evenWidth > safeSourceWidth ||
      evenHeight > safeSourceHeight
    ) {
      return undefined;
    }

    return { width: evenWidth, height: evenHeight };
  }

  private static detectHasAudioTrack(metadata: media.AVMetadata | undefined): boolean {
    if (!metadata) {
      return false;
    }
    const raw = String(metadata.hasAudio ?? '').trim().toLowerCase();
    return raw === 'true' || raw === '1' || raw === 'yes';
  }

  private static errorToMessage(error: Object | string | Error | undefined): string {
    if (typeof error === 'string') {
      return error;
    }
    if (!error) {
      return 'unknown';
    }
    if (error instanceof Error) {
      const name = String(error.name ?? 'Error');
      const message = String(error.message ?? '');
      const stack = String(error.stack ?? '').trim();
      if (stack.length > 0) {
        return `${name}: ${message}; stack=${stack}`;
      }
      return `${name}: ${message}`;
    }
    const text = JSON.stringify(error);
    if (text && text !== '{}') {
      return text;
    }
    return String(error);
  }

  private static resolveAssetUri(assetResult: PhotoAssetHandle | string | null | undefined): string | undefined {
    if (!assetResult) {
      return undefined;
    }
    if (typeof assetResult === 'string') {
      return assetResult;
    }
    if (typeof assetResult.uri === 'string') {
      return assetResult.uri;
    }
    if (typeof assetResult.path === 'string') {
      return assetResult.path;
    }
    return undefined;
  }

  private static normalizePickerString(value: string | undefined | null): string {
    if (typeof value !== 'string') {
      return '';
    }
    const trimmed = value.trim();
    if (!trimmed) {
      return '';
    }
    if (trimmed.toLowerCase() === '[object object]') {
      return '';
    }
    return trimmed;
  }

  private static extractPickerUri(entry: string | PickerUriHandle | undefined | null): string {
    if (typeof entry === 'string') {
      return LxAppMedia.normalizePickerString(entry);
    }
    if (!entry || typeof entry !== 'object') {
      return '';
    }
    const candidate = entry as PickerUriHandle;
    const picked = [
      LxAppMedia.normalizePickerString(candidate.uri),
      LxAppMedia.normalizePickerString(candidate.path),
      LxAppMedia.normalizePickerString(candidate.fileUri),
      LxAppMedia.normalizePickerString(candidate.photoUri)
    ].find((value: string) => value.length > 0);
    if (picked) {
      return picked;
    }
    return LxAppMedia.normalizePickerString(`${entry}`);
  }

  private static describePickerEntry(entry: string | PickerUriHandle | undefined | null): string {
    if (typeof entry === 'string') {
      return entry;
    }
    try {
      return JSON.stringify(entry) ?? String(entry);
    } catch (_error) {
      return String(entry);
    }
  }

  private static normalizeEntry(entry: PreviewMediaPayload, index: number): MediaPreviewItem | null {
    if (!entry || typeof entry.path !== 'string') {
      LxAppMedia.logError(`previewMedia: invalid entry at index=${index}, pathType=${typeof entry?.path}, sample=${LxAppMedia.describePickerEntry(entry)}`);
      return null;
    }
    if (entry.path.length === 0) {
      LxAppMedia.logError(`previewMedia: empty path at index=${index}`);
      return null;
    }

    // Support both images and videos (media_type: 0 = image, 1 = video)
    const mediaType = typeof entry.media_type === 'number' ? entry.media_type : Number(entry.media_type ?? 0);
    if (mediaType !== 0 && mediaType !== 1) { return null; }

    const rawPath = entry.path.trim();
    if (rawPath.includes('[object Object]')) {
      LxAppMedia.logError(`previewMedia: invalid marshaled path at index=${index}, path=${rawPath}`);
      return null;
    }
    const uri = LxAppMedia.normalizeUri(rawPath);
    if (!uri) {
      LxAppMedia.logError(`previewMedia: failed to normalize path at index=${index}, path=${rawPath}`);
      return null;
    }

    return {
      id: `${Date.now()}_${index}`,
      uri: uri,
      mediaType: mediaType,
      rotate: LxAppMedia.normalizeRotation(entry.rotate),
      objectFit: LxAppMedia.normalizeObjectFit(entry.object_fit),
      durationMs: LxAppMedia.normalizePositiveMs(entry.durationMs)
    };
  }

  private static isValidAdvance(raw: string): boolean {
    const value = raw.trim().toLowerCase();
    return value === 'manual' || value === 'next' || value === 'loop';
  }

  private static normalizeRotation(raw: number | undefined): number | undefined {
    if (typeof raw !== 'number' || !Number.isFinite(raw)) {
      return undefined;
    }
    const value = Math.trunc(raw);
    if (value === 0 || value === 90 || value === 180 || value === 270) {
      return value;
    }
    return undefined;
  }

  private static normalizeObjectFit(raw: string | undefined): string | undefined {
    if (typeof raw !== 'string') {
      return undefined;
    }
    const fit = raw.trim().toLowerCase();
    if (fit === 'cover' || fit === 'contain' || fit === 'fill' || fit === 'fit') {
      return fit;
    }
    return undefined;
  }

  private static normalizeAdvance(raw: string | undefined): 'manual' | 'next' | 'loop' {
    if (typeof raw !== 'string') {
      return 'manual';
    }
    const value = raw.trim().toLowerCase();
    if (value === 'next' || value === 'loop') {
      return value;
    }
    return 'manual';
  }

  private static normalizePositiveMs(raw: number | undefined): number | undefined {
    if (typeof raw !== 'number' || !Number.isFinite(raw) || raw <= 0) {
      return undefined;
    }
    return Math.max(1, Math.trunc(raw));
  }

  private static navigateToMediaPreview(
    request: PreviewMediaRequestPayload,
    items: MediaPreviewItem[]
  ): boolean {
    try {
      const params: MediaPreviewPageParams = {
        items: items,
        initialIndex: Math.trunc(request.startIndex),
        callbackId: request.callbackId,
        presentedCallbackId: request.presentedCallbackId,
        changeCallbackId: request.changeCallbackId,
        advance: LxAppMedia.normalizeAdvance(request.advance),
        showIndexIndicator: request.showIndexIndicator
      };

      MediaPreviewOverlayManager.show(params);
      return true;
    } catch (error) {
      LxLog.e(TAG, `Error showing media preview overlay: ${error}`);
      return false;
    }
  }

  private static normalizeUri(path: string): string {
    if (!path) {
      return '';
    }
    if (path.startsWith('file://') || path.startsWith('content://') || path.startsWith('data:') || path.startsWith('http://') || path.startsWith('https://')) {
      return path;
    }
    return `file://${path}`;
  }

  private static toFsPath(value: string): string {
    const normalized = String(value ?? '').replace(/%7C/g, '|').trim();
    if (!normalized) {
      return '';
    }
    if (normalized.startsWith('file://')) {
      return normalized.substring('file://'.length);
    }
    return normalized;
  }

  private static detectMediaTypeFromUri(uri: string): 'image' | 'video' {
    const lower = (uri || '').toLowerCase();
    const q = lower.split('?')[0];
    const dot = q.lastIndexOf('.');
    const ext = dot !== -1 ? q.substring(dot + 1) : '';
    const videoExts = ['mp4', 'mov', 'm4v', '3gp', '3gpp', 'avi', 'mkv', 'webm'];
    if (videoExts.indexOf(ext) !== -1) {
      return 'video';
    }
    return 'image';
  }
}
