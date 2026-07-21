/**
 * OS process capability for trusted LingXia home lxapps.
 *
 * Add `@lingxia/types/process` to the Logic tsconfig only when the host sets
 * `capabilities.process: true` and the home lxapp declares the `process`
 * security privilege. These declarations have no runtime payload.
 */

export type RongSpawnStdio = "pipe" | "ignore" | "inherit";
export type RongEnvMap = Record<string, string | undefined>;

export interface RongSpawnOptions {
  cwd?: string;
  env?: RongEnvMap;
  shell?: boolean;
  stdin?: "pipe" | string | ArrayBuffer | ArrayBufferView | null;
  stdout?: RongSpawnStdio;
  stderr?: RongSpawnStdio;
  timeout?: number;
  killSignal?: string | number;
  signal?: AbortSignal;
  onExit?(
    subprocess: RongSubprocess,
    exitCode: number | null,
    signalCode: number | null,
    error?: unknown,
  ): void | Promise<void>;
}

export interface RongSpawnOptionsWithCmd extends RongSpawnOptions {
  cmd: string[];
}

export interface RongSyncSubprocess {
  readonly exitCode: number | null;
  readonly success: boolean;
  readonly signalCode: number | null;
  readonly stdout: Uint8Array;
  readonly stderr: Uint8Array;
}

export interface RongReadableProcessStream extends ReadableStream<Uint8Array> {
  bytes(): Promise<Uint8Array>;
  text(): Promise<string>;
  json(): Promise<unknown>;
  blob(): Promise<Blob>;
  lines(): AsyncIterableIterator<string>;
}

export interface RongOutputHandle {
  write(chunk: string | ArrayBuffer | ArrayBufferView): void;
  flush(): void;
}

export interface RongWritableProcessStream extends WritableStream<Uint8Array> {
  write(chunk: string | ArrayBuffer | ArrayBufferView): Promise<this>;
  flush(): Promise<void>;
  end(): Promise<void>;
}

export interface RongSubprocess {
  readonly pid: number | null;
  readonly exitCode: number | null;
  readonly signalCode: number | null;
  readonly killed: boolean;
  readonly success: boolean;
  readonly exited: Promise<number | null>;
  stdin: RongWritableProcessStream | null;
  stdout: RongReadableProcessStream | null;
  stderr: RongReadableProcessStream | null;
  kill(signal?: string | number): boolean;
  wait(): Promise<number | null>;
  unref(): void;
}

export interface RongShellResult {
  stdout: Uint8Array;
  stderr: Uint8Array;
  exitCode: number | null;
  success: boolean;
}

export interface RongShellCommand extends PromiseLike<RongShellResult> {
  cwd(path: string): RongShellCommand;
  env(values: RongEnvMap): RongShellCommand;
  quiet(): RongShellCommand;
  nothrow(): RongShellCommand;
  throws(value?: boolean): RongShellCommand;
  run(): Promise<RongShellResult>;
  text(): Promise<string>;
  json(): Promise<unknown>;
  blob(): Promise<Blob>;
  lines(): AsyncIterableIterator<string>;
}

export interface RongShellTag {
  (strings: TemplateStringsArray, ...values: unknown[]): RongShellCommand;
  (command: string): RongShellCommand;
  cwd(path?: string): string | RongShellTag | undefined;
  env(values?: RongEnvMap): RongEnvMap | RongShellTag | undefined;
  throws(value?: boolean): RongShellTag;
  nothrow(): RongShellTag;
  quiet(): RongShellTag;
  escape(value: unknown): string;
}

export interface RongShellError extends Error {
  command: string;
  exitCode: number | null;
  stdout: Uint8Array;
  stderr: Uint8Array;
}

declare global {
  /** Process APIs exposed only by a host with `capabilities.process`. */
  interface RongNamespace {
    readonly argv: string[];
    readonly args: string[];
    readonly env: RongEnvMap;
    readonly stdin: RongReadableProcessStream;
    readonly stdout: RongOutputHandle;
    readonly stderr: RongOutputHandle;
    spawn(cmd: string[], options?: RongSpawnOptions): RongSubprocess;
    spawn(options: RongSpawnOptionsWithCmd): RongSubprocess;
    spawnSync(cmd: string[], options?: RongSpawnOptions): RongSyncSubprocess;
    spawnSync(options: RongSpawnOptionsWithCmd): RongSyncSubprocess;
    readonly $: RongShellTag;
    readonly ShellError: {
      new (message: string): RongShellError;
      prototype: RongShellError;
    };
  }

  const Rong: RongNamespace;
}
