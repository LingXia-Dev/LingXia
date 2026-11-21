# vello_gpu, vendored for LingXia

The crates.io package of vello_gpu 0.3.0 (linebender/vello `f3000c8d`, `vello_gpu/`) plus one
change, used through `[patch.crates-io]` in the workspace `Cargo.toml` and in each app workspace
that builds the View (`examples/lingxia-showcase/native/Cargo.toml`), until upstream takes it
(upstream proposal: not opened yet). Delete this directory and the patch entries once a vello_gpu
release carries it.

## Patches

`patches/` holds each as a diff against the package's `src/`.

1. `0001-pipeline-cache.patch` (`src/render/wgpu/mod.rs`): `Renderer::new_with_cache` takes an
   optional `wgpu::PipelineCache` and compiles every render pipeline through it; `new_with` passes
   `None`. A cache filled by an earlier launch lets the driver skip shader compilation, which is
   most of the renderer's creation on mobile GPUs. It is a parameter rather than a
   `RenderSettings` field because `RenderSettings` is `Copy` and shared with the WebGL backend.
