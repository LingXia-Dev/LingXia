# Lingxia Messaging

A crate for handling asynchronous messaging and events within the LingXia project.
It provides mechanisms for communication between different parts of the application,
such as between the Rust core and platform-native UI threads.

## Core Patterns

This crate is designed to support two primary messaging patterns:

### 1. Flexible Callback System

This pattern supports oneshot, stream, and handler callbacks:
- **Oneshot callbacks**: Used for one-time asynchronous operations where a result is expected.
  A temporary callback is registered, and it is invoked once with the result of the operation.
  After invocation, the callback is automatically removed.
- **Stream callbacks**: Used for continuous data flow where multiple results may be sent
  over time. The callback remains active for multiple invocations until explicitly removed.
- **Handler callbacks**: Register a function once and have it invoked directly when
  `invoke_callback(id, ...)` is called. Useful when you want immediate dispatch (no receiver
  polling) and will manage unregistration manually.

All callback types use unified string payloads for simplicity and flexibility.

Example (handler callback):
```rust
use lingxia_messaging::{register_handler, invoke_callback, remove_callback};

let callback_id = register_handler(|result| {
    println!("Got callback: success={}, data={}", result.success, result.data);
});

// Somewhere else (e.g., platform thread):
let _ = invoke_callback(callback_id, true, "{\"foo\":\"bar\"}");

// When done:
let _ = remove_callback(callback_id);
```

Handler threading and return semantics:
- Handlers execute on the thread that calls `invoke_callback`; schedule onto your runtime inside the handler if needed.
- `invoke_callback` returns `false` when the ID is unknown, the channel is closed, the stream channel is full, or a handler panicked (handler is removed on panic). It returns `true` on successful delivery. Add logging on the caller side if you need visibility into failures.

### 2. Publish-Subscribe (Event Bus)

This crate includes a persistent event bus. This allows different parts of the application
to subscribe to named events (e.g., `location_change`) and receive updates whenever those
events are published.
