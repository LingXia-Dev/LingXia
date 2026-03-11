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
