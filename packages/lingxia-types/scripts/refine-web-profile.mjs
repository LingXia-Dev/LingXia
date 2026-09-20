// Rong 0.6.1 omits stream controller contracts. Keep the correction in generator
// input until upstream emits these declarations, then remove this refinement.
export function refineWebProfile(source) {
  return source
    .replace('read(): Promise<{ done: boolean; value: R }>', 'read(): Promise<{ done: false; value: R } | { done: true; value?: undefined }>')
    .replace('underlyingSource?: any', 'underlyingSource?: UnderlyingSource<R>')
    .replace('underlyingSink?: any', 'underlyingSink?: UnderlyingSink<W>')
    + `
interface ReadableStreamDefaultController<R> {
  readonly desiredSize: number | null;
  enqueue(chunk: R): void;
  close(): void;
  error(reason?: unknown): void;
}
interface WritableStreamDefaultController {
  error(reason?: unknown): void;
}
interface UnderlyingSource<R> {
  start?(controller: ReadableStreamDefaultController<R>): unknown;
  pull?(controller: ReadableStreamDefaultController<R>): void | PromiseLike<void>;
  cancel?(reason: unknown): void | PromiseLike<void>;
}
interface UnderlyingSink<W> {
  start?(controller: WritableStreamDefaultController): unknown;
  write?(chunk: W, controller: WritableStreamDefaultController): void | PromiseLike<void>;
  close?(): void | PromiseLike<void>;
  abort?(reason: unknown): void | PromiseLike<void>;
}
`;
}
