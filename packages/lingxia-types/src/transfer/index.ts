/**
 * Transfer task APIs.
 */

declare const appDownloadPathBrand: unique symbol;
declare const systemDownloadsPathBrand: unique symbol;

export type DownloadDestination = 'app' | 'downloads';

/** Runtime-managed app download path, usually under `lx://userdata`. */
export type AppDownloadFilePath = string & {
  readonly [appDownloadPathBrand]: 'app-download-file-path';
};

/** Native system Downloads path. Do not pass this to `FileManager`. */
export type SystemDownloadsPath = string & {
  readonly [systemDownloadsPathBrand]: 'system-downloads-path';
};

export interface DownloadOptionsBase {
  /** HTTP(S) source URL. */
  url: string;
  /**
   * Optional request headers.
   * Restricted headers such as `Referer` are ignored by the runtime.
   */
  headers?: Record<string, string>;
  /** Request timeout in milliseconds. */
  timeout?: number;
  /** Optional abort signal. */
  signal?: AbortSignal;
}

export interface AppDownloadOptions extends DownloadOptionsBase {
  /**
   * Optional app-owned durable output path.
   *
   * Omit `filePath` to receive a temporary result in `tempFilePath`. Relative
   * paths resolve under user data. `lx://` paths must target `lx://userdata`;
   * `lx://usercache` is not accepted here.
   */
  filePath?: string;
  /**
   * App-owned output. Omit to use a temporary output unless `filePath` is set.
   */
  destination?: 'app';
}

export interface DownloadsDownloadOptions extends DownloadOptionsBase {
  /**
   * Optional filename hint for the system Downloads destination.
   * This is not an app-owned FileManager path.
   */
  filePath?: string;
  /** Save into the user's system Downloads directory. */
  destination: 'downloads';
}

/**
 * Download options.
 *
 * - `app`: app-owned temporary output, or durable `lx://userdata` output when
 *   `filePath` is set
 * - `downloads`: user-visible system Downloads output, requiring
 *   `security.privileges: ["downloads"]` in `lxapp.json`
 *
 * Default: `app`.
 */
export type DownloadOptions<TDestination extends DownloadDestination = DownloadDestination> =
  TDestination extends 'downloads' ? DownloadsDownloadOptions : AppDownloadOptions;

export type DownloadResultForDestination<TDestination extends DownloadDestination> =
  TDestination extends 'downloads' ? DownloadsDownloadResult : AppDownloadResult;

export interface DownloadProgressEvent<TResult extends DownloadResult = DownloadResult> {
  kind: 'progress' | 'paused' | 'resumed' | 'canceled' | 'completed';
  downloadedBytes?: number;
  totalBytes?: number;
  /** Present only when the total size is known. */
  progress?: number;
  result?: TResult;
}

export interface DownloadIteratorResult<TResult extends DownloadResult = DownloadResult> {
  done: boolean;
  value?: DownloadProgressEvent<TResult>;
}

export type AppDownloadResult =
  | {
      /**
       * Temporary result.
       *
       * Not durable; move or copy it to `lx://userdata` if you need to keep it.
       *
       * When `filePath` is omitted, the runtime must be able to infer a file
       * type from the URL or the server's `Content-Type` header.
       */
      tempFilePath: string;
      filePath?: never;
      mimeType?: string;
      size: number;
    }
  | {
      /** Durable destination under `lx://userdata`. */
      filePath: AppDownloadFilePath;
      tempFilePath?: never;
      mimeType?: string;
      size: number;
    };

export interface DownloadsDownloadResult {
  /** Native system Downloads path. Do not pass this to `FileManager`. */
  filePath: SystemDownloadsPath;
  tempFilePath?: never;
  mimeType?: string;
  size: number;
}

export type DownloadResult = AppDownloadResult | DownloadsDownloadResult;

export interface DownloadTask<TDownloadResult extends DownloadResult = DownloadResult>
  extends PromiseLike<TDownloadResult>,
    AsyncIterable<DownloadProgressEvent<TDownloadResult>> {
  next(): Promise<DownloadIteratorResult<TDownloadResult>>;
  /** Stops iteration only. Does not cancel the underlying download task. */
  return(): Promise<DownloadIteratorResult<TDownloadResult>>;
  catch<TRejected = never>(
    onrejected?: ((reason: unknown) => TRejected | PromiseLike<TRejected>) | null,
  ): Promise<TDownloadResult | TRejected>;
  finally(onfinally?: (() => void) | null): Promise<TDownloadResult>;
  pause(): Promise<void>;
  resume(): Promise<void>;
  cancel(): Promise<void>;
  /** Alias for cancel(), matching browser/mini-program abort naming. */
  abort(): Promise<void>;
  wait(): Promise<TDownloadResult>;
}

export interface UploadOptions {
  /** HTTP(S) destination URL. */
  url: string;
  /** Local file path or runtime-managed URI to upload. */
  filePath: string;
  /** Multipart field name. Default: `file`. */
  name?: string;
  /**
   * Optional request headers.
   * Restricted headers such as `Referer` are ignored by the runtime.
   */
  headers?: Record<string, string>;
  /** Optional extra `multipart/form-data` text fields. */
  formData?: Record<string, string>;
  /** Request timeout in milliseconds. */
  timeout?: number;
  /** Override multipart filename. */
  fileName?: string;
  /** Override file MIME type. */
  mimeType?: string;
  /** Optional abort signal. */
  signal?: AbortSignal;
}

export interface UploadProgressEvent {
  kind: 'progress' | 'canceled' | 'completed';
  uploadedBytes?: number;
  totalBytes?: number;
  progress?: number;
  result?: UploadResult;
}

export interface UploadIteratorResult {
  done: boolean;
  value?: UploadProgressEvent;
}

export interface UploadResult {
  /** HTTP status code returned by the server. */
  statusCode: number;
  /** Response body decoded as UTF-8 text. */
  data: string;
}

export interface UploadTask extends PromiseLike<UploadResult>, AsyncIterable<UploadProgressEvent> {
  next(): Promise<UploadIteratorResult>;
  /** Stops iteration only. Does not cancel the underlying upload task. */
  return(): Promise<UploadIteratorResult>;
  catch<TResult = never>(
    onrejected?: ((reason: unknown) => TResult | PromiseLike<TResult>) | null,
  ): Promise<UploadResult | TResult>;
  finally(onfinally?: (() => void) | null): Promise<UploadResult>;
  cancel(): Promise<void>;
  wait(): Promise<UploadResult>;
}
