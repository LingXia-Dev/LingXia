import { copyFile, mkdir, writeFile } from 'node:fs/promises';

await mkdir(new URL('../dist/generated/', import.meta.url), { recursive: true });
await mkdir(new URL('../dist/esm/', import.meta.url), { recursive: true });
await Promise.all([
  copyFile(new URL('../src/logic-globals.d.ts', import.meta.url), new URL('../dist/logic-globals.d.ts', import.meta.url)),
  copyFile(new URL('../automation-test-globals.d.ts', import.meta.url), new URL('../dist/automation-test-globals.d.ts', import.meta.url)),
  copyFile(new URL('../src/generated/logic-web.d.ts', import.meta.url), new URL('../dist/generated/logic-web.d.ts', import.meta.url)),
]);

// The package itself is CommonJS (no top-level `"type"`), so the ESM pass in
// dist/esm/ needs its own scope marker for Node and every bundler to read those
// .js files as modules.
await writeFile(
  new URL('../dist/esm/package.json', import.meta.url),
  `${JSON.stringify({ type: 'module', sideEffects: false }, null, 2)}\n`
);
