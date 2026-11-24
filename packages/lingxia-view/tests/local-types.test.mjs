import { test } from 'node:test';
import assert from 'node:assert/strict';
import ts from 'typescript';
import { mkdtempSync, writeFileSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';
import schema from '../../../contracts/view/schema.json' with { type: 'json' };

const entry = fileURLToPath(new URL('../src/index.js', import.meta.url));
const options = {
  strict: true, types: [], target: ts.ScriptTarget.ES2020, module: ts.ModuleKind.NodeNext,
  moduleResolution: ts.ModuleResolutionKind.NodeNext, jsx: ts.JsxEmit.Preserve,
};

// Compiles `source` as a page module; returns its diagnostics and, when `emit`, its declaration file.
function compile(source, emit = false) {
  const dir = mkdtempSync(join(tmpdir(), 'lingxia-view-local-'));
  try {
    const file = join(dir, 'cells.tsx');
    writeFileSync(file, `import { useLocal, type Local, type LocalText } from ${JSON.stringify(entry)};\n${source}`);
    let declaration = '';
    const host = ts.createCompilerHost(options);
    host.writeFile = (name, text) => { if (name.endsWith('cells.d.ts')) declaration = text; };
    const program = ts.createProgram([file], emit ? { ...options, declaration: true, emitDeclarationOnly: true, outDir: dir } : { ...options, noEmit: true }, host);
    const diagnostics = ts.getPreEmitDiagnostics(program).map(diagnostic => ts.flattenDiagnosticMessageText(diagnostic.messageText, '\n'));
    if (emit) program.emit();
    return { diagnostics, declaration };
  } finally { rmSync(dir, { recursive: true, force: true }); }
}

test('useLocal results print as named types, never as an expanded object', () => {
  const { diagnostics, declaration } = compile(`export function cells() {
  return { text: useLocal(''), named: useLocal('note', 'x'), choice: useLocal<'a' | 'b'>('a'), count: useLocal(0),
    flag: useLocal(false), maybe: useLocal<string | null>(null), record: useLocal({ a: 1 }) };
}`, true);
  assert.deepEqual(diagnostics, []);
  for (const line of ['text: LocalText<string>;', 'named: LocalText<string>;', 'choice: LocalText<"a" | "b">;', 'count: Local<number>;',
    'flag: Local<boolean>;', 'maybe: Local<string | null>;', 'record: Local<{\n'])
    assert.ok(declaration.includes(line), `${line} in\n${declaration}`);
  assert.doesNotMatch(declaration, /readonly value:/);
});

test('only a string cell has focus()', () => {
  assert.deepEqual(compile(`export function F() { useLocal('').focus(); useLocal('n', 'x').focus(); useLocal<'a' | 'b'>('a').focus(); return null; }`).diagnostics, []);
  const absent = ['useLocal(0)', 'useLocal(false)', 'useLocal({ a: 1 })', 'useLocal([1])', 'useLocal<string | null>(null)'];
  const { diagnostics } = compile(`export function F() {\n${absent.map(cell => `  ${cell}.focus();`).join('\n')}\n  return null;\n}`);
  assert.equal(diagnostics.filter(text => /Property 'focus' does not exist on type 'Local<.*>'/.test(text)).length, absent.length, diagnostics.join('\n'));
  assert.equal(diagnostics.length, absent.length, diagnostics.join('\n'));
});

test('every element with a binding takes a cell of its type from useLocal', () => {
  const initial = { string: "''", boolean: 'false', number: '0' };
  const extra = {
    radio: ' options={[]}', select: ' options={[]}', slider: ' aria-label="x"', dialog: ' aria-label="x" drag="y" dragSnapPoints={[0, 100]}',
    view: ' drag="y" dragSnapPoints={[0, 100]}', button: ' drag="y" dragSnapPoints={[0, 100]}',
  };
  const lines = [];
  for (const [name, element] of Object.entries(schema.elements)) {
    const type = element.binding?.type ?? element.props?.bind?.type.match(/^Local<(\w+)>$/)?.[1];
    if (!type || element.component) continue;
    lines.push(`  const ${name}Cell = useLocal(${initial[type]}); <${name} bind={${name}Cell}${extra[name] ?? ''}/>;`);
    // A cell typed in a part's props passes through too.
    lines.push(`  const ${name}Held: Local<${type}> = ${name}Cell; <${name} bind={${name}Held}${extra[name] ?? ''}/>;`);
  }
  assert.ok(lines.length >= 18, 'every bindable element is covered');
  assert.deepEqual(compile(`export function F() {\n${lines.join('\n')}\n  return null;\n}`).diagnostics, []);
});

test('a text cell is a Local<string> wherever one is taken', () => {
  assert.deepEqual(compile(`function Part(props: { bind: Local<string> }) { return <input bind={props.bind}/>; }
export function F() { const text: LocalText = useLocal(''); <Part bind={text}/>; <Part bind={useLocal('x')}/>; return null; }`).diagnostics, []);
});
