import { media } from '@kit.MediaKit';
import { image } from '@kit.ImageKit';
import { fileIo } from '@kit.CoreFileKit';

/**
 * Process-level cache for first-frame `PixelMap` images of local video files,
 * used by `LxNativeVideoPlayer`'s playlist mode to bridge the visual gap
 * during item transitions (cold start, manual jumps, error recovery).
 *
 * Extraction is independent of `OH_AVPlayer` — uses
 * [media.AVImageGenerator], a system demuxer that decodes a single keyframe
 * without holding a playback session's codec/buffer resources. Local files
 * only; remote URLs short-circuit to undefined.
 *
 * Memory:
 * - Bounded `Map`-based LRU keyed by URL with a manual byte-budget check.
 * - Caps total stored pixels at a conservative ~16MB (low-mem devices: ~6MB).
 *
 * Threading:
 * - All public API is async (Promise-based). Decoding runs on the
 *   media-kit's internal worker; we serialize per-URL via an in-flight map.
 *
 * Lifecycle:
 * - Process-level singleton. No `release()` — correct lifetime is forever.
 */
export class LxMediaFrameCache {

  private static _instance: LxMediaFrameCache | undefined;

  static getInstance(): LxMediaFrameCache {
    if (!LxMediaFrameCache._instance) {
      LxMediaFrameCache._instance = new LxMediaFrameCache();
    }
    return LxMediaFrameCache._instance;
  }

  private readonly cache: Map<string, image.PixelMap> = new Map();
  private cachedBytes: number = 0;
  private readonly maxBytes: number = LxMediaFrameCache.computeMaxBytes();
  private readonly inFlight: Map<string, Promise<image.PixelMap | undefined>> = new Map();

  private constructor() {}

  /** Synchronous cache lookup. Returns the PixelMap or undefined. */
  peek(url: string): image.PixelMap | undefined {
    const found = this.cache.get(url);
    if (found !== undefined) {
      // Map iteration is insertion-order; touch entry to mark it most-recent.
      this.cache.delete(url);
      this.cache.set(url, found);
    }
    return found;
  }

  /**
   * Asynchronously extract and cache the first frame for [url].
   * Local URLs only (`file://...` or absolute path); other schemes resolve
   * to undefined.
   */
  async load(url: string): Promise<image.PixelMap | undefined> {
    const cached = this.peek(url);
    if (cached) return cached;

    const pending = this.inFlight.get(url);
    if (pending) return pending;

    const task = this.decode(url).then((pixelMap) => {
      this.inFlight.delete(url);
      if (pixelMap) this.put(url, pixelMap);
      return pixelMap;
    }).catch(() => {
      this.inFlight.delete(url);
      return undefined;
    });
    this.inFlight.set(url, task);
    return task;
  }

  /** Fire-and-forget prefetch. No-op if already cached or in-flight. */
  prefetch(url: string): void {
    if (this.cache.has(url)) return;
    if (this.inFlight.has(url)) return;
    this.load(url).catch(() => { /* swallow */ });
  }

  evictAll(): void {
    this.cache.clear();
    this.cachedBytes = 0;
  }

  // ---------- internal ----------

  private put(url: string, pixelMap: image.PixelMap): void {
    const bytes = LxMediaFrameCache.estimateBytes(pixelMap);
    if (bytes > this.maxBytes) {
      // Single image too large; refuse caching and let it GC after caller use.
      return;
    }
    while (this.cachedBytes + bytes > this.maxBytes && this.cache.size > 0) {
      const iter = this.cache.keys().next();
      if (iter.done === true) break;
      const oldestKey: string = iter.value;
      const old = this.cache.get(oldestKey);
      if (old) this.cachedBytes -= LxMediaFrameCache.estimateBytes(old);
      this.cache.delete(oldestKey);
    }
    this.cache.set(url, pixelMap);
    this.cachedBytes += bytes;
  }

  private async decode(url: string): Promise<image.PixelMap | undefined> {
    const path = LxMediaFrameCache.resolveLocalPath(url);
    if (!path) return undefined;

    let srcFile: fileIo.File | undefined;
    let generator: media.AVImageGenerator | undefined;
    try {
      srcFile = await fileIo.open(path, fileIo.OpenMode.READ_ONLY);
      const stat = await fileIo.stat(path);
      generator = await media.createAVImageGenerator();
      generator.fdSrc = {
        fd: srcFile.fd,
        offset: 0,
        length: Number(stat.size ?? 0),
      };
      const queryOptions: media.PixelMapParams = {
        width: 1080,
        height: 1920,
      };
      const pixelMap = await generator.fetchFrameByTime(
        0,
        media.AVImageQueryOptions.AV_IMAGE_QUERY_NEXT_SYNC,
        queryOptions
      );
      return pixelMap;
    } catch (e) {
      return undefined;
    } finally {
      if (generator !== undefined) {
        try { await generator.release(); } catch (_e) { /* ignore */ }
      }
      if (srcFile !== undefined) {
        try { await fileIo.close(srcFile.fd); } catch (_e) { /* ignore */ }
      }
    }
  }

  private static resolveLocalPath(url: string): string | undefined {
    const trimmed = url.trim();
    if (!trimmed) return undefined;
    if (trimmed.startsWith('http://') || trimmed.startsWith('https://')) return undefined;
    if (trimmed.startsWith('file://')) return trimmed.slice('file://'.length);
    if (trimmed.startsWith('/')) return trimmed;
    // Other schemes (lx://, content://) need app-specific resolution which
    // happens upstream — caller should pass the resolved local path here.
    return undefined;
  }

  private static estimateBytes(pixelMap: image.PixelMap): number {
    try {
      const info = pixelMap.getImageInfoSync();
      return Math.max(1, info.size.width * info.size.height * 4);
    } catch (_e) {
      return 4 * 1024 * 1024;
    }
  }

  private static computeMaxBytes(): number {
    // Conservative: ~16MB on regular devices. The cache is small intentionally;
    // playlist transitions only need 1-2 frames live at a time.
    return 16 * 1024 * 1024;
  }
}
