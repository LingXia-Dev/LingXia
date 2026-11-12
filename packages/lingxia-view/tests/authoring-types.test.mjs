import { test } from 'node:test';
import assert from 'node:assert/strict';
import ts from 'typescript';
import { fileURLToPath } from 'node:url';
import { acceptsProp } from '../compiler/index.mjs';
import { elements } from '../src/generated/contract.js';
import { fieldOptions } from '../src/generated/wire.js';

test('authoring types preserve binding ownership, page contracts and children', () => {
  const filename = fileURLToPath(new URL('./fixtures/authoring.tsx', import.meta.url));
  const program = ts.createProgram([filename], {
    strict: true, noEmit: true, types: [], target: ts.ScriptTarget.ES2020,
    module: ts.ModuleKind.NodeNext, moduleResolution: ts.ModuleResolutionKind.NodeNext,
    jsx: ts.JsxEmit.Preserve,
  });
  const diagnostics = ts.getPreEmitDiagnostics(program);
  assert.equal(diagnostics.length, 0, ts.formatDiagnosticsWithColorAndContext(diagnostics, {
    getCurrentDirectory: () => '', getCanonicalFileName: value => value, getNewLine: () => '\n',
  }));

  const checker = program.getTypeChecker();
  const authoring = program.getSourceFiles().find(file => file.fileName.endsWith('/generated/authoring.d.ts'));
  let intrinsics;
  const visit = node => {
    if (ts.isInterfaceDeclaration(node) && node.name.text === 'IntrinsicElements') intrinsics = node;
    ts.forEachChild(node, visit);
  };
  visit(authoring);
  const intrinsicElements = checker.getTypeAtLocation(intrinsics).getProperties();
  assert.deepEqual(intrinsicElements.map(symbol => symbol.name).sort(), Object.keys(elements).filter(name => !elements[name].component).sort());
  for (const element of intrinsicElements) {
    const type = checker.getTypeOfSymbolAtLocation(element, intrinsics);
    for (const prop of type.getProperties())
      assert.ok(acceptsProp(element.name, prop.name), `${element.name}.${prop.name} must be accepted at runtime`);
  }
  // Every role the runtime admits from page code is typed; `dialog` is the <dialog> element's own.
  let props;
  const find = node => {
    if (ts.isInterfaceDeclaration(node) && node.name.text === 'Props') props = node;
    ts.forEachChild(node, find);
  };
  find(authoring);
  const role = checker.getTypeAtLocation(props).getProperty('role');
  const typed = checker.getTypeOfSymbolAtLocation(role, props).types.filter(type => type.isStringLiteral()).map(type => type.value);
  assert.deepEqual(typed.sort(), fieldOptions.role.filter(name => name !== 'dialog').sort());
});
