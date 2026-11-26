// Builds the window feedback page into `src/feedback/page.js`. `--check` fails on drift instead of writing.
import { buildPages } from '../../../packages/lingxia-view/compiler/index.mjs';
import { loadDependency } from '../../../packages/lingxia-view/compiler/dependencies.mjs';
import { mkdtemp, readFile, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';

const root = fileURLToPath(new URL('.', import.meta.url));
const target = fileURLToPath(new URL('../src/feedback/page.js', import.meta.url));
const catalog = { __names: ['answer'], __modes: { answer: 'call' } };
const { transform } = await loadDependency('esbuild');

const out = await mkdtemp(join(tmpdir(), 'lx-feedback-'));
try {
  await buildPages(root, ['pages/feedback/index.tsx'], out, { 'pages/feedback/index.tsx': catalog },
    { chunks: false, messages: null, inputsRecord: join(out, 'gpu-inputs.json') });
  const page = await readFile(join(out, 'pages/feedback/index.js'), 'utf8');
  // Minified as a production chunk is; the host reads the manifest from the first line.
  const newline = page.indexOf('\n');
  const { code } = await transform(page.slice(newline + 1), { minify: true, format: 'esm', target: 'es2020', legalComments: 'none' });
  const built = `${page.slice(0, newline + 1)}${code}`;
  const current = await readFile(target, 'utf8').catch(() => null);
  if (process.argv.includes('--check')) {
    if (current?.replace(/\r\n/g, '\n') !== built) {
      console.error('crates/lingxia/src/feedback/page.js is stale; run node crates/lingxia/feedback/build.mjs');
      process.exitCode = 1;
    }
  } else if (current !== built) {
    await writeFile(target, built);
    console.log('wrote crates/lingxia/src/feedback/page.js');
  }
} finally {
  await rm(out, { recursive: true, force: true });
}
