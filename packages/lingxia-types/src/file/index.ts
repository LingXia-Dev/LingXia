/**
 * File system APIs.
 * Corresponds to: lingxia-logic/src/fs.rs
 */

export interface OpenFileOptions {
  /** Local file path or runtime-managed temp path. */
  filePath: string;
  /** Optional coarse file type hint such as `pdf`, `docx`, or `xlsx`. */
  fileType?: string;
  /**
   * `auto`: prefer native review, then fall back to external open.
   * `review`: require native review UI and reject when unsupported.
   * `external`: hand off directly to the system / external app.
   */
  mode?: 'auto' | 'review' | 'external';
  /** Hint for whether the native review UI should expose its action menu when supported. */
  showMenu?: boolean;
}

/** Desktop only. Currently supported on macOS. Windows is planned. */
export interface FileDialogFilter {
  /** Optional label shown in the native dialog. */
  name?: string;
  /** Allowed extensions without dots, e.g. ['pdf', 'txt']. */
  extensions: string[];
}

/** Desktop only. Currently supported on macOS. Windows is planned. */
export interface ChooseFileOptions {
  /** Allow selecting multiple files. Default: false */
  multiple?: boolean;
  /** Optional file filters. Empty or omitted means all file types. */
  filters?: FileDialogFilter[];
  /** Dialog window title. Platform provides a default if omitted. */
  title?: string;
  /** Initial directory the dialog opens in. Platform default if omitted. */
  defaultPath?: string;
}

/** Desktop only. Currently supported on macOS. Windows is planned. */
export interface ChooseFileResult {
  /** True if the user dismissed the dialog without selecting. */
  canceled: boolean;
  /** Absolute system paths of selected files. Empty when canceled. */
  paths: string[];
}

/** Desktop only. Currently supported on macOS. Windows is planned. */
export interface ChooseDirectoryOptions {
  /** Dialog window title. Platform provides a default if omitted. */
  title?: string;
  /** Initial directory the dialog opens in. Platform default if omitted. */
  defaultPath?: string;
}

/** Desktop only. Currently supported on macOS. Windows is planned. */
export interface ChooseDirectoryResult {
  /** True if the user dismissed the dialog without selecting. */
  canceled: boolean;
  /** Absolute system path of the selected directory. Undefined when canceled. */
  path?: string;
}

export interface DownloadOptions {
  /** HTTP(S) source URL. */
  url: string;
  /** Optional request headers. */
  headers?: Record<string, string>;
  /** Request timeout in milliseconds. */
  timeout?: number;
  /** Optional destination path. Relative paths resolve under user data. */
  filePath?: string;
  /** Optional abort signal. */
  signal?: AbortSignal;
}

export interface DownloadProgressEvent {
  kind: 'progress' | 'paused' | 'resumed' | 'canceled' | 'success';
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

export interface DownloadResult {
  /** Final accessible file path or URI. */
  filePath: string;
  /** Suggested filename resolved by runtime from headers/url. */
  fileName: string;
  /** MIME type when available. */
  mimeType?: string;
  /** File size in bytes. */
  size: number;
}

export interface DownloadTask extends PromiseLike<DownloadResult>, AsyncIterable<DownloadProgressEvent> {
  next(): Promise<DownloadIteratorResult>;
  return(): Promise<DownloadIteratorResult>;
  catch<TResult = never>(
    onrejected?: ((reason: unknown) => TResult | PromiseLike<TResult>) | null,
  ): Promise<DownloadResult | TResult>;
  finally(onfinally?: (() => void) | null): Promise<DownloadResult>;
  pause(): Promise<void>;
  resume(): Promise<void>;
  cancel(): Promise<void>;
  abort(): Promise<void>;
  wait(): Promise<DownloadResult>;
}
