import { copyFile, mkdir, readFile, writeFile } from 'node:fs/promises';
import ts from 'typescript';

await mkdir(new URL('../dist/generated/', import.meta.url), { recursive: true });
await mkdir(new URL('../dist/testing/', import.meta.url), { recursive: true });
await Promise.all([
  copyFile(new URL('../src/logic-globals.d.ts', import.meta.url), new URL('../dist/logic-globals.d.ts', import.meta.url)),
  copyFile(new URL('../automation-test-globals.d.ts', import.meta.url), new URL('../dist/automation-test-globals.d.ts', import.meta.url)),
  copyFile(new URL('../src/generated/logic-web.d.ts', import.meta.url), new URL('../dist/generated/logic-web.d.ts', import.meta.url)),
]);

const publicApiSource = await readFile(new URL('../src/testing/public-api.ts', import.meta.url), 'utf8');
const publicApiEsm = ts.transpileModule(publicApiSource, {
  compilerOptions: {
    module: ts.ModuleKind.ES2020,
    target: ts.ScriptTarget.ES2020,
  },
  fileName: 'public-api.ts',
}).outputText;
await writeFile(new URL('../dist/testing/public-api.mjs', import.meta.url), publicApiEsm);
