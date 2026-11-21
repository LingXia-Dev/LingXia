import { test } from 'node:test';
import assert from 'node:assert/strict';
import ts from 'typescript';
import { mkdtemp, mkdir, writeFile, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { moduleChecker, parseSource } from '../compiler/source.mjs';
import { checkerOptions } from '../compiler/tsconfig.mjs';

test("the audit's checker reads a page's generated ./$types beside it", async () => {
  const root = await mkdtemp(join(tmpdir(), 'lx-page-types-'));
  try {
    await mkdir(join(root, 'pages/home'), { recursive: true });
    await mkdir(join(root, '.lingxia/types/pages/home'), { recursive: true });
    await writeFile(join(root, '.lingxia/types/pages/home/$types.d.ts'), 'export type View = { data: { count: number } };\n');
    const path = join(root, 'pages/home/index.tsx');
    const source = "import type { View } from './$types';\nexport function render({ data }: View) { return data.count; }\n";
    const file = parseSource(path, source);
    const checker = moduleChecker([file], checkerOptions(root));
    let found;
    const visit = node => {
      if (ts.isPropertyAccessExpression(node) && node.getText() === 'data.count') found = checker.typeToString(checker.getTypeAtLocation(node));
      ts.forEachChild(node, visit);
    };
    visit(file);
    assert.equal(found, 'number');
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});
