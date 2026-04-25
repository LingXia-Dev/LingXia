/**
 * Transfer task APIs.
 */

export interface DownloadOptions {
  /** HTTP(S) source URL. */
  url: string;
  /**
   * Optional request headers.
   * Restricted headers such as `Referer` are ignored by the runtime.
   */
  headers?: Record<string, string>;
  /** Request timeout in milliseconds. */
  timeout?: number;
  /**
   * Optional durable destination.
   *
   * - Omit `filePath` to receive a temporary file in `tempFilePath`
   * - Relative paths resolve under user data
   * - `lx://` paths must target `lx://userdata`
   *
   * `lx://usercache` is not accepted here.
   */
  filePath?: string;
  /** Optional abort signal. */
  signal?: AbortSignal;
}

export interface DownloadProgressEvent {
  kind: 'progress' | 'paused' | 'resumed' | 'canceled' | 'completed';
  downloadedBytes?: number;
  totalBytes?: number;
  /** Present only when the total size is known. */
  progress?: number;
  result?: DownloadResult;
}

export interface DownloadIteratorResult {
  done: boolean;
  value?: DownloadProgressEvent;
}

export type DownloadResult =
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
      filePath: string;
      tempFilePath?: never;
      mimeType?: string;
      size: number;
    };

export interface DownloadTask extends PromiseLike<DownloadResult>, AsyncIterable<DownloadProgressEvent> {
  next(): Promise<DownloadIteratorResult>;
  /** Stops iteration only. Does not cancel the underlying download task. */
  return(): Promise<DownloadIteratorResult>;
  catch<TResult = never>(
    onrejected?: ((reason: unknown) => TResult | PromiseLike<TResult>) | null,
  ): Promise<DownloadResult | TResult>;
  finally(onfinally?: (() => void) | null): Promise<DownloadResult>;
  pause(): Promise<void>;
  resume(): Promise<void>;
  cancel(): Promise<void>;
  /** Alias for cancel(), matching browser/mini-program abort naming. */
  abort(): Promise<void>;
  wait(): Promise<DownloadResult>;
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
