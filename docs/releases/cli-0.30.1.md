# LingXia 0.30.1

A CLI and Runner patch; the SDKs and crates stay at 0.30.0.

## CLI

- Projects that pin a LingXia commit (`lingxia.lock`) build the Android and Harmony SDKs from that source again on Windows. The build ran `npm`, which Windows only has as `npm.cmd`, and failed with `run npm in …\.lingxia\lingxia-src\packages: program not found`.

## Runner

- On macOS a mouse wheel no longer triggers pull-down refresh; use the trackpad pull, ⌘R or the context menu.
- The macOS sidebar uses larger text: 15pt page rows and 14pt group headers.
