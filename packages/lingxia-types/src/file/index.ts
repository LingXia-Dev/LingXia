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

export interface ExistsOptions {
  path: string;
}

export interface StatOptions {
  path: string;
}

export interface FileStats {
  isFile: boolean;
  isDirectory: boolean;
  isSymlink: boolean;
  size: number;
  lastModifiedTime?: number;
  lastAccessedTime?: number;
  createTime?: number;
}

export interface ReadDirOptions {
  path: string;
}

export interface DirEntry {
  name: string;
  isFile: boolean;
  isDirectory: boolean;
  isSymlink: boolean;
}

export interface MkdirOptions {
  path: string;
  recursive?: boolean;
}

export interface ReadTextFileOptions {
  filePath: string;
  encoding: 'utf8' | 'base64';
}

export interface ReadBinaryFileOptions {
  filePath: string;
  encoding?: undefined;
}

export type ReadFileOptions = ReadTextFileOptions | ReadBinaryFileOptions;

export interface ReadTextFileResult {
  data: string;
}

export interface ReadBinaryFileResult {
  data: ArrayBuffer;
}

export type ReadFileResult = ReadTextFileResult | ReadBinaryFileResult;

export type BinaryFileData = ArrayBuffer | ArrayBufferView;

export interface WriteTextFileOptions {
  filePath: string;
  data: string;
  encoding?: 'utf8' | 'base64';
  /** Defaults to false. */
  overwrite?: boolean;
}

export interface WriteBinaryFileOptions {
  filePath: string;
  data: BinaryFileData;
  encoding?: never;
  /** Defaults to false. */
  overwrite?: boolean;
}

export type WriteFileOptions = WriteTextFileOptions | WriteBinaryFileOptions;

export interface CopyFileOptions {
  srcPath: string;
  destPath: string;
  /** Defaults to false. */
  overwrite?: boolean;
}

export interface RenameOptions {
  oldPath: string;
  newPath: string;
  /** Defaults to false. */
  overwrite?: boolean;
}

export interface RemoveOptions {
  path: string;
  recursive?: boolean;
}

export interface FileManager {
  exists(options: ExistsOptions): Promise<boolean>;
  stat(options: StatOptions): Promise<FileStats>;
  readDir(options: ReadDirOptions): Promise<AsyncIterableIterator<DirEntry>>;
  mkdir(options: MkdirOptions): Promise<void>;
  readFile(options: ReadTextFileOptions): Promise<ReadTextFileResult>;
  readFile(options: ReadBinaryFileOptions): Promise<ReadBinaryFileResult>;
  readFile(options: ReadFileOptions): Promise<ReadFileResult>;
  writeFile(options: WriteFileOptions): Promise<void>;
  copyFile(options: CopyFileOptions): Promise<void>;
  rename(options: RenameOptions): Promise<void>;
  remove(options: RemoveOptions): Promise<void>;
}
