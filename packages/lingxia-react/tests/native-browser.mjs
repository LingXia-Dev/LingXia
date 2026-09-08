import assert from 'node:assert/strict';
import { mkdtemp, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { build } from 'esbuild';
import { runBrowser } from '../../lingxia-elements/tests/browser-harness.mjs';

const directory = await mkdtemp(path.join(tmpdir(), 'lingxia-react-browser-'));
try {
  await build({ entryPoints: [fileURLToPath(new URL('./native-ref.jsx', import.meta.url))],
    bundle: true, format: 'esm', outfile: path.join(directory, 'ref.js'), define: { 'process.env.NODE_ENV': '"development"' } });
  const result = await runBrowser(directory, async () => {
    window.__LX_RUNTIME_PLATFORM__ = 'all';
    window.LingXiaBridge = { nativeComponents: { send() {}, register() { return () => {}; } } };
    const { run } = await import('/ref.js');
    return await run();
  });
  assert.equal(result.errors.length, 0, JSON.stringify(result.errors));
  assert.ok(result.handles >= 2, 'StrictMode exercises ref setup again');
  assert.equal(result.press, 1, 'StrictMode preserves event bindings');
  console.log('React native refs and StrictMode events passed');
} finally {
  await rm(directory, { recursive: true, force: true });
}
