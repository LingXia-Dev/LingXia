export interface NativeStream<T> extends AsyncIterable<T> {
  close(): void;
}

export interface BrowserNativeClient {
  invoke<T = void>(route: string, input?: unknown): Promise<T>;
  stream<T = unknown>(route: string, input?: unknown): NativeStream<T>;
}

export function browserNative(page: { readonly native: unknown }): BrowserNativeClient {
  return page.native as BrowserNativeClient;
}
