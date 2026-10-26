import test from 'node:test';
import assert from 'node:assert/strict';
import { mkdtemp, readFile, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { buildPages } from '../../../packages/lingxia-ui/compiler/index.mjs';

const root = fileURLToPath(new URL('../lxapp/', import.meta.url));

// The fixture page exercises every View capability; it must compile with no authoring problems.
test('the GPU-only view lab compiles and is a registered page', async () => {
  const out = await mkdtemp(join(tmpdir(), 'view-lab-'));
  try {
    await buildPages(root, ['pages/view-lab/index.tsx'], out);
    assert.match(await readFile(join(out, 'pages/view-lab/index.js'), 'utf8'), /^\/\/ LingXia UI module/);
  } finally {
    await rm(out, { recursive: true, force: true });
  }
  const app = JSON.parse(await readFile(join(root, 'lxapp.json'), 'utf8'));
  assert.ok(app.pages.some(page => page.path === 'pages/view-lab/index'));
});

test('the motion lab compiles only through its experimental opt-in', async () => {
  const out = await mkdtemp(join(tmpdir(), 'motion-lab-'));
  try {
    await buildPages(root, ['pages/motion-lab/index.tsx'], out);
    const module = await readFile(join(out, 'pages/motion-lab/index.js'), 'utf8');
    assert.match(module, /"transition-transform"/);
    // Without the source opt-in the same classes stay gated.
    const source = await readFile(join(root, 'pages/motion-lab/index.tsx'), 'utf8');
    assert.match(source, /^\/\/ @ui-experimental motion$/m);
  } finally {
    await rm(out, { recursive: true, force: true });
  }
});
