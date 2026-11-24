import { build } from 'esbuild';
import ts from 'typescript';
import { collectClassAnalysis, elementKind, enclosingTags, stringLiterals } from './class-usage.mjs';
import { admissionProblems } from './admission.mjs';
import { perfWarnings } from './perf.mjs';
import { stabilizeClosures, volatileComponent, ownDeclarations } from './stable-closures.mjs';
import { spliceInserts, insertionMapComment } from './insertion-map.mjs';
import { staticStyleWarnings } from './static-style.mjs';
import { inertControlWarnings } from './inert-controls.mjs';
import { moduleJsxProblems } from './module-jsx.mjs';
import { vectorPathProblems } from './vector-paths.mjs';
import { transformedHolderProblems } from './transformed-holders.mjs';
import { sourceChecker, moduleChecker, parseSource, scriptModule, scriptLoader } from './source.mjs';
import { moduleBanner } from './compatibility.mjs';
import { compileUtilities, tailwindWarm } from './tailwind.mjs';
import { nearestSupported } from './suggest.mjs';
import { loadStylesheet, validateTokens, fontFaces } from './stylesheets.mjs';
import { checkerOptions, viewTsconfig } from './tsconfig.mjs';
import { authored, packageOf, sourceLabel, peerTokenPackages } from './packages.mjs';
import { existsSync, statSync } from 'node:fs';
import { colorProblem, seriesProblem } from '../src/style/color.js';
import { readFile, mkdir, writeFile, realpath, rename, rm } from 'node:fs/promises';
import { resolve, dirname, join, relative, isAbsolute, sep, basename } from 'node:path';
import { fileURLToPath } from 'node:url';
import { elements, kindOf, style as styleTable, commonProps, limits } from '../src/generated/contract.js';
import { classElementProblem, appliesTo, gridProblem, gridRefuses, propHint, tagHint, styleValueProblem } from '../src/style/hints.js';

const runtime = fileURLToPath(new URL('../src/internal.js', import.meta.url));
const publicEntry = fileURLToPath(new URL('../src/index.js', import.meta.url));
const jsx = fileURLToPath(new URL('../src/runtime/jsx.js', import.meta.url));
const host = fileURLToPath(new URL('../src/host.js', import.meta.url));
const runtimeRoot = dirname(runtime).toLowerCase();
const has = (object, key) => Object.prototype.hasOwnProperty.call(object, key);

// Several dev sessions build one project at once: a reader sees the old file or the new one, never half of each.
let temps = 0;
async function writeAtomic(path, contents) {
  const temp = `${path}.${process.pid}.${++temps}.tmp`;
  try {
    await writeFile(temp, contents);
    await renameOver(temp, path);
  } catch (error) {
    await rm(temp, { force: true });
    throw error;
  }
}

// On Windows a scanner or indexer briefly holds a file it just saw written, and a rename over
// it fails EPERM/EBUSY until it lets go; retry for up to a second, as npm's graceful-fs does.
async function renameOver(from, to) {
  for (let delay = 10; ; delay *= 2) {
    try { return await rename(from, to); }
    catch (error) {
      if (process.platform !== 'win32' || !['EPERM', 'EBUSY', 'EACCES'].includes(error.code) || delay > 640) throw error;
      await new Promise(resolve => setTimeout(resolve, delay));
    }
  }
}

// The inputs record is a cache: one that does not parse is rebuilt from this build's pages.
async function readRecord(path) {
  try {
    const record = JSON.parse(await readFile(path, 'utf8'));
    return record && typeof record === 'object' && !Array.isArray(record) ? record : {};
  } catch { return {}; }
}
const common = new Set(commonProps);
const editors = new Set(['input', 'textarea']);
const controls = new Set(['button', 'checkbox', 'switch', 'radio', 'link', 'link-box', 'slider']);
const focusables = new Set([...editors, ...controls, 'select', 'slider']);
// Elements that take input of their own; they may sit inside a pressable control.
const interactiveTags = new Set(['button', 'link', 'checkbox', 'switch', 'radio', 'input', 'textarea', 'select', 'slider', 'webview']);

export function acceptsProp(tag, key) {
  const element = elements[tag];
  // data-* props carry nothing but what `data-*` variants match.
  return common.has(key) || has(element.props, key) || /^data-[a-z][a-z0-9-]*$/.test(key) ||
    Boolean(element.binding && (key === 'bind' || key === 'value' || [element.binding.event, element.binding.end, element.binding.pair?.values, element.binding.pair?.labels].includes(key)));
}

export function styleProblem(tag, name) {
  if (!has(styleTable, name) || styleTable[name].internal) return `UI_STYLE: unknown style property ${name}`;
  if (!kindOf(tag).style.includes(name)) return `UI_STYLE: ${name} is not supported on <${kindOf(tag).of ?? tag}>${appliesTo(name)}`;
}

const position = (file, node) => {
  const { line, character } = file.getLineAndCharacterOfPosition(node.getStart(file));
  return `${line + 1}:${character + 1}`;
};

/** Every authoring problem in one module, as `file:line:col: CODE: message`. */
// Controls a screen reader announces need a name; the fix is named in the message.
const labelled = new Set(['input', 'textarea', 'slider', 'radio', 'plot', 'select', 'dialog', 'webview']);
const contentNamed = new Set(['button', 'link', 'checkbox', 'switch']);
function unnamed(tag, node, file) {
  const attributes = node.attributes.properties;
  if (attributes.some(ts.isJsxSpreadAttribute)) return undefined;
  const has = name => attributes.some(prop => prop.name?.getText(file) === name);
  if (tag === 'image' && !has('alt')) return 'UI_A11Y: <image> needs alt (describe it, or alt="" when decorative)';
  if (labelled.has(tag) && !has('aria-label'))
    return `UI_A11Y: <${tag}> needs an accessible name; add aria-label="…"${editors.has(tag) ? ' (a placeholder is a hint, not a name)' : ''}`;
  if (contentNamed.has(tag) && !has('aria-label') && ts.isJsxOpeningElement(node) && !spoken(node.parent))
    return `UI_A11Y: <${tag}> has no text to name it; add text or aria-label="…"`;
  if (contentNamed.has(tag) && !has('aria-label') && ts.isJsxSelfClosingElement(node))
    return `UI_A11Y: <${tag}> has no text to name it; add text or aria-label="…"`;
  return undefined;
}
// Whether children may produce text: literal text, an expression, or a text element.
function spoken(element) {
  return element.children.some(child => {
    if (ts.isJsxText(child)) return child.text.trim() !== '';
    if (ts.isJsxExpression(child)) return Boolean(child.expression);
    if (ts.isJsxElement(child)) {
      const name = child.openingElement.tagName.getText();
      return ['text', 'span'].includes(name) || /^[A-Z]/.test(name) || spoken(child);
    }
    if (ts.isJsxSelfClosingElement(child)) return /^[A-Z]/.test(child.tagName.getText());
    return false;
  });
}

function numericStyleLiterals(checker, node, seen = new Set()) {
  while (node && (ts.isParenthesizedExpression(node) || ts.isAsExpression(node) || ts.isSatisfiesExpression(node) || ts.isNonNullExpression(node)))
    node = node.expression;
  if (!node || seen.has(node)) return [];
  seen = new Set(seen).add(node);
  if (ts.isNumericLiteral(node)) return [Number(node.text)];
  if (ts.isPrefixUnaryExpression(node) && (node.operator === ts.SyntaxKind.MinusToken || node.operator === ts.SyntaxKind.PlusToken)) {
    let inner = node.operand;
    while (inner && ts.isParenthesizedExpression(inner)) inner = inner.expression;
    if (inner && ts.isNumericLiteral(inner)) return [node.operator === ts.SyntaxKind.MinusToken ? -Number(inner.text) : Number(inner.text)];
  }
  if (ts.isConditionalExpression(node)) return [...numericStyleLiterals(checker, node.whenTrue, seen), ...numericStyleLiterals(checker, node.whenFalse, seen)];
  if (ts.isIdentifier(node)) {
    let sym = checker.getSymbolAtLocation(node);
    if (sym && (sym.flags & ts.SymbolFlags.Alias)) sym = checker.getAliasedSymbol(sym);
    const decl = sym?.valueDeclaration;
    if (decl && ts.isVariableDeclaration(decl) && (decl.parent.flags & ts.NodeFlags.Const) && decl.initializer)
      return numericStyleLiterals(checker, decl.initializer, seen);
  }
  return [];
}

// Whether a fragment is what the exported `render` returns: the page root must be one box.
function pageRoot(node) {
  let at = node;
  while (ts.isParenthesizedExpression(at.parent) || ts.isConditionalExpression(at.parent) || ts.isBinaryExpression(at.parent)) at = at.parent;
  const owner = ts.isReturnStatement(at.parent) ? ts.findAncestor(at.parent, ts.isFunctionLike) : ts.isArrowFunction(at.parent) && at.parent.body === at ? at.parent : undefined;
  if (!owner) return false;
  const name = owner.name?.getText() ?? (ts.isVariableDeclaration(owner.parent) ? owner.parent.name.getText() : undefined);
  return name === 'render';
}

export function audit(source, filename, { literalColors = false } = {}) {
  const file = parseSource(filename, source);
  const checker = sourceChecker(file);
  const problems = [];
  const report = (node, message) => problems.push(`${filename}:${position(file, node)}: ${message}`);
  const visit = node => {
    if (ts.isJsxFragment(node) && pageRoot(node)) report(node, 'UI_FRAGMENT: a page returns one layout node; put the fragment inside <page>');
    if (ts.isJsxElement(node) && ['button', 'checkbox'].includes(node.openingElement.tagName.getText(file))) {
      const content = node.children.filter(child => !(ts.isJsxText(child) && !child.text.trim()));
      const only = content.length === 1 && (ts.isJsxElement(content[0]) || ts.isJsxSelfClosingElement(content[0])) ? content[0] : undefined;
      const inner = only && (ts.isJsxElement(only) ? only.openingElement.tagName : only.tagName).getText(file);
      if (inner && interactiveTags.has(inner))
        report(node.openingElement, `UI_BUTTON: <${node.openingElement.tagName.getText(file)}> holds only a <${inner}>; put the action on that control`);
    }
    if ((ts.isJsxOpeningElement(node) || ts.isJsxSelfClosingElement(node)) && ts.isIdentifier(node.tagName) && /^[A-Z]/.test(node.tagName.text)) {
      // `<Part key={Part.name}>`: release builds rename functions, so the key and every local
      // state under it would change between builds.
      for (const prop of node.attributes.properties) {
        const value = ts.isJsxAttribute(prop) && prop.name.getText(file) === 'key' && prop.initializer && ts.isJsxExpression(prop.initializer) && prop.initializer.expression;
        if (value && ts.isPropertyAccessExpression(value) && value.name.text === 'name' && ts.isIdentifier(value.expression) && value.expression.text === node.tagName.text)
          report(prop, `UI_KEY: ${node.tagName.text}.name is not a stable key; release builds rename functions, so key by a label or index`);
      }
    }
    if ((ts.isJsxOpeningElement(node) || ts.isJsxSelfClosingElement(node)) && ts.isIdentifier(node.tagName) && /^[a-z]/.test(node.tagName.text)) {
      const tag = node.tagName.text;
      // An element behind a component (`<list>` behind `<List>`) is not written as JSX.
      const intrinsic = has(elements, tag) && !elements[tag].component;
      const problem = intrinsic && unnamed(tag, node, file);
      if (problem) report(node, problem);
      if (intrinsic) for (const [at, message] of admissionProblems(node, file)) report(at, message);
      if (tag === 'link') {
        const named = name => node.attributes.properties.some(prop => ts.isJsxSpreadAttribute(prop) || prop.name.getText(file) === name);
        if (!named('to') && !named('onClick') && !named('popoverTarget') && !named('interestFor')) report(node, 'UI_LINK: a <link> needs to, onClick, popoverTarget or interestFor, disabled or not; disabled only stops the press');
        const around = enclosingTags(node);
        const text = around.findIndex(name => name === 'text');
        if (elementKind(node) === 'link-box' && around.slice(0, text < 0 ? undefined : text).includes('link'))
          report(node, 'UI_LINK: a block link cannot sit inside another block link');
      }
      if (!intrinsic) report(node, `UI_TAG: unsupported tag <${tag}>${tag === 'list' ? '; use <List> from @lingxia/view' : tagHint(tag)}`);
      else for (const prop of node.attributes.properties) {
        if (ts.isJsxSpreadAttribute(prop)) {
          for (const symbol of checker.getTypeAtLocation(prop.expression).getProperties())
            if (!acceptsProp(tag, symbol.getName())) report(prop, `UI_PROP: ${symbol.getName()} is not a property of <${tag}> (spread)${propHint(symbol.getName())}`);
          continue;
        }
        const key = prop.name.getText(file);
        if (!acceptsProp(tag, key))
          report(prop, `UI_PROP: ${key} is not a property of <${tag}>${propHint(key)}`);
        const series = tag === 'plot' && key === 'colors' && prop.initializer && ts.isJsxExpression(prop.initializer) && prop.initializer.expression;
        if (!literalColors && series && ts.isArrayLiteralExpression(series)) for (const item of series.elements) {
          const problem = ts.isStringLiteralLike(item) && seriesProblem(item.text);
          if (problem) report(item, problem);
        }
        const style = key === 'style' && prop.initializer && ts.isJsxExpression(prop.initializer) && prop.initializer.expression;
        if (style && ts.isObjectLiteralExpression(style)) for (const entry of style.properties) {
          if (!ts.isPropertyAssignment(entry) && !ts.isShorthandPropertyAssignment(entry)) continue;
          const name = entry.name.getText(file).replace(/['"]/g, '');
          if (name === 'animation') report(entry, 'UI_ANIMATION: inline animation is not supported; use animate-* utilities');
          else { const problem = styleProblem(elementKind(node), name); if (problem) report(entry, problem); }
          const literal = ts.isPropertyAssignment(entry) && ts.isStringLiteralLike(entry.initializer) && entry.initializer.text;
          const colorIssue = !literalColors && literal !== false && styleTable[name]?.lower === 'color' && colorProblem(literal, name);
          if (colorIssue) report(entry, colorIssue);
          const valueExpr = ts.isPropertyAssignment(entry) ? entry.initializer : entry.name;
          for (const number of numericStyleLiterals(checker, valueExpr)) {
            const valueIssue = styleValueProblem(name, number);
            if (valueIssue) report(entry, valueIssue);
          }
          for (const strNode of stringLiterals(checker, valueExpr)) {
            const match = strNode.text.trim().match(/^(-?(?:\d+(?:\.\d+)?|\.\d+))%$/);
            if (match) {
              const valueIssue = styleValueProblem(name, Number(match[1]));
              if (valueIssue) report(entry, valueIssue);
            }
          }
        }
      }
    }
    if (contextCall(node) && !moduleContext(node.parent)) report(node, 'UI_CONTEXT: createContext belongs at module top level, as a const');
    if (ts.isIdentifier(node) && ['window', 'document', 'HTMLElement', 'localStorage'].includes(node.text)) {
      const parent = node.parent;
      const propertyName = parent.name === node && !ts.isShorthandPropertyAssignment(parent);
      const symbol = ts.isShorthandPropertyAssignment(parent) ? checker.getShorthandAssignmentValueSymbol(parent) : checker.getSymbolAtLocation(node);
      if (!propertyName && !symbol) report(node, `UI_DOM: ${node.text} is unavailable in GPU Views`);
    }
    // `@lingxia/logic` carries the host's `lx`; a View imports only its types.
    if (ts.isImportDeclaration(node) && ts.isStringLiteral(node.moduleSpecifier) && node.moduleSpecifier.text === '@lingxia/logic') {
      const clause = node.importClause;
      const values = clause && !clause.isTypeOnly && (clause.name || (clause.namedBindings && (ts.isNamespaceImport(clause.namedBindings) || clause.namedBindings.elements.some(element => !element.isTypeOnly))));
      if (values) report(node, "UI_LX: @lingxia/logic is Logic-only; a View imports its types with `import type`, and does a call in a Logic action");
    }
    // `lx` is Logic's; a View reaches it only through an action. A local binding named lx is fine.
    if (ts.isIdentifier(node) && node.text === 'lx') {
      const parent = node.parent;
      const propertyName = parent.name === node && !ts.isShorthandPropertyAssignment(parent);
      const symbol = ts.isShorthandPropertyAssignment(parent) ? checker.getShorthandAssignmentValueSymbol(parent) : checker.getSymbolAtLocation(node);
      const ambient = !symbol || (symbol.declarations ?? []).every(declaration => declaration.getSourceFile().isDeclarationFile);
      if (!propertyName && ambient) report(node, 'UI_LX: lx is Logic-only; do the call in a Logic action and call the action from the View');
    }
    ts.forEachChild(node, visit);
  };
  visit(file);
  return problems;
}

const contextCall = node => ts.isCallExpression(node) && ts.isIdentifier(node.expression) && node.expression.text === 'createContext';
// `const X = createContext(…)` directly in a module: the only form with a stable identity to stamp.
const moduleContext = declaration => ts.isVariableDeclaration(declaration) && ts.isIdentifier(declaration.name) &&
  ts.isVariableStatement(declaration.parent.parent) && ts.isSourceFile(declaration.parent.parent.parent) &&
  Boolean(declaration.parent.flags & ts.NodeFlags.Const);

// Each JSX site carries its source position so runtime errors name the line that built the node.
export function annotate(source, filename, label, log) {
  const file = parseSource(filename, source);
  const inserts = [];
  const visit = node => {
    if (ts.isJsxOpeningElement(node) || ts.isJsxSelfClosingElement(node)) {
      const at = (node.typeArguments ?? node.tagName).end;
      inserts.push([at, ` __loc={${JSON.stringify(`${label}:${position(file, node)}`)}}`]);
    }
    ts.forEachChild(node, visit);
  };
  visit(file);
  return spliceInserts(source, inserts.sort((a, b) => b[0] - a[0]), log);
}

/**
 * Gives each `useId()` call its site, `<enclosing function>#<ordinal>`: ids then survive edits
 * elsewhere in the module and hot swaps.
 */
export function stampIds(source, filename, log) {
  if (!source.includes('useId(')) return source;
  const file = parseSource(filename, source);
  const inserts = [], counts = new Map();
  const visit = node => {
    if (ts.isCallExpression(node) && ts.isIdentifier(node.expression) && node.expression.text === 'useId' && !node.arguments.length) {
      const owner = ts.findAncestor(node.parent, n => ts.isFunctionLike(n));
      const name = owner?.name?.getText(file) ?? (owner && ts.isVariableDeclaration(owner.parent) ? owner.parent.name.getText(file) : '') ?? '';
      const ordinal = counts.get(owner) ?? 0;
      counts.set(owner, ordinal + 1);
      inserts.push([node.arguments.end, JSON.stringify(`${name}#${ordinal}`)]);
    }
    ts.forEachChild(node, visit);
  };
  visit(file);
  return spliceInserts(source, inserts.sort((a, b) => b[0] - a[0]), log);
}

/**
 * Stamps each top-level component (a capitalized function declaration, or a const bound to a
 * function) and each module-level context with `__lxid`, `<module path without extension>#<name>`:
 * local state and context lookup are keyed by it, so minified and dev builds of one source agree.
 */
export function identify(source, filename, label) {
  const file = parseSource(filename, source);
  const module = label.replace(scriptModule, '');
  const names = [], volatile = [], moduleNames = ownDeclarations(file);
  for (const statement of file.statements) {
    if (ts.isFunctionDeclaration(statement) && statement.name && /^[A-Z]/.test(statement.name.text) && statement.body) {
      names.push(statement.name.text);
      if (volatileComponent(statement, moduleNames)) volatile.push(statement.name.text);
    }
    if (ts.isVariableStatement(statement) && (statement.declarationList.flags & ts.NodeFlags.Const))
      for (const declaration of statement.declarationList.declarations)
        if (ts.isIdentifier(declaration.name) && /^[A-Z]/.test(declaration.name.text) && declaration.initializer &&
            (ts.isArrowFunction(declaration.initializer) || ts.isFunctionExpression(declaration.initializer))) {
          names.push(declaration.name.text);
          if (volatileComponent(declaration.initializer, moduleNames)) volatile.push(declaration.name.text);
        } else if (declaration.initializer && contextCall(declaration.initializer) && moduleContext(declaration)) names.push(declaration.name.text);
  }
  if (!names.length) return source;
  // A component that reads the clock, randomness or a module `let` re-runs on every render.
  return `${source}\n${names.map(name => `${name}.__lxid = ${JSON.stringify(`${module}#${name}`)};`).join('\n')}\n` +
    volatile.map(name => `${name}.__lxVolatile = true;\n`).join('');
}

function classProblems(rule, name, tag, suggestions, grid, focusable = false) {
  if (!rule) {
    const nearest = suggestions.get(name);
    return [`UI_CLASS_UNKNOWN: ${name} is not a Tailwind utility${nearest && nearest !== name ? `; nearest supported: ${nearest}` : ''}`];
  }
  if (rule.error?.category === 'color') return [`UI_COLOR: ${name}: ${rule.error.message}`];
  if (rule.error) return [`UI_CLASS_UNSUPPORTED: ${name}: the native renderer does not support ${rule.error.message}`];
  if (!kindOf(tag) || rule.when?.children) return [];
  // A part's properties were checked when it compiled; it needs a control that draws it.
  if (rule.when?.part) {
    const { part, checked } = rule.when;
    if (tag === 'switch' && (part === 'track' || part === 'thumb')) return [];
    if (tag !== 'slider') return [`UI_CLASS_ELEMENT: ${name} styles ${part === 'track' || part === 'thumb' ? 'a <slider> or <switch>' : 'a <slider>'}'s ${part}, not <${tag}>`];
    if (checked) return [`UI_CLASS_ELEMENT: ${name}: a <slider> is never checked; checked: goes on a <switch> part`];
    return part === 'track' && rule.style?.width !== undefined ? [`UI_CLASS_ELEMENT: ${name}: a <slider> track spans the slider; size the slider instead`] : [];
  }
  const runs = tag === 'span' || tag === 'link';
  const props = [...Object.keys(rule.style ?? {}).filter(prop => !(runs && rule.pairedLeading && prop === 'lineHeight')), ...(rule.gradient ? ['backgroundImage'] : []),
    ...(rule.transform?.translate ? ['translate'] : []), ...(rule.transform?.scale ? ['scale'] : []),
    ...(rule.numeric ? ['fontVariantNumeric'] : []),
    ...(rule.shadow ? ['boxShadow'] : []), ...(rule.backdrop ? ['backdropFilter'] : [])];
  const state = rule.when?.state;
  const unsupported = props.find(prop => !kindOf(tag).style.includes(prop));
  const problems = unsupported ? [classElementProblem(name, unsupported, tag)] : [];
  const flexOnly = grid && !Object.keys(rule.when ?? {}).length && gridRefuses(props);
  if (flexOnly) problems.push(gridProblem(name, flexOnly));
  if (state === 'focus' && !editors.has(tag) && tag !== 'select') problems.push(`UI_CLASS_ELEMENT: ${name} needs an editor or <select>; <${tag}> never takes focus`);
  if (state === 'focusVisible' && !focusables.has(tag) && !focusable) problems.push(`UI_CLASS_ELEMENT: ${name} needs an element that takes focus; <${tag}> never does`);
  if (state === 'active' && !controls.has(tag)) problems.push(`UI_CLASS_ELEMENT: ${name} needs a control; <${tag}> is never pressed`);
  const layout = state && state !== 'disabled' ? props.filter(prop => !styleTable[prop]?.paint) : [];
  if (layout.length) problems.push(`UI_CLASS_ELEMENT: ${name}: ${state} styles change paint only, not ${layout.join(', ')}`);
  if (state === 'disabled' && !has(kindOf(tag).props, 'disabled')) problems.push(`UI_CLASS_ELEMENT: ${name} needs a disableable element, not <${tag}>`);
  return problems;
}

// Problems read top-down: the page first, then the files it imports, each in
// line order; one message found in several places is listed once with every place.
function report(problems, page) {
  const located = [], loose = [];
  for (const line of new Set(problems)) {
    const match = line.match(/^(.*?):(\d+):(\d+): (.*)$/s);
    if (match) located.push({ file: match[1], line: Number(match[2]), column: Number(match[3]), message: match[4] });
    else loose.push(line);
  }
  const files = [...new Set([page, ...located.map(problem => problem.file)])];
  located.sort((a, b) => files.indexOf(a.file) - files.indexOf(b.file) || a.line - b.line || a.column - b.column);
  const grouped = new Map();
  for (const problem of located) {
    const key = `${problem.file}\u0000${problem.message}`;
    if (!grouped.has(key)) grouped.set(key, { ...problem, places: [] });
    grouped.get(key).places.push(`${problem.line}:${problem.column}`);
  }
  const text = [...loose, ...[...grouped.values()].map(({ file, places, message }) => `${file}:${places.join(', ')}: ${message}`)];
  return { count: text.length, text: text.join('\n') };
}

// A warm dev compiler rebuilds the files of one page per save; unchanged ones keep their analysis.
/** Whether a callee is `cx` from `@lingxia/view` in `file`, under any local or namespace name. */
function cxCallee(file) {
  const named = new Set(), namespaces = new Set();
  for (const statement of file.statements) {
    if (!ts.isImportDeclaration(statement) || !ts.isStringLiteral(statement.moduleSpecifier) || statement.moduleSpecifier.text !== '@lingxia/view') continue;
    const bindings = statement.importClause?.namedBindings;
    if (bindings && ts.isNamespaceImport(bindings)) namespaces.add(bindings.name.text);
    else if (bindings) for (const element of bindings.elements) if ((element.propertyName ?? element.name).text === 'cx') named.add(element.name.text);
  }
  return callee => ts.isIdentifier(callee) ? named.has(callee.text)
    : ts.isPropertyAccessExpression(callee) && ts.isIdentifier(callee.expression) && namespaces.has(callee.expression.text) && callee.name.text === 'cx';
}

// Identifier and shorthand keys of `cx` maps are class names no string literal spells; a computed key is run-time.
function cxMapKeys(node, names) {
  while (ts.isParenthesizedExpression(node) || ts.isAsExpression(node) || ts.isSatisfiesExpression(node) || ts.isNonNullExpression(node)) node = node.expression;
  if (ts.isArrayLiteralExpression(node)) for (const element of node.elements) cxMapKeys(element, names);
  else if (ts.isConditionalExpression(node)) { cxMapKeys(node.whenTrue, names); cxMapKeys(node.whenFalse, names); }
  else if (ts.isBinaryExpression(node) && [ts.SyntaxKind.AmpersandAmpersandToken, ts.SyntaxKind.BarBarToken, ts.SyntaxKind.QuestionQuestionToken].includes(node.operatorToken.kind)) {
    cxMapKeys(node.left, names); cxMapKeys(node.right, names);
  } else if (ts.isObjectLiteralExpression(node))
    for (const property of node.properties)
      if ((ts.isPropertyAssignment(property) || ts.isShorthandPropertyAssignment(property)) && ts.isIdentifier(property.name)) names.add(property.name.text);
}

const analyses = new Map();
let analysisHits = 0, analysisMisses = 0;
// `label` names the file in source positions, `id` in component and closure ids.
// `dev` builds map the transformed text back to the author's for esbuild's source map.
function analyse(path, contents, label, id, loader, literalColors, dev) {
  const key = `${literalColors}\0${dev}\0${label}\0${id}\0${path}`;
  const cached = analyses.get(key);
  if (cached?.contents === contents) { analysisHits++; return cached; }
  analysisMisses++;
  const file = parseSource(path, contents);
  const names = new Set();
  const cx = cxCallee(file);
  const collect = node => {
    if (ts.isStringLiteralLike(node) || ts.isTemplateHead(node) || ts.isTemplateMiddle(node) || ts.isTemplateTail(node))
      for (const name of node.text.split(/\s+/)) if (name) names.add(name);
    if (ts.isCallExpression(node) && cx(node.expression)) for (const argument of node.arguments) cxMapKeys(argument, names);
    ts.forEachChild(node, collect);
  };
  collect(file);
  const stages = dev ? [] : undefined;
  const stamped = stampIds(contents, path, stages);
  const transformed = loader.endsWith('x') ? stabilizeClosures(annotate(stamped, path, id, stages), path, id, stages) : stamped;
  const annotated = identify(transformed, path, id);
  const analysed = {
    contents, file, names, problems: audit(contents, path, { literalColors }),
    annotated: dev ? annotated + insertionMapComment(contents, annotated, stages, basename(path)) : annotated,
  };
  analyses.delete(key);
  if (analyses.size >= 512) analyses.delete(analyses.keys().next().value);
  analyses.set(key, analysed);
  return analysed;
}

// Problems name a file as runtime errors do: the lxapp's by path, another package's by its name.
function packageRelative(root, line) {
  const match = /^(.+?\.(?:[cm]?[jt]sx?|css)):(\d+)/.exec(line);
  return match && isAbsolute(match[1]) ? sourceLabel(root, match[1]) + line.slice(match[1].length) : line;
}

/**
 * The `@theme` tokens of each package a page bundles that declares them (`"lingxia": { "tokens": … }`),
 * in package-name order. A package adds scale and motion tokens and `@utility` classes; colours are
 * the app's theme pack, and two packages may not give one token different values.
 */
async function packageTokens(root, packages, problems) {
  const sheets = [], owners = new Map();
  for (const { dir, name, tokens } of packages.values()) {
    const path = typeof tokens === 'string' ? resolve(dir, tokens) : '';
    const inside = relative(dir, path);
    if (!path || inside.startsWith('..') || isAbsolute(inside) || !existsSync(path)) {
      problems.push(`${name}/package.json: UI_CSS: lingxia.tokens names a stylesheet inside the package that exists, not ${JSON.stringify(tokens)}`);
      continue;
    }
    const label = sourceLabel(root, path);
    let sheet;
    try {
      sheet = await loadStylesheet(path);
      validateTokens(sheet);
    } catch (error) {
      problems.push(`${label}:${error.line ?? 1}: ${error.reason ?? error.message}`);
      continue;
    }
    let clean = true;
    for (const node of sheet.nodes) {
      if (node.type === 'comment' || (node.type === 'atrule' && ['theme', 'utility'].includes(node.name))) continue;
      problems.push(`${label}:${node.source?.start?.line ?? 1}: UI_CSS: a package's tokens are @theme and @utility only; colours and faces belong to the app`);
      clean = false;
    }
    sheet.walkAtRules('theme', rule => rule.each(decl => {
      if (decl.type !== 'decl') return;
      const other = owners.get(decl.prop);
      if (other && other.value !== decl.value) {
        problems.push(`${label}:${decl.source?.start?.line ?? 1}: UI_CSS: ${decl.prop} is ${decl.value} here and ${other.value} in ${other.name}; set it once in the app's stylesheet`);
        clean = false;
      }
      owners.set(decl.prop, { name, value: decl.value });
    }));
    if (clean) sheets.push(sheet.toString());
  }
  return sheets;
}

/**
 * Compiles GPU pages. `pages` are page source paths relative to `root`, the pages of an lxapp whose
 * Views export `render`. Output is `<base>.js` beside a copy of the page config,
 * when the page has one; a page config naming a renderer is refused.
 * Returns the `UI_PERF`, `UI_STYLE_STATIC`, `UI_INERT` and entering/leaving `UI_TRANSFORM` warnings (also printed; they never stop the build), whether Tailwind ran
 * warm, how many files were analysed afresh, and `untracedClasses`: each place a className cannot be
 * followed to literals (`{ location, expression, reason }`).
 */
/**
 * `literalColors` admits hex and palette colours for renderer test fixtures whose probes read exact
 * pixels; app builds never set it. `strictClasses` warns at every className not followed to literals,
 * not only at those built at run time. `messages` is the compiled i18n module to bundle and
 * `inputsRecord` the file a dev build records its inputs in; both default under `<root>/.lingxia`.
 */
export async function buildPages(root, pages, output, catalogs = {}, { dev = false, literalColors = false, strictClasses = false, messages: messagesPath, inputsRecord } = {}) {
  const { version } = JSON.parse(await readFile(new URL('../package.json', import.meta.url), 'utf8'));
  root = await realpath(resolve(root)); output = resolve(output);
  // Every page's authoring problems are collected, so one build names all of them.
  const inputs = {}, warnings = [], failures = [];
  // Each place a className cannot be followed to literals, once per build.
  const untracedClasses = new Map();
  const tsconfig = viewTsconfig(root);
  // Whether the theme's Tailwind compile was warm, and how many files needed fresh analysis.
  let warm = true;
  analysisHits = analysisMisses = 0;
  // The lxapp build compiles i18n/ here; registering it before the page module loads lets
  // `t` run in the first render.
  const compiledMessages = messagesPath ? resolve(root, messagesPath) : join(root, '.lingxia', 'i18n', 'index.js');
  const messages = existsSync(compiledMessages)
    ? `import * as messages from ${JSON.stringify(compiledMessages)}; import { registerCatalogues } from ${JSON.stringify(runtime)}; registerCatalogues(messages);`
    : '';
  for (const page of pages) {
    const source = resolve(root, page);
    const rel = relative(root, source);
    const base = page.replace(/\.[jt]sx?$/, '');
    if (rel.startsWith('..') || isAbsolute(rel) || base === page) throw Error(`Invalid GPU page path: ${page}`);
    const configPath = join(root, `${base}.json`);
    const config = existsSync(configPath) ? JSON.parse(await readFile(configPath, 'utf8')) : null;
    if (config && 'renderer' in config) throw Error(`${base}.json: renderer: remove it; the renderer follows from the pages`);
    const catalog = catalogs[page] ?? { __names: [], __modes: {} };
    if (!Array.isArray(catalog.__names) || catalog.__names.some(name => typeof name !== 'string' || !['call', 'stream'].includes(catalog.__modes?.[name])))
      throw Error(`${page}: invalid GPU action metadata`);
    const candidates = new Set(), themes = new Map(), sourceFiles = [], problems = [], fonts = [], bundledPackages = new Map();
    let bundled;
    try {
      bundled = await build({
        stdin: { contents: `${messages}import { render } from ${JSON.stringify(source)}; import { publishHost } from ${JSON.stringify(host)}; export { configureClasses, configureFonts, configureSourceMap, enableHotSwap } from ${JSON.stringify(runtime)}; publishHost(render, ${JSON.stringify(catalog)}, ${JSON.stringify(base.split('\\').join('/'))});`, resolveDir: root },
        bundle: true, write: false, format: 'esm', target: 'es2020', jsxFactory: 'h', jsxFragment: 'Fragment', inject: [jsx], logLevel: 'silent',
        tsconfigRaw: tsconfig,
        // Dev builds keep a source map so runtime errors name the author's line.
        ...(dev ? { sourcemap: 'external', absWorkingDir: root, outfile: join(root, 'page.js'), metafile: true } : {}),
        alias: { '@lingxia/view': publicEntry },
        plugins: [{ name: 'lingxia-view-language', setup(builder) {
          builder.onLoad({ filter: /\.css$/ }, async ({ path }) => {
            const stylesheet = await loadStylesheet(path);
            validateTokens(stylesheet);
            themes.set(path, stylesheet.toString());
            // Declared faces ship in the lxapp and load by their lxapp path.
            for (const face of fontFaces(stylesheet)) {
              const file = isAbsolute(face.url) ? face.url : resolve(dirname(path), face.url);
              const inside = relative(root, file).split('\\').join('/');
              if (inside.startsWith('..') || isAbsolute(inside)) problems.push(`${path}:${face.line}: UI_FONT: ${inside} is outside the lxapp; bundle the face file with it`);
              else if (!existsSync(file)) problems.push(`${path}:${face.line}: UI_FONT: ${inside} is declared by @font-face but missing`);
              // Faces load through the image transport and share its per-file cap.
              else if (statSync(file).size > limits.imageBytes) problems.push(`${path}:${face.line}: UI_FONT: ${inside} is over ${limits.imageBytes >> 20} MiB; subset the face`);
              else fonts.push({ family: face.family, src: inside, ...(face.weight ? { weight: face.weight } : {}), ...(face.style ? { style: face.style } : {}) });
            }
            return { contents: '', loader: 'js' };
          });
          // A JSON file the page imports can hold class names; esbuild still loads it.
          builder.onLoad({ filter: /\.json$/ }, async ({ path }) => {
            const walk = value => {
              if (typeof value === 'string') { for (const name of value.split(/\s+/)) if (name) candidates.add(name); }
              else if (value && typeof value === 'object') Object.values(value).forEach(walk);
            };
            // Malformed JSON is esbuild's to report.
            try { walk(JSON.parse(await readFile(path, 'utf8'))); } catch {}
            return undefined;
          });
          builder.onLoad({ filter: scriptModule }, async ({ path }) => {
            if (path.toLowerCase().startsWith(runtimeRoot) || path === compiledMessages || dirname(path) === dirname(compiledMessages)) return;
            if (!authored(root, path)) {
              const owner = packageOf(path);
              if (owner) bundledPackages.set(owner.name, owner);
            }
            const contents = await readFile(path, 'utf8');
            const loader = scriptLoader(path);
            const label = relative(root, path).split('\\').join('/');
            const analysed = analyse(path, contents, label, sourceLabel(root, path), loader, literalColors, dev);
            problems.push(...analysed.problems);
            sourceFiles.push(analysed.file);
            for (const name of analysed.names) candidates.add(name);
            return { contents: analysed.annotated, loader, resolveDir: dirname(path) };
          });
        } }],
        banner: { js: moduleBanner(version) },
      });
    } catch (error) {
      if (!problems.length) throw error;
    }
    if (themes.size > 1) problems.push(`${page}: UI_CSS: import one shared @theme stylesheet per page`);
    const tokenPackages = peerTokenPackages(root, bundledPackages, problems);
    // Packages' tokens sit below the app's own, so the app's stylesheet overrides any of them.
    const theme = [...await packageTokens(root, tokenPackages, problems), ...themes.values()].join('\n');
    warm &&= tailwindWarm(theme);
    const checker = moduleChecker(sourceFiles, checkerOptions(root));
    const { usages, untraced } = collectClassAnalysis(sourceFiles, checker);
    for (const place of untraced) {
      const location = packageRelative(root, place.location);
      untracedClasses.set(`${location}\0${place.reason}`, { ...place, location });
    }
    // A className followed to literals in full adds its classes, though no single string spells them
    // (a template over known parts, a cx map passed by name); one with any untraced part adds none.
    for (const { value, complete } of usages) if (complete) for (const name of value.split(/\s+/)) if (name) candidates.add(name);
    // `font-sans` lowers the theme's `--font-sans`; a non-default one is the page's family.
    const classes = await compileUtilities(new Set([...candidates, 'font-sans']), theme, { literalColors });
    const sans = classes.find(rule => rule.name === 'font-sans');
    if (sans?.error && !candidates.has('font-sans')) problems.push(`${page}: UI_FONT: --font-sans: ${sans.error.message}`);
    const pageFamily = sans?.style?.fontFamily === 'sans-serif' ? undefined : sans?.style?.fontFamily;
    const rulesByName = new Map();
    for (const rule of classes) rulesByName.set(rule.name, [...(rulesByName.get(rule.name) ?? []), rule]);
    const unknown = [...new Set(usages.flatMap(({ value }) => value.split(/\s+/)).filter(name => name && !rulesByName.has(name)))];
    const suggestions = await nearestSupported(unknown, theme);
    // A name the checks reach but the collection missed is a valid utility the table lacks, not a typo.
    const uncollected = new Map(unknown.length ? (await compileUtilities(new Set(unknown), theme, { literalColors })).map(rule => [rule.name, rule]) : []);
    for (const { value, location, tag, focusable } of usages) {
      const names = value.split(/\s+/).filter(Boolean);
      const grid = names.includes('grid');
      for (const name of names) {
        const late = rulesByName.has(name) ? undefined : uncollected.get(name);
        if (late && !late.error) {
          problems.push(`${location}: UI_CLASS_UNKNOWN: ${name} is a utility, but the build found it in no string of this page's sources, so it was not compiled; write '${name}' as one whole word in a string literal`);
          continue;
        }
        const found = rulesByName.get(name) ?? [late];
        for (const problem of new Set(found.flatMap(rule => classProblems(rule, name, tag, suggestions, grid, focusable)))) problems.push(`${location}: ${problem}`);
      }
    }
    const ownFile = file => !file.fileName.includes(`${sep}node_modules${sep}`);
    const transformed = transformedHolderProblems(sourceFiles, checker, rulesByName, ownFile);
    problems.push(...transformed.errors);
    problems.push(...moduleJsxProblems(sourceFiles.filter(ownFile)));
    problems.push(...vectorPathProblems(sourceFiles.filter(ownFile)));
    if (problems.length) {
      const lines = report(problems.map(line => packageRelative(root, line)), rel);
      failures.push(`${page}: ${lines.count} authoring problem${lines.count === 1 ? '' : 's'}\n${lines.text}`);
      continue;
    }
    const perf = await perfWarnings(root, sourceFiles, checker, rulesByName);
    if (perf.length) {
      const lines = report(perf.map(line => packageRelative(root, line)), rel);
      warnings.push(...lines.text.split('\n'));
      console.warn(`${page}: ${lines.count} performance warning${lines.count === 1 ? '' : 's'}\n${lines.text}`);
    }
    const own = sourceFiles.filter(ownFile);
    const inert = [...inertControlWarnings(own), ...transformed.warnings];
    if (inert.length) {
      const lines = report(inert.map(line => packageRelative(root, line)), rel);
      warnings.push(...lines.text.split('\n'));
      console.warn(`${page}: ${lines.count} control warning${lines.count === 1 ? '' : 's'}\n${lines.text}`);
    }
    const statics = staticStyleWarnings(own);
    if (statics.length) {
      const lines = report(statics.map(line => packageRelative(root, line)), rel);
      warnings.push(...lines.text.split('\n'));
      console.warn(`${page}: ${lines.count} style warning${lines.count === 1 ? '' : 's'}\n${lines.text}`);
    }
    const dest = join(output, `${base}.js`);
    await mkdir(dirname(dest), { recursive: true });
    const code = bundled.outputFiles.find(file => !file.path.endsWith('.map'));
    const map = bundled.outputFiles.find(file => file.path.endsWith('.map'));
    let module = code.text.replace(/\n\/\/# sourceMappingURL=.*\n?$/, '\n') + `\nconfigureClasses(${JSON.stringify(classes)});\n` +
      (fonts.length || pageFamily ? `configureFonts(${JSON.stringify(fonts)}${pageFamily ? `, ${JSON.stringify(pageFamily)}` : ''});\n` : '');
    if (map) {
      const { sources, mappings } = JSON.parse(map.text);
      // The lxapp's files and other packages' are named as in build problems; the View runtime stays anonymous.
      const named = sources.map(path => {
        const file = resolve(root, path);
        return path === '<stdin>' || file.toLowerCase().startsWith(runtimeRoot) ? null : sourceLabel(root, file);
      });
      module += `configureSourceMap(${JSON.stringify({ sources: named, mappings })});\n`;
      module += 'enableHotSwap();\n';
    }
    await writeAtomic(dest, module);
    // Dev builds record each page's inputs so `lingxia dev` can rebuild only the pages an edit touches:
    // the lxapp's files, a linked package's sources and token file (`../kit/src/x.tsx`), and a linked
    // View runtime's. Installed packages, the View included, do not change under a session.
    if (bundled.metafile) {
      const tokenFiles = [...tokenPackages.values()].filter(owner => typeof owner.tokens === 'string').map(owner => relative(root, resolve(owner.dir, owner.tokens)));
      inputs[page] = [...Object.keys(bundled.metafile.inputs), ...tokenFiles]
        .filter(path => path !== '<stdin>' && !path.split(/[\\/]/).includes('node_modules'))
        .map(path => path.split('\\').join('/'));
    }
    if (config) await writeAtomic(join(output, `${base}.json`), await readFile(configPath));
  }
  // A value built at run time may name classes the table lacks; strictClasses also names each className not followed to literals.
  const classWarnings = [...untracedClasses.values()].filter(place => place.built || strictClasses)
    .map(place => `${place.location}: UI_CLASS: ${place.reason}; ${place.built ? 'its classes exist only at run time, so the table may lack them' : 'write it where the build can follow it to literals'}`);
  if (classWarnings.length) {
    const lines = report(classWarnings, '');
    warnings.push(...lines.text.split('\n'));
    console.warn(`${lines.count} class warning${lines.count === 1 ? '' : 's'}\n${lines.text}`);
  }
  if (failures.length) throw Error(failures.join('\n'));
  if (Object.keys(inputs).length) {
    const path = inputsRecord ? resolve(root, inputsRecord) : join(root, '.lingxia', 'gpu-inputs.json');
    // A page deleted since its last build leaves the record; an edit it shared would plan it again.
    const previous = Object.fromEntries(Object.entries(await readRecord(path))
      .filter(([page, files]) => Array.isArray(files) && existsSync(resolve(root, page))));
    await mkdir(dirname(path), { recursive: true });
    await writeAtomic(path, JSON.stringify({ ...previous, ...inputs }, null, 1) + '\n');
  }
  return { warnings, warm, analysed: analysisMisses, untracedClasses: [...untracedClasses.values()] };
}
