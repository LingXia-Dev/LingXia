import {
  reactive,
  ref,
  onUnmounted,
  watch,
  type Ref,
} from "vue";
import type {
  StreamHandle,
  Subscription as BridgeSubscription,
  Channel as BridgeChannel,
  LxBridgeError,
} from "@lingxia/bridge";

type ActionMap = Record<string, (...args: unknown[]) => unknown>;
type Snapshot = Record<string, unknown>;

const reactiveSnapshot = reactive<Snapshot>({});
let subscribed = false;
let subscribeRetryTimer: ReturnType<typeof setTimeout> | null = null;
let initialSnapshotResolved = false;
let snapshotRequestInFlight = false;

function getParamsSignature(params: unknown): string {
  try {
    return JSON.stringify(params) ?? "undefined";
  } catch {
    return "__nonserializable__";
  }
}

function toBridgeError(
  err: unknown,
  fallbackCode: string,
  fallbackMessage: string,
): LxBridgeError {
  return err && typeof err === "object" && "code" in err
    ? (err as LxBridgeError)
    : { code: fallbackCode, message: String(err ?? fallbackMessage) };
}

function replaceReactiveSnapshot(next: unknown): void {
  const normalized: Snapshot =
    next && typeof next === "object" ? (next as Snapshot) : {};

  for (const key of Object.keys(reactiveSnapshot)) {
    if (!Object.prototype.hasOwnProperty.call(normalized, key)) {
      delete reactiveSnapshot[key];
    }
  }
  Object.assign(reactiveSnapshot, normalized);
}

function scheduleSubscribeRetry(): void {
  if (subscribeRetryTimer !== null || subscribed) return;
  subscribeRetryTimer = setTimeout(() => {
    subscribeRetryTimer = null;
    ensureBridgeSubscription();
  }, 10);
}

function requestInitialSnapshot(bridge: Window["LingXiaBridge"] | undefined): void {
  if (initialSnapshotResolved || snapshotRequestInFlight) return;
  if (!bridge?.call) return;
  snapshotRequestInFlight = true;
  bridge
    .call("state.getSnapshot", { scope: "page" })
    .then(() => {
      initialSnapshotResolved = true;
    })
    .catch(() => {
      scheduleSubscribeRetry();
    })
    .finally(() => {
      snapshotRequestInFlight = false;
    });
}

function ensureBridgeSubscription(): void {
  if (subscribed) return;
  const bridge = window.LingXiaBridge;
  const subscribeState = bridge?.state?.subscribe;
  if (!subscribeState) {
    scheduleSubscribeRetry();
    return;
  }
  subscribeState((next) => {
    replaceReactiveSnapshot(next);
  });
  subscribed = true;
  requestInitialSnapshot(bridge);
}

function resolveActions<TActions extends ActionMap>(): TActions {
  const actions: ActionMap = {};
  const bridge = window.__pageBridge;
  if (!bridge?.__names) {
    return actions as TActions;
  }

  for (const name of bridge.__names) {
    if (typeof name !== "string") continue;
    const fn = bridge[name];
    if (typeof fn === "function") {
      actions[name] = fn as (...args: unknown[]) => unknown;
    }
  }

  return actions as TActions;
}

export function useLxPage<
  TData = Snapshot,
  TActions extends ActionMap = ActionMap,
>(): { data: TData; actions: TActions } {
  ensureBridgeSubscription();
  return { data: reactiveSnapshot as TData, actions: resolveActions<TActions>() };
}

export interface LxStreamOptions<TData, TReduced> {
  manual?: boolean;
  reduce?: (accumulated: TReduced, chunk: TData) => TReduced;
  initial?: TReduced;
}

export interface LxStreamState<TData, TResult = unknown> {
  data: Ref<TData | undefined>;
  result: Ref<TResult | undefined>;
  error: Ref<LxBridgeError | undefined>;
  streaming: Ref<boolean>;
  cancel: () => void;
  call: () => void;
}

export function useLxStream<TData = unknown, TResult = unknown, TReduced = TData>(
  factory: () => StreamHandle<TData, TResult>,
  options?: LxStreamOptions<TData, TReduced>,
): LxStreamState<TReduced extends TData ? TData : TReduced, TResult> {
  type TOut = TReduced extends TData ? TData : TReduced;

  const data = ref<TOut | undefined>(
    (options?.reduce ? options.initial : undefined) as TOut | undefined,
  ) as Ref<TOut | undefined>;
  const result = ref<TResult | undefined>(undefined) as Ref<TResult | undefined>;
  const error = ref<LxBridgeError | undefined>(undefined);
  const streaming = ref(false);

  let handle: StreamHandle<TData, TResult> | null = null;
  let acc: TReduced | undefined = options?.initial;
  let runId = 0;

  function cancel(): void {
    runId += 1;
    handle?.cancel();
    handle = null;
    streaming.value = false;
  }

  function call(): void {
    // Cancel any previous stream.
    handle?.cancel();
    const currentRunId = runId + 1;
    runId = currentRunId;

    acc = options?.initial;
    data.value = (options?.reduce ? options.initial : undefined) as TOut | undefined;
    result.value = undefined;
    error.value = undefined;
    streaming.value = true;

    try {
      handle = factory();
    } catch (err: unknown) {
      if (runId !== currentRunId) return;
      handle = null;
      error.value = toBridgeError(
        err,
        "STREAM_CALL_FAILED",
        "Failed to start stream",
      );
      streaming.value = false;
      return;
    }

    handle.on("data", (chunk: TData) => {
      if (runId !== currentRunId) return;
      if (options?.reduce) {
        acc = options.reduce(acc as TReduced, chunk);
        data.value = acc as TOut;
      } else {
        data.value = chunk as unknown as TOut;
      }
    });

    handle.on("end", (res: TResult) => {
      if (runId !== currentRunId) return;
      handle = null;
      result.value = res;
      streaming.value = false;
    });

    handle.on("error", (err: LxBridgeError) => {
      if (runId !== currentRunId) return;
      handle = null;
      error.value = err;
      streaming.value = false;
    });
  }

  if (!options?.manual) {
    call();
  }

  onUnmounted(() => {
    runId += 1;
    handle?.cancel();
    handle = null;
  });

  return { data, result, error, streaming, cancel, call };
}

export interface LxSubscriptionOptions {
  params?: Record<string, unknown>;
}

export interface LxSubscriptionState<TData> {
  data: Ref<TData | undefined>;
  error: Ref<LxBridgeError | undefined>;
  active: Ref<boolean>;
  close: () => void;
}

export function useLxSubscription<TData = unknown>(
  topic: string | Ref<string>,
  options?: LxSubscriptionOptions,
): LxSubscriptionState<TData> {
  const data = ref<TData | undefined>(undefined) as Ref<TData | undefined>;
  const error = ref<LxBridgeError | undefined>(undefined);
  const active = ref(false);

  let sub: BridgeSubscription<TData> | null = null;
  let subscribeVersion = 0;

  function close(): void {
    subscribeVersion += 1;
    sub?.close();
    sub = null;
    active.value = false;
  }

  function subscribe(topicValue: string): void {
    const currentVersion = subscribeVersion + 1;
    subscribeVersion = currentVersion;
    sub?.close();
    sub = null;
    error.value = undefined;
    active.value = false;
    const bridge = window.LingXiaBridge;
    if (!bridge?.subscribe) return;

    bridge
      .subscribe<TData>(topicValue, options?.params)
      .then((s) => {
        if (subscribeVersion !== currentVersion) {
          s.close();
          return;
        }
        sub = s;
        active.value = true;

        s.on("data", (payload: TData) => {
          if (subscribeVersion !== currentVersion) return;
          data.value = payload;
        });
        s.on("error", (err: LxBridgeError) => {
          if (subscribeVersion !== currentVersion) return;
          sub = null;
          error.value = err;
          active.value = false;
        });
      })
      .catch((err: unknown) => {
        if (subscribeVersion !== currentVersion) return;
        error.value = toBridgeError(
          err,
          "SUBSCRIBE_FAILED",
          "Failed to subscribe",
        );
        active.value = false;
      });
  }

  const topicRef = typeof topic === "string" ? ref(topic) : topic;
  watch(
    [topicRef, () => getParamsSignature(options?.params)],
    ([topicValue]) => subscribe(topicValue),
    { immediate: true },
  );

  onUnmounted(() => {
    subscribeVersion += 1;
    sub?.close();
    sub = null;
  });

  return { data, error, active, close };
}

export interface LxChannelOptions {
  params?: Record<string, unknown>;
}

export interface LxChannelState<TData> {
  data: Ref<TData | undefined>;
  error: Ref<LxBridgeError | undefined>;
  connected: Ref<boolean>;
  send: (payload: unknown) => void;
  close: (code?: string, reason?: string) => void;
}

export function useLxChannel<TData = unknown>(
  topic: string | Ref<string>,
  options?: LxChannelOptions,
): LxChannelState<TData> {
  const data = ref<TData | undefined>(undefined) as Ref<TData | undefined>;
  const error = ref<LxBridgeError | undefined>(undefined);
  const connected = ref(false);

  let ch: BridgeChannel<TData> | null = null;
  let channelVersion = 0;

  function send(payload: unknown): void {
    ch?.send(payload);
  }

  function close(code?: string, reason?: string): void {
    channelVersion += 1;
    ch?.close(code, reason);
    ch = null;
    connected.value = false;
  }

  function openChannel(topicValue: string): void {
    const currentVersion = channelVersion + 1;
    channelVersion = currentVersion;
    ch?.close();
    ch = null;
    error.value = undefined;
    connected.value = false;

    const bridge = window.LingXiaBridge;
    if (!bridge?.channel?.open) return;

    bridge.channel
      .open<TData>(topicValue, options?.params)
      .then((c) => {
        if (channelVersion !== currentVersion) {
          c.close();
          return;
        }
        ch = c;
        connected.value = true;

        c.on("data", (payload: TData) => {
          if (channelVersion !== currentVersion) return;
          data.value = payload;
        });
        c.on("close", () => {
          if (channelVersion !== currentVersion) return;
          ch = null;
          connected.value = false;
        });
        c.on("error", (err: LxBridgeError) => {
          if (channelVersion !== currentVersion) return;
          ch = null;
          error.value = err;
          connected.value = false;
        });
      })
      .catch((err: unknown) => {
        if (channelVersion !== currentVersion) return;
        error.value = toBridgeError(
          err,
          "CHANNEL_OPEN_FAILED",
          "Failed to open channel",
        );
        connected.value = false;
      });
  }

  const topicRef = typeof topic === "string" ? ref(topic) : topic;
  watch(
    [topicRef, () => getParamsSignature(options?.params)],
    ([topicValue]) => openChannel(topicValue),
    { immediate: true },
  );

  onUnmounted(() => {
    channelVersion += 1;
    ch?.close();
    ch = null;
  });

  return { data, error, connected, send, close };
}
