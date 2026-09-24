import path from 'node:path';
import { defineConfig } from 'rolldown';

const here = path.dirname(new URL(import.meta.url).pathname);

export default defineConfig({
  input: path.join(here, '../src/hook.ts'),
  external: ['vue'],
  resolve: { alias: { '@lingxia/bridge': path.join(here, 'bridge-host.ts') } },
  output: { file: path.join(here, '../dist/test/hooks.mjs'), format: 'esm' },
});
