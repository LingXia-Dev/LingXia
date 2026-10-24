import { test } from 'node:test';
import assert from 'node:assert/strict';
import ts from 'typescript';
import { fileURLToPath } from 'node:url';
import { acceptsProp } from '../src/authoring/props.js';
import { tags } from '../src/schema.js';

test('authoring types preserve binding ownership, page contracts and children', () => {
  const filename = fileURLToPath(new URL('./fixtures/authoring.tsx', import.meta.url));
  const program = ts.createProgram([filename], {
    strict: true, noEmit: true, types: [], target: ts.ScriptTarget.ES2020,
    module: ts.ModuleKind.NodeNext, moduleResolution: ts.ModuleResolutionKind.NodeNext,
    jsx: ts.JsxEmit.React, jsxFactory: 'h',
  });
  const diagnostics = ts.getPreEmitDiagnostics(program);
  assert.equal(diagnostics.length, 0, ts.formatDiagnosticsWithColorAndContext(diagnostics, {
    getCurrentDirectory: () => '', getCanonicalFileName: value => value, getNewLine: () => '\n',
  }));

  const checker = program.getTypeChecker();
  const authoring = program.getSourceFiles().find(file => file.fileName.endsWith('/authoring/index.d.ts'));
  let intrinsics;
  const visit = node => {
    if (ts.isInterfaceDeclaration(node) && node.name.text === 'IntrinsicElements') intrinsics = node;
    ts.forEachChild(node, visit);
  };
  visit(authoring);
  const elements = checker.getTypeAtLocation(intrinsics).getProperties();
  assert.deepEqual(elements.map(symbol => symbol.name).sort(), [...Object.keys(tags).filter(tag => tag !== 'radio-option'), 'view'].sort());
  for (const element of elements) {
    const type = checker.getTypeOfSymbolAtLocation(element, intrinsics);
    for (const prop of type.getProperties())
      assert.ok(acceptsProp(element.name, prop.name), `${element.name}.${prop.name} must be accepted at runtime`);
  }
});
