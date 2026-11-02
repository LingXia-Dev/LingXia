# GPU showcase

Android host with an embedded lxapp whose pages all render through the native
GPU renderer. The React showcase in `../lingxia-showcase` is the functional
reference; this project mirrors its pages, not its host.

```sh
cd lxapp && npm install && cd ..
lingxia dev -p android --release
```

Every page renders natively because `lxapp/lxapp.json` declares `"renderer": "gpu"`;
a page's View is `index.tsx` beside the Logic in `index.ts`. Shared View code lives in
`lxapp/shared/ui` (the design system) and `lxapp/shared` (charts, types).

Checks that need no device:

```sh
cd lxapp && npm run typecheck
node --experimental-strip-types --test ../tests/*.test.mjs
```

Device probes and renderer fixtures live with the Android SDK tests in
`lingxia-sdk/android/tests/gpu`; they are not part of the app bundle.
`tests/PARITY.md` tracks page parity with the React reference.
