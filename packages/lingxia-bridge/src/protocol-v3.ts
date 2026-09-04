import type {} from "./types";

/** Document-side framing for host-attested RequiredV3 documents. */

export const V3_PROTOCOL = 3 as const;
export const DEFAULT_MAX_V3_FRAME_BYTES = 64 * 1024;

export type V3DocumentToNativeKind =
  | "hello"
  | "req"
  | "res"
  | "notify"
  | "cancel"
  | "ch.open"
  | "ch.data"
  | "ch.close"
  | "state.ack"
  | "console";

export type V3NativeToDocumentKind =
  | "helloAck"
  | "ready"
  | "req"
  | "res"
  | "event"
  | "state.snapshot"
  | "state.patch"
  | "ch.ack"
  | "ch.data"
  | "ch.close";

const documentToNativeKinds = new Set<V3DocumentToNativeKind>([
  "hello",
  "req",
  "res",
  "notify",
  "cancel",
  "ch.open",
  "ch.data",
  "ch.close",
  "state.ack",
  "console",
]);

const nativeToDocumentKinds = new Set<V3NativeToDocumentKind>([
  "helloAck",
  "ready",
  "req",
  "res",
  "event",
  "state.snapshot",
  "state.patch",
  "ch.ack",
  "ch.data",
  "ch.close",
]);

export type V3CodecError =
  | "FRAME_TOO_LARGE"
  | "MALFORMED_ENVELOPE"
  | "UNSUPPORTED_VERSION"
  | "UNSUPPORTED_NATIVE_KIND"
  | "INVALID_DOCUMENT_BINDING"
  | "INVALID_DOCUMENT_PAYLOAD"
  | "SECURITY_FIELD_IN_PAYLOAD"
  | "SESSION_MISMATCH"
  | "UNEXPECTED_SECRET";

export type V3CodecResult<T> =
  | { readonly ok: true; readonly value: T }
  | { readonly ok: false; readonly error: V3CodecError };

export type V3DocumentBinding = {
  readonly sessionId: string;
  readonly secret: string;
};

export type V3NativeEnvelope = {
  readonly kind: V3NativeToDocumentKind;
  readonly payload: Readonly<Record<string, unknown>>;
};

export type V3DocumentCodec = {
  encode(
    kind: V3DocumentToNativeKind,
    payload: Record<string, unknown>,
  ): V3CodecResult<Record<string, unknown>>;
  parse(frame: string, maxFrameBytes?: number): V3CodecResult<V3NativeEnvelope>;
};

export type V3BootstrapActivation =
  | { readonly kind: "absent" }
  | { readonly kind: "required"; readonly codec: V3DocumentCodec }
  | { readonly kind: "blocked" };

function isRecord(value: unknown): value is Record<string, unknown> {
  return value !== null && typeof value === "object" && !Array.isArray(value);
}

function hasDuplicateTopLevelSecurityKey(frame: string): boolean {
  const securityKeys = new Set(["v", "kind", "sessionId", "secret"]);
  const seen = new Set<string>();
  let depth = 0;

  for (let index = 0; index < frame.length; index++) {
    const char = frame[index];
    if (char === "{" || char === "[") {
      depth++;
      continue;
    }
    if (char === "}" || char === "]") {
      depth--;
      continue;
    }
    if (char !== '"') continue;

    const start = index;
    index++;
    while (index < frame.length) {
      if (frame[index] === "\\") {
        index += 2;
        continue;
      }
      if (frame[index] === '"') break;
      index++;
    }
    if (index >= frame.length) return false;
    if (depth !== 1) continue;

    let next = index + 1;
    while (/\s/.test(frame[next] ?? "")) next++;
    if (frame[next] !== ":") continue;
    let key: unknown;
    try {
      key = JSON.parse(frame.slice(start, index + 1));
    } catch {
      return false;
    }
    if (typeof key === "string" && securityKeys.has(key)) {
      if (seen.has(key)) return true;
      seen.add(key);
    }
  }
  return false;
}

/**
 * Captures the document secret in a closure. Neither the returned codec nor
 * parsed native messages expose it.
 */
export function createV3DocumentCodec(binding: V3DocumentBinding): V3CodecResult<V3DocumentCodec> {
  if (
    !isRecord(binding) ||
    typeof binding.sessionId !== "string" ||
    binding.sessionId.length === 0 ||
    typeof binding.secret !== "string" ||
    binding.secret.length === 0
  ) {
    return { ok: false, error: "INVALID_DOCUMENT_BINDING" };
  }

  const { sessionId, secret } = binding;
  return {
    ok: true,
    value: {
      encode(kind, payload) {
        if (!documentToNativeKinds.has(kind) || !isRecord(payload)) {
          return { ok: false, error: "INVALID_DOCUMENT_PAYLOAD" };
        }
        if (["v", "kind", "sessionId", "secret"].some((field) => field in payload)) {
          return { ok: false, error: "SECURITY_FIELD_IN_PAYLOAD" };
        }
        return {
          ok: true,
          value: { ...payload, v: V3_PROTOCOL, kind, sessionId, secret },
        };
      },
      parse(frame, maxFrameBytes = DEFAULT_MAX_V3_FRAME_BYTES) {
        // Envelope checks deliberately precede typed payload handling.
        if (new TextEncoder().encode(frame).byteLength > maxFrameBytes) {
          return { ok: false, error: "FRAME_TOO_LARGE" };
        }
        if (hasDuplicateTopLevelSecurityKey(frame)) {
          return { ok: false, error: "MALFORMED_ENVELOPE" };
        }

        let candidate: unknown;
        try {
          candidate = JSON.parse(frame);
        } catch {
          return { ok: false, error: "MALFORMED_ENVELOPE" };
        }
        if (!isRecord(candidate)) return { ok: false, error: "MALFORMED_ENVELOPE" };
        if (candidate.v !== V3_PROTOCOL) {
          return { ok: false, error: "UNSUPPORTED_VERSION" };
        }
        if (
          typeof candidate.kind !== "string" ||
          !nativeToDocumentKinds.has(candidate.kind as V3NativeToDocumentKind)
        ) {
          return { ok: false, error: "UNSUPPORTED_NATIVE_KIND" };
        }
        if (typeof candidate.sessionId !== "string" || candidate.sessionId.length === 0) {
          return { ok: false, error: "MALFORMED_ENVELOPE" };
        }
        if (candidate.sessionId !== sessionId) return { ok: false, error: "SESSION_MISMATCH" };
        if (Object.prototype.hasOwnProperty.call(candidate, "secret")) {
          return { ok: false, error: "UNEXPECTED_SECRET" };
        }

        const { v: _v, kind: _kind, sessionId: _sessionId, ...payload } = candidate;
        return {
          ok: true,
          value: { kind: candidate.kind as V3NativeToDocumentKind, payload },
        };
      },
    },
  };
}

/**
 * Atomically takes the host-installed one-shot handoff. The secret remains in
 * the returned codec closure and is never copied into bridge globals/config.
 */
export function consumeV3Bootstrap(): V3BootstrapActivation {
  if (typeof window === "undefined") return { kind: "absent" };
  const take = window.__LingXiaTakeControlBootstrap;
  if (typeof take !== "function") return { kind: "absent" };

  try {
    const bootstrap = take();
    if (
      !bootstrap ||
      bootstrap.requiredProtocol !== V3_PROTOCOL ||
      typeof bootstrap.publicSessionId !== "string" ||
      bootstrap.publicSessionId.length === 0 ||
      typeof bootstrap.secret !== "string" ||
      bootstrap.secret.length === 0
    ) {
      return { kind: "blocked" };
    }
    const codec = createV3DocumentCodec({
      sessionId: bootstrap.publicSessionId,
      secret: bootstrap.secret,
    });
    return codec.ok
      ? { kind: "required", codec: codec.value }
      : { kind: "blocked" };
  } catch {
    return { kind: "blocked" };
  } finally {
    // A malformed replacement must not leave a callable handoff for later.
    try {
      delete window.__LingXiaTakeControlBootstrap;
    } catch {
      // A hostile non-configurable property has no attested secret to retain.
    }
  }
}
