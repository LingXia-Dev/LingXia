# LingXia Bridge Protocol Specification

This document is the **normative specification** for the LingXia bridge protocol and the **single source of truth** for protocol behavior in this repository.

---

## Table of Contents

1. [Overview](#1-overview)
2. [Terminology](#2-terminology)
3. [Protocol Fundamentals](#3-protocol-fundamentals)
4. [Session Lifecycle](#4-session-lifecycle)
5. [LXRPC: Remote Procedure Calls](#5-lxrpc-remote-procedure-calls)
6. [LXS: State Synchronization](#6-lxs-state-synchronization)
7. [Capability Model](#7-capability-model)
8. [Error Codes](#8-error-codes)
- [Appendix A: View SDK Conventions](#appendix-a-view-sdk-conventions)
- [Appendix B: Future Extensions](#appendix-b-future-extensions)

---

## 1. Overview

### 1.1 Scope

This spec defines the **on‑wire protocol** between Logic and View runtimes: handshake, LXRPC messages, LXS state sync, and capability rules. It does **not** define UI APIs or product‑level behaviors.

### 1.2 Architecture (Informative)

```
┌─────────────────────┐                      ┌─────────────────────┐
│     Logic Layer     │                      │     View Layer      │
│   (AppService JS)   │◄────── Bridge ──────►│     (WebView)       │
│                     │                      │                     │
│  - Business logic   │      LXRPC           │  - UI rendering     │
│  - Page handlers    │      LXS             │  - User interaction │
│  - Host API calls   │                      │  - DOM management   │
└─────────────────────┘                      └─────────────────────┘
```

### 1.3 Normative Language

The key words **MUST**, **MUST NOT**, **SHOULD**, **SHOULD NOT**, and **MAY** are interpreted as described in RFC 2119.

---

## 2. Terminology

| Term | Definition |
|------|------------|
| **Logic Layer** | The JavaScript runtime executing business logic (AppService), separate from the WebView |
| **View Layer** | The WebView rendering UI, receiving state updates from Logic |
| **Transport** | The mechanism for crossing the native boundary (MessagePort, WebKit handler, JS interface) |
| **LXRPC** | LingXia Remote Procedure Call — request/response and notification semantics |
| **LXS** | LingXia State Sync — state synchronization via snapshots and patches |
| **Capability (cap)** | A permission category mapped from method names (e.g., `host`, `page`) |
| **Nonce** | A per-session secret used to authenticate handshake messages |

---

## 3. Protocol Fundamentals

### 3.1 Layers

The protocol separates concerns into distinct layers:

| Layer | Responsibility |
|-------|----------------|
| **Transport** | Byte/object delivery across native boundary |
| **LXRPC** | Request/response, notifications, cancellation |
| **LXS** | State synchronization with ordering and backpressure |
| **Capability** | Authorization model (delegated to host app) |

All transports MUST preserve **message ordering per logical channel** (FIFO).

### 3.2 Message Envelope

Every message MUST include:

```json
{
  "v": 2,
  "kind": "<message-type>",
  ...
}
```

| Field | Type | Description |
|-------|------|-------------|
| `v` | integer | Protocol version. MUST be `2` |
| `kind` | string | Message type identifier |

**Extensibility rules:**
- Receivers MUST ignore unknown top-level fields (forward compatibility)
- Receivers MUST reject unknown `kind` values with `BRIDGE_MALFORMED_MESSAGE` when a response is expected

### 3.3 Required Fields by Kind

All messages MUST include `v` and `kind`. The table below lists additional required fields:

| Kind | Required Fields |
|------|-----------------|
| `hello` | `nonce`, `role`, `protocolsSupported` |
| `helloAck` | `nonce`, `protocol`, `sessionId` |
| `ready` | `sessionId` |
| `req` | `id`, `method`, `cap` |
| `res` | `id`, `ok` (and if `ok:false` → `error.code`) |
| `notify` | `method`, `cap` |
| `cancel` | `id` |
| `state.snapshot` | `rev`, `state` |
| `state.patch` | `baseRev`, `rev`, `ops` |
| `state.ack` | `rev` |

Optional but common:
- `params` for `req`/`notify`
- `result` for `res` when `ok:true`
- `scope` for `state.*`
- `ack` for `state.patch`
- `trace` fields for observability

### 3.4 Message Types Summary

| Kind | Direction | Response | Description |
|------|-----------|----------|-------------|
| `hello` | Either | `helloAck` | Initiate handshake |
| `helloAck` | Either | — | Acknowledge handshake |
| `ready` | Either | — | Signal ready for messages |
| `req` | Either | `res` | Request with guaranteed response |
| `res` | Either | — | Response to `req` |
| `notify` | Either | — | Fire-and-forget notification |
| `cancel` | Either | — | Best-effort cancellation |
| `state.snapshot` | Logic→View | — | Full state replacement |
| `state.patch` | Logic→View | `state.ack` | Incremental state update |
| `state.ack` | View→Logic | — | Acknowledge patch receipt |

---

## 4. Session Lifecycle

### 4.1 Handshake Flow

Before any application messages, both sides MUST complete a handshake:

```
View                                    Logic
  │                                       │
  │─────────── hello ────────────────────►│
  │                                       │
  │◄──────── helloAck ────────────────────│
  │                                       │
  │◄─────────  ready ─────────────────────│
  │                                       │
  │         [Bridge Ready]                │
  │                                       │
```

### 4.2 Nonce Authentication

To prevent spoofing on weak transports, sessions MUST use a **per-session nonce**:

- MUST be unguessable (≥128 bits of entropy)
- MUST be unique per WebView/session
- MUST be supplied out-of-band by the native embedding layer
- MUST NOT be readable by untrusted third-party content

**Policy note (non-normative):** in development builds the nonce MAY be omitted by configuration. In production builds, the receiver SHOULD require a non-empty nonce and reject mismatches.

### 4.3 Pre-Ready Behavior

Until `ready` is received, a runtime MUST either:
- **Queue** outbound messages (bounded queue), OR
- **Reject immediately** with `BRIDGE_NOT_READY`

It MUST NOT "fake" business timeouts before the bridge is ready.

### 4.4 Handshake Messages

#### `hello`

```json
{
  "v": 2,
  "kind": "hello",
  "nonce": "<string>",
  "role": "view|logic",
  "protocolsSupported": [2]
}
```

#### `helloAck`

```json
{
  "v": 2,
  "kind": "helloAck",
  "nonce": "<string>",
  "protocol": 2,
  "sessionId": "<string>"
}
```

#### `ready`

```json
{
  "v": 2,
  "kind": "ready",
  "sessionId": "<string>"
}
```

Receiver MUST validate that `sessionId` matches the most recent `helloAck`.

---

## 5. LXRPC: Remote Procedure Calls

### 5.1 Request (`req`)

A request expects exactly one response.

```json
{
  "v": 2,
  "kind": "req",
  "id": "<unique-id>",
  "method": "<method-name>",
  "params": "<json>",
  "cap": "<capability>"
}
```

| Field | Required | Description |
|-------|----------|-------------|
| `id` | Yes | Unique request identifier |
| `method` | Yes | Method to invoke |
| `params` | No | Any JSON value (SHOULD be an object) |
| `cap` | Yes | Capability category |

### 5.2 Response (`res`)

```json
{
  "v": 2,
  "kind": "res",
  "id": "<matching-id>",
  "ok": true,
  "result": { }
}
```

Or on failure:

```json
{
  "v": 2,
  "kind": "res",
  "id": "<matching-id>",
  "ok": false,
  "error": {
    "code": "<error-code>",
    "message": "<optional-message>",
    "data": { },
    "retryable": false
  }
}
```

**Key semantic:** `res` represents the **final outcome**, not just "accepted".

### 5.3 Notification (`notify`)

Fire-and-forget message with no response.

```json
{
  "v": 2,
  "kind": "notify",
  "method": "<method-name>",
  "params": "<json>",
  "cap": "<capability>"
}
```

### 5.4 Cancellation (`cancel`)

Best-effort cancellation of an in-flight request.

```json
{
  "v": 2,
  "kind": "cancel",
  "id": "<request-id>"
}
```

- Receiver SHOULD attempt cancellation
- Sender MUST still expect a terminal `res` (typically with `BRIDGE_CANCELED`)

---

## 6. LXS: State Synchronization

LXS handles state synchronization between Logic and View layers, separate from LXRPC.

### 6.1 Design Principles

- State sync is **unidirectional** (Logic → View)
- Deletions MUST be explicitly representable
- Updates are **order-checked** via `rev`/`baseRev`
- Backpressure via explicit acknowledgment

### 6.2 Snapshot (`state.snapshot`)

Full state replacement.

```json
{
  "v": 2,
  "kind": "state.snapshot",
  "scope": "<optional-scope>",
  "rev": 42,
  "state": { }
}
```

### 6.3 Patch (`state.patch`)

Incremental update using JSON Patch (RFC 6902) subset: `add`, `remove`, `replace`.

```json
{
  "v": 2,
  "kind": "state.patch",
  "scope": "<optional-scope>",
  "baseRev": 42,
  "rev": 43,
  "ops": [
    { "op": "replace", "path": "/loading", "value": false },
    { "op": "remove", "path": "/error" }
  ],
  "ack": true
}
```

**Rules:**
- `rev` MUST be strictly increasing per `scope`
- `baseRev` MUST equal receiver's current `rev`
- `remove` MUST be used for deletions (no `undefined` semantics)

### 6.4 Acknowledgment (`state.ack`)

```json
{
  "v": 2,
  "kind": "state.ack",
  "scope": "<optional-scope>",
  "rev": 43
}
```

If `state.patch.ack` is `true`, receiver MUST send `state.ack` after applying.

### 6.5 Recovery

If receiver detects `baseRev` mismatch:
1. Stop applying further patches for that `scope`
2. Request `state.snapshot` via LXRPC method `state.getSnapshot`

#### `state.getSnapshot` (LXRPC)

Request:

```json
{ "v": 2, "kind": "req", "id": "<id>", "method": "state.getSnapshot", "params": { "scope": "<optional>" }, "cap": "state" }
```

Response `res.ok:true`:

```json
{ "rev": 42, "state": { }, "scope": "<optional>" }
```

### 6.6 Backpressure

Senders MUST bound in-flight unacked patches per `scope`:
- Recommended max: 8 unacked patches
- If exceeded: coalesce into a fresh `state.snapshot`

---

## 7. Capability Model

### 7.1 Capability Mapping

Every `method` maps to exactly one capability string:

| Method Pattern | Capability |
|----------------|------------|
| `host.*` | `"host"` |
| `xxx.yyy` | `"xxx"` (first segment) |
| Other | `"page"` |

### 7.2 Validation Rules

- `req`, `notify` MUST carry a `cap` field
- Receiver MUST derive required capability from `method`
- If `cap` doesn't match required capability → `BRIDGE_MALFORMED_MESSAGE`

### 7.3 Authorization

**Capability enforcement is delegated to the host app.**

Default behavior is allow-all unless the host config supplies an explicit allowlist. Host app `HostHandler` implementations are responsible for:
- User authorization prompts
- App signature verification
- Policy enforcement

Host APIs are inherently whitelisted—only explicitly registered handlers can be invoked.

---

## 8. Error Codes

Bridge-level error codes are stable and MUST NOT change semantics:

| Code | Description |
|------|-------------|
| `BRIDGE_NOT_READY` | Bridge handshake not complete |
| `BRIDGE_TIMEOUT` | Request timed out |
| `BRIDGE_CANCELED` | Request was canceled |
| `BRIDGE_PROTOCOL_MISMATCH` | Unsupported protocol version |
| `BRIDGE_HANDSHAKE_FAILED` | Handshake failed |
| `BRIDGE_MALFORMED_MESSAGE` | Invalid message format |
| `BRIDGE_METHOD_NOT_FOUND` | Method not registered |
| `BRIDGE_CAPABILITY_DENIED` | Host app denied the call |
| `BRIDGE_INTERNAL_ERROR` | Unexpected internal error |
| `BRIDGE_OUTBOX_FULL` | Message queue overflow |

Domain methods MAY define additional namespaced codes (e.g., `NAV_INVALID_URL`).

---

## Appendix A: View SDK Conventions

*This section is non-normative.*

The View layer SDK exposes:

```typescript
// Request with response
window.LingXiaBridge.call(method, params?, options?): Promise<result>

// Fire-and-forget
window.LingXiaBridge.notify(method, params?, options?): void

// State subscription
window.LingXiaBridge.subscribe((data, { rev, initial }) => void): () => void
```

**CLI-generated page functions** (`useLingXia()`) use `notify` for page method calls. Business results are expressed via state (LXS) rather than `req/res` return values.

**TypeScript support:**
- Augment `LingXiaBridgeMethodMap` for typed `call()` methods
- Use `useLingXia<TData, TActions>()` for typed hooks

---

## Appendix B: Future Extensions

### B.1 Subscriptions (`sub`/`event`/`unsub`)

Push stream subscriptions are reserved for future implementation. Message kinds `sub`, `event`, and `unsub` are reserved but not yet specified.

---
