# GPU showcase

Independent native host and embedded GPU lxapp. React showcase remains the
read-only functional reference in `../lingxia-showcase`.

```powershell
cd lxapp
npm install
cd ..
lingxia dev -p android --release --name showcase-gpu
```

Every page declares `renderer: "gpu"` and compiles its `.ui.tsx` View.
From the repository root, build workspace JS packages before the first build
(`cd packages; npm ci; npm run build --workspaces --if-present`).

Android uses `com.lingxia.example.gpu` (`.dev` in development), separate from
the React example's package and storage. Platform host sources originate from
the existing showcase; Android is the current device verification target.

`lxapp/` contains only the mini-app's TS/JS, configuration and assets.
Android automation probes, verification scripts and fixtures live separately
under [the Android SDK tests](../../lingxia-sdk/android/tests/gpu/README.md);
they are not app code or bundled resources.

See `tests/PARITY.md` for migration coverage and pending acceptance checks.
For parity tests, install the React reference's lxapp dependencies too; the chart
tests compare against its ECharts implementation.
