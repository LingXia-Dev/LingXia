# GPU product reorganization validation

The GPU work is reorganized on `origin/main` at
`e1bed14edb43fb47a03b5124494a800e611ce22a`, retaining the current 0.28.0 runtime
contracts. The implementation is split into renderer/authoring, runtime/Android
integration, independent showcase/device tools, and quality gates/documentation.
Native navbar/tabbar remain host-owned. Unrelated capture work is excluded.

The six TSX review findings are fixed, including transactional empty components,
single-child forwarding, duplicate local diagnostics, readonly results/messages,
JSON local types, and precise component styling slots. `List` derives its native
inventory from typed items and keys. See `ui-authoring.md` for the contract.

## Main integration checks

- Retained main's network-owner attribution and download-kind tagging while
  sharing authorization and cancellation between Logic and GPU image loading.
- Removed obsolete Vue/build-field assumptions and aligned example minimum
  runtime and package versions to 0.28.0.
- Registered the UI crate in dependency-layer and release-inventory checks.
- Removed the example's machine-specific cloud dependency; cloud remains an
  explicitly injected provider. General API/tab tests use an unconditional group.
- Moved recovery text into generated Android i18n inputs. Fresh SDK builds need
  the normal icon/i18n bootstrap; generated resources are not source artifacts.
- Device tools accept SDK/device configuration. Probe dex compilation uses an
  input archive to avoid Windows command-line limits in longer checkout paths.

## Verification

All results below are from the reorganized sources, not the earlier branch:

| Gate | Result |
|---|---|
| UI authoring/compiler/runtime JS | 198 passed; subsequent local-type refinement checked again |
| Showcase View and Logic TypeScript | Passed |
| Showcase behavior/parity | 34 passed |
| Rust UI / lxapp / transfer library tests | 303 / 411 / 30 passed |
| Workspace all-target check | Passed |
| CLI GPU integration tests | 3 passed |
| Windows SDK browser-shell clippy | Passed |
| Android SDK unit tests | 300 passed, no skips |
| Crate layers and release inventory | Passed |
| Release bundle | 21 GPU modules; native compatibility manifests retained |
| Physical Pixel 3, Android 12 core suite | Home, ToDo, images, tab switching, font scale: all passed |
| Physical device instrumentation | Product contracts and session recovery: 2 passed |
| Tab recording | 47 switches; 363 decoded frames; zero black-content frames |

The device contract test includes real HTTP image cancellation/retry, keyed row
state, list edge geometry and recovery after a failed session. Font scaling was
restored to 1.0. Normal tab-switch logs contained no UI update/compatibility/local
state errors. The Home screenshot was inspected after the recording.

The final APK is versionCode 2027 / versionName 0.2.27. Its packaged native library
matches the freshly built library. Final acceptance artifacts are stored locally
under `target/gpu-core-2027-acceptance` and `target/gpu-reorg-*`.

- APK SHA-256: `0bd369a784f434c284e49971af079326ab9df628c2c997e72ccb29eefacf637c`
- Native SHA-256: `796851dadd0a94ca0a9e2bc3a078d985d5ea3b42d942fdf413fc01ea29bc365c`
- Bundle integrity file SHA-256: `274e1b000753bf9df621a7d152ccd1442decb6f382786654a16f5dfd9b61d1f0`

An initial staged APK retained a removed host capability field and was rejected
by main's configuration validator; the final artifact uses the current manifest's
capabilities. Earlier failed runs are not counted as acceptance.

## Scope

The recording checks black content, not a frame-pacing performance target.
This run does not certify TalkBack speech, private cloud services or every device
API. iOS/Harmony GPU host adapters and the excluded native-component gallery
remain outside this implementation; shared renderer code does not imply those
adapters are implemented. An independent architecture re-review found no new
integration blockers, but did not substitute for the checks above.
