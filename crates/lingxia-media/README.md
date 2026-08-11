# lingxia-media

Shared media playback abstractions for LingXia.

## What it provides

- `StreamProvider` and `StreamSession` traits
- `FrameSink` for pushing decoded audio/video frames into platform decoders
- Global provider registration/lookup helpers
- Stream seek callback registration

## Primary module

- `playback`: runtime-neutral video/audio streaming interfaces used by higher-level
  playback integrations

## Notes

This crate defines playback contracts. Device capture and other input concerns
belong to device I/O; concrete playback, decoder, and platform implementations
live in platform/runtime crates.
