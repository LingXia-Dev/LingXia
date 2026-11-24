import { test } from 'node:test';
import assert from 'node:assert/strict';
import { mkdtempSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import ts from 'typescript';
import { choiceLine, choicesOf, exclusiveGroups } from '../../scripts/choices.mjs';

/** The choices in type `Props` of a source, as the generator reads them. */
function read(source) {
  const file = join(mkdtempSync(join(tmpdir(), 'liuli-choices-')), 'props.ts');
  writeFileSync(file, source);
  const program = ts.createProgram([file], { strict: true, noEmit: true });
  const checker = program.getTypeChecker();
  const props = program.getSourceFile(file).statements.find(node => (ts.isTypeAliasDeclaration(node) || ts.isInterfaceDeclaration(node)) && node.name.text === 'Props');
  return choicesOf(ts, checker, checker.getTypeAtLocation(props.name));
}

test('a binding and a link, each barring the other side with never, are two choices; props of every side are not one', () => {
  const choices = read(`
    type Binding = { bind: string; value?: never; onChange?: never } | { bind?: never; value: string; onChange: (v: string) => void };
    type Link = { onClick: () => void; to?: never } | { onClick?: never; to: string };
    export type Props = { label: string; hasChevron?: boolean } & Binding & Link;
  `);
  assert.deepEqual(choices, [[['bind'], ['value', 'onChange']], [['onClick'], ['to']]]);
  assert.equal(choiceLine(choices[0]), '`bind` | `value` + `onChange`: props from one side only');
});

test('props given together or not at all, or only beside a flag, exclude nothing', () => {
  assert.deepEqual(read(`export type Props = { label: string } & ({ onRemove: () => void; removeLabel: string } | { onRemove?: never; removeLabel?: never });`), []);
  assert.deepEqual(read(`export type Props = { hasClear: true; clearLabel?: string } | { hasClear?: false; clearLabel?: never };`), []);
  assert.deepEqual(read(`export interface Props { label: string; to?: string }`), []);
});

test('three sides that bar each other are one choice', () => {
  assert.deepEqual(exclusiveGroups(['a', 'b', 'c', 'd'], [new Set(['a', 'd']), new Set(['b', 'd']), new Set(['c', 'd'])]), [[['a'], ['b'], ['c']]]);
  assert.equal(choiceLine([['a'], ['b'], ['c']]), '`a` | `b` | `c`: props from one side only');
});
