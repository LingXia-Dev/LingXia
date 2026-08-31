This is `servo-net` 0.5.0 with a minimal read-only network observer for
embedders. Servo's public embedding API currently exposes request interception,
while its response and failure data remain internal to the network/devtools
pipeline.

The observer forwards existing request, response, body, and failure data without
changing fetch behavior. Keep this patch narrow so it can be replaced by an
upstream API when Servo exposes one.
