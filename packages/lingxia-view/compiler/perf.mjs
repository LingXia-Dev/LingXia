import ts from 'typescript';
import { readFile, stat } from 'node:fs/promises';
import { resolve, relative, isAbsolute } from 'node:path';
import { classPieces, stringLiterals } from './class-usage.mjs';
import { limits } from '../src/generated/contract.js';

/** Literal rows a page may build outside `<list>` before the build suggests virtualising. */
export const literalRows = 50;
/** Longest side, in pixels, of a bundled image that a phone screen can use. */
export const imageSide = 2048;

const at = node => {
  const file = node.getSourceFile();
  const { line, character } = file.getLineAndCharacterOfPosition(node.getStart(file));
  return `${file.fileName}:${line + 1}:${character + 1}`;
};
const functionLike = node => node && (ts.isArrowFunction(node) || ts.isFunctionExpression(node) || ts.isFunctionDeclaration(node));
const tagName = node => (ts.isJsxElement(node) ? node.openingElement : node).tagName;
const mapper = call => ts.isPropertyAccessExpression(call.expression) && ['map', 'flatMap'].includes(call.expression.name.text);
const arrayFrom = call => ts.isPropertyAccessExpression(call.expression) && call.expression.name.text === 'from' &&
  ts.isIdentifier(call.expression.expression) && call.expression.expression.text === 'Array';
const yieldsJsx = node => {
  let found = false;
  const visit = child => { if (found) return; if (ts.isJsxElement(child) || ts.isJsxSelfClosingElement(child)) found = true; else ts.forEachChild(child, visit); };
  visit(node);
  return found;
};

/** How many items an expression statically holds: array literals, `Array.from({ length: N })`, `Array(N)`, const bindings. */
function staticLength(checker, node, seen = new Set()) {
  if (!node || seen.has(node)) return undefined;
  seen.add(node);
  while (ts.isParenthesizedExpression(node) || ts.isAsExpression(node) || ts.isSatisfiesExpression(node)) node = node.expression;
  if (ts.isArrayLiteralExpression(node)) return node.elements.some(ts.isSpreadElement) ? undefined : node.elements.length;
  if (ts.isCallExpression(node) || ts.isNewExpression(node)) {
    const [first] = node.arguments ?? [];
    if (ts.isCallExpression(node) && arrayFrom(node) && first && ts.isObjectLiteralExpression(first)) {
      const length = first.properties.find(prop => ts.isPropertyAssignment(prop) && prop.name.getText() === 'length');
      return length && ts.isNumericLiteral(length.initializer) ? Number(length.initializer.text) : undefined;
    }
    if (ts.isIdentifier(node.expression) && node.expression.text === 'Array' && first && ts.isNumericLiteral(first)) return Number(first.text);
    if (ts.isPropertyAccessExpression(node.expression) && ['fill', 'keys'].includes(node.expression.name.text))
      return staticLength(checker, node.expression.expression, seen);
  }
  if (ts.isIdentifier(node)) {
    let symbol = checker.getSymbolAtLocation(node);
    if (symbol && (symbol.flags & ts.SymbolFlags.Alias)) symbol = checker.getAliasedSymbol(symbol);
    const declaration = symbol?.valueDeclaration;
    if (declaration && ts.isVariableDeclaration(declaration) && (declaration.parent.flags & ts.NodeFlags.Const))
      return staticLength(checker, declaration.initializer, seen);
  }
  return undefined;
}

/** The function a component tag names, through imports. */
function component(checker, tag) {
  let symbol = checker.getSymbolAtLocation(tag);
  if (symbol && (symbol.flags & ts.SymbolFlags.Alias)) symbol = checker.getAliasedSymbol(symbol);
  const declaration = symbol?.valueDeclaration;
  if (functionLike(declaration)) return declaration;
  if (declaration && ts.isVariableDeclaration(declaration) && functionLike(declaration.initializer)) return declaration.initializer;
  return undefined;
}

/** Callbacks whose JSX is built once per row: `.map`/`Array.from` callbacks, `<List renderItem>`, `<list row>`. */
function rowCallbacks(files) {
  const rows = new Set();
  const add = fn => { if (functionLike(fn)) rows.add(fn); };
  for (const file of files) {
    const visit = node => {
      if (ts.isCallExpression(node) && (mapper(node) || arrayFrom(node))) add(node.arguments[arrayFrom(node) ? 1 : 0]);
      if (ts.isJsxAttribute(node) && ['renderItem', 'row'].includes(node.name.getText()) && node.initializer && ts.isJsxExpression(node.initializer))
        add(node.initializer.expression);
      ts.forEachChild(node, visit);
    };
    visit(file);
  }
  return rows;
}

const UNKNOWN = undefined;
const unwrap = node => {
  while (node && (ts.isParenthesizedExpression(node) || ts.isAsExpression(node) || ts.isSatisfiesExpression(node) || ts.isNonNullExpression(node))) node = node.expression;
  return node;
};

/**
 * The value of `node` under `props` (binding element → literal), as `{ value }`, or undefined when
 * it is not decided statically. Only literals, known props, const bindings to these, `!`, `&&`,
 * `||`, `?:` and (in)equality with literals are evaluated.
 */
function evaluate(checker, node, props, seen = new Set()) {
  node = unwrap(node);
  if (!node || seen.has(node)) return UNKNOWN;
  seen.add(node);
  if (ts.isStringLiteralLike(node)) return { value: node.text };
  if (ts.isNumericLiteral(node)) return { value: Number(node.text) };
  if (node.kind === ts.SyntaxKind.TrueKeyword) return { value: true };
  if (node.kind === ts.SyntaxKind.FalseKeyword) return { value: false };
  if (node.kind === ts.SyntaxKind.NullKeyword) return { value: null };
  if (ts.isIdentifier(node)) {
    if (node.text === 'undefined') return { value: undefined };
    const declaration = checker.getSymbolAtLocation(node)?.valueDeclaration;
    if (declaration && props.has(declaration)) return props.get(declaration);
    if (declaration && ts.isVariableDeclaration(declaration) && ts.isIdentifier(declaration.name) && (declaration.parent.flags & ts.NodeFlags.Const))
      return evaluate(checker, declaration.initializer, props, seen);
    return UNKNOWN;
  }
  if (ts.isConditionalExpression(node)) {
    const test = evaluate(checker, node.condition, props, seen);
    return test && evaluate(checker, test.value ? node.whenTrue : node.whenFalse, props, seen);
  }
  if (ts.isPrefixUnaryExpression(node) && node.operator === ts.SyntaxKind.ExclamationToken) {
    const operand = evaluate(checker, node.operand, props, seen);
    return operand && { value: !operand.value };
  }
  if (ts.isBinaryExpression(node)) {
    const kind = node.operatorToken.kind, left = evaluate(checker, node.left, props, seen);
    if (kind === ts.SyntaxKind.AmpersandAmpersandToken) return left && (left.value ? evaluate(checker, node.right, props, seen) : left);
    if (kind === ts.SyntaxKind.BarBarToken) return left && (left.value ? left : evaluate(checker, node.right, props, seen));
    const equality = [ts.SyntaxKind.EqualsEqualsEqualsToken, ts.SyntaxKind.ExclamationEqualsEqualsToken, ts.SyntaxKind.EqualsEqualsToken, ts.SyntaxKind.ExclamationEqualsToken];
    if (!equality.includes(kind)) return UNKNOWN;
    const right = left && evaluate(checker, node.right, props, seen);
    if (!right) return UNKNOWN;
    const same = kind === ts.SyntaxKind.EqualsEqualsEqualsToken || kind === ts.SyntaxKind.ExclamationEqualsEqualsToken ? left.value === right.value : left.value == right.value;
    return { value: kind === ts.SyntaxKind.EqualsEqualsEqualsToken || kind === ts.SyntaxKind.EqualsEqualsToken ? same : !same };
  }
  return UNKNOWN;
}

/** The props a component call site fixes: literal or known attributes, destructuring defaults when no spread could set them. */
function callProps(checker, element, fn, props) {
  const parameter = fn.parameters[0];
  if (!parameter || !ts.isObjectBindingPattern(parameter.name)) return new Map();
  const attributes = (ts.isJsxElement(element) ? element.openingElement : element).attributes.properties;
  const given = new Map();
  let spread = false;
  for (const attribute of attributes) {
    if (ts.isJsxSpreadAttribute(attribute)) { spread = true; given.clear(); continue; }
    const init = attribute.initializer;
    given.set(attribute.name.getText(), !init ? { value: true } : ts.isStringLiteral(init) ? { value: init.text } : evaluate(checker, init.expression, props));
  }
  const fixed = new Map();
  for (const binding of parameter.name.elements) {
    if (binding.dotDotDotToken || !ts.isIdentifier(binding.name)) continue;
    const name = (binding.propertyName ?? binding.name).getText();
    if (name === 'children') continue;
    const value = given.has(name) ? given.get(name) : spread ? UNKNOWN
      : binding.initializer ? evaluate(checker, binding.initializer, new Map()) : { value: undefined };
    if (value) fixed.set(binding, value);
  }
  return fixed;
}

/** Content the runtime lifts into the top layer: every `<dialog>`, and a `<view>` with `popover`. */
function topLayer(node) {
  if (!ts.isJsxElement(node) && !ts.isJsxSelfClosingElement(node)) return false;
  const tag = tagName(node).getText();
  if (tag === 'dialog') return true;
  if (tag !== 'view') return false;
  const popover = (ts.isJsxElement(node) ? node.openingElement : node).attributes.properties
    .find(attribute => ts.isJsxAttribute(attribute) && attribute.name.getText() === 'popover');
  return Boolean(popover) && !(popover.initializer && ts.isJsxExpression(popover.initializer) && popover.initializer.expression?.kind === ts.SyntaxKind.FalseKeyword);
}

const terminates = statement => ts.isReturnStatement(statement) || ts.isThrowStatement(statement) ||
  (ts.isBlock(statement) && statement.statements.length > 0 && terminates(statement.statements.at(-1)));

/**
 * Every row callback and the components it renders, each walked once per set of props its call
 * sites fix. `visitRow(node, props)` sees each node a row can build; branches decided by those
 * props and top-layer content are skipped. Calls with props nobody fixes walk everything.
 */
function walkRows(files, checker, visitRow) {
  const callbacks = rowCallbacks(files);
  const walked = new Map(), queue = [], ids = new Map();
  const enqueue = (fn, props) => {
    const id = binding => ids.get(binding) ?? ids.set(binding, ids.size).get(binding);
    const key = [...props].map(([binding, { value }]) => `${id(binding)}=${JSON.stringify(value) ?? 'undefined'}`).join(',');
    const keys = walked.get(fn) ?? walked.set(fn, new Set()).get(fn);
    if (keys.has(key)) return;
    keys.add(key);
    queue.push([fn, props]);
  };
  const walk = (node, props, row) => {
    if (!node || topLayer(node)) return;
    if (functionLike(node)) return walk(node.body, props, callbacks.has(node));
    if (row) visitRow(node, props);
    if ((ts.isJsxElement(node) || ts.isJsxSelfClosingElement(node)) && /^[A-Z]/.test(tagName(node).getText())) {
      const fn = component(checker, tagName(node));
      if (fn?.body) enqueue(fn, callProps(checker, node, fn, props));
    }
    const known = condition => evaluate(checker, condition, props);
    if (ts.isConditionalExpression(node)) {
      const test = known(node.condition);
      walk(node.condition, props, row);
      if (!test || test.value) walk(node.whenTrue, props, row);
      if (!test || !test.value) walk(node.whenFalse, props, row);
      return;
    }
    if (ts.isBinaryExpression(node) && [ts.SyntaxKind.AmpersandAmpersandToken, ts.SyntaxKind.BarBarToken].includes(node.operatorToken.kind)) {
      const left = known(node.left);
      walk(node.left, props, row);
      if (!left || Boolean(left.value) === (node.operatorToken.kind === ts.SyntaxKind.AmpersandAmpersandToken)) walk(node.right, props, row);
      return;
    }
    if (ts.isIfStatement(node)) {
      const test = known(node.expression);
      walk(node.expression, props, row);
      if (!test || test.value) walk(node.thenStatement, props, row);
      if (node.elseStatement && (!test || !test.value)) walk(node.elseStatement, props, row);
      return;
    }
    if (ts.isBlock(node) || ts.isSourceFile(node)) {
      for (const statement of node.statements) {
        walk(statement, props, row);
        // `if (decided) return …`: what follows never runs.
        if (ts.isIfStatement(statement) && terminates(statement.thenStatement) && known(statement.expression)?.value) return;
      }
      return;
    }
    ts.forEachChild(node, child => { walk(child, props, row); });
  };
  // A callback inside a component is walked with the props that component is called with; one in
  // a component no row reaches, or in no component, starts with every prop unknown.
  const components = new Set();
  for (const file of files) {
    const visit = node => {
      if ((ts.isJsxElement(node) || ts.isJsxSelfClosingElement(node)) && /^[A-Z]/.test(tagName(node).getText())) components.add(component(checker, tagName(node)));
      ts.forEachChild(node, visit);
    };
    visit(file);
  }
  const within = (fn, set) => { for (let up = fn.parent; up; up = up.parent) if (set.has(up)) return true; return false; };
  const drain = () => { while (queue.length) { const [fn, props] = queue.pop(); walk(fn.body, props, true); } };
  for (const callback of callbacks) if (!within(callback, components)) enqueue(callback, new Map());
  drain();
  for (const callback of callbacks) if (!walked.has(callback) && !within(callback, walked)) enqueue(callback, new Map());
  drain();
}

const composites = rule => !rule.when?.state && (
  // Fully transparent draws nothing, so only a partial opacity composites.
  ((rule.style?.opacity ?? 1) > 0 && (rule.style?.opacity ?? 1) < 1) ||
  Boolean(rule.backdrop && (rule.backdrop.blur > 0 || (rule.backdrop.saturation ?? 1) !== 1)));

/** PNG and JPEG pixel size from the file header. */
function dimensions(bytes) {
  if (bytes.length > 24 && bytes.readUInt32BE(0) === 0x89504e47) return [bytes.readUInt32BE(16), bytes.readUInt32BE(20)];
  if (bytes[0] !== 0xff || bytes[1] !== 0xd8) return undefined;
  for (let i = 2; i + 9 < bytes.length;) {
    if (bytes[i] !== 0xff) return undefined;
    const marker = bytes[i + 1];
    if (marker >= 0xc0 && marker <= 0xcf && ![0xc4, 0xc8, 0xcc].includes(marker)) return [bytes.readUInt16BE(i + 7), bytes.readUInt16BE(i + 5)];
    i += 2 + bytes.readUInt16BE(i + 2);
  }
  return undefined;
}

const mib = bytes => `${(bytes / 1048576).toFixed(1)} MiB`;

/**
 * Patterns that build but cost every frame, as `file:line:col: UI_PERF: …`: literal row sets above
 * `literalRows` outside `<list>`, opacity or backdrop layers repeated per row, and bundled images
 * too large to load or larger than any screen paints them.
 */
export async function perfWarnings(root, files, checker, rulesByName) {
  const warnings = [], perRow = new Set();
  walkRows(files, checker, (node, props) => {
    if (!ts.isJsxAttribute(node) || !node.initializer) return;
    const name = node.name.getText();
    const expression = ts.isJsxExpression(node.initializer) ? node.initializer.expression : node.initializer;
    if (name === 'className') for (const piece of classPieces(checker, expression, condition => evaluate(checker, condition, props)))
      for (const cls of piece.text.split(/\s+/).filter(Boolean))
        if ((rulesByName.get(cls) ?? []).some(composites))
          perRow.add(`${at(piece.node)}: UI_PERF: ${cls} composites a layer for every row; apply it once to the container, or use a translucent colour`);
    if (name === 'style' && expression && ts.isObjectLiteralExpression(expression))
      for (const prop of expression.properties)
        if (ts.isPropertyAssignment(prop) && prop.name.getText().replace(/['"]/g, '') === 'opacity' && ts.isNumericLiteral(prop.initializer) && Number(prop.initializer.text) < 1)
          perRow.add(`${at(prop)}: UI_PERF: opacity composites a layer for every row; apply it once to the container, or use a translucent colour`);
  });
  const images = [];
  for (const file of files) {
    const visit = node => {
      if (ts.isCallExpression(node) && (mapper(node) || arrayFrom(node))) {
        const callback = node.arguments[arrayFrom(node) ? 1 : 0];
        const count = mapper(node) ? staticLength(checker, node.expression.expression) : staticLength(checker, node);
        if (functionLike(callback) && yieldsJsx(callback) && count > literalRows)
          warnings.push(`${at(node)}: UI_PERF: ${count} rows built at once; above ${literalRows} use <List> so only rows near the viewport exist`);
      }
      if (ts.isJsxAttribute(node) && node.initializer && node.name.getText() === 'src' && tagName(node.parent.parent).getText() === 'image') {
        const expression = ts.isJsxExpression(node.initializer) ? node.initializer.expression : node.initializer;
        for (const literal of stringLiterals(checker, expression)) images.push(literal);
      }
      ts.forEachChild(node, visit);
    };
    visit(file);
  }
  warnings.push(...perRow);
  for (const literal of images) {
    const src = literal.text;
    if (!src || /^[a-z][a-z0-9+.-]*:/i.test(src) || src.startsWith('/')) continue;
    const path = resolve(root, src), rel = relative(root, path);
    if (rel.startsWith('..') || isAbsolute(rel)) continue;
    const size = await stat(path).then(info => info.size, () => undefined);
    if (size === undefined) continue;
    if (size > limits.imageBytes) {
      warnings.push(`${at(literal)}: UI_PERF: ${src} is ${mib(size)}; images above ${mib(limits.imageBytes)} fail to load`);
      continue;
    }
    const pixels = dimensions(await readFile(path));
    if (pixels && Math.max(...pixels) > imageSide)
      warnings.push(`${at(literal)}: UI_PERF: ${src} is ${pixels[0]}×${pixels[1]}; every load decodes it, so ship it at most ${imageSide} px on its long side`);
  }
  return warnings;
}
