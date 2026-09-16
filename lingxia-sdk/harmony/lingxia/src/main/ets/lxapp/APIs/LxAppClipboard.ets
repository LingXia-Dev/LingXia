import { hilog } from '@kit.PerformanceAnalysisKit';
import { BusinessError, pasteboard } from '@kit.BasicServicesKit';
import { image } from '@kit.ImageKit';
import { fileIo } from '@kit.CoreFileKit';
import { LxLog } from '../../app/LxLog';
import { onCallback } from 'liblingxia.so';

const DOMAIN = 0x0000;
const TAG = 'LingXia.Clipboard';

function ok(): string {
  return '{"ok":true}';
}

// ArkTS rejects untyped object literals, so every payload has a declared
// shape. `JSON.stringify` drops the optional fields left undefined.
interface ClipboardFailure {
  ok: boolean;
  error: number;
  detail: string;
}

interface ClipboardReadPayload {
  ok: boolean;
  canceled: boolean;
  text?: string;
  imagePath?: string;
  imageMime?: string;
}

interface ClipboardTypesPayload {
  ok: boolean;
  canceled: boolean;
  types: string[];
}

function fail(code: number, detail: string): string {
  const payload: ClipboardFailure = { ok: false, error: code, detail: detail };
  return JSON.stringify(payload);
}

function encodeRead(text?: string, imagePath?: string): string {
  const payload: ClipboardReadPayload = { ok: true, canceled: false };
  if (text !== undefined) {
    payload.text = text;
  }
  if (imagePath !== undefined) {
    payload.imagePath = imagePath;
    payload.imageMime = 'image/png';
  }
  return JSON.stringify(payload);
}

function mapError(error: Object): string {
  const business = error as BusinessError;
  if (business?.code === 201) {
    return fail(3008, 'clipboard permission denied');
  }
  return fail(1001, `${business?.message ?? error}`);
}

function complete(callbackId: string, payload: string): void {
  onCallback(callbackId, true, payload);
}

export class LxAppClipboard {
  public static write(kind: string, payload: String, callbackId: string): boolean {
    const id = String(callbackId ?? '').trim();
    if (!id) {
      return false;
    }
    LxAppClipboard.writeAsync(kind, String(payload ?? ''), id).catch((error: Object) => {
      LxLog.e(TAG, `clipboard write failed: ${error}`);
      complete(id, mapError(error));
    });
    return true;
  }

  public static read(kind: string, imageOutputPath: String, callbackId: string): boolean {
    const id = String(callbackId ?? '').trim();
    if (!id) {
      return false;
    }
    LxAppClipboard.readAsync(kind, String(imageOutputPath ?? ''), id).catch((error: Object) => {
      LxLog.e(TAG, `clipboard read failed: ${error}`);
      complete(id, mapError(error));
    });
    return true;
  }

  public static clear(callbackId: string): boolean {
    const id = String(callbackId ?? '').trim();
    if (!id) {
      return false;
    }
    LxAppClipboard.clearAsync(id).catch((error: Object) => {
      LxLog.e(TAG, `clipboard clear failed: ${error}`);
      complete(id, mapError(error));
    });
    return true;
  }

  public static types(callbackId: string): boolean {
    const id = String(callbackId ?? '').trim();
    if (!id) {
      return false;
    }
    LxAppClipboard.typesAsync(id).catch((error: Object) => {
      LxLog.e(TAG, `clipboard types failed: ${error}`);
      complete(id, mapError(error));
    });
    return true;
  }

  private static async writeAsync(kind: string, payload: string, callbackId: string): Promise<void> {
    const board = pasteboard.getSystemPasteboard();
    if (kind === 'text') {
      const data = pasteboard.createData(pasteboard.MIMETYPE_TEXT_PLAIN, payload);
      await board.setData(data);
      complete(callbackId, ok());
      return;
    }
    if (kind === 'image') {
      const pixelMap = await LxAppClipboard.pixelMapFromFile(payload);
      if (!pixelMap) {
        complete(callbackId, fail(1002, 'clipboard image filePath is not a readable image'));
        return;
      }
      const data = pasteboard.createData(pasteboard.MIMETYPE_PIXELMAP, pixelMap);
      await board.setData(data);
      complete(callbackId, ok());
      return;
    }
    complete(callbackId, fail(1002, 'unknown clipboard type'));
  }

  // `getData` needs `ohos.permission.READ_PASTEBOARD` (or a PasteButton
  // gesture the runtime does not own); without it the system throws 201, which
  // `mapError` reports as 3008 so the lxapp can explain rather than retry.
  private static async readAsync(kind: string, imageOutputPath: string, callbackId: string): Promise<void> {
    const board = pasteboard.getSystemPasteboard();
    const has = await board.hasData();
    if (!has) {
      complete(callbackId, encodeRead());
      return;
    }
    const wantText = kind.length === 0 || kind === 'text';
    const wantImage = kind.length === 0 || kind === 'image';
    // Without READ_PASTEBOARD, `getData` does not throw: it resolves data with
    // the payload stripped. The permission-free peek still sees the types, so
    // a wanted type that is peeked but missing from the data is a denial —
    // report it as one rather than as an empty clipboard.
    const peekedText = wantText && board.hasDataType(pasteboard.MIMETYPE_TEXT_PLAIN);
    const peekedImage = wantImage && board.hasDataType(pasteboard.MIMETYPE_PIXELMAP);
    const data = await board.getData();
    const readableText = data.hasType(pasteboard.MIMETYPE_TEXT_PLAIN);
    const readableImage = data.hasType(pasteboard.MIMETYPE_PIXELMAP);
    if ((peekedText && !readableText) || (peekedImage && !readableImage)) {
      complete(callbackId, fail(3008, 'clipboard permission denied'));
      return;
    }
    let text: string | undefined;
    let imagePath: string | undefined;
    if (wantText && readableText) {
      text = data.getPrimaryText();
    }
    if (wantImage && imageOutputPath.length > 0 && readableImage) {
      const pixelMap = data.getPrimaryPixelMap();
      if (pixelMap) {
        imagePath = await LxAppClipboard.writePixelMap(pixelMap, imageOutputPath);
      }
    }
    complete(callbackId, encodeRead(text, imagePath));
  }

  private static async clearAsync(callbackId: string): Promise<void> {
    const board = pasteboard.getSystemPasteboard();
    await board.clearData();
    complete(callbackId, ok());
  }

  // `hasDataType` peeks without READ_PASTEBOARD; `getData` would need it and
  // turn a harmless type query into a permission failure.
  private static async typesAsync(callbackId: string): Promise<void> {
    const board = pasteboard.getSystemPasteboard();
    const types: string[] = [];
    if (board.hasDataType(pasteboard.MIMETYPE_TEXT_PLAIN)) {
      types.push('text');
    }
    if (board.hasDataType(pasteboard.MIMETYPE_PIXELMAP)) {
      types.push('image');
    }
    const payload: ClipboardTypesPayload = { ok: true, canceled: false, types: types };
    complete(callbackId, JSON.stringify(payload));
  }

  private static async pixelMapFromFile(path: string): Promise<image.PixelMap | undefined> {
    try {
      const source = image.createImageSource(path);
      return await source.createPixelMap();
    } catch (error) {
      hilog.warn(DOMAIN, TAG, `pixelMapFromFile failed: ${error}`);
      return undefined;
    }
  }

  private static async writePixelMap(pixelMap: image.PixelMap, dest: string): Promise<string | undefined> {
    try {
      const packer = image.createImagePacker();
      const bytes = await packer.packing(pixelMap, { format: 'image/png', quality: 100 });
      const file = fileIo.openSync(dest, fileIo.OpenMode.CREATE | fileIo.OpenMode.TRUNC | fileIo.OpenMode.WRITE_ONLY);
      fileIo.writeSync(file.fd, bytes);
      fileIo.closeSync(file);
      return dest;
    } catch (error) {
      hilog.warn(DOMAIN, TAG, `writePixelMap failed: ${error}`);
      return undefined;
    }
  }
}
