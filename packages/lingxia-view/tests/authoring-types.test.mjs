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
  // Every role the runtime admits from page code is typed, `dialog` for a popover <view> included.
  let props;
  const find = node => {
    if (ts.isInterfaceDeclaration(node) && node.name.text === 'Props') props = node;
    ts.forEachChild(node, find);
  };
  find(authoring);
  const role = checker.getTypeAtLocation(props).getProperty('role');
  const typed = checker.getTypeOfSymbolAtLocation(role, props).types.filter(type => type.isStringLiteral()).map(type => type.value);
  assert.deepEqual(typed.sort(), [...fieldOptions.role].sort());
});

test("a link's target is checked against the project's page registry", () => {
  const filename = fileURLToPath(new URL('./fixtures/link-target.tsx', import.meta.url));
  const program = ts.createProgram([filename], {
    strict: true, noEmit: true, types: [], target: ts.ScriptTarget.ES2020,
    module: ts.ModuleKind.NodeNext, moduleResolution: ts.ModuleResolutionKind.NodeNext,
    jsx: ts.JsxEmit.Preserve,
  });
  const diagnostics = ts.getPreEmitDiagnostics(program);
  assert.equal(diagnostics.length, 0, ts.formatDiagnosticsWithColorAndContext(diagnostics, {
    getCurrentDirectory: () => '', getCanonicalFileName: value => value, getNewLine: () => '\n',
  }));
});

test('tsc blames the trait a mistake breaks, not another trait of the element', () => {
  const filename = fileURLToPath(new URL('./fixtures/trait-mistakes.tsx', import.meta.url));
  const program = ts.createProgram([filename], {
    strict: true, noEmit: true, types: [], target: ts.ScriptTarget.ES2020,
    module: ts.ModuleKind.NodeNext, moduleResolution: ts.ModuleResolutionKind.NodeNext,
    jsx: ts.JsxEmit.Preserve,
  });
  const reasons = new Map(ts.getPreEmitDiagnostics(program).map(diagnostic => {
    const name = diagnostic.file.text.slice(0, diagnostic.start).match(/(?:const|function) (\w+)[^\n]*$/)[1];
    return [name, ts.flattenDiagnosticMessageText(diagnostic.messageText, '\n').split('\n').slice(-2).join(' ').trim()];
  }));
  for (const [name, reason] of [
    ['open', /^Type .* is not assignable to type .*Property 'popover' is missing in type .* but required in type '\{ popover: true \| "auto" \| "hint";/],
    ['manualClose', /^Types of property 'onCloseRequest' are incompatible\. +Type '\(\) => false' is not assignable to type 'undefined'\.$/],
    ['undragged', /Property 'drag' is missing in type .* but required in type '\{ drag: "x" \| "y";/],
    ['deaf', /Property 'snapIndex' is missing in type .* but required in type '\{ snapIndex: number;/],
    ['deafDrag', /Property 'snapIndex' is missing in type .* but required in type '\{ snapIndex: number;/],
    ['Both', /^Types of property 'snapIndex' are incompatible\. +Type 'number' is not assignable to type 'undefined'\.$/],
    ['actions', /Property 'drag' is missing in type .* but required in type '\{ drag: "x" \| "y";/],
    ['openAndSnap', /Property 'popover' is missing in type .* but required in type '\{ popover: true/],
    ['closeAndSnap', /Property 'drag' is missing in type .* but required in type '\{ drag: "x" \| "y";/],
    ['unheard', /Property 'onSnapChange' is missing in type .* but required in type '\{ snapIndex: number; onSnapChange: /],
    ['silent', /Property 'player' is missing in type .* but required in type '\{ src\?: undefined; sources\?: undefined; player: Video; \}'/],
    ['twice', /^Types of property 'sources' are incompatible\. +Type '\{ src: string; label: string; \}\[\]' is not assignable to type 'undefined'\.$/],
    ['unpointed', /Property 'onPointer' is missing in type .* but required in type '\{ onPointer: /],
  ]) assert.match(reasons.get(name) ?? 'no error', reason, name);
  assert.equal(reasons.size, 13);
});
