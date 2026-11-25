// Liuli's boundary rules that can be checked from source alone. Run: `node scripts/check.mjs`, or
// for another Liuli package `node ../core/scripts/check.mjs .` from its folder.
// Prints one line per problem as `file:line: rule: message` and exits non-zero when any is found.
import { existsSync, readFileSync, readdirSync, statSync } from 'node:fs';
import { basename, dirname, join, relative, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import ts from 'typescript';

const core = resolve(process.argv[2] ?? resolve(dirname(fileURLToPath(import.meta.url)), '..'));
const src = join(core, 'src');
const problems = [];

const walk = dir => readdirSync(dir).flatMap(name => {
  const path = join(dir, name);
  return statSync(path).isDirectory() ? walk(path) : [path];
});
const rel = path => relative(core, path).replaceAll('\\', '/');
// The generated words module holds the catalogue formatter, not component code.
const files = walk(src).filter(path => /\.(tsx?|mjs)$/.test(path) && rel(path) !== 'src/i18n/words.ts');

const kind = path => {
  const name = basename(path);
  if (name.endsWith('.test.mjs') || rel(path).startsWith('src/testing/')) return 'test';
  // The examples' shared helper is example code: its classes are its own, and its presses answer too.
  if (name.endsWith('.examples.tsx') || rel(path) === 'src/example.tsx') return 'examples';
  if (name.endsWith('.recipe.ts')) return 'recipe';
  if (name.endsWith('.doc.ts')) return 'doc';
  // A package's document source runs in Logic, which owns the clock; render rules on time do not apply.
  if (rel(path).startsWith('src/source/')) return 'logic';
  return 'source';
};

const oldLayoutTags = new Set(['column', 'row', 'grid', 'stack', 'spacer']);
const engineGlobals = new Set(['Intl', 'globalThis', 'window', 'document', 'navigator', 'setTimeout', 'setInterval', 'requestAnimationFrame']);
// Physical sides; Liuli uses logical ones so every recipe mirrors in RTL.
const physical = /^(?:[a-z-]+:)*-?(?:pl|pr|ml|mr|left|right|border-l|border-r|rounded-l|rounded-r|rounded-tl|rounded-tr|rounded-bl|rounded-br|text-left|text-right|scroll-ml|scroll-mr)(?:-|$)/;
// Raw steps of the pack's ramps and raw timings; a recipe uses Liuli's names, so a pack moves every component together.
const rawStep = /^(?:[a-z-]+:)*(?:text-(?:xs|sm|base|lg|xl|[2-9]xl)|rounded(?:-(?:s|e|t|b|ss|se|es|ee))?-(?:sm|md|lg|xl|[23]xl)|duration-\d+|ease-(?:in|out|in-out|linear))$/;
const translucentInk = /^(?:[a-z-]+:)*text-[a-z][a-z0-9-]*\/\d+$/;
// The names every icon set answers to; an icon asked for by another name would have nothing to draw.
const iconNames = new Set(Object.keys(JSON.parse(readFileSync(resolve(dirname(fileURLToPath(import.meta.url)), '../icons/icons.json'), 'utf8')).icons));
const iconProps = new Set(['startIcon', 'endIcon']);
const replacedProps = new Set(['disabled', 'checked', 'open', 'onPress', 'onClose', 'onValueChange', 'style', 'ref']);

// A specimen that can be pressed answers visibly: no empty handler, and no button or link left
// without one. `Said`, `Held` and `unused` in src/example.tsx are the ways to answer.
const pressed = new Set(['Button', 'IconButton', 'Link']);
// `to` answers by going to its page.
const answering = new Set(['onClick', 'clickAction', 'to', 'isDisabled', 'isLoading']);
// An overlay's trigger answers a press by opening it.
const overlays = new Set(['Popover', 'HoverCard', 'Tooltip', 'DropdownMenu']);
const fixtureOf = node => {
  while (node && !ts.isSatisfiesExpression(node)) node = node.parent;
  return node ? /^Partial<(\w+)Props>$/.exec(node.type.getText())?.[1] : undefined;
};
// A Playground fixture hands the trigger over as the overlay's `children`.
const isTrigger = node =>
  (ts.isJsxElement(node.parent) && ts.isIdentifier(node.parent.openingElement.tagName) && overlays.has(node.parent.openingElement.tagName.text)) ||
  (ts.isPropertyAssignment(node.parent) && node.parent.name.getText() === 'children' && overlays.has(fixtureOf(node.parent)));
function answers(file, report) {
  const visit = node => {
    if (ts.isArrowFunction(node) && ts.isBlock(node.body) && node.body.statements.length === 0 && !ts.isVariableDeclaration(node.parent))
      report(node, 'answer', 'an empty handler: a press must answer visibly; use Said or Held from src/example.tsx, or `unused` on a control that cannot be used');
    if ((ts.isJsxSelfClosingElement(node) || ts.isJsxOpeningElement(node)) && ts.isIdentifier(node.tagName) && pressed.has(node.tagName.text)) {
      const attributes = node.attributes.properties;
      if (!isTrigger(node) && !attributes.some(each => ts.isJsxSpreadAttribute(each) || answering.has(each.name.getText(file))))
        report(node, 'answer', `<${node.tagName.text}> has no handler: a press must answer visibly; give it onClick, with Said from src/example.tsx where nothing else shows the press`);
    }
    ts.forEachChild(node, visit);
  };
  visit(file);
}
// The gallery's hand-written pages are specimens too; its generated ones repeat the examples.
const galleryDir = resolve(core, '../gallery');
if (basename(core) === 'core' && existsSync(galleryDir)) {
  for (const path of ['content', 'shared'].flatMap(dir => walk(join(galleryDir, dir))).filter(path => path.endsWith('.tsx'))) {
    const text = readFileSync(path, 'utf8');
    if (text.startsWith('// Generated')) continue;
    const file = ts.createSourceFile(path, text, ts.ScriptTarget.Latest, true, ts.ScriptKind.TSX);
    answers(file, (node, rule, message) => problems.push(`${rel(path)}:${file.getLineAndCharacterOfPosition(node.getStart(file)).line + 1}: ${rule}: ${message}`));
  }
}

// Props a person reads or hears: in a part's source they come from the catalogue or from the app, never a literal.
const wordProps = new Set(['aria-label', 'aria-description', 'aria-roledescription', 'aria-valuetext', 'label', 'title', 'placeholder', 'alt', 'description', 'disabledMessage', 'closeLabel']);

for (const path of files) {
  const text = readFileSync(path, 'utf8');
  const file = ts.createSourceFile(path, text, ts.ScriptTarget.Latest, true, path.endsWith('.tsx') ? ts.ScriptKind.TSX : path.endsWith('.mjs') ? ts.ScriptKind.JS : ts.ScriptKind.TS);
  const role = kind(path);
  const report = (node, rule, message) => {
    const { line } = file.getLineAndCharacterOfPosition(node.getStart(file));
    problems.push(`${rel(path)}:${line + 1}: ${rule}: ${message}`);
  };

  if (role === 'examples') answers(file, report);

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
      if (role !== 'logic' && ts.isPropertyAccessExpression(node) && ts.isIdentifier(node.expression) && node.expression.text === 'Date' && node.name.text === 'now')
        report(node, 'engine', 'Date.now() in render; time arrives as a prop');
      if (role !== 'logic' && ts.isNewExpression(node) && ts.isIdentifier(node.expression) && node.expression.text === 'Date')
        report(node, 'engine', 'new Date() in render; time arrives as a prop');
      if (ts.isPropertyAccessExpression(node) && ['os', 'formFactor'].includes(node.name.text))
        report(node, 'same-on-every-os', `env.${node.name.text} must not change how a component looks or behaves`);
    }
    if (ts.isJsxOpeningElement(node) || ts.isJsxSelfClosingElement(node)) {
      const tag = node.tagName.getText(file);
      if (oldLayoutTags.has(tag)) report(node, 'layout', `<${tag}> no longer exists; use <view> with Tailwind`);
      for (const attribute of node.attributes.properties) {
        if (!ts.isJsxAttribute(attribute) || !attribute.initializer) continue;
        const prop = attribute.name.getText(file);
        const literal = ts.isStringLiteral(attribute.initializer) ? attribute.initializer : ts.isJsxExpression(attribute.initializer) && attribute.initializer.expression && ts.isStringLiteral(attribute.initializer.expression) ? attribute.initializer.expression : undefined;
        if (literal && (((tag === 'Icon' || tag === 'Glyph') && prop === 'name') || iconProps.has(prop)) && !iconNames.has(literal.text))
          report(attribute, 'icon', `${literal.text} is not an icon name; see icons/icons.json`);
        if (role === 'source' && literal && wordProps.has(prop) && /\p{L}/u.test(literal.text))
          report(attribute, 'words', `${prop}="${literal.text}" is fixed English; name a @liuli.* word with liuliText so it follows the page's language`);
        if (prop !== 'className') continue;
        const value = ts.isJsxExpression(attribute.initializer) ? attribute.initializer.expression : attribute.initializer;
        checkClassName(value, role, report);
      }
    }
    if (role === 'source' && ts.isJsxText(node) && /\p{L}/u.test(node.text))
      report(node, 'words', `"${node.text.trim()}" is fixed English; name a @liuli.* word with liuliText so it follows the page's language`);
    if (role === 'recipe') checkRecipeLeaf(node, report);
    if (role === 'source' && (ts.isInterfaceDeclaration(node) || ts.isTypeAliasDeclaration(node)) && /Props$/.test(node.name.text))
      checkProps(node, report);
    for (const token of classTokens(node, role)) {
      if (/(^|:)dark:/.test(token.text) || token.text.startsWith('dark:')) report(token.node, 'class', `${token.text}: colours follow the theme; no dark: in Liuli`);
      if (role === 'recipe' && physical.test(token.text)) report(token.node, 'rtl', `${token.text} is a physical side; use the logical form (ps-, me-, start-, text-start)`);
      if (role === 'recipe' && rawStep.test(token.text)) report(token.node, 'token', `${token.text} is a raw step; use Liuli's name (text-body, rounded-control, duration-fast, ease-standard)`);
      // An ink with alpha lays each path of an icon on its own, so its crossings draw darker; a disabled ink is an opaque role.
      if (role === 'recipe' && translucentInk.test(token.text) && (/(^|:)disabled:/.test(token.text) || token.node.parent.name?.getText() === 'disabled'))
        report(token.node, 'disabled-ink', `${token.text}: a disabled text colour has no alpha; use text-faint or text-muted`);
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
    // `value` is the one changing value's name whatever its type, a checked item's or a toggle's too.
    if (member.type?.kind === ts.SyntaxKind.BooleanKeyword && name !== 'value' && !/^(is|has)[A-Z]/.test(name))
      report(member, 'props', `${name}: a boolean prop is named is* or has*`);
  }
}

// One path per paint in each icon: shapes in separate paths are each laid on their own, so with an ink that has alpha
// their crossings draw darker. Icons are written by scripts/icons.mjs, which joins them.
const iconDir = join(src, 'Icon', 'icons');
for (const path of existsSync(iconDir) ? walk(iconDir).filter(path => path.endsWith('.ts')) : []) {
  const text = readFileSync(path, 'utf8');
  const list = /paths: \[(.*)\]/.exec(text)?.[1] ?? '';
  const stroked = (list.match(/(?:^|, )'/g) ?? []).length;
  const filled = (list.match(/fill: /g) ?? []).length;
  if (stroked > 1 || filled > 1) problems.push(`${rel(path)}: icon: ${stroked} stroked and ${filled} filled paths; one path per paint, so an ink with alpha is laid once (node scripts/icons.mjs)`);
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
  // The gallery may show exports the reference does not list: the tile specimen and the page's hero.
  for (const match of doc.matchAll(/(?:specimen|hero): '(\w+)'/g)) listed.add(match[1]);
  // `playground()` is the playground's props, not an example.
  if (/fixture: true/.test(doc)) listed.add('playground');
  if (!new RegExp(`name: '${name}'`).test(doc)) problems.push(`src/${name}/${parts.doc}: doc: name is not '${name}'`);
  for (const example of listed) if (!exported.has(example)) problems.push(`src/${name}/${parts.doc}: doc: lists example ${example}, which ${parts.examples} does not export`);
  for (const example of exported) if (!listed.has(example)) problems.push(`src/${name}/${parts.examples}: doc: ${example} is not listed in the doc`);
}

for (const problem of problems) console.log(problem);
if (problems.length) { console.log(`\n${problems.length} problem${problems.length === 1 ? '' : 's'}`); process.exit(1); }
console.log('liuli check: clean');
