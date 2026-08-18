# lingxia-media

Shared media playback and realtime-capture abstractions for LingXia.

## Features

- `playback` (default): `StreamProvider` / `StreamSession` / `FrameSink`
- `capture`: multi-track realtime pipeline over an explicit `CaptureProviderSet`

`capture` depends on `lingxia-platform/capture-contract` and must not depend on
`lingxia-device-io`. Enabling the contract never enables a concrete native
provider.

## Modules

- `playback`: runtime-neutral video/audio streaming interfaces
- `capture`: authorization-aware multi-track capture, generations, backpressure

Consumers that do not use playback can set `default-features = false`.
