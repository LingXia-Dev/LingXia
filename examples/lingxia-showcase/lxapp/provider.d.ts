// `lx.cloud` and `lx.auth` are not part of the LingXia runtime. They come from
// a private provider crate the CLI injects for one build only
// (`lingxia build --with-provider cloud`), so `@lingxia/types` cannot declare
// them and this example carries its own view of the surface it calls. Anything
// the showcase does not use is deliberately absent; the provider is the source
// of truth for the rest.
//
// The members mirror what `pages/cloud/index.ts` actually calls. Keep them in
// step with the provider, or the page compiles against a shape that no longer
// exists.

interface CloudIdentity {
  user?: { id?: string; name?: string; avatar?: string };
  tenant?: { id?: string; name?: string; shortName?: string; logoUrl?: string };
  active?: boolean;
  activate?: () => Promise<unknown>;
  logout?: () => Promise<void>;
}

interface CloudMqttStatus {
  state?: string;
  [key: string]: unknown;
}

interface CloudMqttMessage {
  topic: string;
  payload: unknown;
  qos?: number;
  receivedAt: number;
}

interface CloudMqttSubscription extends AsyncIterable<CloudMqttMessage> {
  close(): Promise<void>;
}

interface CloudMqttApi {
  getStatus(): CloudMqttStatus;
  onStatusChange(handler: (status: CloudMqttStatus) => void): () => void;
  subscribe(
    topic: string,
    options?: { parse?: 'auto' | 'json' | 'text' | 'none' },
  ): Promise<CloudMqttSubscription>;
}

interface CloudApi {
  invoke(name: string, payload?: unknown): Promise<unknown>;
  readonly mqtt: CloudMqttApi;
}

interface CloudAuthApi {
  list(): Promise<CloudIdentity[]>;
  login(): Promise<unknown>;
  add(): Promise<unknown>;
}

declare global {
  interface Lx {
    /** Provider-supplied; absent unless the build injected the cloud provider. */
    readonly cloud: CloudApi;
    /** Provider-supplied; absent unless the build injected the cloud provider. */
    readonly auth: CloudAuthApi;
  }
}

export {};
