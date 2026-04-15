/**
 * File system APIs.
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

export interface FileDialogFilter {
  /** Optional label shown in the native dialog. */
  name?: string;
  /** Allowed extensions without dots, e.g. ['pdf', 'txt']. */
  extensions: string[];
}

export interface ChooseFileOptions {
  /** Allow selecting multiple files. Default: false */
  multiple?: boolean;
  /** Optional file filters. Empty or omitted means all file types. */
  filters?: FileDialogFilter[];
  /** Initial directory the dialog opens in. Platform default if omitted. */
  defaultPath?: string;
}

export interface ChooseFileResult {
  /** True if the user dismissed the dialog without selecting. */
  canceled: boolean;
  /** Native-consumable file references (paths or URIs). Empty when canceled. */
  paths: string[];
}

export interface ChooseDirectoryOptions {
  /** Initial directory the dialog opens in. Platform default if omitted. */
  defaultPath?: string;
}

export interface ChooseDirectoryResult {
  /** True if the user dismissed the dialog without selecting. */
  canceled: boolean;
  /** Native-consumable directory reference (path or URI). Undefined when canceled. */
  path?: string;
}
