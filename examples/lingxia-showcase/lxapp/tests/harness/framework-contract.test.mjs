import assert from 'node:assert/strict';
import { readFileSync, readdirSync } from 'node:fs';
import test from 'node:test';
import { babelParse, parse } from 'vue/compiler-sfc';

const reactUi = readFileSync(new URL('../../pages/ui/index.tsx', import.meta.url), 'utf8');
const vueUi = readFileSync(new URL('../../pages/ui/index.vue', import.meta.url), 'utf8');

test('React and Vue expose the redirected UI instance identity', () => {
  assert.match(reactUi, /data-testid="ui-page"/);
  assert.match(reactUi, /data-instance-tag=\{instanceTag\}/);
  assert.match(vueUi, /data-testid="ui-page"/);
  assert.match(vueUi, /:data-instance-tag="instanceTag"/);
});

// Template expressions use different local variable names in the two Views.
// Compare the literal portions; runtime specs still assert concrete ids.
function idPattern(expression) {
  if (expression?.type === 'StringLiteral') return expression.value;
  if (expression?.type === 'TemplateLiteral') return expression.quasis.map(part => part.value.cooked).join('${*}');
  throw new Error(`Unclassified data-testid expression: ${expression?.type}`);
}

function reactIds(source) {
  const file = babelParse(source, { sourceType: 'module', plugins: ['typescript', 'jsx'] });
  const ids = new Set();
  const visit = (node, owner) => {
    if (!node || typeof node !== 'object') return;
    if (node.type === 'FunctionDeclaration') owner = node.id?.name;
    if (node.type === 'JSXOpeningElement') {
      for (const attr of node.attributes) {
        if (attr.type !== 'JSXAttribute' || !['data-testid', 'testId'].includes(attr.name.name)) continue;
        const value = attr.value?.type === 'JSXExpressionContainer' ? attr.value.expression : attr.value;
        // InfoRow forwards this prop unchanged; its call site supplies the id.
        if (owner === 'InfoRow' && attr.name.name === 'data-testid' && value?.type === 'Identifier' && value.name === 'testId') continue;
        const id = idPattern(value);
        if (attr.name.name === 'testId' && node.name.name === 'PageStackCard') {
          const shared = readFileSync(new URL('../../shared/components/page-stack.tsx', import.meta.url), 'utf8');
          for (const pattern of reactIds(shared)) ids.add(pattern.replace('${*}', id));
        } else {
          if (attr.name.name === 'testId') assert.equal(node.name.name, 'InfoRow', 'classify the testId forwarding component');
          ids.add(id);
        }
      }
    }
    for (const value of Object.values(node)) {
      if (Array.isArray(value)) value.forEach(child => visit(child, owner));
      else if (value && typeof value === 'object' && 'type' in value) visit(value, owner);
    }
  };
  visit(file);
  return ids;
}

function vueIds(source) {
  const ids = new Set();
  const visit = node => {
    for (const prop of node.props ?? []) {
      if (prop.name === 'data-testid') {
        assert.ok(prop.value, 'data-testid needs a value');
        ids.add(prop.value.content);
      } else if (prop.name === 'bind' && prop.arg?.content === 'data-testid') {
        assert.ok(prop.exp, 'bound data-testid needs an expression');
        const file = babelParse(prop.exp.content);
        ids.add(idPattern(file.program.body[0].expression));
      }
    }
    for (const child of node.children ?? []) visit(child);
  };
  const { descriptor, errors } = parse(source);
  assert.deepEqual(errors, []);
  assert.ok(descriptor.template?.ast, 'Vue page needs a template');
  visit(descriptor.template.ast);
  return ids;
}

test('every React page testid is available in its Vue counterpart', () => {
  const pages = new URL('../../pages/', import.meta.url);
  let compared = 0;
  for (const directory of readdirSync(pages, { withFileTypes: true })) {
    if (!directory.isDirectory()) continue;
    const files = readdirSync(new URL(`${directory.name}/`, pages));
    if (!files.includes('index.tsx')) continue;
    assert.ok(files.includes('index.vue'), `${directory.name} is missing its Vue View`);
    const react = reactIds(readFileSync(new URL(`${directory.name}/index.tsx`, pages), 'utf8'));
    const vue = vueIds(readFileSync(new URL(`${directory.name}/index.vue`, pages), 'utf8'));
    assert.deepEqual([...react].filter(id => !vue.has(id)), [], `${directory.name}: missing Vue testids`);
    compared++;
  }
  assert.ok(compared > 0, 'no page pairs checked');
});


test('testid extraction rejects unknown bindings and preserves dynamic patterns', () => {
  assert.throws(() => reactIds("const testId = 'missing'; const view = <div data-testid={testId} />;"), /Unclassified/);
  assert.deepEqual([...reactIds('const view = <div data-testid={`item-${id}`} />;')], ['item-${*}']);
  assert.deepEqual([...vueIds('<template><div :data-testid="`item-${entry}`" /></template>')], ['item-${*}']);
  assert.throws(() => vueIds('<template><div :data-testid="unknown" /></template>'), /Unclassified/);
});
