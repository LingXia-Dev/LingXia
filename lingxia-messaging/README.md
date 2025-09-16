# Lingxia Messaging

A crate for handling asynchronous messaging and events within the LingXia project.
It provides mechanisms for communication between different parts of the application,
such as between the Rust core and platform-native UI threads.

## Core Patterns

This crate is designed to support two primary messaging patterns:

### 1. Request-Response (Callback)

This pattern is used for one-time asynchronous operations where a result is expected.
A temporary callback is registered, and it is invoked once with the result of the operation.

**Example:**
```rust
// In the Rust core, preparing to call a platform-native function
let (callback_id, receiver) = lingxia_messaging::get_callback();

// Pass `callback_id` to the platform...

// Wait for the result
let result = receiver.await;


// In the platform-native FFI layer, when the operation is complete
lingxia_messaging::invoke_callback(callback_id, true, "some data".to_string());
```

### 2. Publish-Subscribe (Event Bus) - *Planned*

This crate will be expanded to include a persistent event bus. This will allow different
parts of the application to subscribe to named events (e.g., `location_change`) and receive
updates whenever those events are published.
