import { hilog } from '@kit.PerformanceAnalysisKit';
import common from '@ohos.app.ability.common';
import fileUri from '@ohos.file.fileuri';
import { fileIo } from '@kit.CoreFileKit';
import { uniformTypeDescriptor } from '@kit.ArkData';
import { BusinessError } from '@kit.BasicServicesKit';
import { image } from '@kit.ImageKit';
import { systemShare } from '@kit.ShareKit';
import util from '@ohos.util';
import { onCallback } from 'liblingxia.so';
import { LxApp } from '../../app/LxApp';

const DOMAIN = 0x0000;
const TAG = 'LingXia.Share';

// Some recipients render the share as an inline preview card only when
// SharedData carries thumbnail bytes; otherwise they fall back to a
// "tap to view" attachment UI. Capped at 256px / ~JPEG 80 keeps it under a
// few KB, which is what the IPC envelope can comfortably carry.
const THUMBNAIL_MAX_EDGE = 256;
const THUMBNAIL_JPEG_QUALITY = 80;
const IMAGE_EXTENSIONS: readonly string[] = ['jpg', 'jpeg', 'png', 'webp', 'heic', 'heif', 'gif', 'bmp'];

interface ShareDataOptions {
  utd: string;
  uri?: string;
  title?: string;
  description?: string;
  content?: string;
  thumbnail?: Uint8Array;
}

export class LxAppShare {
  public static share(
    title: string,
    text: string,
    url: string,
    filesJson: string,
    callbackId: string
  ): boolean {
    const context = LxApp.context;
    const id = String(callbackId ?? '').trim();
    if (!context || !id) {
      if (id) {
        onCallback(id, false, '1000');
      }
      return false;
    }

    let files: string[];
    try {
      files = parseFiles(filesJson);
    } catch (error) {
      hilog.warn(DOMAIN, TAG, `parseFiles failed: ${error}`);
      onCallback(id, false, '1002');
      return false;
    }
    const normalizedTitle = String(title ?? '').trim();
    const normalizedText = String(text ?? '').trim();
    const normalizedUrl = String(url ?? '').trim();
    const content = [normalizedText, normalizedUrl]
      .filter((value: string) => value.length > 0)
      .join('\n');
    if (normalizedTitle.length === 0 && content.length === 0 && files.length === 0) {
      onCallback(id, false, '1002');
      return false;
    }
    if (files.length > 1) {
      hilog.warn(DOMAIN, TAG, 'share: HarmonyOS file sharing currently supports one file');
      onCallback(id, false, '1002');
      return false;
    }

    shareWithSystem(context, normalizedTitle, normalizedText, content, normalizedUrl, files, id)
      .catch((error: Error) => {
        hilog.error(DOMAIN, TAG, `share dispatch failed: ${error?.message ?? error}`);
        onCallback(id, false, '1000');
      });
    return true;
  }
}

function parseFiles(filesJson: string): string[] {
  const value = String(filesJson ?? '').trim();
  if (value.length === 0) {
    return [];
  }
  const parsed = JSON.parse(value) as Array<string>;
  if (!Array.isArray(parsed)) {
    return [];
  }
  return (parsed ?? [])
    .map((path: string): string => String(path ?? '').trim())
    .filter((path: string) => path.length > 0);
}

async function shareWithSystem(
  context: common.UIAbilityContext,
  title: string,
  text: string,
  content: string,
  url: string,
  files: string[],
  callbackId: string
): Promise<void> {
  const shareData = await buildSharedData(context, title, text, content, url, files);
  const controller = new systemShare.ShareController(shareData);
  controller.on('dismiss', () => {
    hilog.info(DOMAIN, TAG, 'Share panel dismissed');
  });
  try {
    await controller.show(context, {
      previewMode: files.length > 0 ? systemShare.SharePreviewMode.DETAIL : systemShare.SharePreviewMode.DEFAULT,
      selectionMode: systemShare.SelectionMode.SINGLE
    });
    onCallback(callbackId, true, '{}');
  } catch (rawError) {
    const error = rawError as BusinessError;
    hilog.error(
      DOMAIN,
      TAG,
      `ShareController.show failed: code=${error?.code ?? '?'} msg=${error?.message ?? rawError}`
    );
    onCallback(callbackId, false, '1000');
  }
}

async function buildSharedData(
  context: common.UIAbilityContext,
  title: string,
  text: string,
  content: string,
  url: string,
  files: string[]
): Promise<systemShare.SharedData> {
  if (files.length > 0) {
    const source = files[0];
    const localPath = await materializeIntoSandbox(context, source);
    const uri = resolveUri(localPath);
    const options: ShareDataOptions = {
      utd: resolveUtd(source),
      uri,
      title: title.length > 0 ? title : displayName(source),
      description: content.length > 0 ? content : displayName(source)
    };
    if (content.length > 0) {
      options.content = content;
    }
    if (isImagePath(source)) {
      const thumbnail = await loadThumbnail(uri);
      if (thumbnail) {
        options.thumbnail = thumbnail;
      }
    }
    return new systemShare.SharedData(options);
  }

  if (url.length > 0) {
    return new systemShare.SharedData({
      utd: uniformTypeDescriptor.UniformDataType.HYPERLINK,
      title: title.length > 0 ? title : undefined,
      description: text.length > 0 ? text : url,
      content: url
    } as ShareDataOptions);
  }

  return new systemShare.SharedData({
    utd: uniformTypeDescriptor.UniformDataType.PLAIN_TEXT,
    title: title.length > 0 ? title : undefined,
    description: content,
    content
  } as ShareDataOptions);
}

// Copy a foreign URI's contents into the app's cache dir and return the
// sandbox-local path. Plain filesystem paths (no scheme) are returned
// unchanged - they're assumed to be in the sandbox already. If the copy
// fails, returns the source so the caller can still attempt the share.
async function materializeIntoSandbox(
  context: common.UIAbilityContext,
  source: string
): Promise<string> {
  if (!source.startsWith('file://') &&
      !source.startsWith('datashare://') &&
      !source.startsWith('content://')) {
    return source;
  }

  const baseName = displayName(source) || 'shared';
  const subdir = `${context.cacheDir}/lx_share`;
  try {
    if (!fileIo.accessSync(subdir)) {
      fileIo.mkdirSync(subdir, true);
    }
  } catch (error) {
    hilog.warn(DOMAIN, TAG, `materializeIntoSandbox: mkdir failed: ${error}`);
  }
  const id = util.generateRandomUUID(true);
  const destPath = `${subdir}/${id}-${baseName}`;

  let src: fileIo.File | undefined;
  let dst: fileIo.File | undefined;
  try {
    src = await fileIo.open(source, fileIo.OpenMode.READ_ONLY);
    dst = await fileIo.open(
      destPath,
      fileIo.OpenMode.READ_WRITE | fileIo.OpenMode.CREATE | fileIo.OpenMode.TRUNC
    );
    await fileIo.copyFile(src.fd, dst.fd);
    return destPath;
  } catch (error) {
    const err = error as BusinessError;
    hilog.warn(
      DOMAIN,
      TAG,
      `materializeIntoSandbox failed for ${source}: code=${err?.code ?? '?'} msg=${err?.message ?? error}`
    );
    return source;
  } finally {
    try { if (dst) await fileIo.close(dst.fd); } catch (_) { /* ignore */ }
    try { if (src) await fileIo.close(src.fd); } catch (_) { /* ignore */ }
  }
}

function resolveUri(path: string): string {
  if (path.startsWith('file://') || path.startsWith('datashare://') || path.startsWith('content://')) {
    return path;
  }
  return fileUri.getUriFromPath(path);
}

function resolveUtd(path: string): string {
  // `getUniformDataTypeByFilenameExtension(ext, fallback)` only consults
  // the fallback when the extension is unknown to the system. We pick
  // `OBJECT` (a generic file-ish type) over `COMPOSITE_OBJECT` because
  // some receivers refuse to accept a `COMPOSITE_OBJECT` payload - they
  // treat it as a structured-data wrapper and reject when there's just a
  // uri inside.
  const dotIndex = path.lastIndexOf('.');
  const extension = dotIndex >= 0 ? path.substring(dotIndex) : '';
  if (extension.length > 0) {
    return uniformTypeDescriptor.getUniformDataTypeByFilenameExtension(
      extension,
      uniformTypeDescriptor.UniformDataType.OBJECT
    );
  }
  return uniformTypeDescriptor.UniformDataType.OBJECT;
}

function isImagePath(path: string): boolean {
  const dotIndex = path.lastIndexOf('.');
  if (dotIndex < 0) {
    return false;
  }
  const ext = path.substring(dotIndex + 1).toLowerCase();
  return IMAGE_EXTENSIONS.includes(ext);
}

// Decode an image at `uri`, downscale to ~THUMBNAIL_MAX_EDGE on the longest
// edge, re-encode to JPEG, return raw bytes. Returns undefined on any error
// - the caller treats that as "share without thumbnail" rather than failing
// the whole share.
async function loadThumbnail(uri: string): Promise<Uint8Array | undefined> {
  let source: image.ImageSource | undefined;
  let pixelMap: image.PixelMap | undefined;
  let packer: image.ImagePacker | undefined;
  try {
    source = image.createImageSource(uri);
    if (!source) {
      hilog.warn(DOMAIN, TAG, `loadThumbnail: createImageSource returned null for ${uri}`);
      return undefined;
    }
    const info = await source.getImageInfo();
    const width = info.size.width;
    const height = info.size.height;
    if (width <= 0 || height <= 0) {
      hilog.warn(DOMAIN, TAG, `loadThumbnail: invalid image size ${width}x${height}`);
      return undefined;
    }
    const longest = Math.max(width, height);
    const scale = longest > THUMBNAIL_MAX_EDGE ? THUMBNAIL_MAX_EDGE / longest : 1;
    const targetWidth = Math.max(1, Math.round(width * scale));
    const targetHeight = Math.max(1, Math.round(height * scale));
    pixelMap = await source.createPixelMap({
      desiredSize: { width: targetWidth, height: targetHeight }
    });
    packer = image.createImagePacker();
    const buffer: ArrayBuffer = await packer.packing(pixelMap, {
      format: 'image/jpeg',
      quality: THUMBNAIL_JPEG_QUALITY
    });
    return new Uint8Array(buffer);
  } catch (error) {
    const err = error as BusinessError;
    hilog.warn(
      DOMAIN,
      TAG,
      `loadThumbnail failed: code=${err?.code ?? '?'} msg=${err?.message ?? error}`
    );
    return undefined;
  } finally {
    try { await packer?.release(); } catch (_) { /* ignore */ }
    try { await pixelMap?.release(); } catch (_) { /* ignore */ }
    try { await source?.release(); } catch (_) { /* ignore */ }
  }
}

function displayName(path: string): string {
  let normalized = path;
  try {
    normalized = decodeURIComponent(path);
  } catch (_error) {
    normalized = path;
  }
  const segments = normalized.split('/');
  return segments.length > 0 ? segments[segments.length - 1] : normalized;
}
