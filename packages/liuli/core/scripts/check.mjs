// Liuli's boundary rules that can be checked from source alone. Run: `node scripts/check.mjs`.
// Prints one line per problem as `file:line: rule: message` and exits non-zero when any is found.
import { existsSync, readFileSync, readdirSync, statSync } from 'node:fs';
import { basename, dirname, join, relative, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import ts from 'typescript';

const core = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const src = join(core, 'src');
const problems = [];

const walk = dir => readdirSync(dir).flatMap(name => {
  const path = join(dir, name);
  return statSync(path).isDirectory() ? walk(path) : [path];
});
const files = walk(src).filter(path => /\.(tsx?|mjs)$/.test(path));
const rel = path => relative(core, path);

const kind = path => {
  const name = basename(path);
  if (name.endsWith('.test.mjs') || rel(path).startsWith('src/testing/')) return 'test';
  if (name.endsWith('.examples.tsx')) return 'examples';
  if (name.endsWith('.recipe.ts')) return 'recipe';
  if (name.endsWith('.doc.ts')) return 'doc';
  return 'source';
};

const oldLayoutTags = new Set(['column', 'row', 'grid', 'stack', 'spacer']);
const engineGlobals = new Set(['Intl', 'globalThis', 'window', 'document', 'navigator', 'setTimeout', 'setInterval', 'requestAnimationFrame']);
// Physical sides; Liuli uses logical ones so every recipe mirrors in RTL.
const physical = /^(?:[a-z-]+:)*-?(?:pl|pr|ml|mr|left|right|border-l|border-r|rounded-l|rounded-r|rounded-tl|rounded-tr|rounded-bl|rounded-br|text-left|text-right|scroll-ml|scroll-mr)(?:-|$)/;
const replacedProps = new Set(['disabled', 'checked', 'open', 'onPress', 'onClose', 'onValueChange', 'style', 'ref']);

for (const path of files) {
  const text = readFileSync(path, 'utf8');
  const file = ts.createSourceFile(path, text, ts.ScriptTarget.Latest, true, path.endsWith('.tsx') ? ts.ScriptKind.TSX : path.endsWith('.mjs') ? ts.ScriptKind.JS : ts.ScriptKind.TS);
  const role = kind(path);
  const report = (node, rule, message) => {
    const { line } = file.getLineAndCharacterOfPosition(node.getStart(file));
    problems.push(`${rel(path)}:${line + 1}: ${rule}: ${message}`);
  };

  const visit = node => {
    // Imports: the public `@lingxia/view` entry, its testing entry in tests, and files inside the package.
    if ((ts.isImportDeclaration(node) || ts.isExportDeclaration(node)) && node.moduleSpecifier && ts.isStringLiteral(node.moduleSpecifier)) {
      const spec = node.moduleSpecifier.text;
      if (spec.startsWith('@lingxia/view/') && !(role === 'test' && spec === '@lingxia/view/testing'))
        report(node, 'import', `${spec} is not public to components; import from @lingxia/view`);
      if (spec.startsWith('.') && relative(core, resolve(dirname(path), spec)).startsWith('..'))
        report(node, 'import', `${spec} reaches outside the package`);
    }
    if (role !== 'test') {
      if (ts.isIdentifier(node) && engineGlobals.has(node.text) && !(ts.isPropertyAccessExpression(node.parent) && node.parent.name === node))
        report(node, 'engine', `${node.text} is not available on every engine; take formatted values as props`);
      if (ts.isPropertyAccessExpression(node) && ts.isIdentifier(node.expression) && node.expression.text === 'Date' && node.name.text === 'now')
        report(node, 'engine', 'Date.now() in render; time arrives as a prop');
      if (ts.isNewExpression(node) && ts.isIdentifier(node.expression) && node.expression.text === 'Date')
        report(node, 'engine', 'new Date() in render; time arrives as a prop');
      if (ts.isPropertyAccessExpression(node) && ['os', 'formFactor'].includes(node.name.text))
        report(node, 'same-on-every-os', `env.${node.name.text} must not change how a component looks or behaves`);
    }
    if (ts.isJsxOpeningElement(node) || ts.isJsxSelfClosingElement(node)) {
      const tag = node.tagName.getText(file);
      if (oldLayoutTags.has(tag)) report(node, 'layout', `<${tag}> no longer exists; use <view> with Tailwind`);
      for (const attribute of node.attributes.properties) {
        if (!ts.isJsxAttribute(attribute) || attribute.name.getText(file) !== 'className' || !attribute.initializer) continue;
        const value = ts.isJsxExpression(attribute.initializer) ? attribute.initializer.expression : attribute.initializer;
        checkClassName(value, role, report);
      }
    }
    if (role === 'recipe') checkRecipeLeaf(node, report);
    if (role === 'source' && (ts.isInterfaceDeclaration(node) || ts.isTypeAliasDeclaration(node)) && /Props$/.test(node.name.text))
      checkProps(node, report);
    for (const token of classTokens(node, role)) {
      if (/(^|:)dark:/.test(token.text) || token.text.startsWith('dark:')) report(token.node, 'class', `${token.text}: colours follow the theme; no dark: in Liuli`);
      if (role === 'recipe' && physical.test(token.text)) report(token.node, 'rtl', `${token.text} is a physical side; use the logical form (ps-, me-, start-, text-start)`);
    }
    ts.forEachChild(node, visit);
  };
  visit(file);
}

/** Class words in recipe string literals. */
function classTokens(node, role) {
  if (role !== 'recipe' || !ts.isStringLiteral(node) || !ts.isPropertyAssignment(node.parent)) return [];
  return node.text.split(/\s+/).filter(Boolean).map(text => ({ text, node }));
}

/** A className is a recipe lookup, a `cx(...)` of lookups, a condition over them, or a passed-in prop; never assembled text. */
function checkClassName(node, role, report) {
  if (!node) return;
  if (ts.isTemplateExpression(node) || ts.isNoSubstitutionTemplateLiteral(node)) return report(node, 'class', 'a template literal assembles class names; pick complete names from a recipe');
  if (ts.isBinaryExpression(node) && node.operatorToken.kind === ts.SyntaxKind.PlusToken) return report(node, 'class', '+ assembles class names; use cx() over recipe leaves');
  if (ts.isStringLiteral(node)) {
    if (role === 'source') report(node, 'class', 'a class literal in a component; put it in the recipe');
    return;
  }
  if (ts.isParenthesizedExpression(node)) return checkClassName(node.expression, role, report);
  if (ts.isConditionalExpression(node)) { checkClassName(node.whenTrue, role, report); return checkClassName(node.whenFalse, role, report); }
  if (ts.isBinaryExpression(node)) {
    const operator = node.operatorToken.kind;
    // `cond && leaf` yields the leaf; only `||` and `??` can yield their left side.
    if (operator === ts.SyntaxKind.BarBarToken || operator === ts.SyntaxKind.QuestionQuestionToken) checkClassName(node.left, role, report);
    if (operator === ts.SyntaxKind.AmpersandAmpersandToken || operator === ts.SyntaxKind.BarBarToken || operator === ts.SyntaxKind.QuestionQuestionToken)
      checkClassName(node.right, role, report);
    return;
  }
  if (ts.isCallExpression(node)) {
    if (!ts.isIdentifier(node.expression) || node.expression.text !== 'cx') return report(node, 'class', 'only cx() from @lingxia/view may join classes');
    for (const argument of node.arguments) checkClassName(argument, role, report);
  }
}

/** Recipe leaves are single string literals. */
function checkRecipeLeaf(node, report) {
  if (!ts.isPropertyAssignment(node)) return;
  const value = node.initializer;
  if (ts.isTemplateExpression(value) || ts.isNoSubstitutionTemplateLiteral(value) || (ts.isBinaryExpression(value) && value.operatorToken.kind === ts.SyntaxKind.PlusToken) || ts.isCallExpression(value))
    report(value, 'recipe', 'a recipe leaf is one string literal');
}

/** Liuli's prop naming: no primitive or web names it replaces, and booleans named is or has. */
function checkProps(node, report) {
  const members = [];
  const collect = type => {
    if (!type) return;
    if (ts.isTypeLiteralNode(type)) members.push(...type.members);
    if (ts.isIntersectionTypeNode(type) || ts.isUnionTypeNode(type)) type.types.forEach(collect);
  };
  if (ts.isInterfaceDeclaration(node)) members.push(...node.members);
  else collect(node.type);
  for (const member of members) {
    if (!ts.isPropertySignature(member) || !member.name) continue;
    const name = member.name.getText().replace(/^['"]|['"]$/g, '');
    if (replacedProps.has(name)) report(member, 'props', `${name} is the primitive's name; Liuli uses is*/on*Change forms`);
    if (/^(left|right)[A-Z]/.test(name)) report(member, 'props', `${name}: use start/end`);
    if (member.type?.kind === ts.SyntaxKind.BooleanKeyword && !/^(is|has)[A-Z]/.test(name))
      report(member, 'props', `${name}: a boolean prop is named is* or has*`);
  }
}

// Every component folder has its five files, a doc whose examples exist, and an export.
const manifest = JSON.parse(readFileSync(join(core, 'package.json'), 'utf8'));
for (const name of readdirSync(src)) {
  const dir = join(src, name);
  if (!/^[A-Z]/.test(name) || !statSync(dir).isDirectory()) continue;
  const parts = { component: `${name}.tsx`, recipe: `${name}.recipe.ts`, doc: `${name}.doc.ts`, examples: `${name}.examples.tsx`, test: `${name}.test.mjs` };
  for (const [part, file] of Object.entries(parts))
    if (!existsSync(join(dir, file))) problems.push(`src/${name}: files: no ${part} file (${file})`);
  if (manifest.exports[`./${name}`] !== `./src/${name}/${name}.tsx`) problems.push(`package.json: exports: no "./${name}" entry for src/${name}/${name}.tsx`);
  if (!existsSync(join(dir, parts.doc)) || !existsSync(join(dir, parts.examples))) continue;
  const doc = readFileSync(join(dir, parts.doc), 'utf8');
  const examples = readFileSync(join(dir, parts.examples), 'utf8');
  const exported = new Set([...examples.matchAll(/^export function (\w+)/gm)].map(match => match[1]));
  const listed = new Set([...doc.matchAll(/\{ name: '(\w+)', title:/g)].map(match => match[1]));
  if (!new RegExp(`name: '${name}'`).test(doc)) problems.push(`src/${name}/${parts.doc}: doc: name is not '${name}'`);
  for (const example of listed) if (!exported.has(example)) problems.push(`src/${name}/${parts.doc}: doc: lists example ${example}, which ${parts.examples} does not export`);
  for (const example of exported) if (!listed.has(example)) problems.push(`src/${name}/${parts.examples}: doc: ${example} is not listed in the doc`);
}

for (const problem of problems) console.log(problem);
if (problems.length) { console.log(`\n${problems.length} problem${problems.length === 1 ? '' : 's'}`); process.exit(1); }
console.log('liuli check: clean');
