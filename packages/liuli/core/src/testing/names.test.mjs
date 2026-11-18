import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readdirSync, readFileSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import ts from 'typescript';

// Liuli's naming rules, held over every exported `*Props` so a retired name cannot come back.
const src = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const components = readdirSync(src, { withFileTypes: true }).filter(entry => entry.isDirectory() && /^[A-Z]/.test(entry.name)).map(entry => entry.name);

/** Every exported `*Props` type's own property names, by `Component.PropsName`. */
const props = new Map();
for (const name of components) {
  const file = join(src, name, `${name}.tsx`);
  let text;
  try { text = readFileSync(file, 'utf8'); } catch { continue; }
  const source = ts.createSourceFile(file, text, ts.ScriptTarget.Latest, true, ts.ScriptKind.TSX);
  for (const node of source.statements) {
    const isExported = node.modifiers?.some(modifier => modifier.kind === ts.SyntaxKind.ExportKeyword);
    if (!isExported || !(ts.isInterfaceDeclaration(node) || ts.isTypeAliasDeclaration(node)) || !/(Props|Binding)$/.test(node.name.text)) continue;
    const names = new Set();
    const walk = child => {
      if (ts.isPropertySignature(child) && child.name) names.add(child.name.getText(source).replace(/^'|'$/g, ''));
      ts.forEachChild(child, walk);
    };
    walk(node);
    props.set(`${name}.${node.name.text}`, { names: [...names], text: node.getText(source) });
  }
}

const offenders = predicate => [...props].flatMap(([owner, { names }]) => names.filter(predicate).map(name => `${owner}: ${name}`));

test('names: the props of every component are read', () => {
  assert.ok(props.size >= components.length, `${props.size} props types for ${components.length} components`);
});

test('rule 2: what opens takes `isOpen`/`onOpenChange` and starts by `isDefaultOpen`; a set of open rows is `open<Rows>`/`onOpen<Rows>Change`/`defaultOpen<Rows>`', () => {
  assert.deepEqual(offenders(name => /Collapsed|Expanded|^defaultClosed$|^expanded$|^isExpanded$|^collapsed$/.test(name)), []);
});
