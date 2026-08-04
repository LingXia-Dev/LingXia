# Showcase automation contracts

These tests validate LingXia's public behavior, not the implementation of the
Showcase. A passing suite must show that a public capability is present, works
through its real boundary, reports failures predictably, and leaves no state
behind.

## Suite boundaries

| Entry | Purpose | Run on |
|---|---|---|
| `shared.test.ts` | All cross-platform API, Logic, Bridge, page, component, and render contracts | every platform and framework |
| `windows.test.ts` | Shared suite plus physical desktop and Windows-only behavior | Windows |
| `macos.test.ts` | Shared suite plus physical desktop and macOS-only behavior | macOS, local |
| `android.test.ts` | Shared suite; external Android system UI remains device-lab work | Android, local |

React and Vue use the same platform entry. The framework is a build argument,
not a separate test definition. `all.test.ts` remains a shared compatibility
entry and deliberately excludes physical platform tests.

CI runs `windows.test.ts` once for each framework. macOS and Android use their
matching thin platform entries locally. Shared cases must never be copied into
a platform entry.

## What every public capability needs

Use these coverage levels in order:

1. **Shape** — the member exists in the real runtime. `api/surface.test.ts`
   derives this inventory from `@lingxia/types/testing`; do not add a second
   hand-maintained API list.
2. **Semantic contract** — valid input, result shape/value, state transition,
   event ordering, and idempotency where applicable.
3. **Failure contract** — invalid input and denied authority reject with a
   stable error code/message and do not mutate state.
4. **Boundary contract** — Bridge serialization, DOM `CustomEvent`, native
   component, host surface, filesystem/network, or OS integration is exercised
   through the real boundary instead of a mock.
5. **Lifecycle contract** — cleanup, restart/relaunch, repeated calls,
   concurrent calls, and leak checks where the capability owns resources.

System-dialog APIs such as media/file pickers, sharing, Wi-Fi, and permissions
also require an external OS-UI case. Their JavaScript case owns the action and
postcondition; UIAutomator (Android) or the desktop AX driver owns the system
dialog. Do not fake those dialogs from page JavaScript.

## Case authoring rules

New contract cases use the thin helper in `support/contract.ts`:

- assign a stable domain ID and list the public members in `covers`;
- use the case `namespace` for storage keys, files, surfaces, and other mutable
  fixtures;
- register cleanup immediately with `defer`; cleanup runs LIFO even after an
  assertion failure;
- assert an observable public result, not a private host field;
- use `PageDriver.waitFor` or `eventually`; fixed sleeps are permitted only for
  a documented physical stabilization interval;
- retry only an expected transient readiness error. `eventually` propagates
  other exceptions unless `retryIf` explicitly classifies them;
- use stable `data-testid` selectors for page behavior;
- for rejected operations, assert the error code/message and unchanged state;
- keep one primary behavior per case so the report identifies the broken
  contract;
- never catch and discard an error merely to make a platform pass.

The helper adds the stable ID to the report, captures page/window screenshots
on failure, and attaches `contract-coverage.json`. It intentionally does not
wrap navigation, pages, or platform drivers in a large DSL.

## Cross-platform policy

Portable tests contain no Windows/macOS/Android branch. Classify differences as:

1. **Same public contract:** one shared case, no platform condition.
2. **Documented contract difference:** one shared scenario fed by a small,
   table-driven capability profile that states `supported`, `absent`,
   `external-ui`, or another explicit outcome.
3. **Physical implementation difference:** a case under `platform/<name>/` or
   `platform/desktop/` using window, AX, pixel, process, or system-UI drivers.

An OS name must not be used as a proxy for privilege, permission, home-app
status, or a runtime capability. Probe the capability or pass an explicit
profile. Platform branches are acceptable inside physical drivers because
coordinate units and native window topology genuinely differ there.

## Running on Windows

From `examples/lingxia-showcase`:

```powershell
lingxia dev --background --platform windows --framework react
cd lxapp
npm run test:automation:windows:react
```

Stop the owning session from the Showcase directory with `lingxia dev stop`.
Use `vue` and `test:automation:windows:vue` for the frontend-adapter suite.

## Running on Android locally

Android requires `JAVA_HOME`, `ANDROID_SDK_ROOT`, `ANDROID_NDK_ROOT`, `adb` on
`PATH`, the Rust target matching the device (`aarch64-linux-android` or
`armv7-linux-androideabi`), and one ARM64/ARMv7 device or emulator. The CLI
currently rejects an x86_64-only emulator.

From the repository root:

```powershell
cargo build -p lingxia-cli -p lingxia-devtools-cli
./scripts/automation/run-android-showcase.ps1 -Framework all
```

Pass `-Device <adb-serial>` when more than one device is connected. The script
runs `lingxia doctor`, checks the selected ABI, then starts Android dev mode.
`lingxia dev` builds and installs the APK, configures `adb reverse`, launches
the host, and waits for the runtime websocket before `lxdev test` starts. The
default `all` mode runs React and Vue in separate sessions and retains test
artifacts plus session logs for both.

The Android JavaScript suite can drive Logic and page DOM and can take page/app
screenshots. It cannot operate permission dialogs, the photo picker, share
sheet, or other Android system UI. Add those actions to an external
UIAutomator/Appium device-lab suite, then return to `lxdev` for the app-state
assertion.
