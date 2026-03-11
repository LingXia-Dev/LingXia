/**
 * File system APIs.
 * Corresponds to: lingxia-logic/src/fs.rs
 */

export interface OpenDocumentOptions {
  filePath: string;
  fileType?: string;
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
  /**
   * Optional external cancellation channel.
   * When aborted, it has the same effect as calling `task.cancel()`.
   * Useful for timeout/router/lifecycle composition.
   */
  signal?: AbortSignal;
}

export interface DownloadResult {
  /** Downloaded file URI under user cache, e.g. lx://usercache/... */
  tempFilePath: string;
  /** Suggested filename resolved by runtime from headers/url. */
  fileName: string;
  /** MIME type when available. */
  mimeType?: string;
  /** File size in bytes. */
  size: number;
}

export interface DownloadProgressEvent {
  kind: 'progress';
  downloadedBytes: number;
  totalBytes?: number;
  /**
   * 0~1 progress value.
   * - precise ratio when totalBytes is known
   * - monotonic estimated progress when totalBytes is unknown
   */
  progress: number;
}

export interface DownloadSuccessEvent {
  kind: 'success';
  result: DownloadResult;
}

export interface DownloadPausedEvent {
  kind: 'paused';
}

export interface DownloadResumedEvent {
  kind: 'resumed';
}

export interface DownloadCanceledEvent {
  kind: 'canceled';
}

export type DownloadEvent =
  | DownloadProgressEvent
  | DownloadSuccessEvent
  | DownloadPausedEvent
  | DownloadResumedEvent
  | DownloadCanceledEvent;

export interface DownloadTask extends AsyncIterable<DownloadEvent> {
  /** Pause current download and keep resume metadata. */
  pause(): Promise<void>;
  /** Resume a paused download from persisted resume metadata. */
  resume(): Promise<void>;
  /** Cancel and remove downloaded temp artifacts. */
  cancel(): Promise<void>;
}
