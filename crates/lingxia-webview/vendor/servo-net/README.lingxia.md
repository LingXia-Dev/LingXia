This is `servo-net` 0.5.0 with two narrow embedder hooks. Servo's public
embedding API exposes request interception, while response, failure, and
download data remain internal to the network/devtools pipeline.

- `NetworkObserver` forwards existing request, response, body, and failure
  data for diagnostics without changing fetch behavior.
- `NavigationObserver` reports a top-level navigation's network error and
  lets the embedder claim a top-level response as a download; a claimed fetch
  is cancelled so the current document stays.

Keep these patches narrow so they can be replaced by upstream APIs when Servo
exposes them.
