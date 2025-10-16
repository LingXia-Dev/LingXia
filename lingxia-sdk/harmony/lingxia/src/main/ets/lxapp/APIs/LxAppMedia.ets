import { hilog } from '@kit.PerformanceAnalysisKit';
import { MediaPreviewOverlayManager, MediaPreviewItem, MediaPreviewPageParams } from './media/MediaPreviewOverlay';
import { photoAccessHelper } from '@kit.MediaLibraryKit';
import { CameraCaptureOverlayManager, CameraCaptureOptions } from './media/CameraCaptureOverlay';
import { fileIo } from '@kit.CoreFileKit';
import { LxApp } from '../LxApp';
import common from '@ohos.app.ability.common';
import { onCallback } from 'liblingxia.so';

const DOMAIN = 0x0000;
const TAG = 'LingXia.Media';

interface PreviewMediaPayload {
  path: string;
  media_type: number;
  cover_path?: string;
}

interface SaveMediaPayload {
    path: string;
    mediaType: number;
}

interface PhotoAssetHandle {
  uri?: string;
  path?: string;
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

export class LxAppMedia {
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
  public static previewMedia(payload: string): boolean {
    try {
      const decoded = typeof payload === 'string' ? payload.replace(/%7C/g, '|') : '';
      const parsed: PreviewMediaPayload[] = JSON.parse(decoded || '[]');
      if (!Array.isArray(parsed) || parsed.length === 0) { return false; }
      const items = parsed
        .map((entry, index) => LxAppMedia.normalizeEntry(entry, index))
        .filter((it) => it !== null) as MediaPreviewItem[];
      if (items.length === 0) { return false; }
      return LxAppMedia.navigateToMediaPreview(items);
    } catch (_) {
      return false;
    }
  }

  public static chooseMedia(payload: string): boolean {
    try {
      const options: ChooseMediaOptions = JSON.parse(payload || '{}');
      if (!options || typeof options.callbackId !== 'string' || options.callbackId.length === 0) {
        hilog.error(DOMAIN, TAG, 'chooseMedia: invalid payload');
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
        CameraCaptureOverlayManager.show(cap);
        return true;
      }

      // Use official system photo picker to avoid restricted READ_IMAGEVIDEO access.
      const ctx = LxApp.context as common.UIAbilityContext | null;
      if (!ctx) {
        onCallback(normalized.callbackId, false, 'No context');
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
          const uris: string[] = Array.isArray(res && res.photoUris) ? res.photoUris : [];
          const isOriginalPick: boolean = (res && (res as photoAccessHelper.PhotoSelectResult).isOriginalPhoto === true);

          if (!uris || uris.length === 0) {
            onCallback(normalized.callbackId, true, '{"cancel":true}');
            return;
          }

          const results: ChooseMediaResult[] = [];
          for (let i = 0; i < Math.min(uris.length, count); i++) {
            const raw = String(uris[i] ?? '');
            const uri = LxAppMedia.normalizeUri(raw);
            if (!uri) { continue; }
            const fileType = LxAppMedia.detectMediaTypeFromUri(uri);
            // Enforce mode-specific filtering as safeguard
            if (normalized.mode === 'images' && fileType !== 'image') { continue; }
            if (normalized.mode === 'videos' && fileType !== 'video') { continue; }
            results.push({ uri, fileType, isOriginal: isOriginalPick });
          }
          if (results.length === 0) {
            onCallback(normalized.callbackId, true, '{"cancel":true}');
            return;
          }
          onCallback(normalized.callbackId, true, JSON.stringify(results));
        } catch (err) {
          const message = err instanceof Error ? err.message : `${err}`;
          hilog.error(DOMAIN, TAG, `chooseMedia (PhotoViewPicker) failed: ${message}`);
          onCallback(normalized.callbackId, false, message);
        }
      })();

      return true;
    } catch (err) {
      const message = err instanceof Error ? err.message : `${err}`;
      hilog.error(DOMAIN, TAG, `chooseMedia: failed to parse payload: ${message}`);
      return false;
    }
  }

  public static async saveMedia(path: string, mediaType: number): Promise<boolean> {
    if (!path || mediaType === undefined || mediaType === null || !LxApp.context) { return false; }

    // Deduplicate rapid duplicate requests for the same file/type (2s window)
    const now = Date.now();
    const mapType = (mediaType === 2) ? (photoAccessHelper.PhotoType.VIDEO as number) : (photoAccessHelper.PhotoType.IMAGE as number);
    const key = `${mapType}:${path}`;
    if (LxAppMedia.lastSaveKey === key && (now - LxAppMedia.lastSaveAt) < 2000) { return true; }
    LxAppMedia.lastSaveKey = key;
    LxAppMedia.lastSaveAt = now;

    const helper = photoAccessHelper.getPhotoAccessHelper(LxApp.context as common.UIAbilityContext);
    const type = mapType;
    const ext = LxAppMedia.getCreateExtension(path, type);

    let uri: string | undefined;
    try {
      uri = LxAppMedia.resolveAssetUri(await helper.createAsset(type, ext));
      if (!uri) { hilog.error(DOMAIN, TAG, 'saveMedia: empty asset uri'); return false; }
      const src = await fileIo.open(path, fileIo.OpenMode.READ_ONLY);
      try {
        const dst = await fileIo.open(uri, fileIo.OpenMode.READ_WRITE | fileIo.OpenMode.CREATE);
        try {
          await fileIo.copyFile(src.fd, dst.fd);
          hilog.info(DOMAIN, TAG, `saveMedia: saved to ${uri}`);
          return true;
        } finally {
          try { await fileIo.close(dst.fd); } catch {}
        }
      } finally {
        try { await fileIo.close(src.fd); } catch {}
      }
    } catch (err) {
      hilog.error(DOMAIN, TAG, `saveMedia failed: ${err}`);
      return false;
    }
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

  private static normalizeEntry(entry: PreviewMediaPayload, index: number): MediaPreviewItem | null {
    if (!entry || typeof entry.path !== 'string' || entry.path.length === 0) {
      return null;
    }

    const rawPath = entry.path.trim();
    const uri = LxAppMedia.normalizeUri(rawPath);
    if (!uri) {
      return null;
    }

    // Support both images and videos (media_type: 0 = image, 1 = video)
    const mediaType = typeof entry.media_type === 'number' ? entry.media_type : Number(entry.media_type ?? 0);
    if (mediaType !== 0 && mediaType !== 1) { return null; }

    return {
      id: `${Date.now()}_${index}`,
      uri: uri,
      mediaType: mediaType
    };
  }

  private static navigateToMediaPreview(items: MediaPreviewItem[]): boolean {
    try {
      const params: MediaPreviewPageParams = {
        items: items,
        initialIndex: 0
      };

      MediaPreviewOverlayManager.show(params);
      return true;
    } catch (error) {
      hilog.error(DOMAIN, TAG, `Error showing media preview overlay: ${error}`);
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
