# Lingxia Messaging

A crate for handling asynchronous messaging and events within the LingXia project.
It provides mechanisms for communication between different parts of the application,
such as between the Rust core and platform-native UI threads.

## Core Patterns

This crate is designed to support two primary messaging patterns:

### 1. Flexible Callback System

This pattern supports both oneshot and stream callbacks:
- **Oneshot callbacks**: Used for one-time asynchronous operations where a result is expected.
  A temporary callback is registered, and it is invoked once with the result of the operation.
  After invocation, the callback is automatically removed.
- **Stream callbacks**: Used for continuous data flow where multiple results may be sent
  over time. The callback remains active for multiple invocations until explicitly removed.

Both callback types use unified string payloads for simplicity and flexibility.

### 2. Publish-Subscribe (Event Bus)

This crate includes a persistent event bus. This allows different parts of the application
to subscribe to named events (e.g., `location_change`) and receive updates whenever those
events are published.
