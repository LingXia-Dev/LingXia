# lingxia-media

Shared media streaming abstractions for LingXia.

## What it provides

- `StreamProvider` and `StreamSession` traits
- `FrameSink` for pushing decoded audio/video frames into platform decoders
- Global provider registration/lookup helpers
- Stream seek callback registration

## Primary module

- `video`: runtime-neutral video/audio streaming interfaces used by higher-level
  playback integrations

## Notes

This crate defines shared contracts. Concrete playback, decoder, and platform
implementations live in platform/runtime crates.
