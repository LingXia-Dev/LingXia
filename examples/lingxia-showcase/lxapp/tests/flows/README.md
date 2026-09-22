# Browser control navigation regression

`browser-control-navigation.test.ts` exercises the host's real Settings/Downloads
links through `t.automation.browser`. It checks trusted RPC after every click,
keeps the same tab, checks document visibility, and reloads once at the end.
It uses the control app's `lx.shell.openBuiltin('downloads')` entrypoint; it does
not bypass the native authority restriction on `browser.open('lingxia://…')`.

Start the host with `lingxia dev`, then from the showcase lxapp directory (with
its npm dependencies installed):

```sh
lxdev --session <id> test tests/flows/browser-control-navigation.test.ts --verbose
```

The defaults match Fusheng. For another host, pass `--arg forwardSelector=…`,
`--arg backSelector=…`, `--arg fromUrl=…`, `--arg toUrl=…`, `--arg rpc=…`, and
optionally `--arg cycles=10`. The selectors must identify real product links.
Do not inject synthetic links or replace the clicks with repeated host opens:
that misses the competing renderer-navigation path from #460.

The host must be foregrounded. On desktop, run
`lxdev desktop window focus --window <id>` using the ID from `lxdev desktop windows`
before starting the test. This local CLI action does not require the optional
in-process desktop automation feature in the host. On Android, ADB connectivity alone is not
readiness: wake the device, unlock it normally, verify `mWakefulness=Awake`
in `adb -s <serial> shell dumpsys power`, and check that the host package has
`mCurrentFocus` and `mDreamingLockscreen=false` in `dumpsys window`. A powered-off
or locked device cannot count as a UI pass. The spec additionally requires the
browser document to be visible.

Android currently has no platform implementation of `open_builtin_browser_page`.
This exact product flow therefore fails at setup there; it is not silently
skipped or counted as passed. Android's queued trusted-loader regression is
covered separately by `android_document` and normalizer unit tests, Android
SDK tests, and an actual Android build. Running this journey on Android requires
a supported native builtin entrypoint in the host/platform first.

Reports include step timings, per-transition RPC evidence, and failure forensics
under the normal `lxdev test` output directory. A rejected or timed-out bridge
RPC fails the run even when the document is visible and fully loaded.
