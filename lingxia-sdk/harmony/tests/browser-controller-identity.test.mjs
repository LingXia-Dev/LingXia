import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import test from 'node:test';
import ts from '../../../packages/node_modules/typescript/lib/typescript.js';

// Execute the production lookup with a native identity stub and controller map;
// no ArkWeb engine is required to exercise retirement/registration ordering.
const source = await readFile(new URL('../lingxia/src/main/ets/lxapp/WebViewCore.ets', import.meta.url), 'utf8');
const ast = ts.createSourceFile('WebViewCore.ts', source, ts.ScriptTarget.Latest, true);
const manager = ast.statements.find(node => ts.isClassDeclaration(node) && node.name?.text === 'WebViewManager');
const method = manager.members.find(node => node.name?.getText(ast) === 'findBrowserWebview');
assert.ok(method, 'production browser controller lookup exists');
const { outputText } = ts.transpileModule(`
let current = '';
function browserTabWebtag(_tabId: string): string { return current; }
export class WebViewManager {
  static controllers = new Map();
  ${method.getText(ast)}
}
export function select(tag: string) { current = tag; }
`, { compilerOptions: { target: ts.ScriptTarget.ES2022, module: ts.ModuleKind.ES2022 } });
const { WebViewManager, select } = await import('data:text/javascript;base64,' + Buffer.from(outputText).toString('base64'));

test('reopening waits for the current controller and never borrows the retiring instance', () => {
  const oldTag = 'app.lingxia.browser:/tabs/newtab#browser-10#2';
  const newTag = 'app.lingxia.browser:/tabs/newtab#browser-11#2';
  const retired = { title: 'retired' };
  const current = { title: 'current' };
  WebViewManager.controllers.set(oldTag, retired);
  select(oldTag);
  assert.equal(WebViewManager.findBrowserWebview('newtab'), retired);
  select(newTag);
  assert.equal(WebViewManager.findBrowserWebview('newtab'), null);
  WebViewManager.controllers.set(newTag, current);
  assert.equal(WebViewManager.findBrowserWebview('newtab'), current);
  WebViewManager.controllers.delete(oldTag);
  assert.equal(WebViewManager.findBrowserWebview('newtab'), current);
  select('');
  assert.equal(WebViewManager.findBrowserWebview('newtab'), null);
});
